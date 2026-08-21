# 变更记录

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

这里记录 MemoryProof 的重要变化。

## [未发布]

暂无未发布变更。

## [1.0.3] - 2026-08-22

### 修复

- 将所有通过校验的公开证据包放入 Pages 发布物，修复矩阵报告链接无法打开的问题。
- 为 `doctor` 和 OpenAI-compatible 探针扩展强制显式网络授权。
- 每个新证据包同时生成 JSON 和 YAML 冻结场景快照。
- 将 Python 适配器模块随平台压缩包发布，并支持从下载二进制旁边加载。
- 在冻结 LLM 生成的探针前校验其 ID 和查询内容。

## [1.0.2] - 2026-08-22

### 变更

- 项目升级为 MemoryProof 总品牌，并保留 ForgetProof 作为兼容的遗忘套件。
- 增加 Isolation 隔离套件、稳定的 `memoryproof.dev/v1` 场景 API 和 `memoryproof.adapter/v1` 协议。
- 增加目标/控制主体隔离、明确的 `UNKNOWN` 语义、证据包兼容和双语离线报告。
- 增加跨平台发行、多架构 OCI 镜像及 SBOM/来源证明、CodeQL、Scorecard 和已验证的静态矩阵。
- 增强失败路径清理、endpoint 级网络授权、响应协议校验，以及安全请求 ID 证据记录。
- 增加 Letta core-memory block 的创建/删除，并为 Mem0、Letta、Zep 增加 mock 契约覆盖。

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
