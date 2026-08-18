# Public conformance submissions

[English](README.md) · [简体中文](README.zh-CN.md)

![ForgetProof evidence flow](../assets/forgetproof-hero.png)

Each JSON file in this directory is a reviewed, redacted pointer to an evidence bundle. The Pages workflow converts it into the static matrix; it does not compute a score.

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

Before merging a submission, verify the referenced bundle with `forgetproof verify`, check that the scenario contains only synthetic data, and record whether the run was maintainer-reproduced or community-submitted.
