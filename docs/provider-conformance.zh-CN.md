# 带凭据的供应商一致性测试

[English](provider-conformance.md)

仓库中的适配器契约证据使用确定性的本地 mock。它能证明适配器实现和
协议，但不能声称某个 Mem0、Letta 或 Zep 云端部署具有相同的删除语义。

`Provider conformance / 云端适配器一致性` 工作流提供独立的真实凭据测试路径：

1. 将供应商 API Key 放入 GitHub Actions Secret：`MEM0_API_KEY`、
   `LETTA_API_KEY` 或 `ZEP_API_KEY`。
2. 将供应商 endpoint 放入 Actions Variable：`MEM0_BASE_URL`、
   `LETTA_BASE_URL` 或 `ZEP_BASE_URL`。如果 endpoint 本身敏感，也可以用
   同名 Secret 覆盖变量。
3. 手动运行工作流并选择供应商；或者设置仓库变量
   `PROVIDER_CONFORMANCE_ENABLED=true` 开启每周定时测试。
4. 审核上传的证据包后，才能提交到 `conformance/evidence/`。

每次远程测试都会显式传入 `--allow-network`，创建合成目标/控制主体，
验证证据包，扫描 artifact 中是否出现 API Key，并将结果作为工作流
artifact 上传。它不会自动提交远程证据。`PASS` 只对具体供应商和版本
有效；无法观察的能力仍保持 `UNKNOWN` 或 `SKIP`。

请使用专用租户并且只写入合成数据。供应商的保留、备份和日志策略需要
单独审查；这些边界仍然不在 MemoryProof 的证明范围内。
