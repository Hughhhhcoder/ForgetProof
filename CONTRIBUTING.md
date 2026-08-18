# Contributing to ForgetProof

[English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

The most valuable contributions are new adapters, reproducible erasure scenarios, and tests that expose a concrete residual-memory boundary.

Before opening a pull request:

1. Run `cargo fmt --all`, `cargo test`, and `python3 -m compileall python`.
2. Keep credentials and real user data out of scenarios and evidence bundles.
3. Add a reference or mocked test for every new adapter capability.
4. Make unsupported observability explicit as `UNKNOWN`; do not weaken a test to make a backend pass.

By contributing, you agree to the Developer Certificate of Origin (DCO). Add `Signed-off-by: Your Name <you@example.com>` to commits.
