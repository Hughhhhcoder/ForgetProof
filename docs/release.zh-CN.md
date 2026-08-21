# 发布运维说明

[English](release.md)

发布工作流会构建四个平台压缩包、无额外依赖的 Python distribution，
以及多架构 GHCR 镜像。没有配置的外部服务不会被工作流静默发布。

## PyPI Trusted Publisher

如果要在不保存长期 Token 的情况下发布 `memoryproof-adapters`：

1. 在 PyPI 添加待确认的 Trusted Publisher：Owner 为 `Hughhhhcoder`，
   Repository 为 `MemoryProof`，Workflow 为
   `.github/workflows/release.yml`，Environment 为 `pypi`。
2. 将仓库 Actions 变量 `PUBLISH_PYPI` 设置为 `true`。
3. 创建新的版本 tag，等待 `Publish PyPI package (opt-in)` job 完成。
4. 在 [PyPI 项目页](https://pypi.org/project/memoryproof-adapters/)和
   `python -m pip index versions memoryproof-adapters` 中验证版本。

在这些设置完成前，wheel 和 sdist 仍会放在 GitHub Release 中，PyPI job
会有意跳过。

## 公开 GHCR 镜像

第一次成功推送镜像后，打开 `Hughhhhcoder/memoryproof` 的包设置，将可见性
改为 **Public**。然后在不带凭据的情况下验证 tag 和 manifest：

```bash
docker pull ghcr.io/hughhhhcoder/memoryproof:latest
docker pull ghcr.io/hughhhhcoder/memoryproof:1.0.4
curl -fsS \
  -H 'Accept: application/vnd.oci.image.index.v1+json' \
  https://ghcr.io/v2/hughhhhcoder/memoryproof/manifests/latest
```

返回 `401` 表示包仍是私有，或可见性变化还没有在 registry 中传播；它
不能证明镜像构建失败。

## 证据与发布来源

发行压缩包包含 CLI 和 Python 适配器模块。公开 Pages 只从通过校验的证据
包生成；带凭据的供应商证据只上传待审核 artifact，不会自动提交。
