use crate::evidence::{details, hash_text, write_bundle};
use crate::model::{
    Assertion, Capabilities, Event, Manifest, Probe, ProfileResult, RunResult, Scenario,
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
    let scenario: Scenario =
        serde_json::from_value(value.clone()).context("invalid scenario schema")?;
    scenario
        .validate()
        .map_err(|errors| anyhow!(errors.join("; ")))?;
    Ok((scenario, value))
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
    let scenario_hash = hash_text(&serde_json::to_string(&scenario_value)?);
    let timeout = scenario.spec.settle.timeout_ms.max(1_000);
    let mut events = Vec::new();
    let mut client = AdapterClient::spawn(
        &scenario.spec.adapter.name,
        &scenario.spec.adapter.mode,
        &scenario.spec.adapter.config,
    )?;

    client.hello(timeout)?;
    record_event(
        &mut events,
        "prepare",
        "hello",
        "PASS",
        details([(
            String::from("protocol"),
            crate::model::PROTOCOL_VERSION.to_owned(),
        )]),
    );
    let capabilities = client.capabilities(timeout)?;
    record_event(
        &mut events,
        "prepare",
        "capabilities",
        "PASS",
        details([
            (String::from("adapter"), capabilities.adapter.clone()),
            (String::from("backend"), capabilities.backend.clone()),
        ]),
    );

    let mut assertions = Vec::new();
    let mut warnings = Vec::new();
    add_capability_assertions(&scenario, &capabilities, &mut assertions);

    let fixture_payloads = scenario
        .spec
        .fixtures
        .iter()
        .map(|fixture| {
            json!({
                "id": fixture.id,
                "content": fixture.content,
                "target": fixture.target,
                "kind": fixture.kind,
            })
        })
        .collect::<Vec<_>>();
    call_checked(
        &mut client,
        &mut events,
        "prepare",
        json!({
            "run_id": run_id,
            "isolation": scenario.spec.isolation,
            "fixtures": fixture_payloads,
        }),
        timeout,
    )?;

    for fixture in &scenario.spec.fixtures {
        call_checked(
            &mut client,
            &mut events,
            "ingest",
            json!({
                "run_id": run_id,
                "fixture": {
                    "id": fixture.id,
                    "content": fixture.content,
                    "target": fixture.target,
                    "kind": fixture.kind,
                }
            }),
            timeout,
        )?;
    }
    settle(&mut client, &mut events, &scenario, timeout)?;

    for probe in &scenario.spec.probes.before {
        let fixture = fixture_by_id(&scenario, &probe.fixture)?;
        let observed = perform_probe(&mut client, &mut events, &run_id, probe, "before", timeout)?;
        if !observed {
            bail!(
                "precondition failed: fixture '{}' was not observable before erase",
                fixture.id
            );
        }
        assertions.push(Assertion {
            id: format!("before.{}", probe.id),
            profile: "PRECONDITION".to_owned(),
            status: "PASS".to_owned(),
            message: format!("fixture '{}' was observable before erase", fixture.id),
            expected: Some(true),
            observed: Some(observed),
            evidence: vec![format!("probe:{}", probe.id)],
        });
    }

    let erase_performed =
        has_capability(&capabilities, erase_capability(&scenario.spec.erase.intent));
    if erase_performed {
        call_checked(
            &mut client,
            &mut events,
            "erase",
            json!({
                "run_id": run_id,
                "intent": scenario.spec.erase.intent,
                "target": scenario.spec.erase.target,
            }),
            timeout,
        )?;
    } else {
        warnings.push(format!(
            "erase intent '{}' is not supported by adapter",
            scenario.spec.erase.intent
        ));
    }
    settle(&mut client, &mut events, &scenario, timeout)?;

    for probe in &scenario.spec.probes.after {
        let fixture = fixture_by_id(&scenario, &probe.fixture)?;
        let observed = perform_probe(&mut client, &mut events, &run_id, probe, "after", timeout)?;
        let expected = fixture.id != scenario.spec.erase.target;
        let profile = profile_for_probe(probe);
        assertions.push(if erase_performed {
            assertion_for_probe(
                &format!("after.{}", probe.id),
                &profile,
                expected,
                observed,
                fixture.id.as_str(),
                probe,
            )
        } else {
            unknown_assertion_for_probe(
                &format!("after.{}", probe.id),
                &profile,
                "erase operation was not available; post-erase observation is inconclusive",
                probe,
            )
        });
        if profile == "FP-Object"
            && selected(&scenario, "FP-Scope")
            && fixture.id != scenario.spec.erase.target
        {
            assertions.push(if erase_performed {
                assertion_for_probe(
                    &format!("scope.{}", probe.id),
                    "FP-Scope",
                    true,
                    observed,
                    fixture.id.as_str(),
                    probe,
                )
            } else {
                unknown_assertion_for_probe(
                    &format!("scope.{}", probe.id),
                    "FP-Scope",
                    "erase operation was not available; scope observation is inconclusive",
                    probe,
                )
            });
        }
    }

    if selected(&scenario, "FP-Scope")
        && !scenario.spec.probes.after.iter().any(|probe| {
            profile_for_probe(probe) == "FP-Object" && probe.fixture != scenario.spec.erase.target
        })
    {
        let control = scenario
            .spec
            .fixtures
            .iter()
            .find(|fixture| !fixture.target)
            .ok_or_else(|| anyhow!("FP-Scope requires a non-target control fixture"))?;
        let probe = Probe {
            id: "scope.control-preserved".to_owned(),
            fixture: control.id.clone(),
            kind: "exact".to_owned(),
            query: String::new(),
        };
        let observed = perform_probe(&mut client, &mut events, &run_id, &probe, "after", timeout)?;
        assertions.push(if erase_performed {
            assertion_for_probe(
                "scope.control-preserved",
                "FP-Scope",
                true,
                observed,
                &control.id,
                &probe,
            )
        } else {
            unknown_assertion_for_probe(
                "scope.control-preserved",
                "FP-Scope",
                "erase operation was not available; scope observation is inconclusive",
                &probe,
            )
        });
    }

    if selected(&scenario, "FP-Derived")
        && !scenario
            .spec
            .probes
            .after
            .iter()
            .any(|probe| profile_for_probe(probe) == "FP-Derived")
        && has_capability(&capabilities, "derived_inspect")
    {
        let target = &scenario.spec.erase.target;
        let probe = Probe {
            id: "derived.target-absent".to_owned(),
            fixture: target.clone(),
            kind: "inspect".to_owned(),
            query: String::new(),
        };
        let observed = perform_probe(&mut client, &mut events, &run_id, &probe, "after", timeout)?;
        assertions.push(if erase_performed {
            assertion_for_probe(
                "derived.target-absent",
                "FP-Derived",
                false,
                observed,
                target,
                &probe,
            )
        } else {
            unknown_assertion_for_probe(
                "derived.target-absent",
                "FP-Derived",
                "erase operation was not available; derived observation is inconclusive",
                &probe,
            )
        });
    }

    if selected(&scenario, "FP-Agent")
        && !scenario
            .spec
            .probes
            .after
            .iter()
            .any(|probe| profile_for_probe(probe) == "FP-Agent")
        && has_capability(&capabilities, "agent_query")
    {
        let target = &scenario.spec.erase.target;
        let probe = Probe {
            id: "agent.target-absent".to_owned(),
            fixture: target.clone(),
            kind: "agent".to_owned(),
            query: String::new(),
        };
        let observed = perform_probe(&mut client, &mut events, &run_id, &probe, "after", timeout)?;
        assertions.push(if erase_performed {
            assertion_for_probe(
                "agent.target-absent",
                "FP-Agent",
                false,
                observed,
                target,
                &probe,
            )
        } else {
            unknown_assertion_for_probe(
                "agent.target-absent",
                "FP-Agent",
                "erase operation was not available; Agent observation is inconclusive",
                &probe,
            )
        });
    }

    let profiles = summarize_profiles(&scenario.spec.profiles, &assertions);
    let status = overall_status(&profiles, &assertions);
    let exit_code = match status.as_str() {
        "PASS" => 0,
        "FAIL" => 1,
        "INCOMPLETE" => 3,
        _ => 2,
    };
    let result = RunResult {
        run_id: run_id.clone(),
        scenario: scenario.metadata.name.clone(),
        adapter: scenario.spec.adapter.name.clone(),
        backend: if capabilities.backend.is_empty() {
            capabilities.adapter.clone()
        } else {
            capabilities.backend.clone()
        },
        scenario_hash: scenario_hash.clone(),
        status,
        exit_code,
        profiles,
        assertions,
        warnings,
    };

    let _ = client.call("cleanup", json!({ "run_id": run_id }), timeout);
    client.close();

    let manifest = Manifest {
        format: "forgetproof.bundle/v1alpha1".to_owned(),
        run_id: result.run_id.clone(),
        created_at_ms: now_ms(),
        scenario_hash,
        adapter: result.adapter.clone(),
        backend: result.backend.clone(),
        protocol: crate::model::PROTOCOL_VERSION.to_owned(),
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
    fs::create_dir_all(root.join(".forgetproof/runs"))?;
    fs::create_dir_all(root.join("scenarios"))?;
    let config = r#"# ForgetProof local configuration
# Adapter commands are built in; credentials stay in environment variables.
[project]
protocol = "forgetproof.adapter/v1alpha1"

[security]
allow_network = false
"#;
    fs::write(root.join("forgetproof.toml"), config)?;
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
    if std::env::var("FORGETPROOF_LLM_BASE_URL").is_ok() || std::env::var("OPENAI_BASE_URL").is_ok()
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
                    });
                }
            }
        }
    }
    scenario.spec.probes.after.extend(additions);
    scenario.kind = "ErasureScenarioLock".to_owned();
    let yaml = serde_yaml::to_string(&scenario)?;
    fs::write(
        output,
        format!("# Generated by forgetproof expand. Probes are frozen before execution.\n{yaml}"),
    )?;
    Ok(())
}

