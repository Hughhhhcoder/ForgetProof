from __future__ import annotations

import re
from typing import Any
from urllib.parse import quote

from .protocol import AdapterError
from .remote import RemoteAdapter, extract_id


class LettaAdapter(RemoteAdapter):
    backend = "letta"
    version = "configured"
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
        "agent_query",
        "async_settle",
        "isolated_namespace",
    )
    modes = ("self-hosted", "cloud")

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        if self.config.get("agent_id"):
            raise AdapterError(
                "refusing to delete a configured existing Agent; omit agent_id so MemoryProof can own temporary Agents",
                "ownership_required",
            )
        subjects = sorted(
            {
                str(item.get("subject"))
                for item in params.get("fixtures", [])
                if isinstance(item, dict) and item.get("subject")
            }
        )
        for subject in subjects:
            result = self.request(
                "POST",
                self.endpoint("agent_create", "/v1/agents"),
                {
                    "name": f"memoryproof-{self.run_id}-{_safe(subject)}",
                    "description": "Temporary MemoryProof isolation test Agent",
                },
            )
            agent_id = extract_id(result)
            if not agent_id:
                raise AdapterError("Letta Agent creation did not return an id", "invalid_response")
            self.subject_ids[subject] = agent_id
            self.created_resources.append(agent_id)
        return {
            "namespace": self.run_id,
            "owned_agents": self.subject_ids,
            "ownership_token": f"memoryproof:{self.run_id}",
        }

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        agent_id = self._agent_for(str(fixture["subject"]))
        return self.request(
            "POST",
            self.endpoint("archival_insert", f"/v1/agents/{agent_id}/archival-memory"),
            {
                "text": fixture["content"],
                "metadata": {
                    "memoryproof_run": self.run_id,
                    "memoryproof_fixture": fixture["id"],
                    "memoryproof_subject": fixture["subject"],
                },
            },
        )

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        agent_id = self._agent_for(subject)
        endpoint = self.endpoint(
            "archival_search",
            f"/v1/agents/{agent_id}/archival-memory/search",
        )
        query = quote(str(probe.get("query") or fixture["content"]))
        return self.request("GET", f"{endpoint}?query={query}&top_k=20")

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        agent_id = self._agent_for(subject)
        return self.request(
            "GET",
            self.endpoint("archival_list", f"/v1/agents/{agent_id}/archival-memory"),
        )

    def agent_query_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        agent_id = self._agent_for(subject)
        query = str(probe.get("query") or "What do you remember that is unique to this test?")
        return self.request(
            "POST",
            self.endpoint("agent_message", f"/v1/agents/{agent_id}/messages"),
            {"messages": [{"role": "user", "content": query}]},
        )

    def erase_remote(self, fixture: dict[str, Any], intent: str, subject: str) -> Any:
        agent_id = self._agent_for(subject)
        if intent in {"subject_erase", "derived_purge"}:
            if intent == "derived_purge":
                raise AdapterError(
                    "Letta requires a configured derived-artifact deletion mapping for derived_purge",
                    "unsupported_capability",
                )
            return self.request("DELETE", self.endpoint("agent_delete", f"/v1/agents/{agent_id}"))
        remote_id = self.remote_ids.get(fixture["id"])
        if not remote_id:
            raise AdapterError("archival insert response did not return a passage id", "invalid_response")
        return self.request(
            "DELETE",
            self.endpoint(
                "archival_delete",
                f"/v1/agents/{agent_id}/archival-memory/{remote_id}",
            ),
        )

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        failures: list[str] = []
        for agent_id in set(self.subject_ids.values()):
            try:
                self.request("DELETE", self.endpoint("agent_delete", f"/v1/agents/{agent_id}"))
            except AdapterError as exc:
                if "HTTP 404" not in str(exc):
                    failures.append(str(exc))
        if failures:
            raise AdapterError("; ".join(failures), "cleanup_error")
        return {"cleaned": True, "owned_agents": sorted(self.subject_ids.values())}

    def _agent_for(self, subject: str) -> str:
        agent_id = self.subject_ids.get(subject)
        if not agent_id:
            raise AdapterError(f"no owned Letta Agent for subject '{subject}'", "ownership_required")
        return agent_id


def _safe(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_-]+", "-", value).strip("-") or "subject"


def main() -> None:
    LettaAdapter().serve()


if __name__ == "__main__":
    main()
