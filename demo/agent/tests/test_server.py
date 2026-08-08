import io
import json
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import server


class AgentDemoTests(unittest.TestCase):
    def test_rejects_unknown_scenario(self):
        with self.assertRaisesRegex(server.AgentError, "unsupported scenario"):
            server.validate_request({"task": "x", "scenario": "browser", "input": {}})

    def test_rejects_host_capability_in_generated_code(self):
        code = "agent.render({type:'panel'}); return fetch('x');"
        with self.assertRaisesRegex(server.AgentError, "forbidden host capability"):
            server.validate_generated_program({"plan": "x", "code": code})

    def test_wrapper_round_trips_untrusted_input_as_json_string(self):
        source = server.build_wrapper("return input;", {"text": "</script> ' 中文"})
        self.assertIn("JSON.parse", source)
        self.assertIn(server.RESULT_MARKER, source)
        self.assertNotIn("const input = {", source)

    @mock.patch("urllib.request.urlopen")
    def test_deepseek_adapter_parses_json_output(self, urlopen):
        code = "agent.render({type:'panel',children:[]}); return input.value;"
        content = json.dumps({"title": "sum", "code": code})
        urlopen.return_value = io.BytesIO(json.dumps({
            "choices": [{"message": {"content": content}}]
        }).encode("utf-8"))
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "test-key"}, clear=False):
            program = server.generate_with_deepseek("return value", "json_analysis", {"value": 3})
        self.assertEqual(program["code"], code)
        request = urlopen.call_args.args[0]
        body = json.loads(request.data)
        self.assertEqual(body["model"], "deepseek-v4-pro")
        self.assertEqual(body["response_format"], {"type": "json_object"})

    @mock.patch.object(server, "execute_agentjs")
    def test_legacy_offline_agent_returns_protocol_shape(self, execute):
        execute.return_value = server.ExecutionResult(
            result=[{"region": "East", "total": 2}], elapsed_ms=4.5, stdout=[]
        )
        response = server.run_agent({
            "task": "aggregate", "scenario": "json_analysis",
            "input": {"orders": []}, "mode": "offline",
        })
        self.assertTrue(response["ok"])
        self.assertEqual(response["model"], "offline-template")
        self.assertEqual(response["result"][0]["region"], "East")

    @mock.patch.object(server, "execute_agentjs")
    def test_chat_request_returns_frontend_protocol(self, execute):
        tree = {"type": "panel", "title": "Result", "children": []}
        execute.return_value = server.ExecutionResult(
            result="92.56%", elapsed_ms=4.5, stdout=[], logs=["checked"], render=tree
        )
        response = server.run_agent({"sessionId": "demo-1", "prompt": "build dashboard"})
        self.assertEqual(response["sessionId"], "demo-1")
        self.assertEqual(response["render"], tree)
        self.assertEqual(response["execution"]["value"], "92.56%")
        self.assertEqual(response["execution"]["logs"], ["checked"])


if __name__ == "__main__":
    unittest.main()
