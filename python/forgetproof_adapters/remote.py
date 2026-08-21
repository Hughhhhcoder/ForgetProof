from __future__ import annotations

import hashlib
import json
import os
import re
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Any

from .protocol import AdapterError, AdapterServer


class RemoteAdapter(AdapterServer):
    """Dependency-free HTTP adapter base with ownership and redaction guards."""

    api_key_env = ""
    base_url_env = ""
    default_base_url = ""
    request_id_headers = ("x-request-id", "x-correlation-id", "traceparent")

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
        self.subject_ids: dict[str, str] = {}
        self.created_resources: list[str] = []
        self.request_ids: list[str] = []
        self.pending_operations: list[dict[str, Any]] = []
        self.owned = False

    def endpoint(self, key: str, default: str) -> str:
        return self.config.get(f"endpoint_{key}", default)

    def _headers(self) -> dict[str, str]:
        headers = {"Accept": "application/json", "Content-Type": "application/json"}
        env_name = self.config.get("api_key_env", self.api_key_env)
        key = os.environ.get(env_name, "") if env_name else ""
        if key:
            default_header, default_scheme = self.auth_defaults()
            auth_header = self.config.get("api_key_header", default_header)
            scheme = self.config.get("api_key_scheme", default_scheme)
            headers[auth_header] = f"{scheme} {key}" if scheme else key
        return headers

    def auth_defaults(self) -> tuple[str, str]:
        """Return the provider's default API-key header and scheme."""

        return "Authorization", "Bearer"

    def request(self, method: str, path: str, body: Any | None = None) -> Any:
        if not self.base_url:
            raise AdapterError(
                "no base_url configured; set adapter.config.base_url or the adapter environment",
                "missing_base_url",
            )
        url = path if path.startswith(("http://", "https://")) else f"{self.base_url}/{path.lstrip('/')}"
        data = None if body is None else json.dumps(body).encode("utf-8")
        request = urllib.request.Request(url, data=data, method=method, headers=self._headers())
        timeout = float(self.config.get("request_timeout_sec", "30"))
        open_request = urllib.request.urlopen
        hostname = urllib.parse.urlparse(url).hostname
        if hostname in {"localhost", "127.0.0.1", "::1"}:
            # Local self-hosted deployments and the deterministic contract
            # mock must not be routed through a workstation HTTP proxy.
            open_request = urllib.request.build_opener(urllib.request.ProxyHandler({})).open
        try:
            with open_request(request, timeout=timeout) as response:
                raw = response.read()
                for name in self.request_id_headers:
                    request_id = response.headers.get(name)
                    if request_id:
                        self.request_ids.append(request_id[:200])
                        break
                status = response.status
        except urllib.error.HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")[:300]
            raise AdapterError(
                f"HTTP {exc.code} from {method} {path}: {detail}",
                "http_error",
            ) from exc
        except urllib.error.URLError as exc:
            raise AdapterError(
                f"request failed for {method} {path}: {exc.reason}",
                "network_error",
            ) from exc
        if not raw:
            return {"accepted": status < 400, "status": status}
        try:
            return json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError as exc:
            raise AdapterError(
                f"backend returned non-JSON data for {method} {path}",
                "invalid_response",
            ) from exc

    def handle_prepare(self, params: dict[str, Any]) -> dict[str, Any]:
        self.run_id = str(params.get("run_id", ""))
        if not self.run_id:
            raise AdapterError("prepare requires run_id", "invalid_prepare")
        self.fixtures.clear()
        self.remote_ids.clear()
        self.subject_ids.clear()
        self.created_resources.clear()
        self.pending_operations.clear()
        self.request_ids.clear()
        self.owned = True
        result = self.prepare_remote(params)
        result.setdefault("ownership_token", f"memoryproof:{self.run_id}")
        result.setdefault("request_ids", list(self.request_ids))
        return result

    def handle_ingest(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture = params.get("fixture") or {}
        fixture_id = str(fixture.get("id", ""))
        if not fixture_id or not fixture.get("content") or not fixture.get("subject"):
            raise AdapterError("fixture requires id, content, and subject", "invalid_fixture")
        if not self.owned:
            raise AdapterError("run does not own a prepared namespace", "ownership_required")
        self.fixtures[fixture_id] = fixture
        result = self.ingest_remote(fixture)
        remote_id = extract_id(result)
        if remote_id:
            self.remote_ids[fixture_id] = remote_id
            self.created_resources.append(remote_id)
        self._register_pending(result)
        return {
            "stored": True,
            "fixture": fixture_id,
            "subject": fixture["subject"],
            "remote_id": remote_id,
            "request_ids": list(self.request_ids),
            "response": summarize(result),
        }

    def handle_probe(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture_id, fixture, probe = self._fixture_probe(params)
        result = self.probe_remote(fixture, probe, str(probe.get("as_subject") or fixture["subject"]))
        return {
            "found": extract_found(result, fixture),
            "request_ids": list(self.request_ids),
            "response": summarize(result),
        }

    def handle_inspect(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture_id, fixture, probe = self._fixture_probe(params)
        result = self.inspect_remote(fixture, probe, str(probe.get("as_subject") or fixture["subject"]))
        return {
            "found": extract_found(result, fixture),
            "request_ids": list(self.request_ids),
            "response": summarize(result),
        }

    def handle_agent_query(self, params: dict[str, Any]) -> dict[str, Any]:
        fixture_id, fixture, probe = self._fixture_probe(params)
        result = self.agent_query_remote(fixture, probe, str(probe.get("as_subject") or fixture["subject"]))
        return {
            "found": extract_found(result, fixture),
            "request_ids": list(self.request_ids),
            "response": summarize(result),
        }

    def handle_erase(self, params: dict[str, Any]) -> dict[str, Any]:
        target = str(params.get("target", ""))
        intent = str(params.get("intent", "object_delete"))
        target_subject = str(params.get("target_subject", ""))
        fixture = self.fixtures.get(target)
        if fixture is None:
            raise AdapterError(f"unknown fixture: {target}", "unknown_fixture")
        if target_subject != fixture.get("subject"):
            raise AdapterError("erase target subject does not match owned fixture", "ownership_required")
        if not self.owned:
            raise AdapterError("run does not own a prepared namespace", "ownership_required")
        result = self.erase_remote(fixture, intent, target_subject)
        self._register_pending(result)
        return {
            "accepted": True,
            "target": target,
            "target_subject": target_subject,
            "request_ids": list(self.request_ids),
            "response": summarize(result),
        }

    def handle_settle(self, params: dict[str, Any]) -> dict[str, Any]:
        result = self.settle_remote(params)
        result.setdefault("request_ids", list(self.request_ids))
        return result

    def handle_cleanup(self, params: dict[str, Any]) -> dict[str, Any]:
        requested = str(params.get("run_id", ""))
        if requested and requested != self.run_id:
            raise AdapterError("cleanup run_id does not match owner", "ownership_required")
        if not self.owned:
            return {"cleaned": True, "owned": False}
        result = self.cleanup_remote(params)
        self.owned = False
        result.setdefault("request_ids", list(self.request_ids))
        return result

    def _fixture_probe(self, params: dict[str, Any]) -> tuple[str, dict[str, Any], dict[str, Any]]:
        probe = params.get("probe") or {}
        fixture_id = str(probe.get("fixture", ""))
        fixture = self.fixtures.get(fixture_id)
        if fixture is None:
            raise AdapterError(f"unknown fixture: {fixture_id}", "unknown_fixture")
        return fixture_id, fixture, probe

    def _register_pending(self, result: Any) -> None:
        if not isinstance(result, dict):
            return
        state = str(result.get("status") or result.get("state") or "").lower()
        operation_id = result.get("event_id") or result.get("job_id") or result.get("operation_id")
        if operation_id or state in {"pending", "processing", "queued", "accepted"}:
            self.pending_operations.append(
                {"id": str(operation_id or ""), "state": state, "result": result}
            )

    def prepare_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"namespace": self.run_id, "owned": True}

    def ingest_remote(self, fixture: dict[str, Any]) -> Any:
        raise AdapterError("ingest mapping is not implemented", "unsupported_method")

    def probe_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        raise AdapterError("probe mapping is not implemented", "unsupported_method")

    def inspect_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        raise AdapterError("inspect mapping is not implemented", "unsupported_method")

    def agent_query_remote(self, fixture: dict[str, Any], probe: dict[str, Any], subject: str) -> Any:
        raise AdapterError("agent query mapping is not implemented", "unsupported_method")

    def erase_remote(self, fixture: dict[str, Any], intent: str, subject: str) -> Any:
        raise AdapterError("erase mapping is not implemented", "unsupported_method")

    def settle_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        if not self.pending_operations:
            return {"state": "stable", "stable": True, "observations": 1}
        endpoint = self.config.get("settle_endpoint")
        if not endpoint:
            return {"state": "unknown", "stable": False, "observations": 1}
        timeout = max(1, int(params.get("timeout_ms", 10_000))) / 1000
        interval = max(0.01, int(params.get("interval_ms", 250)) / 1000)
        deadline = time.monotonic() + timeout
        observations = 0
        while time.monotonic() < deadline:
            observations += 1
            result = self.request(
                "GET",
                endpoint.replace("{run_id}", urllib.parse.quote(self.run_id)),
            )
            state = str(result.get("state") or result.get("status") or "unknown").lower()
            if state in {"stable", "complete", "completed", "done", "success"}:
                self.pending_operations.clear()
                return {"state": "stable", "stable": True, "observations": observations}
            if state in {"failed", "error"}:
                return {"state": "unknown", "stable": False, "observations": observations}
            time.sleep(interval)
        return {"state": "timeout", "stable": False, "observations": observations}

    def cleanup_remote(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"cleaned": True, "owned": True}


def extract_id(value: Any) -> str:
    if isinstance(value, dict):
        for key in ("id", "memory_id", "uuid", "message_id", "node_id", "episode_id", "agent_id"):
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


def _markers(content: str) -> list[str]:
    markers = re.findall(r"[A-Za-z0-9][A-Za-z0-9_-]{5,}", content)
    return list(dict.fromkeys(markers))


def extract_found(value: Any, fixture: dict[str, Any]) -> bool:
    content = str(fixture.get("content", ""))
    fixture_id = str(fixture.get("id", ""))
    markers = _markers(content)

    def visit(item: Any) -> bool:
        if isinstance(item, dict):
            if isinstance(item.get("found"), bool):
                return bool(item["found"])
            for key in (
                "results",
                "memories",
                "data",
                "facts",
                "nodes",
                "messages",
                "edges",
                "episodes",
                "blocks",
                "archival",
            ):
                if key in item and visit(item[key]):
                    return True
            for key in (
                "fixture_id",
                "text",
                "memory",
                "content",
                "summary",
                "fact",
                "name",
                "value",
                "description",
                "label",
            ):
                candidate = item.get(key)
                if candidate is not None and (content in str(candidate) or any(marker in str(candidate) for marker in markers)):
                    return True
            candidate_id = item.get("id") or item.get("memory_id") or item.get("uuid")
            return str(candidate_id) == fixture_id if candidate_id is not None else False
        if isinstance(item, list):
            return any(visit(child) for child in item)
        if isinstance(item, str):
            return content in item or any(marker in item for marker in markers)
        return False

    return visit(value)


def summarize(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            str(key): summarize(child)
            for key, child in list(value.items())[:30]
            if key.lower() not in {"content", "text", "memory", "messages", "headers", "authorization"}
        }
    if isinstance(value, list):
        return {"count": len(value)}
    if isinstance(value, str):
        return {"sha256": hashlib.sha256(value.encode()).hexdigest(), "length": len(value)}
    return value
