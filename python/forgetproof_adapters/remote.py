from __future__ import annotations

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from typing import Any

from .protocol import AdapterError, AdapterServer


class RemoteAdapter(AdapterServer):
    """Dependency-free HTTP adapter base.

    Endpoint paths are intentionally configurable. Hosted and self-hosted
    deployments often expose the same concepts under different prefixes.
    """

    api_key_env = ""
    base_url_env = ""
    default_base_url = ""

    def __init__(self) -> None:
        super().__init__()
        self.base_url = (
            self.config.get("base_url")
            or os.environ.get(self.base_url_env, "")
            or self.default_base_url
        ).rstrip("/")
        self.run_id = ""
        self.fixtures: dict[str, dict[str, Any]] = {}
        self.remote_ids: dict[str, str] = {}

    def endpoint(self, key: str, default: str) -> str:
        return self.config.get(f"endpoint_{key}", default)

    def _headers(self) -> dict[str, str]:
        headers = {"Accept": "application/json", "Content-Type": "application/json"}
        env_name = self.config.get("api_key_env", self.api_key_env)
        if env_name:
            key = os.environ.get(env_name, "")
            if key:
                headers["Authorization"] = f"Bearer {key}"
                headers["X-API-Key"] = key
        return headers

    def request(self, method: str, path: str, body: Any | None = None) -> Any:
        if not self.base_url:
            raise AdapterError(
                "no base_url configured; set it in scenario adapter.config or the adapter environment",
                "missing_base_url",
            )
        if path.startswith("http://") or path.startswith("https://"):
            url = path
        else:
            url = f"{self.base_url}/{path.lstrip('/')}"
        data = None if body is None else json.dumps(body).encode("utf-8")
        request = urllib.request.Request(url, data=data, method=method, headers=self._headers())
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read()
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")[:500]
            raise AdapterError(f"HTTP {exc.code} from {method} {path}: {detail}", "http_error") from exc
        except urllib.error.URLError as exc:
            raise AdapterError(f"request failed for {method} {path}: {exc.reason}", "network_error") from exc
        if not raw:
            return {}
        try:
            return json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError:
            return {"text": raw.decode("utf-8", errors="replace")[:500]}

    def handle_prepare(self, params: dict[str, Any]) -> dict[str, Any]:
        self.run_id = str(params.get("run_id", ""))
        self.fixtures.clear()
        self.remote_ids.clear()
        return self.prepare_remote(params)

    def handle_ingest(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture = params.get("fixture") or {}
        fixture_id = str(fixture.get("id", ""))
        if not fixture_id or not fixture.get("content"):
            raise AdapterError("fixture requires id and content", "invalid_fixture")
        self.fixtures[fixture_id] = fixture
        result = self.ingest_remote(fixture)
        remote_id = extract_id(result)
        if remote_id:
            self.remote_ids[fixture_id] = remote_id
        return {"stored": True, "fixture": fixture_id, "remote_id": remote_id, "response": summarize(result)}

    def handle_probe(self, params: dict[str, Any]) -> dict[str, Any]:
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        fixture = self.fixtures.get(fixture_id)
        if fixture is None:
            raise AdapterError(f"unknown fixture: {fixture_id}", "unknown_fixture")
        result = self.probe_remote(fixture, probe)
        return {"found": extract_found(result, fixture), "response": summarize(result)}

    def handle_inspect(self, params: dict[str, Any]) -> dict[str, Any]:
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        fixture = self.fixtures.get(fixture_id)
        if fixture is None:
            raise AdapterError(f"unknown fixture: {fixture_id}", "unknown_fixture")
        result = self.inspect_remote(fixture, probe)
        return {"found": extract_found(result, fixture), "response": summarize(result)}

    def handle_erase(self, params: dict[str, Any]) -> dict[str, Any]:
        target = str(params.get("target", ""))
        intent = str(params.get("intent", "object_delete"))
        fixture = self.fixtures.get(target)
        if fixture is None:
            raise AdapterError(f"unknown fixture: {target}", "unknown_fixture")
        result = self.erase_remote(fixture, intent)
        return {"deleted": True, "target": target, "response": summarize(result)}

    def handle_settle(self, params: dict[str, Any]) -> dict[str, Any]:
        return self.settle_remote(params)

    def handle_cleanup(self, params: dict[str, Any]) -> dict[str, Any]:
        return self.cleanup_remote(params)

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"namespace": self.run_id}

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        raise AdapterError("ingest mapping is not implemented", "unsupported_method")

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        raise AdapterError("probe mapping is not implemented", "unsupported_method")

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any]) -> Any:
        raise AdapterError("inspect mapping is not implemented", "unsupported_method")

    def erase_remote(self, fixture: dict[str, Any], intent: str) -> Any:
        raise AdapterError("erase mapping is not implemented", "unsupported_method")

    def settle_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"stable": True}

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"cleaned": True}


def extract_id(value: Any) -> str:
    if isinstance(value, dict):
        for key in ("id", "memory_id", "uuid", "message_id", "node_id"):
            candidate = value.get(key)
            if candidate:
                return str(candidate)
        for child in value.values():
            found = extract_id(child)
            if found:
                return found
    elif isinstance(value, list):
        for child in value:
            found = extract_id(child)
            if found:
                return found
    return ""


def extract_found(value: Any, fixture: dict[str, Any]) -> bool:
    if isinstance(value, dict):
        if isinstance(value.get("found"), bool):
            return value["found"]
        for key in ("results", "memories", "data", "facts", "nodes", "messages", "edges"):
            if key in value and extract_found(value[key], fixture):
                return True
        fixture_id = str(fixture.get("id", ""))
        content = str(fixture.get("content", ""))
        for key in ("id", "memory_id", "uuid", "fixture_id", "text", "memory", "content", "summary"):
            candidate = value.get(key)
            if candidate is not None and (str(candidate) == fixture_id or content in str(candidate)):
                return True
    elif isinstance(value, list):
        return any(extract_found(child, fixture) for child in value)
    elif isinstance(value, str):
        return str(fixture.get("content", "")) in value
    return False


def summarize(value: Any) -> Any:
    if isinstance(value, dict):
        return {str(key): summarize(child) for key, child in list(value.items())[:20] if key not in {"content", "text", "memory", "messages"}}
    if isinstance(value, list):
        return {"count": len(value)}
    if isinstance(value, str):
        return {"sha256": __import__("hashlib").sha256(value.encode()).hexdigest(), "length": len(value)}
    return value
