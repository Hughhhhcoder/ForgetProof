use crate::evidence::{details, hash_text, write_bundle};
use crate::model::{
    capability_available, display_profile, erase_capability, profile_capabilities,
    profile_for_probe, Assertion, Capabilities, Event, Fixture, Manifest, Probe, ProfileResult,
    RunResult, Scenario, BUNDLE_FORMAT, PROTOCOL_VERSION,
};
use crate::protocol::AdapterClient;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct RunOutcome {
    pub result: RunResult,
    pub directory: PathBuf,
    pub bundle_hash: String,
}

pub fn load_scenario(path: &Path) -> Result<(Scenario, Value)> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read scenario {}", path.display()))?;
    let value: Value = if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        serde_json::from_str(&source).context("invalid JSON scenario")?
    } else {
        serde_yaml::from_str(&source).context("invalid YAML scenario")?
    };
    let mut scenario: Scenario =
        serde_json::from_value(value).context("invalid scenario schema")?;
    scenario
        .validate()
        .map_err(|errors| anyhow!(errors.join("; ")))?;
    scenario.normalize_legacy();
    scenario
        .validate()
        .map_err(|errors| anyhow!(errors.join("; ")))?;
    let normalized = serde_json::to_value(&scenario)?;
    Ok((scenario, normalized))
}

