from __future__ import annotations

import re
from typing import Any
from urllib.parse import urlencode

from .protocol import AdapterError
from .remote import RemoteAdapter, extract_id


class ZepAdapter(RemoteAdapter):
    backend = "zep"
    version = "configured"
    api_key_env = "ZEP_API_KEY"
    base_url_env = "ZEP_BASE_URL"
    default_base_url = "http://localhost:8000"
    capabilities = (
        "object_delete",
        "scope_delete",
        "probe",
        "lexical_search",
        "semantic_search",
        "inspect",
        "derived_inspect",
        "async_settle",
        "isolated_namespace",
    )
    modes = ("self-hosted", "cloud")

    def __init__(self) -> None:
        super().__init__()
        self.thread_ids: dict[str, str] = {}

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        subjects = sorted(
            {
                str(item.get("subject"))
                for item in params.get("fixtures", [])
                if isinstance(item, dict) and item.get("subject")
            }
        )
        for subject in subjects:
            user_id = f"memoryproof-{self.run_id}-{_safe(subject)}"
            user_result = self.request(
                "POST",
                self.endpoint("user_create", "/api/v2/users"),
                {"user_id": user_id, "first_name": "MemoryProof", "last_name": "Test"},
            )
            self.subject_ids[subject] = str(
                user_result.get("user_id") if isinstance(user_result, dict) else user_id
            )
            thread_id = f"{user_id}-thread"
            self.request(
                "POST",
                self.endpoint("thread_create", "/api/v2/threads"),
                {"thread_id": thread_id, "user_id": self.subject_ids[subject]},
            )
            self.thread_ids[subject] = thread_id
            self.created_resources.extend([self.subject_ids[subject], thread_id])
        return {
            "namespace": self.run_id,
            "owned_users": self.subject_ids,
            "owned_threads": self.thread_ids,
            "ownership_token": f"memoryproof:{self.run_id}",
        }

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        thread_id = self._thread_for(str(fixture["subject"]))
        return self.request(
            "POST",
            self.endpoint("episode_add", f"/api/v2/threads/{thread_id}/messages"),
            {
                "messages": [{"role": "user", "content": fixture["content"]}],
                "metadata": {
                    "memoryproof_run": self.run_id,
                    "memoryproof_fixture": fixture["id"],
                    "memoryproof_subject": fixture["subject"],
                },
            },
        )

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        thread_id = self._thread_for(subject)
        query = str(probe.get("query") or fixture["content"])
        return self.request(
            "GET",
            f"{self.endpoint('search', f'/api/v2/threads/{thread_id}/search')}?{urlencode({'query': query})}",
        )

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        user_id = self._user_for(subject)
        return self.request(
            "GET",
            self.endpoint("graph", f"/api/v2/users/{user_id}/graph"),
        )

    def erase_remote(self, fixture: dict[str, Any], intent: str, subject: str) -> Any:
        user_id = self._user_for(subject)
        if intent in {"subject_erase"}:
            return self.request("DELETE", self.endpoint("user_delete", f"/api/v2/users/{user_id}"))
        if intent == "derived_purge":
            raise AdapterError(
                "Zep requires a configured graph-derived deletion mapping for derived_purge",
                "unsupported_capability",
            )
        remote_id = self.remote_ids.get(fixture["id"])
        if not remote_id:
            raise AdapterError("episode response did not return an id", "invalid_response")
        thread_id = self._thread_for(subject)
        return self.request(
            "DELETE",
            self.endpoint(
                "episode_delete",
                f"/api/v2/threads/{thread_id}/messages/{remote_id}",
            ),
        )

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        failures: list[str] = []
        for user_id in set(self.subject_ids.values()):
            try:
                self.request("DELETE", self.endpoint("user_delete", f"/api/v2/users/{user_id}"))
            except AdapterError as exc:
                if "HTTP 404" not in str(exc):
                    failures.append(str(exc))
        if failures:
            raise AdapterError("; ".join(failures), "cleanup_error")
        return {"cleaned": True, "owned_users": sorted(self.subject_ids.values())}

    def _user_for(self, subject: str) -> str:
        user_id = self.subject_ids.get(subject)
        if not user_id:
            raise AdapterError(f"no owned Zep user for subject '{subject}'", "ownership_required")
        return user_id

    def _thread_for(self, subject: str) -> str:
        thread_id = self.thread_ids.get(subject)
        if not thread_id:
            raise AdapterError(f"no owned Zep thread for subject '{subject}'", "ownership_required")
        return thread_id


def _safe(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_-]+", "-", value).strip("-") or "subject"


def main() -> None:
    ZepAdapter().serve()


if __name__ == "__main__":
    main()
