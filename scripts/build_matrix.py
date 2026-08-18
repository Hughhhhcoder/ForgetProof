from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "conformance"
OUTPUT = ROOT / "site" / "matrix.json"
ALLOWED_STATUSES = {"PASS", "FAIL", "SKIP", "UNKNOWN"}


def main() -> None:
    results = []
    for path in sorted(SOURCE.glob("*.json")):
        if path.name.startswith("_"):
            continue
        data = json.loads(path.read_text())
        bundle = data.get("bundle", path.stem)
        for profile in data.get("profiles", []):
            status = profile.get("status", "UNKNOWN")
            if status not in ALLOWED_STATUSES:
                raise SystemExit(f"{path}: unsupported status {status!r}")
            if not profile.get("evidence", bundle):
                raise SystemExit(f"{path}: profile is missing evidence")
            results.append(
                {
                    "adapter": data.get("adapter", "unknown"),
                    "backend": data.get("backend", "unknown"),
                    "profile": profile.get("profile", "unknown"),
                    "status": status,
                    "evidence": profile.get("evidence", bundle),
                }
            )
    OUTPUT.write_text(json.dumps({"generated_by": "forgetproof", "results": results}, indent=2) + "\n")


if __name__ == "__main__":
    main()
