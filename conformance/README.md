# Public Memory Assurance Evidence

[English](README.md) · [简体中文](README.zh-CN.md)

![MemoryProof evidence flow](../assets/memoryproof-hero.png)

This directory contains small, reviewed, redacted evidence bundles that can be opened without a hosted service. The Pages workflow verifies each bundle’s checksums before adding its profiles to the public matrix.

## What a submission must contain

Each directory under `conformance/evidence/` is a complete bundle produced by `memoryproof run` and must include:

- `manifest.json`, `scenario.lock.json`, `events.ndjson`, and `results.json`;
- `report.html` and `junit.xml`;
- `checksums.sha256` and `bundle.hash`;
- synthetic fixtures only, with credentials and raw customer content removed.

The matrix shows `PASS`, `FAIL`, `SKIP`, or `UNKNOWN` per profile. It never turns these states into a single score. A failing or unknown result is welcome when it is reproducible and honestly described.

## Reproduce a local submission

```bash
cargo run -- run examples/reference-clean.yml --output /tmp/memoryproof-evidence
python3 scripts/build_matrix.py
cargo run -- matrix validate site/matrix.json
```

For a remote provider, pass `--allow-network` only after checking the scenario, credentials, tenant, and cleanup behavior. Do not publish a remote bundle until it has been manually reviewed.

## Review checklist

1. Verify the bundle with `memoryproof verify <bundle>`.
2. Confirm `scenario.lock.json` contains synthetic canaries only.
3. Confirm the backend name and version are recorded.
4. Confirm target/control scope and the exact failing or passing probe are visible.
5. Confirm all unsupported boundaries are `UNKNOWN`, `SKIP`, or explicitly `OUT OF SCOPE`.
6. Mark the source as maintainer-reproduced or community-submitted in the accompanying release note or pull request.

The public matrix is an evidence index, not a vendor leaderboard.

The `*-adapter-contract` bundles are maintainer-reproduced runs against the
deterministic local mock in `scripts/mock_remote_backend.py`. They verify the
official adapter contract without claiming that a particular hosted provider
has the same behavior. Provider conformance claims must include the provider
version and a separately reviewed, credentialed run.
