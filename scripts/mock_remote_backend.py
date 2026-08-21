#!/usr/bin/env python3
"""Small deterministic HTTP backend for official adapter contract tests.

It is intentionally not a provider emulator. It implements only the narrow
request/response shapes used by the dependency-free Mem0, Letta, and Zep
adapters so contributors can reproduce adapter behavior without credentials.
It stores everything in memory and exits without writing user data to disk.
"""

from __future__ import annotations

import argparse
import json
import re
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse


class State:
    def __init__(self, provider: str) -> None:
        self.provider = provider
        self.lock = threading.Lock()
        self.counter = 0
        self.records: dict[str, dict] = {}
        self.agents: dict[str, dict] = {}
        self.users: dict[str, dict] = {}
        self.threads: dict[str, dict] = {}

    def new_id(self, prefix: str) -> str:
        with self.lock:
            self.counter += 1
            return f"mock-{prefix}-{self.counter}"


def _json(handler: BaseHTTPRequestHandler) -> dict:
    length = int(handler.headers.get("Content-Length", "0"))
    if not length:
        return {}
    try:
        value = json.loads(handler.rfile.read(length))
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def _contains(query: str, content: str) -> bool:
    if not query:
        return True
    markers = re.findall(r"[A-Za-z0-9][A-Za-z0-9_-]{5,}", content)
    return query in content or any(marker in query for marker in markers)


