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

    def __init__(self) -> None:
        super().__init__()
        self.block_ids: dict[str, str] = {}

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
        self.block_ids.clear()
        for subject in subjects:
            fixture = next(
                item
                for item in params.get("fixtures", [])
                if str(item.get("subject")) == subject
            )
            block_result = self.request(
                "POST",
                self.endpoint("block_create", "/v1/blocks/"),
                {
                    "label": f"memoryproof-{_safe(subject)}",
                    "value": str(fixture.get("content", "")),
                    "description": "Temporary MemoryProof core-memory block",
                    "metadata": {
                        "memoryproof_run": self.run_id,
                        "memoryproof_subject": subject,
                    },
                },
            )
            block_id = extract_id(block_result)
            if not block_id:
                raise AdapterError("Letta block creation did not return an id", "invalid_response")
            self.block_ids[subject] = block_id
            self.created_resources.append(block_id)
            result = self.request(
                "POST",
                self.endpoint("agent_create", "/v1/agents"),
                {
                    "name": f"memoryproof-{self.run_id}-{_safe(subject)}",
                    "description": "Temporary MemoryProof isolation test Agent",
                    "block_ids": [block_id],
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
            "owned_blocks": self.block_ids,
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
        try:
            return self.request("GET", f"{endpoint}?query={query}&top_k=20")
        except AdapterError as exc:
            if str(exc).startswith("HTTP 404"):
                return {"results": []}
            raise

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        agent_id = self._agent_for(subject)
        try:
            archival = self.request(
                "GET",
                self.endpoint("archival_list", f"/v1/agents/{agent_id}/archival-memory"),
            )
        except AdapterError as exc:
            if str(exc).startswith("HTTP 404"):
                archival = {"results": []}
            else:
                raise
        blocks: list[Any] = []
        block_id = self.block_ids.get(subject)
        if block_id:
            try:
                blocks.append(
                    self.request(
                        "GET",
                        self.endpoint("block_get", f"/v1/blocks/{block_id}"),
                    )
                )
            except AdapterError as exc:
                if not str(exc).startswith("HTTP 404"):
                    raise
        return {"archival": archival, "blocks": blocks}

    def agent_query_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        agent_id = self._agent_for(subject)
        query = str(probe.get("query") or "What do you remember that is unique to this test?")
        try:
            return self.request(
                "POST",
                self.endpoint("agent_message", f"/v1/agents/{agent_id}/messages"),
                {"messages": [{"role": "user", "content": query}]},
            )
        except AdapterError as exc:
            if str(exc).startswith("HTTP 404"):
                return {"messages": []}
            raise

    def erase_remote(self, fixture: dict[str, Any], intent: str, subject: str) -> Any:
        agent_id = self._agent_for(subject)
        if intent in {"subject_erase", "derived_purge"}:
            if intent == "derived_purge":
                raise AdapterError(
                    "Letta requires a configured derived-artifact deletion mapping for derived_purge",
                    "unsupported_capability",
                )
            result = self.request("DELETE", self.endpoint("agent_delete", f"/v1/agents/{agent_id}"))
            block_id = self.block_ids.get(subject)
            if block_id:
                try:
                    self.request(
                        "DELETE",
                        self.endpoint("block_delete", f"/v1/blocks/{block_id}"),
                    )
                except AdapterError as exc:
                    if not str(exc).startswith("HTTP 404"):
                        raise
            return {"agent": result, "block_deleted": bool(block_id)}
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
        for block_id in set(self.block_ids.values()):
            try:
                self.request("DELETE", self.endpoint("block_delete", f"/v1/blocks/{block_id}"))
            except AdapterError as exc:
                if "HTTP 404" not in str(exc):
                    failures.append(str(exc))
        if failures:
            raise AdapterError("; ".join(failures), "cleanup_error")
        return {
            "cleaned": True,
            "owned_agents": sorted(self.subject_ids.values()),
            "owned_blocks": sorted(self.block_ids.values()),
        }

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
