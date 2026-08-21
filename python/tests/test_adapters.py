import json
import os
import threading
import unittest
from http.server import ThreadingHTTPServer
from unittest.mock import patch

from forgetproof_adapters.letta import LettaAdapter
from forgetproof_adapters.mem0 import Mem0Adapter
from forgetproof_adapters.protocol import AdapterError, PROTOCOL_VERSION
from forgetproof_adapters.reference import ReferenceAdapter
from forgetproof_adapters.remote import extract_found
from forgetproof_adapters.zep import ZepAdapter
from scripts.mock_remote_backend import Handler, State


def fixture_payload(fixture_id: str, subject: str, content: str, role: str) -> dict:
    return {
        "id": fixture_id,
        "subject": subject,
        "content": content,
        "role": role,
        "target": role == "target",
    }


class ReferenceAdapterTests(unittest.TestCase):
    def setUp(self):
        os.environ["MEMORYPROOF_ADAPTER_NAME"] = "reference-clean"
        os.environ["MEMORYPROOF_ADAPTER_MODE"] = "clean"
        self.adapter = ReferenceAdapter()
        self.target = fixture_payload("target", "target-subject", "unique target canary 7f3e9d", "target")
        self.control = fixture_payload("control", "control-subject", "unique control canary 91c4a2", "control")
        self.adapter.handle_prepare(
            {
                "run_id": "test-run",
                "fixtures": [self.target, self.control],
            }
        )
        self.adapter.handle_ingest({"fixture": self.target})
        self.adapter.handle_ingest({"fixture": self.control})

    def probe(self, fixture_id: str, subject: str, kind: str = "exact") -> bool:
        return self.adapter.handle_probe(
            {"probe": {"fixture": fixture_id, "as_subject": subject, "kind": kind}}
        )["found"]

    def test_clean_removes_target_scope_and_preserves_control(self):
        self.assertTrue(self.probe("target", "target-subject"))
        self.assertTrue(self.probe("control", "control-subject"))
        self.adapter.handle_erase(
            {"target": "target", "target_subject": "target-subject", "intent": "subject_erase"}
        )
        self.assertFalse(self.probe("target", "target-subject"))
        self.assertFalse(
            self.adapter.handle_inspect(
                {"probe": {"fixture": "target", "as_subject": "target-subject"}}
            )["found"]
        )
        self.assertTrue(self.probe("control", "control-subject"))

    def test_isolation_rejects_cross_subject_reads(self):
        self.assertTrue(self.probe("target", "target-subject"))
        self.assertFalse(self.probe("target", "control-subject"))
        self.assertTrue(self.probe("control", "control-subject"))
        self.assertFalse(self.probe("control", "target-subject"))

    def test_leaky_mode_leaves_only_derived_boundary(self):
        os.environ["MEMORYPROOF_ADAPTER_MODE"] = "leaky"
        adapter = ReferenceAdapter()
        adapter.handle_prepare({"run_id": "leaky", "fixtures": [self.target, self.control]})
        adapter.handle_ingest({"fixture": self.target})
        adapter.handle_ingest({"fixture": self.control})
        adapter.handle_erase(
            {"target": "target", "target_subject": "target-subject", "intent": "subject_erase"}
        )
        self.assertFalse(
            adapter.handle_probe(
                {"probe": {"fixture": "target", "as_subject": "target-subject", "kind": "exact"}}
            )["found"]
        )
        self.assertTrue(
            adapter.handle_inspect(
                {"probe": {"fixture": "target", "as_subject": "target-subject"}}
            )["found"]
        )

    def test_cleanup_requires_matching_run_owner(self):
        with self.assertRaises(AdapterError):
            self.adapter.handle_cleanup({"run_id": "another-run"})


class ResponseParsingTests(unittest.TestCase):
    def test_structured_search_response(self):
        fixture = {"id": "target", "content": "unique canary 7f3e9d"}
        response = {"results": [{"id": "remote-1", "content": "unique canary 7f3e9d"}]}
        self.assertTrue(extract_found(response, fixture))

    def test_unrelated_response_is_not_a_match(self):
        fixture = {"id": "target", "content": "unique canary 7f3e9d"}
        response = {"results": [{"id": "remote-1", "content": "another memory"}]}
        self.assertFalse(extract_found(response, fixture))

    def test_protocol_constant_is_stable(self):
        self.assertEqual(PROTOCOL_VERSION, "memoryproof.adapter/v1")


class MockRemoteAdapterTests(unittest.TestCase):
    def setUp(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.state = State("test")
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.server.shutdown)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.thread.join, 2)

    def run_subject_erase_cycle(self, adapter_class, name: str, mode: str):
        self.server.state = State(name)
        base_url = f"http://127.0.0.1:{self.server.server_port}"
        target = fixture_payload("target", "target-subject", f"{name} target canary 7f3e9d", "target")
        control = fixture_payload("control", "control-subject", f"{name} control canary 91c4a2", "control")
        env = {
            "MEMORYPROOF_ADAPTER_NAME": name,
            "MEMORYPROOF_ADAPTER_MODE": mode,
            "MEMORYPROOF_CONFIG_JSON": json.dumps({"base_url": base_url}),
        }
        with patch.dict(os.environ, env, clear=False):
            adapter = adapter_class()
            prepared = adapter.handle_prepare({"run_id": f"{name}-run", "fixtures": [target, control]})
            if name != "mem0":
                self.assertTrue(prepared["request_ids"])
            ingested = adapter.handle_ingest({"fixture": target})
            self.assertTrue(ingested["request_ids"])
            adapter.handle_ingest({"fixture": control})
            self.assertTrue(
                adapter.handle_probe(
                    {"probe": {"fixture": "target", "as_subject": "target-subject"}}
                )["found"]
            )
            self.assertTrue(
                adapter.handle_inspect(
                    {"probe": {"fixture": "target", "as_subject": "target-subject"}}
                )["found"]
            )
            adapter.handle_erase(
                {
                    "target": "target",
                    "target_subject": "target-subject",
                    "intent": "subject_erase",
                }
            )
            self.assertFalse(
                adapter.handle_probe(
                    {"probe": {"fixture": "target", "as_subject": "target-subject"}}
                )["found"]
            )
            self.assertFalse(
                adapter.handle_inspect(
                    {"probe": {"fixture": "target", "as_subject": "target-subject"}}
                )["found"]
            )
            self.assertTrue(
                adapter.handle_probe(
                    {"probe": {"fixture": "control", "as_subject": "control-subject"}}
                )["found"]
            )
            adapter.handle_cleanup({"run_id": f"{name}-run"})

    def test_mem0_subject_erase_is_scoped(self):
        self.run_subject_erase_cycle(Mem0Adapter, "mem0", "oss")

    def test_letta_deletes_agent_and_core_block(self):
        self.run_subject_erase_cycle(LettaAdapter, "letta", "self-hosted")

    def test_zep_user_erase_is_scoped(self):
        self.run_subject_erase_cycle(ZepAdapter, "zep", "self-hosted")


if __name__ == "__main__":
    unittest.main()
