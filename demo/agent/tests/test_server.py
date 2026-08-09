import io
import http.client
import json
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import server


class AgentProtocolTests(unittest.TestCase):
    def test_desktop_api_key_prompt_sets_session_environment(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertTrue(server.configure_desktop_api_key(" new-key "))
            self.assertEqual(os.environ["DEEPSEEK_API_KEY"], "new-key")

    def test_desktop_api_key_extracts_key_from_powershell_assignment(self):
        pasted = '$env:DEEPSEEK_API_KEY = “sk-demo_123”'
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertTrue(server.configure_desktop_api_key(pasted))
            self.assertEqual(os.environ["DEEPSEEK_API_KEY"], "sk-demo_123")

    def test_rejects_non_ascii_api_key_without_sk_token(self):
        with self.assertRaises(server.AgentError) as raised:
            server.normalize_deepseek_api_key("这里不是密钥")
        self.assertEqual(raised.exception.code, "invalid_api_key")

    def test_desktop_api_key_prompt_can_select_offline_mode(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertFalse(server.configure_desktop_api_key(None))
            self.assertNotIn("DEEPSEEK_API_KEY", os.environ)

    def test_desktop_api_key_prompt_preserves_existing_key(self):
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "existing"}, clear=True):
            self.assertTrue(server.configure_desktop_api_key("replacement"))
            self.assertEqual(os.environ["DEEPSEEK_API_KEY"], "existing")

    def test_desktop_launch_switches_the_same_window_to_chat(self):
        launch = server.DesktopLaunchApi("http://127.0.0.1/chat")
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(launch.start("key"), "http://127.0.0.1/chat")
            self.assertEqual(os.environ["DEEPSEEK_API_KEY"], "key")

    def test_desktop_launch_ignores_duplicate_clicks(self):
        launch = server.DesktopLaunchApi("http://127.0.0.1/chat")
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(launch.start("first"), "http://127.0.0.1/chat")
            self.assertEqual(launch.start("second"), "http://127.0.0.1/chat")
            self.assertEqual(os.environ["DEEPSEEK_API_KEY"], "first")

    def test_accepts_frozen_request_protocol(self):
        request = server.validate_request({
            "sessionId": "demo-001",
            "prompt": "生成面板",
            "scenario": "test262_dashboard",
            "mode": "fixed",
        })
        self.assertEqual(request.session_id, "demo-001")
        self.assertEqual(request.prompt, "生成面板")

    def test_legacy_request_maps_to_frozen_protocol(self):
        request = server.validate_request({
            "task": "统计",
            "scenario": "json_analysis",
            "mode": "offline",
            "input": {"orders": []},
        })
        self.assertEqual(request.prompt, "统计")
        self.assertEqual(request.mode, "fixed")
        self.assertTrue(request.session_id.startswith("demo-"))

    def test_chat_defaults_to_deepseek_when_key_is_configured(self):
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "test-key"}, clear=False):
            request = server.validate_request({"sessionId": "demo-chat", "prompt": "1+1"})
        self.assertEqual(request.scenario, "chat")
        self.assertEqual(request.mode, "deepseek")

    def test_chat_defaults_to_fixed_without_key(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            request = server.validate_request({"sessionId": "demo-chat", "prompt": "1+1"})
        self.assertEqual(request.mode, "fixed")

    def test_rejects_unknown_scenario(self):
        with self.assertRaisesRegex(server.AgentError, "不支持"):
            server.validate_request({"prompt": "x", "scenario": "browser"})

    def test_rejects_invalid_session_id(self):
        with self.assertRaisesRegex(server.AgentError, "sessionId"):
            server.validate_request({"sessionId": "../escape", "prompt": "x"})

    def test_rejects_host_capability_in_generated_code(self):
        with self.assertRaisesRegex(server.AgentError, "禁止"):
            server.validate_generated_script({"title": "x", "code": "return fetch('x');"})

    def test_requires_render_call_for_orchestrated_script(self):
        with self.assertRaisesRegex(server.AgentError, "agent.render"):
            server.validate_generated_script({"title": "x", "code": "return 1;"}, require_render=True)
        with self.assertRaisesRegex(server.AgentError, "只能调用一次"):
            server.validate_generated_script({
                "title": "x",
                "code": "agent.render({type:'text'}); agent.render({type:'text'}); return 1;",
            }, require_render=True)

    def test_wrapper_round_trips_input_and_uses_rust_host(self):
        source = server.build_wrapper(
            "agent.render({type: 'text'}); return input;",
            {"text": "</script> ' 中文"},
        )
        self.assertNotIn("JSON.parse", source)
        self.assertIn(r'''const input = {"text":"</script> ' \u4e2d\u6587"};''', source)
        self.assertIn(server.RESULT_MARKER, source)
        self.assertNotIn("const agent =", source)
        self.assertIn("(function ()", source)

    def test_wrapper_allows_generated_input_binding(self):
        source = server.build_wrapper(
            "const input = {value: 7}; agent.render({type: 'text', value: input.value}); return input.value;",
            {"value": 3},
        )
        self.assertIn("const input = {value: 7};", source)
        self.assertNotIn("(function (input)", source)

    def test_wrapper_normalizes_undefined_result_to_json_null(self):
        source = server.build_wrapper("agent.render({type: 'panel', children: []});", {})
        self.assertIn("__agentValue === undefined ? null : __agentValue", source)
        self.assertIn('__agentJson === undefined ? "null" : __agentJson', source)

    def test_render_tree_enforces_type_and_depth(self):
        self.assertEqual(server.validate_render_tree({"type": "panel", "children": []})["type"], "panel")
        with self.assertRaisesRegex(server.AgentError, "不支持"):
            server.validate_render_tree({"type": "html"})
        tree = {"type": "panel"}
        cursor = tree
        for _ in range(server.MAX_RENDER_DEPTH + 1):
            child = {"type": "panel"}
            cursor["children"] = [child]
            cursor = child
        with self.assertRaisesRegex(server.AgentError, "过深"):
            server.validate_render_tree(tree)
    def test_render_tree_normalizes_text_value_aliases(self):
        tree = server.validate_render_tree({
            "type": "panel",
            "children": [
                {"type": "text", "content": "2"},
                {"type": "text", "text": "four"},
            ],
        })
        self.assertEqual(tree["children"][0]["value"], "2")
        self.assertNotIn("content", tree["children"][0])
        self.assertEqual(tree["children"][1]["value"], "four")


class DeepSeekGeneratorTests(unittest.TestCase):
    @mock.patch("urllib.request.urlopen")
    def test_maps_disconnected_api_to_structured_error(self, urlopen):
        urlopen.side_effect = http.client.RemoteDisconnected("closed")
        request = server.AgentRequest("demo-1", "x", {}, "chat", "deepseek")
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "test-key"}, clear=False):
            with self.assertRaises(server.AgentError) as raised:
                server.DeepSeekCodeGenerator().generate([], request)
        self.assertEqual(raised.exception.code, "deepseek_unavailable")
        self.assertEqual(raised.exception.status, 502)

    def test_parses_fenced_or_prefixed_json(self):
        payload = '{"title":"sum","code":"return 1;"}'
        self.assertEqual(server.parse_model_json("```json\n" + payload + "\n```"), json.loads(payload))
        self.assertEqual(server.parse_model_json("Here is the result:\n" + payload), json.loads(payload))

    @mock.patch("urllib.request.urlopen")
    def test_parses_json_output_and_sends_history(self, urlopen):
        content = json.dumps({
            "title": "sum",
            "code": "agent.render({type: 'panel', children: []}); return input.value;",
        })
        urlopen.return_value = io.BytesIO(json.dumps({
            "choices": [{"message": {"content": content}}]
        }).encode("utf-8"))
        request = server.AgentRequest("demo-1", "return value", {"value": 3}, "json_analysis", "deepseek")
        history = [server.Message("user", "previous"), server.Message("assistant", "previous code")]
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": " test-key "}, clear=False):
            script = server.DeepSeekCodeGenerator().generate(history, request)
        self.assertEqual(script.title, "sum")
        api_request = urlopen.call_args.args[0]
        body = json.loads(api_request.data)
        self.assertEqual(body["model"], "deepseek-v4-pro")
        self.assertEqual(body["response_format"], {"type": "json_object"})
        self.assertEqual(body["messages"][1]["content"], "previous")
        self.assertNotIn("test-key", api_request.data.decode("utf-8"))

    def test_missing_key_is_structured_error(self):
        request = server.AgentRequest("demo-1", "x", {}, "json_analysis", "deepseek")
        with mock.patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(server.AgentError, "DEEPSEEK_API_KEY") as raised:
                server.DeepSeekCodeGenerator().generate([], request)
        self.assertEqual(raised.exception.code, "missing_api_key")


