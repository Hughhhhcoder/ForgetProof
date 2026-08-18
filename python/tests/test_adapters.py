import os
import unittest

from forgetproof_adapters.reference import ReferenceAdapter
from forgetproof_adapters.remote import extract_found


class ReferenceAdapterTests(unittest.TestCase):
    def setUp(self):
        os.environ["FORGETPROOF_ADAPTER_NAME"] = "reference-clean"
        os.environ["FORGETPROOF_ADAPTER_MODE"] = "clean"
        self.adapter = ReferenceAdapter()
        self.adapter.handle_prepare({"run_id": "test-run"})
        self.fixture = {"id": "target", "content": "unique canary", "target": True}
        self.control = {"id": "control", "content": "control canary", "target": False}
        self.adapter.handle_ingest({"fixture": self.fixture})
        self.adapter.handle_ingest({"fixture": self.control})

    def test_clean_removes_raw_and_derived(self):
        self.assertTrue(self.adapter.handle_probe({"probe": {"fixture": "target", "kind": "exact"}})["found"])
        self.adapter.handle_erase({"target": "target", "intent": "subject_erase"})
        self.assertFalse(self.adapter.handle_probe({"probe": {"fixture": "target", "kind": "exact"}})["found"])
        self.assertFalse(self.adapter.handle_inspect({"probe": {"fixture": "target"}})["found"])
        self.assertTrue(self.adapter.handle_probe({"probe": {"fixture": "control", "kind": "exact"}})["found"])


class ResponseParsingTests(unittest.TestCase):
    def test_structured_search_response(self):
        fixture = {"id": "target", "content": "unique canary"}
        response = {"results": [{"id": "remote-1", "content": "unique canary"}]}
        self.assertTrue(extract_found(response, fixture))


if __name__ == "__main__":
    unittest.main()
