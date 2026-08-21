use crate::model::{
    normalize_value, Assertion, Event, Manifest, ProfileResult, RunResult, BUNDLE_FORMAT,
};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn hash_text(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

pub fn redact_content(content: &str, raw_payloads: bool) -> String {
    if raw_payloads {
        content.to_owned()
    } else {
        format!("sha256:{} len:{}", hash_text(content), content.len())
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn write_bundle(
    directory: &Path,
    scenario: &Value,
    events: &[Event],
    result: &RunResult,
    manifest: &Manifest,
) -> Result<String> {
    fs::create_dir_all(directory)?;

    let redacted_scenario = redact_scenario(scenario);
    write_json(&directory.join("scenario.lock.json"), &redacted_scenario)?;
    fs::write(
        directory.join("scenario.lock.yml"),
        serde_yaml::to_string(&redacted_scenario)?,
    )?;

    let events_path = directory.join("events.ndjson");
    let mut events_file = File::create(&events_path)?;
    for event in events {
        serde_json::to_writer(&mut events_file, event)?;
        events_file.write_all(b"\n")?;
    }

    write_json(&directory.join("results.json"), result)?;
    fs::write(directory.join("report.html"), render_html(result))?;
    fs::write(directory.join("junit.xml"), render_junit(result))?;
    write_json(&directory.join("manifest.json"), manifest)?;

    let files = manifest
        .files
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let checksums = checksum_text(directory, &files)?;
    fs::write(directory.join("checksums.sha256"), &checksums)?;
    let bundle_hash = sha256_hex(checksums.as_bytes());
    fs::write(directory.join("bundle.hash"), format!("{bundle_hash}\n"))?;
    Ok(bundle_hash)
}

pub fn refresh_bundle(directory: &Path) -> Result<String> {
    let result = load_result(directory)?;
    fs::write(directory.join("report.html"), render_html(&result))?;
    fs::write(directory.join("junit.xml"), render_junit(&result))?;
    let manifest: Manifest = serde_json::from_slice(&fs::read(directory.join("manifest.json"))?)?;
    let checksums = checksum_text(
        directory,
        &manifest
            .files
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )?;
    fs::write(directory.join("checksums.sha256"), &checksums)?;
    let bundle_hash = sha256_hex(checksums.as_bytes());
    fs::write(directory.join("bundle.hash"), format!("{bundle_hash}\n"))?;
    Ok(bundle_hash)
}

pub fn verify_bundle(directory: &Path) -> Result<Verification> {
    let manifest_path = directory.join("manifest.json");
    let manifest: Manifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .with_context(|| format!("missing {}", manifest_path.display()))?,
    )
    .context("invalid manifest.json")?;
    let checksums_path = directory.join("checksums.sha256");
    let file = File::open(&checksums_path)
        .with_context(|| format!("missing {}", checksums_path.display()))?;
    let mut errors = Vec::new();
    if manifest.format != BUNDLE_FORMAT && manifest.format != "forgetproof.bundle/v1alpha1" {
        errors.push(format!(
            "unsupported evidence format '{}', expected '{}'",
            manifest.format, BUNDLE_FORMAT
        ));
    }
    let expected_files = manifest.files.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen_files = BTreeSet::new();
    let mut checksum_text = String::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        checksum_text.push_str(&line);
        checksum_text.push('\n');
        let mut parts = line.splitn(2, "  ");
        let expected = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        if expected.len() != 64
            || !expected.chars().all(|c| c.is_ascii_hexdigit())
            || name.is_empty()
        {
            errors.push(format!("invalid checksum line: {line}"));
            continue;
        }
        if !seen_files.insert(name.to_owned()) {
            errors.push(format!("duplicate checksum entry: {name}"));
        }
        if !expected_files.contains(name) {
            errors.push(format!(
                "checksum lists file not declared in manifest: {name}"
            ));
        }
        let path = directory.join(name);
        if !safe_bundle_path(name) {
            errors.push(format!("unsafe bundle path: {name}"));
            continue;
        }
        match fs::read(&path) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual != expected {
                    errors.push(format!("checksum mismatch: {name}"));
                }
            }
            Err(_) => errors.push(format!("missing bundle file: {name}")),
        }
    }
    for expected in expected_files {
        if !seen_files.contains(&expected) {
            errors.push(format!(
                "manifest file missing from checksum list: {expected}"
            ));
        }
    }
    let expected_bundle = fs::read_to_string(directory.join("bundle.hash"))
        .unwrap_or_default()
        .trim()
        .to_owned();
    let actual_bundle = sha256_hex(checksum_text.as_bytes());
    if expected_bundle != actual_bundle {
        errors.push("bundle.hash does not match checksums.sha256".to_owned());
    }
    Ok(Verification {
        valid: errors.is_empty(),
        bundle_hash: actual_bundle,
        errors,
    })
}

