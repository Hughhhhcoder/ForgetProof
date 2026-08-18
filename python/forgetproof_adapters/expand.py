from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any


def main() -> None:
    scenario = json.load(sys.stdin)
    base_url = os.environ.get("FORGETPROOF_LLM_BASE_URL") or os.environ.get("OPENAI_BASE_URL")
    api_key = os.environ.get("FORGETPROOF_LLM_API_KEY") or os.environ.get("OPENAI_API_KEY", "")
    model = os.environ.get("FORGETPROOF_LLM_MODEL") or os.environ.get("OPENAI_MODEL", "gpt-4o-mini")
    if not base_url:
        json.dump(scenario, sys.stdout, separators=(",", ":"))
        return

    prompt = {
        "instruction": "Generate additional deterministic recall probes for this ForgetProof scenario.",
        "rules": [
            "Return a JSON array only.",
            "Each item must contain id, fixture, kind, query.",
            "kind must be lexical or semantic.",
            "Use the exact fixture ids from the scenario.",
            "Do not include secrets or change the erase operation.",
        ],
        "scenario": scenario,
    }
    payload = {
        "model": model,
        "temperature": 0,
        "messages": [
            {"role": "system", "content": "You produce strict JSON for a test generator."},
            {"role": "user", "content": json.dumps(prompt, ensure_ascii=False)},
        ],
    }
    request = urllib.request.Request(
        base_url.rstrip("/") + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body: dict[str, Any] = json.loads(response.read().decode("utf-8"))
    except (urllib.error.URLError, urllib.error.HTTPError) as exc:
        raise SystemExit(f"LLM expansion request failed: {exc}") from exc

    content = body["choices"][0]["message"]["content"]
    content = content.strip()
    if content.startswith("```"):
        content = content.split("\n", 1)[1].rsplit("```", 1)[0]
    variants = json.loads(content)
    if not isinstance(variants, list):
        raise SystemExit("LLM expansion response must be a JSON array")
    allowed = {str(fixture["id"]) for fixture in scenario["spec"]["fixtures"]}
    existing = {str(probe["id"]) for probe in scenario["spec"]["probes"]["after"]}
    for variant in variants:
        if not isinstance(variant, dict):
            continue
        if variant.get("fixture") not in allowed:
            continue
        if variant.get("kind") not in {"lexical", "semantic"}:
            continue
        if not variant.get("query") or variant.get("id") in existing:
            continue
        scenario["spec"]["probes"]["after"].append(
            {
                "id": str(variant["id"]),
                "fixture": str(variant["fixture"]),
                "kind": str(variant["kind"]),
                "query": str(variant["query"]),
            }
        )
        existing.add(str(variant["id"]))
    json.dump(scenario, sys.stdout, ensure_ascii=False, separators=(",", ":"))


if __name__ == "__main__":
    main()
