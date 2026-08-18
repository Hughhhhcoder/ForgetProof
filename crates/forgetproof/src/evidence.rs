use crate::model::{Assertion, Event, Manifest, ProfileResult, RunResult};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

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

    let scenario_path = directory.join("scenario.lock.json");
    write_json(&scenario_path, &redact_scenario(scenario))?;

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

    let files = [
        "manifest.json",
        "scenario.lock.json",
        "events.ndjson",
        "results.json",
        "report.html",
        "junit.xml",
    ];
    let mut checksums = String::new();
    for file in files {
        let bytes = fs::read(directory.join(file))?;
        checksums.push_str(&format!("{}  {}\n", sha256_hex(&bytes), file));
    }
    fs::write(directory.join("checksums.sha256"), &checksums)?;
    let bundle_hash = sha256_hex(checksums.as_bytes());
    fs::write(directory.join("bundle.hash"), format!("{bundle_hash}\n"))?;
    Ok(bundle_hash)
}

pub fn verify_bundle(directory: &Path) -> Result<Verification> {
    let checksums_path = directory.join("checksums.sha256");
    let file = File::open(&checksums_path)
        .with_context(|| format!("missing {}", checksums_path.display()))?;
    let mut errors = Vec::new();
    let mut checksum_text = String::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        checksum_text.push_str(&line);
        checksum_text.push('\n');
        let mut parts = line.splitn(2, "  ");
        let expected = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        if expected.is_empty() || name.is_empty() {
            errors.push(format!("invalid checksum line: {line}"));
            continue;
        }
        let path = directory.join(name);
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

fn redact_scenario(scenario: &Value) -> Value {
    let raw_payloads = scenario
        .get("spec")
        .and_then(|spec| spec.get("privacy"))
        .and_then(|privacy| privacy.get("raw_payloads"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if raw_payloads {
        return scenario.clone();
    }
    let mut safe = scenario.clone();
    if let Some(fixtures) = safe
        .get_mut("spec")
        .and_then(Value::as_object_mut)
        .and_then(|spec| spec.get_mut("fixtures"))
        .and_then(Value::as_array_mut)
    {
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
    safe
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
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>ForgetProof · {scenario}</title>
<style>
:root {{ color-scheme: light dark; --bg:#0b1020; --card:#121a2c; --text:#e8edf7; --muted:#9aa8c0; --pass:#35d07f; --fail:#ff667a; --unknown:#ffca5f; }}
* {{ box-sizing:border-box; }} body {{ margin:0; padding:36px; font:15px/1.5 ui-sans-serif,system-ui,-apple-system,sans-serif; background:var(--bg); color:var(--text); }}
main {{ max-width:1100px; margin:auto; }} .hero {{ display:flex; justify-content:space-between; gap:24px; align-items:flex-start; margin-bottom:28px; }}
h1 {{ margin:0 0 8px; font-size:clamp(28px,5vw,52px); letter-spacing:-.04em; }} h2 {{ margin-top:32px; }} p {{ color:var(--muted); }}
.card {{ background:var(--card); border:1px solid #273452; border-radius:18px; padding:20px; overflow:auto; }} .status {{ border-radius:999px; padding:8px 14px; font-weight:700; letter-spacing:.04em; }}
.pass {{ color:var(--pass); }} .fail,.error {{ color:var(--fail); }} .unknown,.skip {{ color:var(--unknown); }} table {{ width:100%; border-collapse:collapse; }} th,td {{ text-align:left; padding:12px 10px; border-bottom:1px solid #273452; vertical-align:top; }} th {{ color:var(--muted); font-size:12px; text-transform:uppercase; letter-spacing:.08em; }} code {{ color:#b8d6ff; }}
.meta {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:12px; }} .meta div {{ padding:12px; border:1px solid #273452; border-radius:12px; }} .label {{ display:block; color:var(--muted); font-size:12px; }}
</style></head><body><main>
<section class="hero"><div><p>FORGETPROOF · Prove your AI forgot.</p><h1>{scenario}</h1><p>{adapter} · {backend}</p></div><div class="status {status_class}">{status}</div></section>
<section class="meta card"><div><span class="label">Run</span><code>{run_id}</code></div><div><span class="label">Scenario hash</span><code>{scenario_hash}</code></div><div><span class="label">Exit code</span><code>{exit_code}</code></div><div><span class="label">Evidence policy</span><code>redacted by default</code></div></section>
<h2>Conformance profiles</h2><section class="card"><table><thead><tr><th>Profile</th><th>Status</th><th>Assertions</th></tr></thead><tbody>{profiles}</tbody></table></section>
<h2>Assertions</h2><section class="card"><table><thead><tr><th>ID</th><th>Profile</th><th>Status</th><th>Message</th></tr></thead><tbody>{assertions}</tbody></table></section>
<p>ForgetProof reports controlled, observable evidence. It does not prove provider logs, backups, model-weight unlearning, or physical storage erasure.</p>
</main></body></html>"#,
        scenario = html_escape(&result.scenario),
        adapter = html_escape(&result.adapter),
        backend = html_escape(&result.backend),
        status = html_escape(&result.status),
        status_class = html_escape(&status_class),
        run_id = html_escape(&result.run_id),
        scenario_hash = html_escape(&result.scenario_hash),
        exit_code = result.exit_code,
        profiles = profiles,
        assertions = assertions,
    )
}

fn render_profile_row(profile: &ProfileResult) -> String {
    format!(
        "<tr><td><code>{}</code></td><td class=\"{}\">{}</td><td>{}</td></tr>",
        html_escape(&profile.profile),
        html_escape(&profile.status.to_lowercase()),
        html_escape(&profile.status),
        html_escape(&profile.assertions.join(", "))
    )
}

fn render_assertion_row(assertion: &Assertion) -> String {
    format!(
        "<tr><td><code>{}</code></td><td>{}</td><td class=\"{}\">{}</td><td>{}</td></tr>",
        html_escape(&assertion.id),
        html_escape(&assertion.profile),
        html_escape(&assertion.status.to_lowercase()),
        html_escape(&assertion.status),
        html_escape(&assertion.message)
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
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><testsuite name=\"ForgetProof\" tests=\"{}\">{}</testsuite>",
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
