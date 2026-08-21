# Credentialed provider conformance

[简体中文](provider-conformance.zh-CN.md)

The repository’s adapter-contract evidence runs against deterministic local
mocks. That proves the adapter implementation and protocol, but it does not
claim that a hosted Mem0, Letta, or Zep deployment has the same deletion
semantics.

The `Provider conformance / 云端适配器一致性` workflow provides the separate
credentialed path:

1. Add the provider API key as a GitHub Actions Secret: `MEM0_API_KEY`,
   `LETTA_API_KEY`, or `ZEP_API_KEY`.
2. Add the provider endpoint as an Actions Variable (`MEM0_BASE_URL`,
   `LETTA_BASE_URL`, or `ZEP_BASE_URL`). A secret with the same name takes
   precedence when the endpoint itself is sensitive.
3. Run the workflow manually with one provider, or set the repository variable
   `PROVIDER_CONFORMANCE_ENABLED=true` to enable the weekly schedule.
4. Review the uploaded bundle before submitting it to `conformance/evidence/`.

Every remote run passes `--allow-network`, creates synthetic target/control
subjects, verifies the bundle, scans artifacts for the API key, and uploads
the result as a workflow artifact. It never commits remote evidence
automatically. A `PASS` from this workflow is provider- and version-specific;
missing capabilities remain `UNKNOWN` or `SKIP`.

Use a dedicated tenant and synthetic data only. Check provider retention,
backup, and log policies separately: those boundaries remain out of scope for
MemoryProof.
