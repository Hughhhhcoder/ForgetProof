# Contributing to MemoryProof

[English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

MemoryProof is an assurance project: the most valuable contributions make an observable boundary clearer, safer, and easier to reproduce. New adapters, deterministic scenarios, and tests that expose a concrete residual-memory path are especially welcome.

## Before you start

- Read the [security policy](SECURITY.md). Use synthetic canaries and isolated tenants only.
- Check existing issues and pull requests before starting a large change.
- For a new adapter, document the backend version, endpoint semantics, deletion scope, and which capabilities are genuinely observable.
- Never add API keys, customer data, private URLs, or unredacted response payloads to the repository.

## Local checks

```bash
cargo fmt --all
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/memoryproof-target \
  cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/memoryproof-target \
  cargo test --workspace -- --test-threads=2
PYTHONPATH=python python3 -m unittest discover -s python/tests -v
python3 scripts/build_matrix.py
```

Also run the relevant reference scenarios:

```bash
cargo run -- run examples/reference-clean.yml
cargo run -- run examples/isolation-reference.yml
cargo run -- run examples/reference-leaky.yml       # expected exit code: 1
cargo run -- run examples/reference-overdelete.yml   # expected exit code: 1
```

## Design rules

1. **Do not create false passes.** If a backend cannot expose a required boundary, report `UNKNOWN` or `SKIP`.
2. **Prove the precondition.** A delete test must first show that the target can be observed.
3. **Keep target and control fixtures separate.** A test that deletes the control subject is a failure.
4. **Keep the adapter protocol boring.** stdout is NDJSON frames only; diagnostics go to stderr.
5. **Make evidence deterministic.** Freeze generated probes, normalize JSON, and include hashes.
6. **Prefer additive changes.** Preserve the v0.1 loader and compatibility binary when practical.
7. **Keep documentation paired.** New user-facing material belongs in the English README first and in the Simplified Chinese README and relevant guide in the same change.

## Adding an adapter

Implement the versioned methods in `python/forgetproof_adapters/`, advertise capabilities honestly, and add a mocked contract test. The adapter must:

- create resources with a unique `memoryproof` run marker;
- refuse destructive operations against resources it did not create;
- expose request IDs without exposing credentials;
- turn unsupported observability into `UNKNOWN`, never an invented success;
- tolerate asynchronous backends through `settle` rather than guessing that a write is stable.

If an upstream API has ambiguous delete semantics, document the ambiguity in the adapter and report it in the evidence bundle.

## Pull requests

- Use a focused branch and a descriptive title.
- Include the problem, the observable behavior before/after, and the test commands you ran.
- Add or update a scenario when behavior changes.
- Keep generated evidence redacted and small; explain the backend version and reproduction command.
- Keep CI green. Maintainers may ask for a draft PR while the contract is being discussed.

## Commit sign-off

By contributing, you agree to the [Developer Certificate of Origin](DCO). Sign each commit with:

```text
Signed-off-by: Your Name <you@example.com>
```

For example:

```bash
git commit -s -m "feat: add an isolation probe"
```

## License and conduct

Contributions are licensed under Apache-2.0. Please follow the [Code of Conduct](CODE_OF_CONDUCT.md) and keep technical disagreement specific, respectful, and evidence-led.