pub fn run_scenario(
    scenario_path: &Path,
    output_root: &Path,
    allow_network: bool,
) -> Result<RunOutcome> {
    let (scenario, scenario_value) = load_scenario(scenario_path)?;
    enforce_network_policy(&scenario, allow_network)?;

    let run_id = format!("run-{}-{}", now_ms(), std::process::id());
    let directory = output_root.join(&run_id);
    fs::create_dir_all(&directory)?;
    let normalized_scenario = serde_json::to_string(&scenario_value)?;
    let scenario_hash = hash_text(&normalized_scenario);
    // Python startup can be materially slower on macOS/Windows runners when
    // several conformance tests start adapters concurrently. The scenario
    // settle timeout still controls backend convergence; this bound only
    // prevents a healthy adapter handshake from being classified as a crash.
    let call_timeout = scenario.spec.settle.timeout_ms.max(30_000);
    let mut events = Vec::new();
    let mut warnings = Vec::new();
    let mut assertions = Vec::new();
    let mut client = AdapterClient::spawn(
        &scenario.spec.adapter.name,
        &scenario.spec.adapter.mode,
        &scenario.spec.adapter.config,
    )?;

    client.hello(call_timeout)?;
    record_event(
        &mut events,
        "prepare",
        "hello",
        "PASS",
        details([(String::from("protocol"), PROTOCOL_VERSION.to_owned())]),
    );
    let capabilities = client.capabilities(call_timeout)?;
    record_event(
        &mut events,
        "prepare",
        "capabilities",
        "PASS",
        details([
            (String::from("adapter"), capabilities.adapter.clone()),
            (String::from("backend"), capabilities.backend.clone()),
            (String::from("version"), capabilities.version.clone()),
        ]),
    );
    add_capability_assertions(&scenario, &capabilities, &mut assertions);

    let fixture_payloads = scenario
        .spec
        .fixtures
        .iter()
        .map(|fixture| fixture_payload(&scenario, fixture))
        .collect::<Vec<_>>();
    call_checked(
        &mut client,
        &mut events,
        "prepare",
        json!({
            "run_id": run_id,
            "suite": scenario.spec.suite,
            "isolation": scenario.spec.isolation,
            "fixtures": fixture_payloads,
        }),
        call_timeout,
    )?;

    for fixture in &scenario.spec.fixtures {
        call_checked(
            &mut client,
            &mut events,
            "ingest",
            json!({
                "run_id": run_id,
                "fixture": fixture_payload(&scenario, fixture),
            }),
            call_timeout,
        )?;
    }

    let initial_stable = settle(
        &mut client,
        &mut events,
        &scenario,
        call_timeout,
        &mut warnings,
        "after-ingest",
    )?;

    let mut precondition_failed = false;
    if scenario.spec.suite == "erasure" {
        for probe in &scenario.spec.probes.before {
            let fixture = fixture_by_id(&scenario, &probe.fixture)?;
            let profile = profile_for_probe(probe, &scenario.spec.suite);
            let observed = run_probe_if_supported(
                &mut client,
                &mut events,
                &scenario,
                &run_id,
                probe,
                "before",
                call_timeout,
                &capabilities,
                &mut assertions,
            )?;
            match observed {
                Some(true) if initial_stable => assertions.push(assertion(
                    &format!("before.{}", probe.id),
                    &profile,
                    "PASS",
                    format!(
                        "{} canary '{}' was observable before the erase operation",
                        probe.kind, fixture.id
                    ),
                    Some(true),
                    Some(true),
                    &probe.kind,
                    format!("probe:{}", probe.id),
                )),
                Some(false) => {
                    precondition_failed = true;
                    assertions.push(assertion(
                        &format!("before.{}", probe.id),
                        &profile,
                        "ERROR",
                        format!(
                            "canary '{}' was not observable before the erase operation",
                            fixture.id
                        ),
                        Some(true),
                        Some(false),
                        &probe.kind,
                        format!("probe:{}", probe.id),
                    ));
                }
                Some(true) => {
                    precondition_failed = true;
                    assertions.push(unknown_assertion(
                        &format!("before.{}", probe.id),
                        &profile,
                        "backend did not reach a stable state before the precondition probe",
                        &probe.kind,
                        format!("probe:{}", probe.id),
                    ));
                }
                None => precondition_failed = true,
            }
        }
    } else {
        for probe in &scenario.spec.probes.before {
            let fixture = fixture_by_id(&scenario, &probe.fixture)?;
            let profile = profile_for_probe(probe, &scenario.spec.suite);
            let expected = scenario.probe_subject(probe)
                == fixture.effective_subject(&scenario.spec.isolation);
            let observed = run_probe_if_supported(
                &mut client,
                &mut events,
                &scenario,
                &run_id,
                probe,
                "isolation",
                call_timeout,
                &capabilities,
                &mut assertions,
            )?;
            if let Some(observed) = observed {
                assertions.push(assertion(
                    &format!("before.{}", probe.id),
                    &profile,
                    if initial_stable && expected == observed {
                        "PASS"
                    } else if !initial_stable {
                        "UNKNOWN"
                    } else {
                        "FAIL"
                    },
                    format!(
                        "query subject '{}' expected fixture '{}' to be {}, observed {}",
                        scenario.probe_subject(probe),
                        fixture.id,
                        if expected { "isolated" } else { "inaccessible" },
                        if observed { "visible" } else { "absent" }
                    ),
                    Some(expected),
                    Some(observed),
                    "namespace",
                    format!("probe:{}", probe.id),
                ));
            }
        }
    }

    if scenario.spec.suite == "erasure" && !precondition_failed {
        let erase_capability = erase_capability(&scenario.spec.erase.intent);
        if capability_available(&capabilities, erase_capability) {
            call_checked(
                &mut client,
                &mut events,
                "erase",
                json!({
                    "run_id": run_id,
                    "intent": scenario.spec.erase.intent,
                    "target": scenario.spec.erase.target,
                    "target_subject": target_subject(&scenario),
                    "control_subject": scenario.spec.isolation.control_subject,
                    "scope": scenario.spec.erase.scope,
                }),
                call_timeout,
            )?;
        } else {
            warnings.push(format!(
                "erase intent '{}' requires unsupported capability '{}'",
                scenario.spec.erase.intent, erase_capability
            ));
            assertions.push(unknown_assertion(
                "operation.erase",
                "erasure.object",
                "the adapter did not advertise the selected erase capability",
                "operation",
                "capabilities".to_owned(),
            ));
        }

        let stable = settle(
            &mut client,
            &mut events,
            &scenario,
            call_timeout,
            &mut warnings,
            "after-erase",
        )?;
        for probe in &scenario.spec.probes.after {
            let fixture = fixture_by_id(&scenario, &probe.fixture)?;
            let profile = profile_for_probe(probe, &scenario.spec.suite);
            let expected = !fixture.is_target();
            let observed = run_probe_if_supported(
                &mut client,
                &mut events,
                &scenario,
                &run_id,
                probe,
                "after",
                call_timeout,
                &capabilities,
                &mut assertions,
            )?;
            if let Some(observed) = observed {
                assertions.push(assertion(
                    &format!("after.{}", probe.id),
                    &profile,
                    if !stable {
                        "UNKNOWN"
                    } else if expected == observed {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    format!(
                        "{} probe for fixture '{}' expected {}, observed {}",
                        probe.kind,
                        fixture.id,
                        if expected { "present" } else { "absent" },
                        if observed { "present" } else { "absent" }
                    ),
                    Some(expected),
                    Some(observed),
                    artifact_for_probe(probe),
                    format!("probe:{}", probe.id),
                ));
                if fixture.is_control()
                    && scenario
                        .spec
                        .profiles
                        .iter()
                        .any(|profile| crate::model::legacy_profile(profile) == "erasure.scope")
                {
                    assertions.push(assertion(
                        &format!("scope.{}", probe.id),
                        "erasure.scope",
                        if !stable {
                            "UNKNOWN"
                        } else if observed {
                            "PASS"
                        } else {
                            "FAIL"
                        },
                        format!(
                            "control fixture '{}' remained {} after target erase",
                            fixture.id,
                            if observed { "observable" } else { "absent" }
                        ),
                        Some(true),
                        Some(observed),
                        "scope",
                        format!("probe:{}", probe.id),
                    ));
                }
            }
        }
    } else if scenario.spec.suite == "erasure" {
        warnings
            .push("erase operation was skipped because the precondition was not proven".to_owned());
    } else {
        for probe in &scenario.spec.probes.after {
            let fixture = fixture_by_id(&scenario, &probe.fixture)?;
            let profile = profile_for_probe(probe, &scenario.spec.suite);
            let expected = scenario.probe_subject(probe)
                == fixture.effective_subject(&scenario.spec.isolation);
            let observed = run_probe_if_supported(
                &mut client,
                &mut events,
                &scenario,
                &run_id,
                probe,
                "isolation",
                call_timeout,
                &capabilities,
                &mut assertions,
            )?;
            if let Some(observed) = observed {
                assertions.push(assertion(
                    &format!("after.{}", probe.id),
                    &profile,
                    if expected == observed { "PASS" } else { "FAIL" },
                    format!(
                        "query subject '{}' expected fixture '{}' to be {}, observed {}",
                        scenario.probe_subject(probe),
                        fixture.id,
                        if expected { "isolated" } else { "inaccessible" },
                        if observed { "visible" } else { "absent" }
                    ),
                    Some(expected),
                    Some(observed),
                    "namespace",
                    format!("probe:{}", probe.id),
                ));
            }
        }
    }

    if let Err(error) = client.call("cleanup", json!({ "run_id": run_id }), call_timeout) {
        warnings.push(format!("cleanup failed: {error:#}"));
        record_event(
            &mut events,
            "cleanup",
            "cleanup",
            "ERROR",
            details([(String::from("error"), "cleanup failed".to_owned())]),
        );
    } else {
        record_event(&mut events, "cleanup", "cleanup", "PASS", BTreeMap::new());
    }
    client.close();

    let profiles = summarize_profiles(&scenario.spec.profiles, &assertions);
    let status = if precondition_failed && assertions.iter().any(|item| item.status == "ERROR") {
        "ERROR".to_owned()
    } else {
        overall_status(&profiles, &assertions)
    };
    let exit_code = match status.as_str() {
        "PASS" => 0,
        "FAIL" => 1,
        "UNKNOWN" | "SKIP" => 3,
        _ => 2,
    };
    let result = RunResult {
        run_id: run_id.clone(),
        suite: scenario.spec.suite.clone(),
        scenario: scenario.metadata.name.clone(),
        adapter: scenario.spec.adapter.name.clone(),
        backend: if capabilities.backend.is_empty() {
            capabilities.adapter.clone()
        } else {
            capabilities.backend.clone()
        },
        backend_version: capabilities.version.clone(),
        protocol: capabilities.protocol.clone(),
        scenario_hash: scenario_hash.clone(),
        status,
        exit_code,
        profiles,
        assertions,
        capabilities: capabilities.capabilities.clone(),
        out_of_scope: vec![
            "provider logs and backups".to_owned(),
            "physical storage erasure".to_owned(),
            "model-weight unlearning".to_owned(),
            "unobservable artifacts outside the adapter boundary".to_owned(),
        ],
        warnings,
    };

    let manifest = Manifest {
        format: BUNDLE_FORMAT.to_owned(),
        run_id: result.run_id.clone(),
        created_at_ms: now_ms(),
        scenario_hash,
        adapter: result.adapter.clone(),
        backend: result.backend.clone(),
        protocol: result.protocol.clone(),
        files: vec![
            "manifest.json".to_owned(),
            "scenario.lock.json".to_owned(),
            "events.ndjson".to_owned(),
            "results.json".to_owned(),
            "report.html".to_owned(),
            "junit.xml".to_owned(),
        ],
        bundle_hash: String::new(),
    };
    let bundle_hash = write_bundle(&directory, &scenario_value, &events, &result, &manifest)?;
    Ok(RunOutcome {
        result,
        directory,
        bundle_hash,
    })
}

pub fn doctor_adapter(
    name: &str,
    mode: &str,
    config: &BTreeMap<String, String>,
) -> Result<Capabilities> {
    let mut client = AdapterClient::spawn(name, mode, config)?;
    client.hello(3_000)?;
    let capabilities = client.capabilities(3_000)?;
    client.close();
    Ok(capabilities)
}

pub fn init_project(root: &Path) -> Result<()> {
    fs::create_dir_all(root.join(".memoryproof/runs"))?;
    fs::create_dir_all(root.join("scenarios"))?;
    let config = r#"# MemoryProof local configuration
# Adapter commands are registered by the CLI; credentials stay in environment variables.
[project]
protocol = "memoryproof.adapter/v1"

[security]
allow_network = false
"#;
    fs::write(root.join("memoryproof.toml"), config)?;
    let gitignore = root.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, "/.memoryproof/\n")?;
    }
    fs::write(
        root.join("scenarios/reference-clean.yml"),
        include_str!("../../../examples/reference-clean.yml"),
    )?;
    fs::write(
        root.join("scenarios/reference-leaky.yml"),
        include_str!("../../../examples/reference-leaky.yml"),
    )?;
    Ok(())
}

