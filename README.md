# ForgetProof

> Prove your AI forgot.

ForgetProof is an open-source conformance test runner for AI memory erasure. It does not store memories. It creates isolated synthetic canaries, asks a memory or agent backend to erase them, probes raw and derived boundaries, and produces evidence that can be rerun in CI.

The important distinction is simple:

```text
DELETE 200 OK  !=  the canary is no longer observable
```

ForgetProof v0.1 ships a Rust CLI, a versioned NDJSON adapter protocol, dependency-free Python adapters for Mem0, Letta, and Zep, reference clean/leaky backends, redacted evidence bundles, JUnit output, Docker, and a GitHub Action.

## Quickstart

Requirements: Rust stable and Python 3.11+.

```bash
cargo run -- adapters list
cargo run -- run examples/reference-clean.yml
```

The clean reference scenario exits `0` and writes a bundle under `.forgetproof/runs/`. Verify it:

```bash
cargo run -- verify .forgetproof/runs/<run-id>
open .forgetproof/runs/<run-id>/report.html
```

Run the intentionally leaky backend to see a real failure:

```bash
cargo run -- run examples/reference-leaky.yml
```

It exits `1` because deletion removes the raw item but leaves an observable derived artifact. The report explains which probes found it.

Freeze extra deterministic probe variants before a run:

```bash
cargo run -- expand examples/reference-clean.yml --output scenario.lock.yml
```

## Conformance profiles

| Profile | What it checks |
| --- | --- |
| `FP-Object` | The erased target is absent from configured recall/list probes. |
| `FP-Scope` | The target is gone while a control fixture in another scope remains. |
| `FP-Derived` | Observable summaries, indexes, graph artifacts, or other derived data are absent. |
| `FP-Agent` | An optional Agent query no longer leaks the unique canary. |

Results are `PASS`, `FAIL`, `SKIP`, `UNKNOWN`, or `ERROR`. ForgetProof never turns an unsupported inspection surface into a pass. It also does not claim provider log deletion, backup deletion, physical storage erasure, model-weight unlearning, or information that cannot be observed through the adapter.

## Adapter protocol

The Rust runner starts one adapter process per run and communicates over stdout using `forgetproof.adapter/v1alpha1` NDJSON. stdout is reserved for protocol frames; diagnostics go to stderr.

Supported methods are `hello`, `capabilities`, `prepare`, `ingest`, `settle`, `probe`, `erase`, `inspect`, `agent_query`, `cleanup`, and `close`. A third-party adapter only needs to implement this protocol and advertise its capabilities. Adapter commands are registered by the runner, so a scenario cannot execute an arbitrary command.

## Remote adapters

Remote adapters are disabled unless `--allow-network` is explicitly provided. Use synthetic canaries and a run-specific namespace. Credentials are read from environment variables:

```bash
MEM0_BASE_URL=https://... MEM0_API_KEY=... \
  cargo run -- run scenarios/mem0.yml --allow-network
```

Endpoint paths can be overridden with `endpoint_*` adapter config values for self-hosted or version-specific deployments. The default mappings are intentionally visible in `python/forgetproof_adapters/` rather than hidden behind a hosted service.

## Evidence bundle

Each run emits:

```text
manifest.json       run metadata and protocol version
scenario.lock.json  redacted scenario snapshot
events.ndjson       ordered method-level journal
results.json        machine-readable assertions
report.html         offline single-file report
junit.xml           CI test report
checksums.sha256    per-file integrity hashes
bundle.hash         hash of the checksum manifest
```

Payloads are redacted by default to hashes, lengths, and structural summaries. `--allow-network` does not change that privacy policy.

## Development

```bash
cargo fmt --all
cargo test
cargo run -- run examples/reference-clean.yml
python3 -m compileall python
```

The default test suite is local and credential-free. Live Mem0, Letta, and Zep checks belong in a separately authorized workflow.

## License

Apache-2.0. See [LICENSE](LICENSE).