class Handler(BaseHTTPRequestHandler):
    server_version = "MemoryProofMock/1"

    @property
    def state(self) -> State:
        return self.server.state  # type: ignore[attr-defined]

    def log_message(self, fmt: str, *args: object) -> None:
        # Keep CI output quiet and deterministic; the adapter owns request IDs.
        return

    def send_json(self, value: object, status: int = 200) -> None:
        raw = json.dumps(value, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("X-Request-Id", f"mock-{self.state.provider}")
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        path = parsed.path
        if path == "/health":
            self.send_json({"status": "ok", "provider": self.state.provider})
            return
        if self.state.provider == "mem0":
            self.mem0_get(path, query)
        elif self.state.provider == "letta":
            self.letta_get(path, query)
        else:
            self.zep_get(path, query)

    def do_POST(self) -> None:  # noqa: N802
        body = _json(self)
        path = urlparse(self.path).path
        if self.state.provider == "mem0":
            self.mem0_post(path, body)
        elif self.state.provider == "letta":
            self.letta_post(path, body)
        else:
            self.zep_post(path, body)

    def do_DELETE(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        path = parsed.path
        if self.state.provider == "mem0":
            self.mem0_delete(path, query)
        elif self.state.provider == "letta":
            self.letta_delete(path)
        else:
            self.zep_delete(path)

    def mem0_post(self, path: str, body: dict) -> None:
        if path in {"/memories", "/v1/memories"}:
            user = str(body.get("user_id", ""))
            messages = body.get("messages") or []
            content = str(messages[0].get("content", "")) if messages else str(body.get("text", ""))
            item_id = self.state.new_id("memory")
            self.state.records[item_id] = {"id": item_id, "user_id": user, "content": content}
            self.send_json({"id": item_id, "status": "created"})
            return
        if path.endswith("/search"):
            self.send_json(self._mem0_results(body.get("user_id", ""), body.get("query", "")))
            return
        self.send_json({"ok": True})

    def mem0_get(self, path: str, query: dict[str, list[str]]) -> None:
        if path in {"/memories", "/v1/memories"}:
            user = (query.get("user_id") or [""])[0]
            self.send_json(self._mem0_results(user, ""))
            return
        self.send_json({"results": []})

    def mem0_delete(self, path: str, query: dict[str, list[str]]) -> None:
        user = (query.get("user_id") or [""])[0]
        if path in {"/memories", "/v1/memories"} and user:
            for item_id in list(self.state.records):
                if self.state.records[item_id].get("user_id") == user:
                    del self.state.records[item_id]
            self.send_json({"deleted": True})
            return
        item_id = path.rsplit("/", 1)[-1]
        self.state.records.pop(item_id, None)
        self.send_json({"deleted": True})

    def _mem0_results(self, user: object, query: object) -> dict:
        user = str(user)
        query = str(query)
        return {"results": [{"id": item_id, "memory": item["content"], "user_id": user} for item_id, item in self.state.records.items() if item.get("user_id") == user and _contains(query, item["content"])]}

    def letta_post(self, path: str, body: dict) -> None:
        if path == "/v1/agents":
            agent_id = self.state.new_id("agent")
            self.state.agents[agent_id] = {"deleted": False, "records": {}}
            self.send_json({"id": agent_id})
            return
        match = re.fullmatch(r"/v1/agents/([^/]+)/archival-memory", path)
        if match:
            agent = self.state.agents.setdefault(match.group(1), {"deleted": False, "records": {}})
            passage_id = self.state.new_id("passage")
            agent["records"][passage_id] = str(body.get("text", ""))
            self.send_json({"id": passage_id})
            return
        match = re.fullmatch(r"/v1/agents/([^/]+)/messages", path)
        if match:
            agent = self.state.agents.get(match.group(1), {"deleted": True, "records": {}})
            texts = list(agent.get("records", {}).values()) if not agent.get("deleted") else []
            self.send_json({"messages": [{"role": "assistant", "content": text} for text in texts]})
            return
        self.send_json({"ok": True})

    def letta_get(self, path: str, query: dict[str, list[str]]) -> None:
        match = re.fullmatch(r"/v1/agents/([^/]+)/archival-memory/search", path)
        if match:
            agent = self.state.agents.get(match.group(1), {"deleted": True, "records": {}})
            wanted = (query.get("query") or [""])[0]
            self.send_json({"results": [{"id": item_id, "text": text} for item_id, text in agent.get("records", {}).items() if not agent.get("deleted") and _contains(wanted, text)]})
            return
        match = re.fullmatch(r"/v1/agents/([^/]+)/archival-memory", path)
        if match:
            agent = self.state.agents.get(match.group(1), {"deleted": True, "records": {}})
            self.send_json({"results": [{"id": item_id, "text": text} for item_id, text in agent.get("records", {}).items()] if not agent.get("deleted") else {"results": []}})
            return
        self.send_json({"results": []})

    def letta_delete(self, path: str) -> None:
        match = re.fullmatch(r"/v1/agents/([^/]+)(?:/archival-memory/([^/]+))?", path)
        if match:
            agent = self.state.agents.get(match.group(1))
            if agent is not None:
                if match.group(2):
                    agent["records"].pop(match.group(2), None)
                else:
                    agent["deleted"] = True
                    agent["records"].clear()
            self.send_json({"deleted": True})
            return
        self.send_json({"deleted": True})

    def zep_post(self, path: str, body: dict) -> None:
        if path == "/api/v2/users":
            user_id = str(body.get("user_id") or self.state.new_id("user"))
            self.state.users[user_id] = {"deleted": False, "threads": []}
            self.send_json({"user_id": user_id})
            return
        if path == "/api/v2/threads":
            thread_id = str(body.get("thread_id") or self.state.new_id("thread"))
            self.state.threads[thread_id] = {"user_id": str(body.get("user_id", "")), "deleted": False, "records": {}}
            self.send_json({"thread_id": thread_id})
            return
        match = re.fullmatch(r"/api/v2/threads/([^/]+)/messages", path)
        if match:
            thread = self.state.threads.setdefault(match.group(1), {"user_id": "", "deleted": False, "records": {}})
            messages = body.get("messages") or []
            content = str(messages[0].get("content", "")) if messages else ""
            episode_id = self.state.new_id("episode")
            thread["records"][episode_id] = content
            self.send_json({"id": episode_id, "episode_id": episode_id})
            return
        self.send_json({"ok": True})

    def zep_get(self, path: str, query: dict[str, list[str]]) -> None:
        match = re.fullmatch(r"/api/v2/threads/([^/]+)/search", path)
        if match:
            thread = self.state.threads.get(match.group(1), {"deleted": True, "records": {}})
            wanted = (query.get("query") or [""])[0]
            self.send_json({"messages": [{"id": item_id, "content": text} for item_id, text in thread.get("records", {}).items() if not thread.get("deleted") and _contains(wanted, text)]})
            return
        match = re.fullmatch(r"/api/v2/users/([^/]+)/graph", path)
        if match:
            user_id = match.group(1)
            nodes = []
            for thread in self.state.threads.values():
                if thread.get("user_id") == user_id and not thread.get("deleted"):
                    nodes.extend({"id": item_id, "name": text} for item_id, text in thread["records"].items())
            self.send_json({"nodes": nodes, "edges": []})
            return
        self.send_json({"results": []})

    def zep_delete(self, path: str) -> None:
        match = re.fullmatch(r"/api/v2/users/([^/]+)", path)
        if match:
            user_id = match.group(1)
            user = self.state.users.setdefault(user_id, {"deleted": False, "threads": []})
            user["deleted"] = True
            for thread in self.state.threads.values():
                if thread.get("user_id") == user_id:
                    thread["deleted"] = True
                    thread["records"].clear()
            self.send_json({"deleted": True})
            return
        match = re.fullmatch(r"/api/v2/threads/([^/]+)/messages/([^/]+)", path)
        if match:
            thread = self.state.threads.get(match.group(1))
            if thread:
                thread["records"].pop(match.group(2), None)
            self.send_json({"deleted": True})
            return
        self.send_json({"deleted": True})


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", choices=["mem0", "letta", "zep"], required=True)
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    server.state = State(args.provider)  # type: ignore[attr-defined]
    print(f"MemoryProof mock {args.provider} listening on 127.0.0.1:{args.port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
