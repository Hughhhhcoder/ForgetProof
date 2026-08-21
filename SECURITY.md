# Security policy

[English](SECURITY.md) · [简体中文](SECURITY.zh-CN.md)

MemoryProof can issue destructive deletion requests against a configured memory backend. Treat every remote run as a destructive test: use a dedicated tenant, synthetic canaries, and credentials with the smallest possible scope.

## Safe operating rules

- Remote access is denied unless `--allow-network` is explicitly supplied.
- Scenarios must use owned, temporary resources. Adapters must refuse cleanup or scope deletion without the current run’s ownership marker.
- Never put API keys, bearer tokens, customer data, private endpoint URLs, or raw production responses in a scenario, issue, pull request, or committed evidence bundle.
- Evidence is redacted by default. Review generated artifacts before sharing them publicly.
- `UNKNOWN` and `OUT OF SCOPE` are security boundaries, not successful deletion claims.

## Reporting a vulnerability

Please use [GitHub private vulnerability reporting](https://github.com/Hughhhhcoder/MemoryProof/security/advisories/new) when available. If the form is unavailable, contact the maintainer through the repository profile and do not include credentials or customer data in the first message.

Include:

- affected commit, release, or adapter;
- a minimal reproduction using synthetic data;
- impact and likely attack path;
- any mitigation that has already been tested.

We will acknowledge a report when we can, coordinate a fix and disclosure timeline with the reporter, and credit the reporter unless they prefer to remain anonymous. Please do not publicly disclose an unpatched vulnerability.

## Scope

The security policy covers the MemoryProof CLI, adapter protocol, official adapters, Docker image, GitHub Action, evidence redaction, and release workflow. It does not make claims about the security of a third-party memory provider or prove that a provider deleted logs, backups, physical media, or model weights.
