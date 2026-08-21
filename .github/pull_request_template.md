## What changed / 改了什么

<!-- Explain the user-visible change in plain language. / 用简单语言说明用户能感知的变化。 -->

## Why / 为什么

<!-- Link an issue or explain the problem this solves. / 关联 issue 或说明解决的问题。 -->

## Validation / 验证

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `PYTHONPATH=python python -m unittest discover -s python/tests -v`
- [ ] Documentation links and redaction boundaries checked / 已检查文档链接和脱敏边界

## Evidence and privacy / 证据与隐私

- [ ] No credentials or real user data are included / 未包含凭据或真实用户数据
- [ ] New unsupported capability is reported as `UNKNOWN` / 新的不支持能力报告为 `UNKNOWN`
