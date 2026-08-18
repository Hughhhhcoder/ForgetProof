# 为 ForgetProof 贡献代码

[English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

最有价值的贡献包括：新的记忆后端适配器、可重复的遗忘场景，以及能够发现具体残留数据边界的测试。

提交 Pull Request 前，请完成：

1. 运行 `cargo fmt --all`、`cargo test` 和 `python3 -m compileall python`。
2. 不要把凭据或真实用户数据放进场景文件和证据包。
3. 每新增一种适配器能力，都要补充参考测试或模拟 API 测试。
4. 无法观察的能力必须明确报告为 `UNKNOWN`，不能为了让某个后端通过而降低测试标准。

贡献代码即表示你同意 Developer Certificate of Origin（DCO）。请在提交信息中加入 `Signed-off-by: Your Name <you@example.com>`。