fn expand_with_compatible_llm(scenario: &Scenario) -> Result<Scenario> {
    let python = std::env::var("FORGETPROOF_PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let mut command = Command::new(python);
    command
        .args(["-m", "forgetproof_adapters.expand"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Ok(current_dir) = std::env::current_dir() {
        let package_root = current_dir.join("python");
        if package_root.join("forgetproof_adapters").is_dir() {
            let existing = std::env::var("PYTHONPATH").unwrap_or_default();
            let value = if existing.is_empty() {
                package_root.display().to_string()
            } else {
                format!("{}:{}", package_root.display(), existing)
            };
            command.env("PYTHONPATH", value);
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
    if matches!(
        scenario.spec.adapter.name.as_str(),
        "reference-clean" | "reference-leaky"
    ) {
        return Ok(());
    }
    if allow_network || std::env::var("FORGETPROOF_ALLOW_NETWORK").as_deref() == Ok("1") {
        return Ok(());
    }
    let local = scenario
        .spec
        .adapter
        .config
        .get("base_url")
        .map(|url| {
            url.starts_with("http://127.0.0.1")
                || url.starts_with("http://localhost")
                || url.starts_with("http://[::1]")
        })
        .unwrap_or(false);
    if !local {
        bail!(
            "network access is disabled; pass --allow-network for '{}' or configure a loopback base_url",
            scenario.spec.adapter.name
        );
    }
    Ok(())
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
) -> Result<()> {
    let value = call_checked(
        client,
        events,
        "settle",
        json!({
            "timeout_ms": scenario.spec.settle.timeout_ms,
            "interval_ms": scenario.spec.settle.interval_ms,
        }),
        timeout,
    )?;
    if value.get("stable").and_then(Value::as_bool) == Some(false) {
        bail!("adapter did not reach a stable state before timeout")
    }
    Ok(())
}

fn perform_probe(
    client: &mut AdapterClient,
    events: &mut Vec<Event>,
    run_id: &str,
    probe: &Probe,
    phase: &str,
    timeout: u64,
) -> Result<bool> {
    let method = match probe.kind.as_str() {
        "inspect" => "inspect",
        "agent" => "agent_query",
        _ => "probe",
    };
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
    Ok(found)
}

fn add_capability_assertions(
    scenario: &Scenario,
    capabilities: &Capabilities,
    assertions: &mut Vec<Assertion>,
) {
    for profile in &scenario.spec.profiles {
        for capability in profile_capabilities(profile) {
            if !has_capability(capabilities, capability) {
                assertions.push(Assertion {
                    id: format!("capability.{profile}.{capability}"),
                    profile: profile.clone(),
                    status: "UNKNOWN".to_owned(),
                    message: format!("adapter does not advertise capability '{capability}'"),
                    expected: None,
                    observed: None,
                    evidence: vec!["capabilities".to_owned()],
                });
            }
        }
    }
}

fn profile_capabilities(profile: &str) -> &'static [&'static str] {
    match profile {
        "FP-Object" => &["object_delete", "probe"],
        "FP-Scope" => &["scope_delete", "probe"],
        "FP-Derived" => &["derived_inspect", "inspect"],
        "FP-Agent" => &["agent_query"],
        _ => &[],
    }
}

fn erase_capability(intent: &str) -> &str {
    match intent {
        "subject_erase" => "scope_delete",
        "derived_purge" => "derived_delete",
        "access_revoke" => "access_revoke",
        _ => "object_delete",
    }
}

fn has_capability(capabilities: &Capabilities, capability: &str) -> bool {
    capabilities
        .capabilities
        .iter()
        .any(|item| item == capability)
}

fn selected(scenario: &Scenario, profile: &str) -> bool {
    scenario.spec.profiles.iter().any(|item| item == profile)
}

fn profile_for_probe(probe: &Probe) -> String {
    match probe.kind.as_str() {
        "inspect" => "FP-Derived".to_owned(),
        "agent" => "FP-Agent".to_owned(),
        _ => "FP-Object".to_owned(),
    }
}

fn assertion_for_probe(
    id: &str,
    profile: &str,
    expected: bool,
    observed: bool,
    fixture: &str,
    probe: &Probe,
) -> Assertion {
    let status = if expected == observed { "PASS" } else { "FAIL" };
    let expectation = if expected { "present" } else { "absent" };
    let actual = if observed { "present" } else { "absent" };
    Assertion {
        id: id.to_owned(),
        profile: profile.to_owned(),
        status: status.to_owned(),
        message: format!(
            "{} probe for fixture '{}' expected {}, observed {}",
            probe.kind, fixture, expectation, actual
        ),
        expected: Some(expected),
        observed: Some(observed),
        evidence: vec![format!("probe:{}", probe.id)],
    }
}

fn unknown_assertion_for_probe(id: &str, profile: &str, message: &str, probe: &Probe) -> Assertion {
    Assertion {
        id: id.to_owned(),
        profile: profile.to_owned(),
        status: "UNKNOWN".to_owned(),
        message: message.to_owned(),
        expected: None,
        observed: None,
        evidence: vec![format!("probe:{}", probe.id)],
    }
}

fn summarize_profiles(profiles: &[String], assertions: &[Assertion]) -> Vec<ProfileResult> {
    profiles
        .iter()
        .map(|profile| {
            let relevant = assertions
                .iter()
                .filter(|assertion| assertion.profile == *profile)
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
                "INCOMPLETE"
            } else {
                "PASS"
            };
            ProfileResult {
                profile: profile.clone(),
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
    } else if profiles.iter().any(|item| item.status == "INCOMPLETE") {
        "INCOMPLETE".to_owned()
    } else {
        "PASS".to_owned()
    }
}

fn fixture_by_id<'a>(scenario: &'a Scenario, id: &str) -> Result<&'a crate::model::Fixture> {
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
    details: BTreeMap<String, String>,
) {
    events.push(Event {
        seq: events.len() as u64 + 1,
        at_ms: now_ms(),
        phase: phase.to_owned(),
        method: method.to_owned(),
        status: status.to_owned(),
        details,
    });
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}
