from __future__ import annotations

import argparse
from typing import Any

from .protocol import AdapterServer


class ReferenceAdapter(AdapterServer):
    backend = "forgetproof-reference"
    version = "0.1.0"
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

    def __init__(self) -> None:
        super().__init__()
        self.leaky = self.mode == "leaky"
        self.run_id = ""
        self.raw: dict[str, str] = {}
        self.derived: dict[str, str] = {}
        self.targets: set[str] = set()

    def handle_prepare(self, params: dict[str, Any]) -> dict[str, Any]:
        self.run_id = str(params.get("run_id", ""))
        self.raw.clear()
        self.derived.clear()
        self.targets.clear()
        return {"namespace": f"reference:{self.run_id}"}

    def handle_ingest(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture = params.get("fixture") or {}
        fixture_id = str(fixture.get("id", ""))
        content = str(fixture.get("content", ""))
        if not fixture_id or not content:
            raise ValueError("fixture requires id and content")
        self.raw[fixture_id] = content
        self.derived[fixture_id] = content
        if fixture.get("target"):
            self.targets.add(fixture_id)
        return {"stored": True, "fixture": fixture_id}

    def handle_probe(self, params: dict[str, Any]) -> dict[str, Any]:
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        kind = str(probe.get("kind", "exact"))
        found = fixture_id in self.raw
        if kind in {"lexical", "semantic"}:
            found = fixture_id in self.raw or fixture_id in self.derived
        if kind == "exact":
            found = fixture_id in self.raw or (self.leaky and fixture_id in self.derived)
        return {"found": found, "scope": "reference"}

    def handle_inspect(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture_id = str((params.get("probe") or {}).get("fixture", ""))
        # Inspect represents observable derived artifacts, not raw memory.
        return {"found": fixture_id in self.derived, "artifact_types": ["summary", "index"]}

    def handle_agent_query(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture_id = str((params.get("probe") or {}).get("fixture", ""))
        found = fixture_id in self.raw or fixture_id in self.derived
        return {"found": found, "response_contains_canary": found}

    def handle_erase(self, params: dict[str, Any]) -> dict[str, Any]:
        target = str(params.get("target", ""))
        intent = str(params.get("intent", "object_delete"))
        if intent == "access_revoke":
            return {"revoked": True, "deleted": False}
        self.raw.pop(target, None)
        if not self.leaky or intent == "derived_purge":
            self.derived.pop(target, None)
        return {"deleted": True, "target": target}

    def handle_cleanup(self, params: dict[str, Any]) -> dict[str, Any]:
        self.raw.clear()
        self.derived.clear()
        self.targets.clear()
        return {"cleaned": True}


def main() -> None:
    parser = argparse.ArgumentParser(description="ForgetProof reference adapter")
    parser.add_argument("--mode", choices=["clean", "leaky"], default=None)
    parser.parse_args()
    ReferenceAdapter().serve()


if __name__ == "__main__":
    main()
