# 公开认证结果提交说明

[English](README.md) · [简体中文](README.zh-CN.md)

![ForgetProof 证据流程](../assets/forgetproof-hero.png)

该目录中的每个 JSON 文件，都是一个经过审阅、已经脱敏的证据包引用。Pages 工作流会把它生成静态认证矩阵，不会计算一个综合分数。

```json
{
  "adapter": "reference-clean",
  "backend": "forgetproof-reference@0.1.0",
  "bundle": "https://example.invalid/bundles/reference-clean",
  "profiles": [
    { "profile": "FP-Object", "status": "PASS", "evidence": "https://example.invalid/report.html" }
  ]
}
```

合并提交前，请使用 `forgetproof verify` 验证证据包，确认场景只包含合成数据，并记录该结果是维护者复现还是社区提交。
