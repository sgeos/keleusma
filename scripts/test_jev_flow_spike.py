"""Offline negative controls for the research runner. No credentials or network."""

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import jev_flow_spike as spike


def response_for(request, cases):
    gold = {case["id"]: case["expected"] for case in cases}
    answers = {}
    for key, question in request["questions"].items():
        if question["type"] == "noul":
            answers[key] = {"type": "noul", "noul": float(gold[key])}
        else:
            answers[key] = {
                "type": "choice", "choice": gold[key], "confidence": 1.0,
                "probabilities": {label: float(label == gold[key]) for label in question["criteria"]},
            }
    return {"model": spike.MODEL, "answers": answers, "usage": {"input_tokens": 100, "output_tokens": 10}}


class SpikeTests(unittest.TestCase):
    def setUp(self):
        self.cases = spike.fixtures()
        self.plan = spike.requests(self.cases)
        self.request = self.plan[0]["request"]
        self.response = response_for(self.request, self.cases)

    def test_only_input_fields_are_sent(self):
        for record in self.plan:
            for case in record["request"]["state"]["cases"]:
                self.assertNotIn("expected", case)
                self.assertNotIn("rationale", case)
        self.assertEqual(self.plan[0]["request"], self.plan[1]["request"])
        for key in self.request["questions"]:
            original = list(self.request["questions"][key]["criteria"])
            reordered = list(self.plan[2]["request"]["questions"][key]["criteria"])
            self.assertEqual(original[::-1], reordered)

    def test_valid_response(self):
        spike.validate(self.request, self.response)

    def test_malformed_responses_rejected(self):
        invalid = []
        item = copy.deepcopy(self.response)
        item["model"] = "unknown"
        invalid.append(item)
        item = copy.deepcopy(self.response)
        item["answers"].pop("c01")
        invalid.append(item)
        for value in (True, float("nan"), -0.01, 1.01):
            item = copy.deepcopy(self.response)
            item["answers"]["c01"]["confidence"] = value
            invalid.append(item)
        item = copy.deepcopy(self.response)
        item["answers"]["c01"]["probabilities"]["supports"] = 0.1
        invalid.append(item)
        item = copy.deepcopy(self.response)
        item["usage"]["input_tokens"] = True
        invalid.append(item)
        for item in invalid:
            with self.subTest(response=item):
                with self.assertRaises(ValueError):
                    spike.validate(self.request, item)

    def test_metrics_detect_false_assurance_and_retrieval_loss(self):
        manifest = {"fixture_sha256": spike.FIXTURE_HASH, "requests_sha256": spike.digest(spike.encoded(self.plan))}
        records = []
        for index, planned in enumerate(self.plan):
            response = response_for(planned["request"], self.cases)
            if index == 0:
                response["answers"]["c02"] = copy.deepcopy(response["answers"]["c01"])
            if index == 12:
                response["answers"]["r11"]["noul"] = 0.0
            records.append(dict(planned, index=index, response=response, http_status="200", curl_exit=0, elapsed_seconds=1.0))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipts.jsonl"
            path.write_text("\n".join(json.dumps(item) for item in [manifest] + records))
            result = spike.analyze(path, self.cases)
            self.assertEqual(result["tasks"]["claims"]["baseline"]["false_supports"], ["c02"])
            self.assertEqual(result["tasks"]["claims"]["baseline"]["high_confidence_errors"], ["c02"])
            self.assertEqual(result["tasks"]["retrieval"]["baseline"]["conservative_filter"]["false_exclusions"], ["r11"])
            self.assertEqual(result["decision_changes"]["repeat"], ["c02", "r11"])
            path.write_text("\n".join(json.dumps(item) for item in [manifest] + records[:-1]))
            with self.assertRaises(ValueError):
                spike.analyze(path, self.cases)

    def test_invalid_credentials_rejected_before_subprocess(self):
        for key in ("", "bad\nkey", "bad\rkey", "bad\x00key"):
            with self.assertRaises(ValueError):
                spike.call(self.request, key)

    def test_transport_keeps_credentials_private(self):
        key = "synthetic-unit-test-credential"
        output = json.dumps(self.response) + "\nHTTP_STATUS=200\n"
        process = mock.Mock(returncode=0, stdout=output, stderr="")
        with mock.patch.object(spike.subprocess, "run", return_value=process) as run:
            receipt = spike.call(self.request, key)
        self.assertIn("response", receipt)
        self.assertNotIn(key, json.dumps(receipt))
        args, kwargs = run.call_args
        self.assertNotIn(key, " ".join(args[0]))
        self.assertNotIn("TYPESAFE_API_KEY", kwargs["env"])
        self.assertIn(key, kwargs["input"])

    def test_service_failure_never_becomes_a_judgment_or_leaks_error_text(self):
        key = "synthetic-unit-test-credential"
        process = mock.Mock(returncode=0, stdout=key + "\nHTTP_STATUS=403\n", stderr=key)
        with mock.patch.object(spike.subprocess, "run", return_value=process) as run:
            receipt = spike.call(self.request, key)
        self.assertEqual(run.call_count, 1)
        self.assertIn("error", receipt)
        self.assertNotIn("response", receipt)
        self.assertNotIn(key, json.dumps(receipt))

    def test_existing_receipts_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "receipts.jsonl"
            path.write_text("existing evidence")
            with mock.patch.dict(spike.os.environ, {"TYPESAFE_API_KEY": "synthetic"}):
                with self.assertRaises(FileExistsError):
                    spike.live(path, self.cases)
            self.assertEqual(path.read_text(), "existing evidence")


if __name__ == "__main__":
    unittest.main()
