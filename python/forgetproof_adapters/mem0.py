from __future__ import annotations

import re
from typing import Any
from urllib.parse import urlencode

from .protocol import AdapterError
from .remote import RemoteAdapter


class Mem0Adapter(RemoteAdapter):
    backend = "mem0"
    version = "configured"
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
        "async_settle",
        "isolated_namespace",
    )
    modes = ("oss", "platform", "cloud")

    def auth_defaults(self) -> tuple[str, str]:
        if self.mode in {"platform", "cloud"}:
            return "Authorization", "Token"
        return "X-API-Key", ""

    def endpoint(self, key: str, default: str) -> str:
        configured = self.config.get(f"endpoint_{key}")
        if configured:
            return configured
        if self.mode in {"platform", "cloud"}:
            return {
                "add": "/v1/memories",
                "search": "/v1/memories/search",
                "list": "/v1/memories",
                "delete": "/v1/memories/{memory_id}",
                "scope_delete": "/v1/memories",
            }.get(key, default)
        return default

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        fixtures = params.get("fixtures") or []
        for item in fixtures:
            subject = str(item.get("subject", ""))
            if subject:
                self.subject_ids[subject] = self._subject_id(subject)
        return {
            "namespace": self.run_id,
            "owned_subjects": sorted(self.subject_ids.values()),
            "ownership_token": f"memoryproof:{self.run_id}",
        }

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        subject_id = self.subject_ids.get(str(fixture["subject"]), self._subject_id(str(fixture["subject"])))
        body: dict[str, Any] = {
            "messages": [{"role": "user", "content": fixture["content"]}],
            "user_id": subject_id,
            "metadata": {
                "memoryproof_run": self.run_id,
                "memoryproof_fixture": fixture["id"],
                "memoryproof_subject": fixture["subject"],
            },
        }
        infer = self.config.get("infer")
        if infer is not None:
            body["infer"] = infer.lower() == "true"
        return self.request("POST", self.endpoint("add", "/memories"), body)

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        subject_id = self.subject_ids.get(subject, self._subject_id(subject))
        query = probe.get("query") or fixture["content"]
        return self.request(
            "POST",
            self.endpoint("search", "/search"),
            {"query": query, "user_id": subject_id, "limit": 20},
        )

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        subject_id = self.subject_ids.get(subject, self._subject_id(subject))
        endpoint = self.endpoint("list", "/memories")
        separator = "&" if "?" in endpoint else "?"
        return self.request("GET", f"{endpoint}{separator}{urlencode({'user_id': subject_id})}")

    def erase_remote(self, fixture: dict[str, Any], intent: str, subject: str) -> Any:
        subject_id = self.subject_ids.get(subject, self._subject_id(subject))
        if intent in {"subject_erase"}:
            endpoint = self.endpoint("scope_delete", "/memories")
            separator = "&" if "?" in endpoint else "?"
            return self.request("DELETE", f"{endpoint}{separator}{urlencode({'user_id': subject_id})}")
        if intent == "derived_purge":
            raise AdapterError(
                "Mem0 does not expose a separately observable derived-delete endpoint",
                "unsupported_capability",
            )
        remote_id = self.remote_ids.get(fixture["id"])
        if not remote_id:
            raise AdapterError("memory create response did not return an id", "invalid_response")
        endpoint = self.endpoint("delete", f"/memories/{remote_id}").replace("{memory_id}", remote_id)
        return self.request("DELETE", endpoint)

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        failures: list[str] = []
        for subject_id in set(self.subject_ids.values()):
            endpoint = self.endpoint("scope_delete", "/memories")
            separator = "&" if "?" in endpoint else "?"
            try:
                self.request("DELETE", f"{endpoint}{separator}{urlencode({'user_id': subject_id})}")
            except AdapterError as exc:
                if "HTTP 404" not in str(exc):
                    failures.append(str(exc))
        if failures:
            raise AdapterError("; ".join(failures), "cleanup_error")
        return {"cleaned": True, "owned_subjects": sorted(self.subject_ids.values())}

    def _subject_id(self, subject: str) -> str:
        safe = re.sub(r"[^A-Za-z0-9_-]+", "-", subject).strip("-") or "subject"
        return f"memoryproof-{self.run_id}-{safe}"


def main() -> None:
    Mem0Adapter().serve()


if __name__ == "__main__":
    main()
