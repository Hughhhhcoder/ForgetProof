# 参与贡献 MemoryProof

[English](CONTRIBUTING.md) · [简体中文](CONTRIBUTING.zh-CN.md)

MemoryProof 是一个“记忆保证”项目：最有价值的贡献，是让某个可观察边界更清楚、更安全、更容易复现。欢迎提交新适配器、确定性的测试场景，以及能暴露具体残留记忆路径的测试。

## 开始前

- 先阅读[安全策略](SECURITY.zh-CN.md)，只使用合成 canary 和隔离租户。
- 开始大型改动前，先检查已有 Issue 和 Pull Request。
- 新适配器需要记录后端版本、端点语义、删除范围，以及真正可以观察到的能力。
- 不要把 API Key、客户数据、私有 URL 或未脱敏响应加入仓库。

## 本地检查

```bash
cargo fmt --all
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/memoryproof-target \
  cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/memoryproof-target \
  cargo test --workspace -- --test-threads=2
PYTHONPATH=python python3 -m unittest discover -s python/tests -v
python3 scripts/build_matrix.py
```

同时运行相关参考场景：

```bash
cargo run -- run examples/reference-clean.yml
cargo run -- run examples/isolation-reference.yml
cargo run -- run examples/reference-leaky.yml       # 预期退出码：1
cargo run -- run examples/reference-overdelete.yml   # 预期退出码：1
```

## 设计规则

1. **不要制造假通过。** 后端无法暴露必需边界时，报告 `UNKNOWN` 或 `SKIP`。
2. **先证明前置条件。** 删除测试必须先证明目标确实可以被观察到。
3. **目标和控制 fixture 分开。** 控制主体被删除就是失败。
4. **让适配器协议保持简单。** stdout 只允许 NDJSON 帧，诊断日志写 stderr。
5. **让证据可确定性复现。** 冻结生成的探针、规范化 JSON 并加入哈希。
6. **优先做向后兼容的增量改动。** 条件允许时保留 v0.1 加载器和兼容二进制。
7. **双语同步。** 新的用户介绍先写英文 README，同时在中文 README 和相关指南中补上对应内容。

## 添加适配器

在 `python/forgetproof_adapters/` 中实现版本化方法，诚实声明能力，并添加模拟 API 的契约测试。适配器必须：

- 使用唯一的 `memoryproof` 运行标记创建资源；
- 拒绝操作不是本次运行创建的资源；
- 记录请求 ID，但不暴露凭据；
- 无法观察的边界报告 `UNKNOWN`，不能伪造成功；
- 通过 `settle` 处理异步后端，不能猜测写入已经稳定。

如果上游 API 的删除语义有歧义，请在适配器和证据包中明确记录。

## Pull Request

- 使用专注的分支和清晰的标题。
- 写明问题、前后可观察行为，以及运行过的测试命令。
- 行为发生变化时，添加或更新场景。
- 生成的证据必须脱敏且尽量小，并说明后端版本和复现命令。
- 保持 CI 通过。维护者可能会先要求以 Draft PR 讨论契约。

## 提交签署

贡献即表示同意 [Developer Certificate of Origin](DCO)。每次提交添加：

```text
Signed-off-by: Your Name <you@example.com>
```

例如：

```bash
git commit -s -m "feat: add an isolation probe"
```

## 许可证与行为

贡献内容采用 Apache-2.0 许可证。请遵守[行为准则](CODE_OF_CONDUCT.zh-CN.md)，以具体证据讨论技术分歧，并保持尊重。
