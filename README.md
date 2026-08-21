# ForgetProof

> Prove your AI forgot.

[English](README.md) · [简体中文](README.zh-CN.md)

<p align="center">
  <a href="https://github.com/Hughhhhcoder/ForgetProof/actions/workflows/ci.yml?query=branch%3Amain"><img src="https://img.shields.io/github/actions/workflow/status/Hughhhhcoder/ForgetProof/ci.yml?branch=main&label=CI&logo=github" alt="CI status" /></a>
  <a href="https://hughhhhcoder.github.io/ForgetProof/"><img src="https://img.shields.io/badge/live-conformance%20matrix-0ea5e9?logo=googlechrome&logoColor=white" alt="Live conformance matrix" /></a>
  <a href="https://github.com/Hughhhhcoder/ForgetProof/releases"><img src="https://img.shields.io/github/v/release/Hughhhhcoder/ForgetProof?display_name=tag&sort=semver&logo=github" alt="Latest release" /></a>
  <a href="https://github.com/Hughhhhcoder/ForgetProof/blob/main/LICENSE"><img src="https://img.shields.io/github/license/Hughhhhcoder/ForgetProof?logo=apache" alt="Apache-2.0 license" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-stable-111827?logo=rust&logoColor=white" alt="Rust stable" />
  <img src="https://img.shields.io/badge/Python-3.11%2B-3776ab?logo=python&logoColor=white" alt="Python 3.11 or newer" />
  <img src="https://img.shields.io/badge/privacy-local--first-10b981?logo=shield&logoColor=white" alt="Local-first privacy" />
  <img src="https://img.shields.io/badge/evidence-SHA--256-f59e0b?logo=datadog&logoColor=white" alt="SHA-256 evidence" />
</p>

<p align="center">
  🧪 <a href="#quickstart">Try it in 60 seconds</a> · 🔍 <a href="#see-the-difference-in-30-seconds">See the proof</a> · 🌐 <a href="https://hughhhhcoder.github.io/ForgetProof/">Open the live matrix</a>
</p>

![ForgetProof hero: a synthetic canary disappears from a memory graph while an evidence ledger verifies the change](assets/forgetproof-hero.png)

ForgetProof is an open-source test runner for one uncomfortable question:

> When an AI memory system says “deleted”, can you show that the information is no longer observable?

It creates isolated synthetic canaries, asks a memory or Agent backend to erase them, checks raw and derived storage boundaries, and writes evidence that can be rerun locally or in CI.

> [!IMPORTANT]
> `DELETE 200 OK` is an API response. ForgetProof tests the stronger claim: **the canary is no longer observable through the paths you can inspect**.

## Why this exists

```text
DELETE 200 OK  !=  the canary is no longer observable
```

Memory systems may keep information in more than one place: the original record, a summary, an embedding, a graph node, a cache, or an Agent’s working context. ForgetProof makes those boundaries visible and reports what was actually checked.

It does not store your production memories. It tests a controlled namespace with synthetic data.

## 🧠 The problem in one glance

| What a backend may say | What may still be alive | What ForgetProof checks |
| --- | --- | --- |
| ✅ `DELETE 200 OK` | 📝 A summary still contains the canary | 🔎 Deterministic before/after probes |
| ✅ The raw row is gone | 🧭 An index, embedding, or graph edge still recalls it | 🧬 Derived-artifact inspection |
| ✅ The Agent cannot see one block | 💬 Another Agent path still leaks the fact | 🤖 Optional black-box Agent query |

The goal is not to punish a backend for being incomplete. The goal is to make the boundary visible, reproducible, and honest.

## How it works

```mermaid
flowchart LR
    A["Scenario + canary"] --> B["Rust runner"]
    B --> C["NDJSON adapter"]
    C --> D["Mem0 / Letta / Zep"]
    B --> E["Before probes"]
    D --> F["Erase request"]
    F --> G["After probes"]
    E --> H["Evidence bundle"]
    G --> H
    H --> I["HTML / JUnit / CI"]
```

The runner proves that the canary was visible before deletion, performs a scoped erase, waits for the backend to settle, and checks the target plus a control fixture afterward. Unsupported inspection is reported as `UNKNOWN`, never as a pass.

## 🔍 See the difference in 30 seconds

|  | Without ForgetProof | With ForgetProof |
| --- | --- | --- |
| **Evidence** | Trust a `200 OK` response | 📦 Keep a verifiable evidence bundle |
| **Coverage** | Check the object you deleted | 🧠 Check raw, summary, vector, graph, cache, and Agent paths that are observable |
| **Regression testing** | Run a one-off script | 🔁 Re-run the same locked scenario in CI |
| **Uncertainty** | Turn missing APIs into “probably fine” | ⚠️ Report `UNKNOWN` when the backend cannot support a claim |

```mermaid
flowchart LR
    A["Before delete<br/>canary is recallable"] --> B["DELETE 200 OK"]
    B --> C{"After: deterministic probes"}
    C -->|"clean"| D["✅ PASS<br/>no observable path"]
    C -->|"leaky"| E["❌ FAIL<br/>summary / index / graph remains"]
```

### A failure is useful

The deliberately leaky reference backend is part of the demo. It deletes the raw item but leaves a derived artifact, so ForgetProof must fail at the derived layer:

```text
status: FAIL
profile: FP-Derived
probe: target-derived-after
reason: the unique canary is still observable in a derived artifact
```

That is the product promise in miniature: a vague “memory problem” becomes a named, reviewable, reproducible failure.

## 🚀 Quickstart

Requirements: Rust stable and Python 3.11+.

Choose the path that fits your workflow:

