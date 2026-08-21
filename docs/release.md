# Release operations

[简体中文](release.zh-CN.md)

The release workflow builds four native archives, the dependency-free Python
distribution, and a multi-architecture GHCR image. The release job does not
silently publish to services that have not been configured.

## PyPI Trusted Publisher

To publish `memoryproof-adapters` without storing a long-lived token:

1. On PyPI, add a pending trusted publisher for owner `Hughhhhcoder`,
   repository `MemoryProof`, workflow `.github/workflows/release.yml`, and
   environment `pypi`.
2. Set the repository Actions variable `PUBLISH_PYPI` to `true`.
3. Create a new version tag and wait for the `Publish PyPI package (opt-in)`
   job to complete.
4. Verify the published version at
   `https://pypi.org/project/memoryproof-adapters/` and with
   `python -m pip index versions memoryproof-adapters`.

Until these settings exist, the wheel and sdist remain available in the
GitHub Release and the PyPI job is intentionally skipped.

## Public GHCR image

After the first successful image push, open the package settings for
`Hughhhhcoder/memoryproof` and set its visibility to **Public**. Verify both
the tag and the manifest without credentials:

```bash
docker pull ghcr.io/hughhhhcoder/memoryproof:latest
docker pull ghcr.io/hughhhhcoder/memoryproof:1.0.4
curl -fsS \
  -H 'Accept: application/vnd.oci.image.index.v1+json' \
  https://ghcr.io/v2/hughhhhcoder/memoryproof/manifests/latest
```

A `401` response means the package is private or the registry has not
propagated the visibility change. It is not evidence that the image build
failed.

## Evidence and release provenance

Release archives include the CLI and Python adapter modules. Public Pages
evidence is generated from checksum-verified bundles; credentialed provider
evidence is uploaded for review and is never committed automatically.
