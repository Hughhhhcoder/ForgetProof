from __future__ import annotations

from typing import Any

from .remote import RemoteAdapter


class Mem0Adapter(RemoteAdapter):
    backend = "mem0"
    version = "api-compatible"
    api_key_env = "MEM0_API_KEY"
    base_url_env = "MEM0_BASE_URL"
    default_base_url = "http://localhost:8888"
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

    def endpoint(self, key: str, default: str) -> str:
        configured = self.config.get(f"endpoint_{key}")
        if configured:
            return configured
        if self.mode in {"platform", "cloud"}:
            platform_defaults = {
                "add": "/v1/memories",
                "search": "/v1/memories/search",
                "list": "/v1/memories",
                "delete": "/v1/memories/{memory_id}",
                "scope_delete": "/v1/memories",
            }
            return platform_defaults.get(key, default)
        return default

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        return self.request(
            "POST",
            self.endpoint("add", "/memories"),
            {
                "messages": [{"role": "user", "content": fixture["content"]}],
                "user_id": self.run_id,
                "metadata": {"forgetproof_run": self.run_id, "forgetproof_fixture": fixture["id"]},
            },
        )

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        kind = str(probe.get("kind", "exact"))
        endpoint = self.endpoint("search", "/search")
        return self.request(
            "POST",
            endpoint,
            {
                "query": probe.get("query") or fixture["content"],
                "user_id": self.run_id,
                "limit": 20,
                "mode": kind,
            },
        )

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        endpoint = self.endpoint("list", "/memories")
        separator = "&" if "?" in endpoint else "?"
        return self.request("GET", f"{endpoint}{separator}user_id={self.run_id}")

    def erase_remote(self, fixture: dict[str, Any], intent: str) -> Any:
        remote_id = self.remote_ids.get(fixture["id"], fixture["id"])
        if intent in {"subject_erase", "derived_purge"}:
            endpoint = self.endpoint("scope_delete", "/memories")
            separator = "&" if "?" in endpoint else "?"
            return self.request("DELETE", f"{endpoint}{separator}user_id={self.run_id}")
        endpoint = self.endpoint("delete", f"/memories/{remote_id}")
        endpoint = endpoint.replace("{memory_id}", remote_id)
        return self.request("DELETE", endpoint)

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        # The namespace is generated per run. Cleanup is intentionally scoped
        # to the synthetic user_id and never issues a provider-wide reset.
        endpoint = self.endpoint("scope_delete", "/memories")
        separator = "&" if "?" in endpoint else "?"
        self.request("DELETE", f"{endpoint}{separator}user_id={self.run_id}")
        return {"cleaned": True, "scope": self.run_id}


def main() -> None:
    Mem0Adapter().serve()


if __name__ == "__main__":
    main()
