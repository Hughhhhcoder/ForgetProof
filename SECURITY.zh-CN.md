# 安全策略

[English](SECURITY.md) · [简体中文](SECURITY.zh-CN.md)

ForgetProof 可以向配置好的记忆后端发起具有破坏性的删除请求。请只使用隔离的测试租户和合成 canary。

除非显式传入 `--allow-network`，测试引擎不会访问远程地址。适配器必须只清理当前运行创建的资源。不要把 API Key 写入场景文件、日志或提交到仓库的证据包。

如果发现安全问题，请私下联系仓库维护者，不要在公开 Issue 中发布凭据或客户数据。