pub fn expand_scenario(input: &Path, output: &Path) -> Result<()> {
    let (mut scenario, _) = load_scenario(input)?;
    if std::env::var("MEMORYPROOF_LLM_BASE_URL").is_ok()
        || std::env::var("FORGETPROOF_LLM_BASE_URL").is_ok()
        || std::env::var("OPENAI_BASE_URL").is_ok()
    {
        scenario = expand_with_compatible_llm(&scenario)?;
    }
    let existing = scenario
        .spec
        .probes
        .after
        .iter()
        .map(|probe| probe.id.clone())
        .collect::<BTreeSet<_>>();
    let mut additions = Vec::new();
    for probe in &scenario.spec.probes.after {
        if probe.kind == "exact" {
            for kind in ["lexical", "semantic"] {
                let id = format!("{}.{}", probe.id, kind);
                if !existing.contains(&id) {
                    let fixture_content = scenario
                        .spec
                        .fixtures
                        .iter()
                        .find(|fixture| fixture.id == probe.fixture)
                        .map(|fixture| fixture.content.as_str())
                        .unwrap_or_default();
                    let query = if kind == "lexical" {
                        fixture_content
                            .split_whitespace()
                            .take(4)
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        format!("What do you remember about {fixture_content}?")
                    };
                    additions.push(Probe {
                        id,
                        fixture: probe.fixture.clone(),
                        kind: kind.to_owned(),
                        query,
                        as_subject: probe.as_subject.clone(),
                    });
                }
            }
        }
    }
    scenario.spec.probes.after.extend(additions);
    scenario.kind = "AssuranceScenarioLock".to_owned();
    let yaml = serde_yaml::to_string(&scenario)?;
    fs::write(
        output,
        format!("# Generated by memoryproof expand. Probes are frozen before execution.\n{yaml}"),
    )?;
    Ok(())
}

