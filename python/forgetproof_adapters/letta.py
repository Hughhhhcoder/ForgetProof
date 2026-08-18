from __future__ import annotations

from typing import Any
from urllib.parse import quote

from .protocol import AdapterError
from .remote import RemoteAdapter, extract_id


class LettaAdapter(RemoteAdapter):
    backend = "letta"
    version = "api-compatible"
    api_key_env = "LETTA_API_KEY"
    base_url_env = "LETTA_BASE_URL"
    default_base_url = "http://localhost:8283"
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
        self.agent_id = ""

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        requested = self.config.get("agent_id", "")
        if requested:
            self.agent_id = requested
        else:
            result = self.request(
                "POST",
                self.endpoint("agent_create", "/v1/agents"),
                {"name": f"forgetproof-{self.run_id}", "description": "ForgetProof isolated test agent"},
            )
            self.agent_id = extract_id(result)
        if not self.agent_id:
            raise AdapterError("Letta agent creation did not return an id", "invalid_response")
        return {"namespace": self.agent_id, "agent_id": self.agent_id}

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        return self.request(
            "POST",
            self.endpoint("archival_insert", f"/v1/agents/{self.agent_id}/archival-memory"),
            {
                "text": fixture["content"],
                "metadata": {"forgetproof_run": self.run_id, "forgetproof_fixture": fixture["id"]},
            },
        )

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        endpoint = self.endpoint("archival_search", f"/v1/agents/{self.agent_id}/archival-memory/search")
        return self.request(
            "GET",
            endpoint + "?query=" + quote(probe.get("query") or fixture["content"]) + "&top_k=20",
        )

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        return self.request(
            "GET",
            self.endpoint("archival_list", f"/v1/agents/{self.agent_id}/archival-memory"),
        )

    def handle_agent_query(self, params: dict[str, Any]) -> dict[str, Any]:
        probe = params.get("probe") or {}
        fixture = self.fixtures.get(str(probe.get("fixture", "")))
        if fixture is None:
            raise AdapterError("unknown fixture", "unknown_fixture")
        result = self.request(
            "POST",
            self.endpoint("agent_message", f"/v1/agents/{self.agent_id}/messages"),
            {"messages": [{"role": "user", "content": f"Recall: {fixture['content']}"}]},
        )
        return {"found": fixture["content"] in str(result), "response": "redacted"}

    def erase_remote(self, fixture: dict[str, Any], intent: str) -> Any:
        if intent in {"subject_erase", "derived_purge"}:
            endpoint = self.endpoint("agent_delete", f"/v1/agents/{self.agent_id}")
            return self.request("DELETE", endpoint)
        remote_id = self.remote_ids.get(fixture["id"], fixture["id"])
        return self.request(
            "DELETE",
            self.endpoint("archival_delete", f"/v1/agents/{self.agent_id}/archival-memory/{remote_id}"),
        )

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        if self.agent_id:
            self.request("DELETE", self.endpoint("agent_delete", f"/v1/agents/{self.agent_id}"))
        return {"cleaned": True, "agent_id": self.agent_id}


def main() -> None:
    LettaAdapter().serve()


if __name__ == "__main__":
    main()