- 🛠️ **From source:** run the commands below with Rust stable.
- 📥 **Released binary:** download the archive for Linux, macOS, or Windows from [Releases](https://github.com/Hughhhhcoder/ForgetProof/releases).
- 🐳 **Docker:** `docker run --rm -v "$PWD":/workspace ghcr.io/hughhhhcoder/forgetproof:v0.1.0 run /workspace/examples/reference-clean.yml --output /workspace/.forgetproof/runs`

```bash
cargo run -- adapters list
cargo run -- run examples/reference-clean.yml
```

The clean reference scenario exits `0` and writes a bundle under `.forgetproof/runs/`. Verify it:

```bash
cargo run -- verify .forgetproof/runs/<run-id>
open .forgetproof/runs/<run-id>/report.html
```

Now run the intentionally leaky backend:

```bash
cargo run -- run examples/reference-leaky.yml
```

It exits `1` because the raw item is deleted while a derived artifact remains observable. The report identifies the failing probe and profile.

### CLI commands

The Rust binary exposes a small command surface for local setup, adapter inspection, scenario execution, and evidence verification:

| Command | Purpose |
| --- | --- |
| `init [path]` | Create a starter ForgetProof project and reference scenarios. |
| `adapters list` | List the built-in and Python adapters. |
| `doctor --adapter <name>` | Inspect an adapter's protocol and capabilities without mutating a backend. |
| `expand <input> --output <file>` | Freeze deterministic lexical and semantic probe variants. |
| `run <scenario>` | Execute a scenario and write an evidence bundle. |
| `verify <bundle>` | Recompute checksums and verify bundle integrity. |
| `report <bundle>` | Regenerate the HTML and JUnit reports from `results.json`. |

For scenario authoring, start with [`examples/reference-clean.yml`](examples/reference-clean.yml) and validate the shape against [`schemas/scenario.schema.json`](schemas/scenario.schema.json). Keep production credentials in environment variables; use `--allow-network` only when the scenario is explicitly authorized to reach a non-loopback backend.

## 🏅 Conformance profiles

| Profile | Plain-English meaning |
| --- | --- |
| `FP-Object` | The erased target is gone from the configured recall and list probes. |
| `FP-Scope` | The target is gone while an unrelated control fixture remains. |
| `FP-Derived` | Observable summaries, indexes, graph artifacts, or other derivatives are gone or invalidated. |
| `FP-Agent` | An optional Agent query no longer leaks the unique canary. |

Results are `PASS`, `FAIL`, `SKIP`, `UNKNOWN`, or `ERROR`. There is no misleading single score.

## ✅ What ForgetProof can prove

| It can show | It cannot claim |
| --- | --- |
| A target was observable before erase. | Provider logs were deleted. |
| A configured API no longer returns the target. | Backups or physical storage were wiped. |
| A visible summary, graph, index, or Agent path still leaks it. | A model’s weights were unlearned. |
| A run’s evidence bundle was not modified after creation. | Anything outside the adapter’s observable boundary. |

## 🔌 Supported adapters

The repository includes dependency-free Python adapters for Mem0, Letta, and Zep, plus clean and deliberately leaky reference backends. Remote access is opt-in:

```bash
MEM0_BASE_URL=https://... MEM0_API_KEY=... \
  cargo run -- run examples/mem0.yml --allow-network
```

Credentials stay in environment variables. Endpoint paths can be overridden with `endpoint_*` adapter settings for self-hosted or version-specific deployments.

## 📦 Evidence bundle

Each run emits a small, offline-readable evidence package:

```text
manifest.json       run metadata and protocol version
scenario.lock.json  redacted scenario snapshot
events.ndjson       ordered method-level journal
results.json        machine-readable assertions
report.html         single-file human report
junit.xml           CI test report
checksums.sha256    per-file integrity hashes
bundle.hash         hash of the checksum manifest
```

Payloads are redacted by default to hashes, lengths, and structural summaries. `--allow-network` does not change that privacy policy.

## 🤖 Use it in CI

The repository provides a Docker-based GitHub Action. A scenario can fail a pull request when an erasure regression is detected and upload the evidence bundle for review.

```yaml
name: Memory erasure

on: [pull_request]

jobs:
  forgetproof:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Hughhhhcoder/ForgetProof@v0.1.0
        with:
          scenario: examples/reference-clean.yml
```

## 🧩 Adapter protocol

The Rust runner starts one adapter process per run and communicates over stdout using `forgetproof.adapter/v1alpha1` NDJSON. stdout is reserved for protocol frames; diagnostics go to stderr.

Supported methods are `hello`, `capabilities`, `prepare`, `ingest`, `settle`, `probe`, `erase`, `inspect`, `agent_query`, `cleanup`, and `close`. A third-party adapter only needs to implement this protocol and advertise its capabilities.

## 🗺️ Learn more

- [中文说明](README.zh-CN.md)
- [Scenario schema](schemas/scenario.schema.json)
- [Contributing guide](CONTRIBUTING.md) · [中文贡献指南](CONTRIBUTING.zh-CN.md)
- [Security policy](SECURITY.md) · [中文安全策略](SECURITY.zh-CN.md)
- [Code of conduct](CODE_OF_CONDUCT.md) · [中文行为准则](CODE_OF_CONDUCT.zh-CN.md)
- [Public conformance submissions](conformance/README.md)
- [Conformance matrix](site/index.html)
- [Changelog](CHANGELOG.md)
- [Container images on GHCR](https://github.com/Hughhhhcoder/ForgetProof/pkgs/container/forgetproof)

## Development

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
PYTHONPATH=python python3 -m unittest discover -s python/tests -v
python3 -m compileall python
```

The default test suite is local and credential-free. Live Mem0, Letta, and Zep checks belong in a separately authorized workflow.

## License

Apache-2.0. See [LICENSE](LICENSE).