fn expand_with_compatible_llm(scenario: &Scenario) -> Result<Scenario> {
    let python = std::env::var("MEMORYPROOF_PYTHON")
        .or_else(|_| std::env::var("FORGETPROOF_PYTHON"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "python".to_owned()
            } else {
                "python3".to_owned()
            }
        });
    let mut command = Command::new(python);
    command
        .args(["-m", "forgetproof_adapters.expand"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Ok(current_dir) = std::env::current_dir() {
        let package_root = current_dir.join("python");
        if package_root.join("forgetproof_adapters").is_dir() {
            let mut paths = vec![package_root];
            if let Some(existing) = std::env::var_os("PYTHONPATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            command.env("PYTHONPATH", std::env::join_paths(paths)?);
        }
    }
    let mut child = command
        .spawn()
        .context("failed to start LLM expansion helper")?;
    let payload = serde_json::to_vec(scenario)?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("LLM expansion stdin unavailable"))?
        .write_all(&payload)?;
    drop(child.stdin.take());
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("LLM expansion helper failed with {}", output.status);
    }
    serde_json::from_slice(&output.stdout).context("LLM expansion returned invalid scenario")
}

fn enforce_network_policy(scenario: &Scenario, allow_network: bool) -> Result<()> {
    if scenario.spec.adapter.name.starts_with("reference-") {
        return Ok(());
    }
    if allow_network
        || std::env::var("MEMORYPROOF_ALLOW_NETWORK").as_deref() == Ok("1")
        || std::env::var("FORGETPROOF_ALLOW_NETWORK").as_deref() == Ok("1")
    {
        return Ok(());
    }
    let base_url = scenario
        .spec
        .adapter
        .config
        .get("base_url")
        .cloned()
        .or_else(|| {
            let env_name = match scenario.spec.adapter.name.as_str() {
                "mem0" => "MEM0_BASE_URL",
                "letta" => "LETTA_BASE_URL",
                "zep" => "ZEP_BASE_URL",
                _ => "",
            };
            (!env_name.is_empty())
                .then(|| std::env::var(env_name).ok())
                .flatten()
        })
        .unwrap_or_default();
    let local = base_url.starts_with("http://127.0.0.1")
        || base_url.starts_with("http://localhost")
        || base_url.starts_with("http://[::1]");
    if !local {
        bail!(
            "network access is disabled; pass --allow-network for '{}' or configure a loopback base_url",
            scenario.spec.adapter.name
        );
    }
    Ok(())
}

