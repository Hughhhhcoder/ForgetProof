from __future__ import annotations

from typing import Any
from urllib.parse import quote

from .remote import RemoteAdapter


class ZepAdapter(RemoteAdapter):
    backend = "zep"
    version = "api-compatible"
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
        "derived_delete",
        "async_settle",
        "isolated_namespace",
    )

    def __init__(self) -> None:
        super().__init__()
        self.user_id = ""
        self.thread_id = ""

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        self.user_id = f"forgetproof-{self.run_id}"
        user_result = self.request(
            "POST",
            self.endpoint("user_create", "/api/v2/users"),
            {"user_id": self.user_id, "first_name": "ForgetProof", "last_name": "Test"},
        )
        self.request(
            "POST",
            self.endpoint("thread_create", "/api/v2/threads"),
            {"thread_id": self.user_id, "user_id": self.user_id},
        )
        self.thread_id = self.user_id
        return {"namespace": self.user_id, "user_id": self.user_id, "user_response": bool(user_result)}

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        return self.request(
            "POST",
            self.endpoint("episode_add", f"/api/v2/threads/{self.thread_id}/messages"),
            {
                "messages": [{"role": "user", "content": fixture["content"]}],
                "metadata": {"forgetproof_run": self.run_id, "forgetproof_fixture": fixture["id"]},
            },
        )

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        return self.request(
            "GET",
            self.endpoint("search", f"/api/v2/threads/{self.thread_id}/search")
            + "?" + "query=" + quote(probe.get("query") or fixture["content"]),
        )

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        return self.request(
            "GET",
            self.endpoint("graph", f"/api/v2/users/{self.user_id}/graph"),
        )

    def erase_remote(self, fixture: dict[str, Any], intent: str) -> Any:
        if intent in {"subject_erase", "derived_purge"}:
            return self.request("DELETE", self.endpoint("user_delete", f"/api/v2/users/{self.user_id}"))
        remote_id = self.remote_ids.get(fixture["id"], fixture["id"])
        return self.request(
            "DELETE",
            self.endpoint("episode_delete", f"/api/v2/threads/{self.thread_id}/messages/{remote_id}"),
        )

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        if self.user_id:
            self.request("DELETE", self.endpoint("user_delete", f"/api/v2/users/{self.user_id}"))
        return {"cleaned": True, "user_id": self.user_id}


def main() -> None:
    ZepAdapter().serve()


if __name__ == "__main__":
    main()
