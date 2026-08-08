import json
import io
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import server


class AgentDemoTests(unittest.TestCase):
    def test_rejects_unknown_scenario(self):
        with self.assertRaisesRegex(server.AgentError, "不支持"):
            server.validate_request({"task": "x", "scenario": "browser", "input": {}})

    def test_rejects_host_capability_in_generated_code(self):
        with self.assertRaisesRegex(server.AgentError, "禁止"):
            server.validate_generated_program({"plan": "x", "code": "return fetch('x');"})

    def test_wrapper_round_trips_untrusted_input_as_json_string(self):
        source = server.build_wrapper("return input;", {"text": "</script> ' 中文"})
        self.assertIn("JSON.parse", source)
        self.assertIn(server.RESULT_MARKER, source)
        self.assertNotIn("const input = {", source)

    @mock.patch("urllib.request.urlopen")
    def test_deepseek_adapter_parses_json_output(self, urlopen):
        content = json.dumps({"plan": "sum", "code": "return input.value;"})
        urlopen.return_value = io.BytesIO(json.dumps({
            "choices": [{"message": {"content": content}}]
        }).encode("utf-8"))
        with mock.patch.dict(os.environ, {"DEEPSEEK_API_KEY": "test-key"}, clear=False):
            program = server.generate_with_deepseek("return value", "json_analysis", {"value": 3})
        self.assertEqual(program["code"], "return input.value;")
        request = urlopen.call_args.args[0]
        body = json.loads(request.data)
        self.assertEqual(body["model"], "deepseek-v4-pro")
        self.assertEqual(body["response_format"], {"type": "json_object"})

    @mock.patch.object(server, "execute_agentjs")
    def test_offline_agent_returns_protocol_shape(self, execute):
        execute.return_value = server.ExecutionResult(result=[{"region": "华东", "total": 2}], elapsed_ms=4.5, stdout=[])
        response = server.run_agent({"task": "统计", "scenario": "json_analysis", "input": {"orders": []}, "mode": "offline"})
        self.assertTrue(response["ok"])
        self.assertEqual(response["model"], "offline-template")
        self.assertEqual(response["result"][0]["region"], "华东")


if __name__ == "__main__":
    unittest.main()