fn fixture_payload(scenario: &Scenario, fixture: &Fixture) -> Value {
    json!({
        "id": fixture.id,
        "content": fixture.content,
        "role": if fixture.is_target() { "target" } else { "control" },
        "target": fixture.is_target(),
        "subject": fixture.effective_subject(&scenario.spec.isolation),
        "namespace": if fixture.namespace.is_empty() { scenario.spec.isolation.scope.clone() } else { fixture.namespace.clone() },
        "kind": fixture.kind,
    })
}

fn target_subject(scenario: &Scenario) -> String {
    scenario
        .spec
        .fixtures
        .iter()
        .find(|fixture| fixture.id == scenario.spec.erase.target)
        .map(|fixture| fixture.effective_subject(&scenario.spec.isolation))
        .unwrap_or_else(|| scenario.spec.isolation.target_subject.clone())
}

fn call_checked(
    client: &mut AdapterClient,
    events: &mut Vec<Event>,
    method: &str,
    params: Value,
    timeout: u64,
) -> Result<Value> {
    let value = client.call(method, params, timeout)?;
    record_event(events, "run", method, "PASS", BTreeMap::new());
    Ok(value)
}

fn settle(
    client: &mut AdapterClient,
    events: &mut Vec<Event>,
    scenario: &Scenario,
    timeout: u64,
    warnings: &mut Vec<String>,
    phase: &str,
) -> Result<bool> {
    let value = client.call(
        "settle",
        json!({
            "timeout_ms": scenario.spec.settle.timeout_ms,
            "interval_ms": scenario.spec.settle.interval_ms,
        }),
        timeout,
    )?;
    let state = value
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if value.get("stable").and_then(Value::as_bool) == Some(true) {
                "stable"
            } else {
                "unknown"
            }
        });
    let stable = state == "stable";
    record_event(
        events,
        phase,
        "settle",
        if stable { "PASS" } else { "UNKNOWN" },
        details([(String::from("state"), state.to_owned())]),
    );
    if !stable {
        warnings.push(format!(
            "backend did not report stable state during {phase}: {state}"
        ));
    }
    Ok(stable)
}

