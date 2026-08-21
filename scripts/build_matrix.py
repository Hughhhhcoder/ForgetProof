"""Build the static MemoryProof assurance matrix from reviewed evidence bundles.

The script deliberately verifies each local bundle before exposing it in the
public matrix. A result without a valid checksum manifest is never published.
"""

from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "conformance" / "evidence"
OUTPUT = ROOT / "site" / "matrix.json"
PUBLIC_EVIDENCE = ROOT / "site" / "evidence"
ALLOWED_STATUSES = {"PASS", "FAIL", "SKIP", "UNKNOWN"}
ALLOWED_FORMATS = {"memoryproof.bundle/v1", "forgetproof.bundle/v1alpha1"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_bundle(directory: Path, manifest: dict) -> str:
    if manifest.get("format") not in ALLOWED_FORMATS:
        raise SystemExit(f"{directory}: unsupported bundle format {manifest.get('format')!r}")
    checksums_path = directory / "checksums.sha256"
    if not checksums_path.is_file():
        raise SystemExit(f"{directory}: missing checksums.sha256")
    expected = {}
    for line in checksums_path.read_text().splitlines():
        digest, separator, name = line.partition("  ")
        if not separator or len(digest) != 64:
            raise SystemExit(f"{directory}: malformed checksum line {line!r}")
        expected[name] = digest
    listed = set(manifest.get("files", []))
    if set(expected) != listed:
        raise SystemExit(f"{directory}: manifest/checksum file list differs")
    for name, digest in expected.items():
        path = directory / name
        if not path.is_file() or sha256(path) != digest:
            raise SystemExit(f"{directory}: checksum mismatch for {name}")
    bundle_hash = hashlib.sha256((checksums_path.read_text()).encode()).hexdigest()
    recorded = (directory / "bundle.hash").read_text().strip() if (directory / "bundle.hash").is_file() else ""
    if recorded and recorded != bundle_hash:
        raise SystemExit(f"{directory}: bundle.hash does not match checksums.sha256")
    return bundle_hash


def main() -> None:
    rows: list[dict] = []
    validated_directories: list[Path] = []
    if SOURCE.exists():
        for directory in sorted(path for path in SOURCE.iterdir() if path.is_dir()):
            manifest_path = directory / "manifest.json"
            results_path = directory / "results.json"
            if not manifest_path.is_file() or not results_path.is_file():
                raise SystemExit(f"{directory}: public evidence needs manifest.json and results.json")
            manifest = json.loads(manifest_path.read_text())
            result = json.loads(results_path.read_text())
            bundle_hash = verify_bundle(directory, manifest)
            validated_directories.append(directory)
            statuses = {item.get("status", "UNKNOWN") for item in result.get("profiles", [])}
            if not statuses:
                statuses = {result.get("status", "UNKNOWN")}
            for status in statuses:
                if status not in ALLOWED_STATUSES:
                    raise SystemExit(f"{directory}: unsupported status {status!r}")
            for profile in result.get("profiles", []):
                status = profile.get("status", "UNKNOWN")
                if status not in ALLOWED_STATUSES:
                    raise SystemExit(f"{directory}: unsupported profile status {status!r}")
                rows.append(
                    {
                        "suite": result.get("suite", "erasure"),
                        "adapter": result.get("adapter", "unknown"),
                        "backend": result.get("backend", "unknown"),
                        "backend_version": result.get("backend_version", "unknown"),
                        "profile": profile.get("profile", "unknown"),
                        "status": status,
                        "evidence_kind": (
                            "official adapter contract · local mock"
                            if directory.name.endswith("-adapter-contract")
                            else "maintainer reference scenario"
                        ),
                        "evidence": f"./evidence/{directory.name}/report.html",
                        "bundle_hash": bundle_hash[:16],
                        "source": manifest.get("source", "maintainer-reproduced"),
                    }
                )
    if PUBLIC_EVIDENCE.exists():
        shutil.rmtree(PUBLIC_EVIDENCE)
    PUBLIC_EVIDENCE.mkdir(parents=True, exist_ok=True)
    for directory in validated_directories:
        shutil.copytree(directory, PUBLIC_EVIDENCE / directory.name)

    OUTPUT.write_text(
        json.dumps(
            {
                "format": "memoryproof.matrix/v1",
                "generated_by": "scripts/build_matrix.py",
                "results": rows,
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
