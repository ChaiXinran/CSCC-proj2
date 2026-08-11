import io
import http.client
import json
import os
import sys
import tempfile
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


class Test262AccuracyTests(unittest.TestCase):
    @staticmethod
    def write_summary(path: Path, passed: int = 48_564) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({
            "total": 53_379,
            "passed": passed,
            "failed": 53_379 - passed - 2,
            "skipped": 2,
            "conformance_percent": passed * 100 / 53_379,
            "elapsed_ms": 123,
        }), encoding="utf-8")

    def test_accuracy_query_detection_is_narrow(self):
        self.assertTrue(server.is_test262_accuracy_query("我们当前 Test262 通过率是多少"))
        self.assertTrue(server.is_test262_accuracy_query("current conformance"))
        self.assertFalse(server.is_test262_accuracy_query("生成一个普通销售表格"))

    def test_fixed_report_directory_wins_before_project_search(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixed = root / "Test262-final" / "full-test262-summary.json"
            other = root / "other" / "newer.json"
            self.write_summary(fixed, 48_564)
            self.write_summary(other, 48_600)
            os.utime(other, (fixed.stat().st_mtime + 10, fixed.stat().st_mtime + 10))
            with mock.patch.dict(os.environ, {}, clear=True), \
                 mock.patch.object(server, "find_project_root", return_value=root), \
                 mock.patch.object(server, "BUNDLE_ROOT", root / "bundle"):
                summary = server.resolve_test262_accuracy()
            self.assertEqual(summary["passed"], 48_564)
            self.assertEqual(summary["source"], "fixed-report")

    def test_project_search_is_used_when_fixed_directory_has_no_full_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_summary(root / "archive" / "full.json", 48_570)
            with mock.patch.dict(os.environ, {}, clear=True), \
                 mock.patch.object(server, "find_project_root", return_value=root), \
                 mock.patch.object(server, "BUNDLE_ROOT", root / "bundle"):
                summary = server.resolve_test262_accuracy()
            self.assertEqual(summary["passed"], 48_570)
            self.assertEqual(summary["source"], "project-search")

    @mock.patch.object(server, "find_agentjs_binary", return_value=Path("agentjs.exe"))
    @mock.patch("subprocess.run")
    def test_missing_reports_trigger_full_test262(self, run, _binary):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "test262").mkdir()

            def complete(command, **_kwargs):
                output = Path(command[command.index("--json") + 1])
                self.write_summary(output)
                return mock.Mock(returncode=0, stdout="", stderr="")

            run.side_effect = complete
            summary = server.run_full_test262(root)
        self.assertEqual(summary["source"], "rerun")
        self.assertIn("--suite", run.call_args.args[0])
        self.assertIn("test", run.call_args.args[0])

    @mock.patch.object(server, "resolve_test262_accuracy")
    @mock.patch.object(server, "execute_agentjs")
    def test_accuracy_question_bypasses_model_and_uses_report(self, execute, resolve):
        resolve.return_value = {
            "total": 53_379, "passed": 48_564, "failed": 4_813, "skipped": 2,
            "conformancePercent": 90.9796, "path": "report.json",
            "modifiedAt": 0, "elapsedMs": 1, "source": "fixed-report",
        }
        execute.return_value = server.ExecutionResult(
            "90.98%", [], [{"type": "panel", "title": "accuracy", "children": []}], 1.0
        )
        with mock.patch.object(server.DeepSeekCodeGenerator, "generate") as generate:
            response = server.run_agent({
                "prompt": "现在准确率是多少", "mode": "deepseek", "engine": "agentjs"
            }, store=server.SessionStore())
        generate.assert_not_called()
        self.assertEqual(response["result"], "90.98%")
        self.assertIn("48564", execute.call_args.args[0])


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

    @mock.patch.object(server, "execute_boa")
    @mock.patch.object(server, "execute_agentjs")
    def test_both_engines_share_one_script_and_return_two_reports(self, agentjs, boa):
        agentjs.return_value = server.ExecutionResult("native", [], [{"type": "panel", "title": "A", "children": []}], 5.0)
        boa.return_value = server.ExecutionResult("boa", [], [{"type": "panel", "title": "B", "children": []}], 7.0)
        response = server.run_agent({"prompt": "compare", "mode": "fixed", "engine": "both"}, store=server.SessionStore())
        self.assertEqual(response["engine"], "both")
        self.assertEqual(set(response["executions"]), {"agentjs", "boa"})
        self.assertEqual(response["executions"]["agentjs"]["elapsedMs"], 5.0)
        self.assertEqual(response["executions"]["boa"]["elapsedMs"], 7.0)
        self.assertEqual(agentjs.call_args.args, boa.call_args.args)
    def test_error_response_uses_frozen_shape(self):
        response = server.error_response(
            server.AgentError("execution_failed", "bad", 422),
            {"sessionId": "demo-1", "prompt": "x"},
        )
        self.assertFalse(response["ok"])
        self.assertIsNone(response["execution"])
        self.assertIsNone(response["render"])
        self.assertEqual(response["error"]["code"], "execution_failed")


class BenchmarkTests(unittest.TestCase):
    def test_parses_cli_internal_timings(self):
        self.assertEqual(server.parse_agentjs_internal_ms("__AGENTJS_INTERNAL_MS__12.345600\n"), 12.3456)
        self.assertEqual(server.parse_boa_internal_ms("Parsing: 1ms\nTotal:     4.49ms\n"), 4.49)
        self.assertEqual(server.parse_boa_internal_ms("Total:     381.00µs\n"), 0.381)

    @mock.patch.object(server, "execute_agentjs_cached_benchmark")
    @mock.patch.object(server, "execute_oxide_benchmark")
    @mock.patch.object(server, "execute_boa")
    @mock.patch.object(server, "execute_agentjs")
    def test_benchmark_discards_warmup_and_reports_median_p95(self, agentjs, boa, oxide, cached):
        result = server.ExecutionResult("same", [], [], 10.0, 4.0)
        agentjs.return_value = result
        boa.return_value = result
        oxide.return_value = server.ExecutionResult("same", [], [], 12.0, None)
        cached.return_value = {
            "result": "same",
            "internal": server.benchmark_summary([3.0] * 30),
            "cacheHits": 34,
            "cacheMisses": 1,
            "processTotalMs": 100.0,
        }
        response = server.run_benchmark({"warmup": 5, "iterations": 30})
        self.assertTrue(response["ok"])
        self.assertEqual(agentjs.call_count, 35)
        self.assertEqual(boa.call_count, 35)
        self.assertEqual(oxide.call_count, 35)
        self.assertEqual(response["engines"]["agentjs"]["internal"]["medianMs"], 4.0)
        self.assertEqual(response["engines"]["boa"]["endToEnd"]["p95Ms"], 10.0)
        self.assertIsNone(response["engines"]["oxide"]["internal"])
        self.assertEqual(response["engines"]["oxide"]["endToEnd"]["medianMs"], 12.0)
        self.assertEqual(response["engines"]["agentjs"]["cached"]["cacheHits"], 34)
        self.assertEqual(len(response["engines"]["agentjs"]["internal"]["samplesMs"]), 30)


if __name__ == "__main__":
    unittest.main()
