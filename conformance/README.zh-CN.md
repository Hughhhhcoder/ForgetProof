# 公开 Memory Assurance 证据

[English](README.md) · [简体中文](README.zh-CN.md)

![MemoryProof 证据流程](../assets/memoryproof-hero.png)

该目录包含经过审阅、已经脱敏、无需在线服务即可打开的小型证据包。Pages 工作流会先校验每个证据包的哈希，再把其中的档案加入公开矩阵。

## 提交必须包含什么

`conformance/evidence/` 下的每个目录都应该是由 `memoryproof run` 生成的完整证据包，并包含：

- `manifest.json`、`scenario.lock.json`、`scenario.lock.yml`、`events.ndjson` 和 `results.json`；
- `report.html` 和 `junit.xml`；
- `checksums.sha256` 和 `bundle.hash`；
- 只有合成 fixture，已移除凭据和客户原文。

矩阵会按档案展示 `PASS`、`FAIL`、`SKIP` 或 `UNKNOWN`，不会把它们压成一个综合分数。只要可复现且描述诚实，失败或未知结果同样欢迎提交。

## 复现本地提交

```bash
cargo run -- run examples/reference-clean.yml --output /tmp/memoryproof-evidence
python3 scripts/build_matrix.py
cargo run -- matrix validate site/matrix.json
```

对于远程供应商，只有在确认场景、凭据、租户和清理行为后，才传入 `--allow-network`。公开远程证据前必须人工审阅。

## 审阅清单

1. 使用 `memoryproof verify <bundle>` 校验证据包。
2. 确认 `scenario.lock.json` 只包含合成 canary。
3. 确认记录了后端名称和版本。
4. 确认目标/控制范围，以及具体失败或通过的探针可见。
5. 确认不支持的边界被标为 `UNKNOWN`、`SKIP` 或明确的 `OUT OF SCOPE`。
6. 在配套 Release Note 或 Pull Request 中标明维护者复现还是社区提交。

公开矩阵是证据索引，不是供应商排行榜。

`*-adapter-contract` 证据包是维护者使用
`scripts/mock_remote_backend.py` 中的确定性本地 mock 复现的结果。它们验证
官方适配器契约，不声称某个托管供应商一定具有相同的行为。供应商认证结果必须
记录供应商版本，并经过单独审阅的带凭据运行。
