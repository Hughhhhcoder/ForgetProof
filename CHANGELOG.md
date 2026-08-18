# Changelog

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

All notable changes to ForgetProof are documented here.

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