#[allow(clippy::too_many_arguments)]
fn run_probe_if_supported(
    client: &mut AdapterClient,
    events: &mut Vec<Event>,
    scenario: &Scenario,
    run_id: &str,
    probe: &Probe,
    phase: &str,
    timeout: u64,
    capabilities: &Capabilities,
    assertions: &mut Vec<Assertion>,
) -> Result<Option<bool>> {
    let required = probe_capability(probe);
    if !capability_available(capabilities, required) {
        assertions.push(unknown_assertion(
            &format!("{phase}.{}.capability", probe.id),
            &profile_for_probe(probe, &scenario.spec.suite),
            &format!("adapter does not advertise capability '{required}'"),
            artifact_for_probe(probe),
            "capabilities".to_owned(),
        ));
        record_event(
            events,
            phase,
            method_for_probe(probe),
            "UNKNOWN",
            details([
                (String::from("probe"), probe.id.clone()),
                (String::from("missing_capability"), required.to_owned()),
            ]),
        );
        return Ok(None);
    }
    let method = method_for_probe(probe);
    let value = client.call(
        method,
        json!({
            "run_id": run_id,
            "phase": phase,
            "probe": {
                "id": probe.id,
                "fixture": probe.fixture,
                "kind": probe.kind,
                "query": probe.query,
                "as_subject": scenario.probe_subject(probe),
            }
        }),
        timeout,
    )?;
    let found = value
        .get("found")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("adapter probe '{}' did not return boolean found", probe.id))?;
    record_event(
        events,
        phase,
        method,
        "PASS",
        details([
            (String::from("probe"), probe.id.clone()),
            (String::from("found"), found.to_string()),
        ]),
    );
    Ok(Some(found))
}

fn probe_capability(probe: &Probe) -> &'static str {
    match probe.kind.as_str() {
        "inspect" => "derived_inspect",
        "agent" => "agent_query",
        "lexical" => "lexical_search",
        "semantic" => "semantic_search",
        _ => "probe",
    }
}

fn method_for_probe(probe: &Probe) -> &'static str {
    match probe.kind.as_str() {
        "inspect" => "inspect",
        "agent" => "agent_query",
        _ => "probe",
    }
}