class SessionStoreTests(unittest.TestCase):
    def test_history_is_bounded_and_snapshot_contains_turns(self):
        store = server.SessionStore(capacity=2)
        script = server.GeneratedScript("agent.render({type:'text'}); return 1;", "one")
        for index in range(15):
            store.append("demo", f"p{index}", script, {"result": index})
        self.assertEqual(len(store.history("demo")), server.MAX_HISTORY_MESSAGES)
        snapshot = store.snapshot("demo")
        self.assertEqual(len(snapshot["turns"]), server.MAX_HISTORY_MESSAGES // 2)
        self.assertEqual(snapshot["turns"][-1]["result"], 14)

    def test_session_capacity_evicts_least_recent(self):
        store = server.SessionStore(capacity=2)
        script = server.GeneratedScript("agent.render({type:'text'}); return 1;")
        for session_id in ["a", "b", "c"]:
            store.append(session_id, "p", script, {})
        self.assertIsNone(store.snapshot("a"))
        self.assertIsNotNone(store.snapshot("c"))


class AgentOrchestratorTests(unittest.TestCase):
    @mock.patch.object(server, "execute_agentjs")
    def test_fixed_chain_returns_complete_response(self, execute):
        execute.return_value = server.ExecutionResult(
            value="92%",
            logs=["checked"],
            render_events=[{"type": "panel", "title": "Result", "children": []}],
            elapsed_ms=4.5,
        )
        store = server.SessionStore()
        response = server.run_agent({
            "sessionId": "demo-001",
            "prompt": "生成结果面板",
            "scenario": "test262_dashboard",
            "mode": "fixed",
            "input": {"modules": []},
        }, store=store)
        self.assertTrue(response["ok"])
        self.assertEqual(response["sessionId"], "demo-001")
        self.assertEqual(response["execution"]["value"], "92%")
        self.assertEqual(response["execution"]["elapsedMs"], 4.5)
        self.assertGreaterEqual(response["execution"]["totalMs"], 4.5)
        self.assertEqual(response["render"]["type"], "panel")
        self.assertIsNone(response["error"])
        self.assertEqual(len(store.snapshot("demo-001")["turns"]), 1)

    @mock.patch.object(server, "execute_agentjs")
    def test_legacy_response_aliases_remain_available(self, execute):
        execute.return_value = server.ExecutionResult([], [], [], 2.0)
        response = server.run_agent({
            "task": "统计",
            "scenario": "json_analysis",
            "mode": "offline",
            "input": {"orders": []},
        }, store=server.SessionStore())
        self.assertEqual(response["mode"], "offline")
        self.assertEqual(response["result"], [])
        self.assertIn("agentjsMs", response["metrics"])
        self.assertIn("totalMs", response["metrics"])

    def test_error_response_uses_frozen_shape(self):
        response = server.error_response(
            server.AgentError("execution_failed", "bad", 422),
            {"sessionId": "demo-1", "prompt": "x"},
        )
        self.assertFalse(response["ok"])
        self.assertIsNone(response["execution"])
        self.assertIsNone(response["render"])
        self.assertEqual(response["error"]["code"], "execution_failed")


if __name__ == "__main__":
    unittest.main()
