# Changelog

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

All notable changes to MemoryProof are documented here.

## [Unreleased]

No unreleased changes.

## [1.0.4] - 2026-08-22

### Security

- Require the explicit `--allow-network` flag for every non-loopback endpoint; legacy environment variables can no longer bypass the CLI authorization boundary.

## [1.0.3] - 2026-08-22

### Fixed

- Stage every checksum-verified public evidence bundle into the Pages artifact so matrix report links work offline.
- Enforce explicit network permission for `doctor` and OpenAI-compatible probe expansion.
- Emit both JSON and YAML frozen scenario snapshots in every new evidence bundle.
- Bundle Python adapter modules with platform archives and resolve them beside the downloaded binary.
- Validate LLM-generated probe identifiers and queries before freezing them.

## [1.0.2] - 2026-08-22

### Changed

- Rebranded the project as MemoryProof, with ForgetProof retained as the compatibility erasure suite.
- Added the Isolation suite, stable `memoryproof.dev/v1` scenario API, and `memoryproof.adapter/v1` protocol.
- Added target/control subject isolation, explicit `UNKNOWN` semantics, bundle compatibility, and a bilingual offline report.
- Added multi-platform release packaging, multi-architecture OCI builds with SBOM/provenance, CodeQL, Scorecard, and a verified static matrix.
- Hardened failure cleanup with an ownership-scoped adapter guard, endpoint-level network authorization, response protocol validation, and safe request-ID evidence.
- Added Letta core-memory block creation/deletion and mock contract coverage for Mem0, Letta, and Zep.

## [0.1.0] - 2026-08-18

### Added

- Rust CLI for scenario execution, adapter discovery, evidence verification, and offline reports.
- Versioned NDJSON adapter protocol with reference-clean and reference-leaky backends.
- Python adapters for Mem0, Letta, and Zep.
- Deterministic conformance profiles: `FP-Object`, `FP-Scope`, `FP-Derived`, and `FP-Agent`.
- Redacted evidence bundles with HTML, JUnit, SHA-256 checksums, and bundle hashes.
- Docker-based GitHub Action and a public conformance matrix.
- English-first documentation with a complete Simplified Chinese translation.

### Security boundaries

ForgetProof does not claim physical disk erasure, provider-log deletion, backup deletion, or model-weight unlearning. Unsupported observations remain `UNKNOWN` or `OUT OF SCOPE`.