fn artifact_for_probe(probe: &Probe) -> &'static str {
    match probe.kind.as_str() {
        "inspect" => "derived",
        "agent" => "agent",
        "lexical" => "lexical-index",
        "semantic" => "semantic-index",
        _ => "object",
    }
}

fn add_capability_assertions(
    scenario: &Scenario,
    capabilities: &Capabilities,
    assertions: &mut Vec<Assertion>,
) {
    for profile in &scenario.spec.profiles {
        let profile = crate::model::legacy_profile(profile);
        for capability in profile_capabilities(&profile) {
            if !capability_available(capabilities, capability) {
                assertions.push(unknown_assertion(
                    &format!("capability.{profile}.{capability}"),
                    &profile,
                    &format!("adapter does not advertise capability '{capability}'"),
                    "capability",
                    "capabilities".to_owned(),
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn assertion(
    id: &str,
    profile: &str,
    status: &str,
    message: String,
    expected: Option<bool>,
    observed: Option<bool>,
    artifact: &str,
    evidence: String,
) -> Assertion {
    Assertion {
        id: id.to_owned(),
        profile: crate::model::legacy_profile(profile),
        status: status.to_owned(),
        message,
        expected,
        observed,
        artifact: artifact.to_owned(),
        evidence: vec![evidence],
    }
}

fn unknown_assertion(
    id: &str,
    profile: &str,
    message: &str,
    artifact: &str,
    evidence: String,
) -> Assertion {
    assertion(
        id,
        profile,
        "UNKNOWN",
        message.to_owned(),
        None,
        None,
        artifact,
        evidence,
    )
}

fn summarize_profiles(profiles: &[String], assertions: &[Assertion]) -> Vec<ProfileResult> {
    profiles
        .iter()
        .map(|raw_profile| {
            let profile = crate::model::legacy_profile(raw_profile);
            let relevant = assertions
                .iter()
                .filter(|item| item.profile == profile)
                .collect::<Vec<_>>();
            let status = if relevant.iter().any(|item| item.status == "ERROR") {
                "ERROR"
            } else if relevant.iter().any(|item| item.status == "FAIL") {
                "FAIL"
            } else if relevant
                .iter()
                .any(|item| matches!(item.status.as_str(), "UNKNOWN" | "SKIP"))
                || relevant.is_empty()
            {
                "UNKNOWN"
            } else {
                "PASS"
            };
            ProfileResult {
                profile: profile.clone(),
                label: display_profile(&profile).to_owned(),
                status: status.to_owned(),
                assertions: relevant.iter().map(|item| item.id.clone()).collect(),
            }
        })
        .collect()
}

fn overall_status(profiles: &[ProfileResult], assertions: &[Assertion]) -> String {
    if assertions.iter().any(|item| item.status == "ERROR") {
        "ERROR".to_owned()
    } else if profiles.iter().any(|item| item.status == "FAIL") {
        "FAIL".to_owned()
    } else if profiles
        .iter()
        .any(|item| matches!(item.status.as_str(), "UNKNOWN" | "SKIP"))
    {
        "UNKNOWN".to_owned()
    } else {
        "PASS".to_owned()
    }
}

fn fixture_by_id<'a>(scenario: &'a Scenario, id: &str) -> Result<&'a Fixture> {
    scenario
        .spec
        .fixtures
        .iter()
        .find(|fixture| fixture.id == id)
        .ok_or_else(|| anyhow!("unknown fixture '{id}'"))
}

fn record_event(
    events: &mut Vec<Event>,
    phase: &str,
    method: &str,
    status: &str,
    event_details: BTreeMap<String, String>,
) {
    events.push(Event {
        seq: events.len() as u64 + 1,
        at_ms: now_ms(),
        phase: phase.to_owned(),
        method: method.to_owned(),
        status: status.to_owned(),
        details: event_details,
    });
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}
