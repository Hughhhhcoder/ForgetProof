# 变更记录

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

这里记录 MemoryProof 的重要变化。

## [未发布]

### 变更

- 项目升级为 MemoryProof 总品牌，并保留 ForgetProof 作为兼容的遗忘套件。
- 增加 Isolation 隔离套件、稳定的 `memoryproof.dev/v1` 场景 API 和 `memoryproof.adapter/v1` 协议。
- 增加目标/控制主体隔离、明确的 `UNKNOWN` 语义、证据包兼容和双语离线报告。
- 增加跨平台发行、多架构 OCI 镜像及 SBOM/来源证明、CodeQL、Scorecard 和已验证的静态矩阵。

## [0.1.0] - 2026-08-18

### 新增

- Rust CLI：执行场景、发现适配器、验证证据和生成离线报告。
- 版本化 NDJSON 适配器协议，以及 reference-clean 和 reference-leaky 参考后端。
- Mem0、Letta、Zep Python 适配器。
- `FP-Object`、`FP-Scope`、`FP-Derived`、`FP-Agent` 确定性认证档案。
- 默认脱敏的证据包，包含 HTML、JUnit、SHA-256 校验清单和 bundle hash。
- Docker 版 GitHub Action 与公开认证矩阵。
- 英文优先文档及完整简体中文翻译。

### 安全边界

MemoryProof 不会声称物理磁盘擦除、服务商日志删除、备份删除或模型权重反学习。无法观察的部分统一保持为 `UNKNOWN` 或 `OUT OF SCOPE`。