#[derive(Debug, Clone)]
pub struct Verification {
    pub valid: bool,
    pub bundle_hash: String,
    pub errors: Vec<String>,
}

pub fn load_result(directory: &Path) -> Result<RunResult> {
    let bytes = fs::read(directory.join("results.json"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn checksum_text(directory: &Path, files: &[&str]) -> Result<String> {
    let mut ordered = files.to_vec();
    ordered.sort_unstable();
    let mut checksums = String::new();
    for file in ordered {
        if !safe_bundle_path(file) {
            bail!("unsafe bundle path: {file}");
        }
        let bytes = fs::read(directory.join(file))
            .with_context(|| format!("missing bundle file: {file}"))?;
        checksums.push_str(&format!("{}  {file}\n", sha256_hex(&bytes)));
    }
    Ok(checksums)
}

fn safe_bundle_path(name: &str) -> bool {
    let path = Path::new(name);
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn redact_scenario(scenario: &Value) -> Value {
    let raw_payloads = scenario
        .get("spec")
        .and_then(|spec| spec.get("privacy"))
        .and_then(|privacy| privacy.get("raw_payloads"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if raw_payloads {
        return normalize_value(scenario);
    }
    let mut safe = normalize_value(scenario);
    if let Some(spec) = safe.get_mut("spec").and_then(Value::as_object_mut) {
        if let Some(fixtures) = spec.get_mut("fixtures").and_then(Value::as_array_mut) {
            for fixture in fixtures {
                if let Some(object) = fixture.as_object_mut() {
                    if let Some(content) = object.get("content").and_then(Value::as_str) {
                        object.insert(
                            "content".to_owned(),
                            Value::String(redact_content(content, false)),
                        );
                    }
                }
            }
        }
        if let Some(probes) = spec.get_mut("probes").and_then(Value::as_object_mut) {
            for phase in ["before", "after"] {
                if let Some(items) = probes.get_mut(phase).and_then(Value::as_array_mut) {
                    for probe in items {
                        if let Some(object) = probe.as_object_mut() {
                            if let Some(query) = object.get("query").and_then(Value::as_str) {
                                object.insert(
                                    "query".to_owned(),
                                    Value::String(redact_content(query, false)),
                                );
                            }
                        }
                    }
                }
            }
        }
        if let Some(adapter) = spec.get_mut("adapter").and_then(Value::as_object_mut) {
            if let Some(config) = adapter.get_mut("config").and_then(Value::as_object_mut) {
                for (key, value) in config.iter_mut() {
                    if is_sensitive_key(key) {
                        *value = Value::String("<redacted>".to_owned());
                    }
                }
            }
        }
    }
    safe
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "key",
        "token",
        "secret",
        "password",
        "credential",
        "authorization",
    ]
    .iter()
    .any(|part| key.contains(part))
}

pub fn render_html(result: &RunResult) -> String {
    let status_class = result.status.to_lowercase();
    let profiles = result
        .profiles
        .iter()
        .map(render_profile_row)
        .collect::<Vec<_>>()
        .join("\n");
    let assertions = result
        .assertions
        .iter()
        .map(render_assertion_row)
        .collect::<Vec<_>>()
        .join("\n");
    let warnings = result
        .warnings
        .iter()
        .map(|warning| format!("<li>{}</li>", html_escape(warning)))
        .collect::<Vec<_>>()
        .join("");
    let out_of_scope = result
        .out_of_scope
        .iter()
        .map(|item| format!("<li>{}</li>", html_escape(item)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="MemoryProof evidence report / MemoryProof 证据报告">
<title>MemoryProof · {scenario}</title>
<style>
:root {{ color-scheme: light dark; --bg:#07101d; --card:#101c31; --line:#2a4269; --text:#eef5ff; --muted:#a9bbd8; --pass:#55e69a; --fail:#ff6d80; --unknown:#ffd166; --cyan:#83ddff; }}
* {{ box-sizing:border-box; }} body {{ margin:0; padding:clamp(18px,4vw,48px); font:15px/1.55 ui-sans-serif,system-ui,-apple-system,sans-serif; background:radial-gradient(circle at 80% 0,#18345d 0,transparent 34rem),var(--bg); color:var(--text); }}
main {{ max-width:1180px; margin:auto; }} .hero {{ display:flex; justify-content:space-between; gap:24px; align-items:flex-start; margin-bottom:26px; }}
h1 {{ margin:0 0 8px; font-size:clamp(30px,6vw,64px); letter-spacing:-.06em; line-height:1; }} h2 {{ margin:34px 0 12px; letter-spacing:-.03em; }} p {{ color:var(--muted); }}
.eyebrow {{ color:var(--cyan); font-size:11px; font-weight:800; letter-spacing:.14em; text-transform:uppercase; }} .status {{ border-radius:999px; padding:8px 15px; font-weight:800; letter-spacing:.04em; }}
.pass {{ color:var(--pass); }} .fail,.error {{ color:var(--fail); }} .unknown,.skip {{ color:var(--unknown); }} table {{ width:100%; border-collapse:collapse; }} th,td {{ text-align:left; padding:12px 10px; border-bottom:1px solid var(--line); vertical-align:top; }} th {{ color:var(--muted); font-size:11px; text-transform:uppercase; letter-spacing:.08em; }} code {{ color:#c9e3ff; overflow-wrap:anywhere; }}
.card {{ background:rgba(16,28,49,.86); border:1px solid var(--line); border-radius:18px; padding:20px; overflow:auto; }} .meta {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:12px; }} .meta div {{ padding:13px; border:1px solid var(--line); border-radius:12px; }} .label {{ display:block; color:var(--muted); font-size:12px; margin-bottom:3px; }}
.claims {{ display:grid; grid-template-columns:repeat(3,1fr); gap:12px; }} .claim {{ border:1px solid var(--line); border-radius:14px; padding:15px; }} .claim h3 {{ margin:0 0 4px; font-size:15px; }} .claim p {{ margin:0; font-size:13px; }}
li {{ color:var(--muted); margin:5px 0; }} footer {{ margin-top:36px; color:var(--muted); font-size:13px; }} @media(max-width:720px) {{ .hero {{ display:block; }} .hero .status {{ display:inline-block; margin-top:14px; }} .claims {{ grid-template-columns:1fr; }} }}
</style></head><body><main>
<section class="hero"><div><span class="eyebrow">MEMORYPROOF · {suite}</span><h1>{scenario}</h1><p>{adapter} · {backend} · {backend_version}</p><p>Prove what was observable, what disappeared, and what remains unknown.<br>证明可观察到什么、什么已经消失，以及什么仍然未知。</p></div><div class="status {status_class}">{status}</div></section>
<section class="meta card"><div><span class="label">Run / 运行</span><code>{run_id}</code></div><div><span class="label">Adapter mode / 适配器模式</span><code>{adapter_mode}</code></div><div><span class="label">Scenario hash / 场景哈希</span><code>{scenario_hash}</code></div><div><span class="label">Protocol / 协议</span><code>{protocol}</code></div><div><span class="label">Exit code / 退出码</span><code>{exit_code}</code></div></section>
<h2>Evidence summary / 证据摘要</h2><section class="claims"><article class="claim"><h3 class="pass">Proved / 已证明</h3><p>Required assertions marked PASS passed deterministic observable checks.</p></article><article class="claim"><h3 class="fail">Observed residue / 发现残留</h3><p>FAIL means a probe still observed the target or a forbidden derivative.</p></article><article class="claim"><h3 class="unknown">Unknown / 未知</h3><p>UNKNOWN means the backend did not expose enough evidence to claim more.</p></article></section>
<h2>Profiles / 认证档案</h2><section class="card"><table><thead><tr><th>Profile / 档案</th><th>Status / 状态</th><th>Assertions / 断言</th></tr></thead><tbody>{profiles}</tbody></table></section>
<h2>Assertions / 断言明细</h2><section class="card"><table><thead><tr><th>ID</th><th>Profile</th><th>Artifact / 边界</th><th>Status</th><th>Message / 说明</th><th>Expected / Observed</th></tr></thead><tbody>{assertions}</tbody></table></section>
<h2>Warnings / 警告</h2><section class="card"><ul>{warnings}</ul></section>
<h2>Out of scope / 无法证明</h2><section class="card"><ul>{out_of_scope}</ul></section>
<footer>MemoryProof reports controlled, observable evidence. It does not prove provider logs, backups, model-weight unlearning, physical storage erasure, or anything outside the adapter boundary.<br>MemoryProof 只报告受控且可观察的证据，不证明服务商日志、备份、模型权重反学习、物理存储擦除或适配器边界之外的事情。</footer>
</main></body></html>"#,
        suite = html_escape(&result.suite),
        scenario = html_escape(&result.scenario),
        adapter = html_escape(&result.adapter),
        backend = html_escape(&result.backend),
        backend_version = html_escape(&result.backend_version),
        adapter_mode = html_escape(&result.adapter_mode),
        status = html_escape(&result.status),
        status_class = html_escape(&status_class),
        run_id = html_escape(&result.run_id),
        scenario_hash = html_escape(&result.scenario_hash),
        protocol = html_escape(&result.protocol),
        exit_code = result.exit_code,
        profiles = profiles,
        assertions = assertions,
        warnings = if warnings.is_empty() {
            "<li>None / 无</li>".to_owned()
        } else {
            warnings
        },
        out_of_scope = out_of_scope,
    )
}

fn render_profile_row(profile: &ProfileResult) -> String {
    format!(
        "<tr><td><code>{}</code><br><span class=\"label\">{}</span></td><td class=\"{}\">{}</td><td>{}</td></tr>",
        html_escape(&profile.profile),
        html_escape(&profile.label),
        html_escape(&profile.status.to_lowercase()),
        html_escape(&profile.status),
        html_escape(&profile.assertions.join(", "))
    )
}

fn render_assertion_row(assertion: &Assertion) -> String {
    let expected = assertion
        .expected
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let observed = assertion
        .observed
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    format!(
        "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td><code>{} / {}</code></td></tr>",
        html_escape(&assertion.id),
        html_escape(&assertion.profile),
        html_escape(&assertion.artifact),
        html_escape(&assertion.status.to_lowercase()),
        html_escape(&assertion.status),
        html_escape(&assertion.message),
        html_escape(&expected),
        html_escape(&observed),
    )
}

pub fn render_junit(result: &RunResult) -> String {
    let cases = result
        .assertions
        .iter()
        .map(|assertion| {
            let body = match assertion.status.as_str() {
                "PASS" => String::new(),
                "FAIL" => format!("<failure message=\"{}\" />", xml_escape(&assertion.message)),
                "ERROR" => format!("<error message=\"{}\" />", xml_escape(&assertion.message)),
                _ => format!("<skipped message=\"{}\" />", xml_escape(&assertion.message)),
            };
            format!(
                "<testcase classname=\"{}\" name=\"{}\">{}</testcase>",
                xml_escape(&assertion.profile),
                xml_escape(&assertion.id),
                body
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"MemoryProof\" tests=\"{}\">{}</testsuite>",
        result.assertions.len(),
        cases
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn xml_escape(value: &str) -> String {
    html_escape(value)
}

pub fn details(pairs: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    pairs.into_iter().collect()
}

#[allow(dead_code)]
fn _assertion_types_are_serializable(_assertion: &Assertion, _profile: &ProfileResult) -> Value {
    Value::Null
}
