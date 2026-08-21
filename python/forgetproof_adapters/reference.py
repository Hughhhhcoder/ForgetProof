from __future__ import annotations

import argparse
import os
from typing import Any

from .protocol import AdapterError, AdapterServer


class ReferenceAdapter(AdapterServer):
    backend = "memoryproof-reference"
    version = "1.0.0"
    capabilities = (
        "object_delete",
        "scope_delete",
        "probe",
        "lexical_search",
        "semantic_search",
        "inspect",
        "derived_inspect",
        "derived_delete",
        "agent_query",
        "async_settle",
        "isolated_namespace",
    )
    modes = ("clean", "leaky", "overdelete", "slow", "crash", "malformed")

    def __init__(self) -> None:
        super().__init__()
        self.leaky = self.mode == "leaky"
        self.overdelete = self.mode == "overdelete"
        self.slow = self.mode == "slow"
        self.crash = self.mode == "crash"
        self.malformed = self.mode == "malformed"
        self.run_id = ""
        self.raw: dict[tuple[str, str], str] = {}
        self.derived: dict[tuple[str, str], str] = {}
        self.fixtures: dict[str, dict[str, Any]] = {}
        self.owned_subjects: set[str] = set()

    def serve(self) -> None:
        if self.malformed:
            for _line in __import__("sys").stdin:
                __import__("sys").stdout.write("this is not a protocol frame\n")
                __import__("sys").stdout.flush()
                return
        super().serve()

    def _maybe_crash(self) -> None:
        if self.crash:
            os._exit(17)

    def handle_prepare(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        self.run_id = str(params.get("run_id", ""))
        self.raw.clear()
        self.derived.clear()
        self.fixtures = {
            str(item.get("id")): item
            for item in params.get("fixtures", [])
            if isinstance(item, dict) and item.get("id")
        }
        self.owned_subjects = {
            str(item.get("subject"))
            for item in self.fixtures.values()
            if item.get("subject")
        }
        return {
            "namespace": f"reference:{self.run_id}",
            "owned_subjects": sorted(self.owned_subjects),
            "ownership_token": f"memoryproof:{self.run_id}",
        }

    def handle_ingest(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        fixture = params.get("fixture") or {}
        fixture_id = str(fixture.get("id", ""))
        content = str(fixture.get("content", ""))
        subject = str(fixture.get("subject", ""))
        if not fixture_id or not content or not subject:
            raise AdapterError("fixture requires id, content, and subject", "invalid_fixture")
        self.fixtures[fixture_id] = fixture
        self.owned_subjects.add(subject)
        key = (subject, fixture_id)
        self.raw[key] = content
        self.derived[key] = content
        return {"stored": True, "fixture": fixture_id, "subject": subject}

    def handle_settle(self, params: dict[str, Any]) -> dict[str, Any]:
        if self.slow:
            return {"state": "timeout", "stable": False, "observations": 1}
        return {"state": "stable", "stable": True, "observations": 1}

    def handle_probe(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        subject = self._probe_subject(probe, fixture_id)
        kind = str(probe.get("kind", "exact"))
        key = (subject, fixture_id)
        if subject in getattr(self, "revoked_subjects", set()):
            found = False
        elif kind in {"lexical", "semantic"}:
            found = key in self.raw or key in self.derived
        else:
            # Exact probes inspect only the raw object boundary. The leaky
            # mode should therefore fail at derived/agent boundaries.
            found = key in self.raw
        return {"found": found, "subject": subject, "artifact": "raw"}

    def handle_inspect(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        subject = self._probe_subject(probe, fixture_id)
        return {
            "found": (subject, fixture_id) in self.derived,
            "subject": subject,
            "artifact_types": ["summary", "index"],
        }

    def handle_agent_query(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        subject = self._probe_subject(probe, fixture_id)
        key = (subject, fixture_id)
        found = key in self.raw or key in self.derived
        return {"found": found, "subject": subject, "response_contains_canary": found}

    def handle_erase(self, params: dict[str, Any]) -> dict[str, Any]:
        self._maybe_crash()
        target = str(params.get("target", ""))
        intent = str(params.get("intent", "object_delete"))
        target_subject = str(params.get("target_subject", ""))
        if not target or not target_subject:
            raise AdapterError("erase requires target and target_subject", "invalid_erase")
        if target_subject not in self.owned_subjects:
            raise AdapterError("target subject is not owned by this run", "ownership_required")
        if intent == "access_revoke":
            if not hasattr(self, "revoked_subjects"):
                self.revoked_subjects: set[str] = set()
            self.revoked_subjects.add(target_subject)
            return {"revoked": True, "deleted": False, "subject": target_subject}

        subjects = set(self.owned_subjects) if self.overdelete else {target_subject}
        keys = [key for key in self.raw if key[0] in subjects and (self.overdelete or key[1] == target)]
        for key in keys:
            self.raw.pop(key, None)
            if not self.leaky or intent == "derived_purge":
                self.derived.pop(key, None)
        if intent in {"subject_erase", "derived_purge"}:
            for key in list(self.raw):
                if key[0] in subjects:
                    self.raw.pop(key, None)
            if not self.leaky or intent == "derived_purge":
                for key in list(self.derived):
                    if key[0] in subjects:
                        self.derived.pop(key, None)
        return {
            "deleted": True,
            "target": target,
            "target_subject": target_subject,
            "affected_subjects": sorted(subjects),
        }

    def handle_cleanup(self, params: dict[str, Any]) -> dict[str, Any]:
        # The in-memory backend has no external side effects. The ownership
        # check mirrors what remote adapters must enforce before deletion.
        requested = str(params.get("run_id", ""))
        if requested and requested != self.run_id:
            raise AdapterError("cleanup run_id does not match owner", "ownership_required")
        self.raw.clear()
        self.derived.clear()
        self.fixtures.clear()
        self.owned_subjects.clear()
        return {"cleaned": True, "ownership_token": f"memoryproof:{self.run_id}"}

    def _probe_subject(self, probe: dict[str, Any], fixture_id: str) -> str:
        explicit = str(probe.get("as_subject", ""))
        if explicit:
            return explicit
        fixture = self.fixtures.get(fixture_id) or {}
        return str(fixture.get("subject", ""))


def main() -> None:
    parser = argparse.ArgumentParser(description="MemoryProof reference adapter")
    parser.add_argument(
        "--mode",
        choices=["clean", "leaky", "overdelete", "slow", "crash", "malformed"],
        default=None,
    )
    parser.parse_args()
    ReferenceAdapter().serve()


if __name__ == "__main__":
    main()
