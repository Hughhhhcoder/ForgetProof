from __future__ import annotations

import json
import logging
import os
import sys
from typing import Any

PROTOCOL_VERSION = "memoryproof.adapter/v1"
LEGACY_PROTOCOL_VERSION = "forgetproof.adapter/v1alpha1"
log = logging.getLogger("memoryproof.adapter")


class AdapterError(Exception):
    """An expected, user-facing adapter error."""

    def __init__(self, message: str, code: str = "adapter_error") -> None:
        super().__init__(message)
        self.code = code


class AdapterServer:
    """Dependency-free JSON-lines server used by every official adapter.

    stdout is reserved for protocol frames. Diagnostic logs always go to stderr.
    """

    adapter_name = "unknown"
    backend = "unknown"
    version = "1.0.0"
    capabilities: tuple[str, ...] = ()
    modes: tuple[str, ...] = ()

    def __init__(self) -> None:
        self.mode = os.environ.get(
            "MEMORYPROOF_ADAPTER_MODE",
            os.environ.get("FORGETPROOF_ADAPTER_MODE", "default"),
        )
        self.adapter_name = os.environ.get(
            "MEMORYPROOF_ADAPTER_NAME",
            os.environ.get("FORGETPROOF_ADAPTER_NAME", self.adapter_name),
        )
        raw_config = os.environ.get(
            "MEMORYPROOF_CONFIG_JSON",
            os.environ.get("FORGETPROOF_CONFIG_JSON", "{}"),
        )
        try:
            self.config: dict[str, str] = json.loads(raw_config)
        except json.JSONDecodeError as exc:
            raise AdapterError(f"invalid adapter configuration: {exc}", "invalid_config") from exc
        if not isinstance(self.config, dict):
            raise AdapterError("adapter configuration must be a JSON object", "invalid_config")
        self.closed = False

    def serve(self) -> None:
        logging.basicConfig(stream=sys.stderr, level=logging.INFO)
        for raw_line in sys.stdin:
            line = raw_line.strip()
            if not line:
                continue
            request: dict[str, Any] | None = None
            try:
                request = json.loads(line)
                response = self.dispatch(request)
            except json.JSONDecodeError as exc:
                response = {
                    "protocol": PROTOCOL_VERSION,
                    "id": None,
                    "ok": False,
                    "error": {"code": "invalid_json", "message": str(exc)},
                }
            except AdapterError as exc:
                response = {
                    "protocol": PROTOCOL_VERSION,
                    "id": request.get("id") if isinstance(request, dict) else None,
                    "ok": False,
                    "error": {"code": exc.code, "message": str(exc)},
                }
            except Exception as exc:  # pragma: no cover - defensive process boundary
                log.exception("adapter request failed")
                response = {
                    "protocol": PROTOCOL_VERSION,
                    "id": request.get("id") if isinstance(request, dict) else None,
                    "ok": False,
                    "error": {"code": "internal_error", "message": str(exc)},
                }
            sys.stdout.write(json.dumps(response, separators=(",", ":")) + "\n")
            sys.stdout.flush()
            if self.closed:
                break

    def dispatch(self, request: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(request, dict):
            raise AdapterError("request must be an object", "invalid_request")
        request_id = request.get("id")
        if not isinstance(request_id, str):
            raise AdapterError("request id must be a string", "invalid_request")
        requested_protocol = request.get("protocol", PROTOCOL_VERSION)
        if requested_protocol != PROTOCOL_VERSION:
            raise AdapterError(
                f"protocol mismatch: expected {PROTOCOL_VERSION}, got {requested_protocol}",
                "protocol_mismatch",
            )
        method = request.get("method")
        if not isinstance(method, str):
            raise AdapterError("method must be a string", "invalid_request")
        params = request.get("params") or {}
        if not isinstance(params, dict):
            raise AdapterError("params must be an object", "invalid_request")
        handler = getattr(self, f"handle_{method}", None)
        if handler is None:
            raise AdapterError(f"unsupported method: {method}", "unsupported_method")
        result = handler(params)
        return {"protocol": PROTOCOL_VERSION, "id": request_id, "ok": True, "result": result}

    def handle_hello(self, params: dict[str, Any]) -> dict[str, Any]:
        requested = params.get("protocol", PROTOCOL_VERSION)
        if requested != PROTOCOL_VERSION:
            raise AdapterError(
                f"protocol mismatch: expected {PROTOCOL_VERSION}, got {requested}",
                "protocol_mismatch",
            )
        return {"protocol": PROTOCOL_VERSION, "adapter": self.adapter_name}

    def handle_capabilities(self, params: dict[str, Any]) -> dict[str, Any]:
        return {
            "protocol": PROTOCOL_VERSION,
            "adapter": self.adapter_name,
            "backend": self.backend,
            "version": self.version,
            "capabilities": list(self.capabilities),
            "modes": list(self.modes),
        }

    def handle_close(self, params: dict[str, Any]) -> dict[str, Any]:
        self.closed = True
        return {"closed": True}

    def handle_prepare(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("prepare is not implemented", "unsupported_method")

    def handle_ingest(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("ingest is not implemented", "unsupported_method")

    def handle_settle(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"state": "stable", "stable": True, "observations": 1}

    def handle_probe(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("probe is not implemented", "unsupported_method")

    def handle_erase(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("erase is not implemented", "unsupported_method")

    def handle_inspect(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("inspect is not implemented", "unsupported_method")

    def handle_agent_query(self, params: dict[str, Any]) -> dict[str, Any]:
        raise AdapterError("agent_query is not implemented", "unsupported_method")

    def handle_cleanup(self, params: dict[str, Any]) -> dict[str, Any]:
        return {"cleaned": True}
