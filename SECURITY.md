# Security policy

[English](SECURITY.md) · [简体中文](SECURITY.zh-CN.md)

ForgetProof can issue destructive deletion requests to a configured memory backend. Use isolated test tenants and synthetic canaries only.

The runner refuses remote access unless `--allow-network` is explicit. Adapters must scope cleanup to resources created by the current run. Never put API keys in scenario files, issue logs, or committed evidence bundles.

Please report security issues privately to the repository maintainers instead of opening a public issue with credentials or customer data.
