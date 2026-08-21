use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").leak()
}

fn temp_root(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("memoryproof-test-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run(binary: &str, scenario: &str, output: &Path) -> std::process::Output {
    Command::new(binary)
        .current_dir(project_root())
        .args([
            "run",
            scenario,
            "--output",
            output.to_str().unwrap(),
            "--allow-network=false",
        ])
        .output()
        .unwrap()
}

fn scenario_with_adapter(root: &Path, template: &str, adapter: &str) -> PathBuf {
    let source = fs::read_to_string(project_root().join(template)).unwrap();
    let source = source.replace("name: reference-clean", &format!("name: {adapter}"));
    let path = root.join(format!("{adapter}.yml"));
    fs::write(&path, source).unwrap();
    path
}

#[test]
fn clean_backend_passes_and_bundle_verifies() {
    let root = project_root();
    let output = temp_root("clean");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        "examples/reference-clean.yml",
        &output,
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bundle = fs::read_dir(&output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let verify = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(root)
        .args(["verify", bundle.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        verify.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&verify.stdout)
    );
    let scenario = fs::read_to_string(bundle.join("scenario.lock.json")).unwrap();
    assert!(!scenario.contains("target alpha 7f3e9d"));
    assert!(scenario.contains("sha256:"));
    let scenario_yaml = fs::read_to_string(bundle.join("scenario.lock.yml")).unwrap();
    assert!(!scenario_yaml.contains("target alpha 7f3e9d"));
    assert!(scenario_yaml.contains("apiVersion:"));
    let manifest = fs::read_to_string(bundle.join("manifest.json")).unwrap();
    assert!(manifest.contains("memoryproof.bundle/v1"));
    assert!(manifest.contains("scenario.lock.yml"));
}

#[test]
fn leaky_backend_fails_at_observable_derived_boundary() {
    let output = temp_root("leaky");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        "examples/reference-leaky.yml",
        &output,
    );
    assert_eq!(
        run.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let bundle = fs::read_dir(&output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let results = fs::read_to_string(bundle.join("results.json")).unwrap();
    assert!(results.contains(r#""status": "FAIL""#));
    assert!(results.contains("target-derived-after"));
}

#[test]
fn overdelete_backend_fails_scope_without_hiding_the_control_regression() {
    let output = temp_root("overdelete");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        "examples/reference-overdelete.yml",
        &output,
    );
    assert_eq!(
        run.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let bundle = fs::read_dir(&output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let results = fs::read_to_string(bundle.join("results.json")).unwrap();
    assert!(results.contains(r#""profile": "erasure.scope""#));
    assert!(results.contains("control fixture 'control' remained absent"));
}

#[test]
fn isolation_backend_passes_cross_subject_checks() {
    let output = temp_root("isolation");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        "examples/isolation-reference.yml",
        &output,
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
}

#[test]
fn tampering_is_detected_and_report_refreshes_checksums() {
    let output = temp_root("tamper");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        "examples/reference-clean.yml",
        &output,
    );
    assert_eq!(run.status.code(), Some(0));
    let bundle = fs::read_dir(&output)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(bundle.join("report.html"), "tampered").unwrap();
    let verify = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .args(["verify", bundle.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(verify.status.code(), Some(1));
    let report = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .args(["report", bundle.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(report.status.code(), Some(0));
    let verify_again = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .args(["verify", bundle.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(verify_again.status.code(), Some(0));
}

#[test]
fn protocol_contract_failures_are_standardized_and_stderr_is_ignored() {
    let root = temp_root("protocol-contract");
    let output_root = root.join("bundles");
    fs::create_dir_all(&output_root).unwrap();

    let stderr_noise = scenario_with_adapter(
        &root,
        "examples/reference-clean.yml",
        "reference-stderr-noise",
    );
    let stderr_run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        stderr_noise.to_str().unwrap(),
        &output_root,
    );
    assert_eq!(
        stderr_run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&stderr_run.stderr)
    );

    for (adapter, expected_message) in [
        ("reference-malformed", "invalid JSON"),
        ("reference-wrong-id", "response id mismatch"),
        ("reference-wrong-version", "protocol mismatch"),
        ("reference-crash", "exited while handling"),
    ] {
        let scenario = scenario_with_adapter(&root, "examples/reference-clean.yml", adapter);
        let output = run(
            env!("CARGO_BIN_EXE_memoryproof"),
            scenario.to_str().unwrap(),
            &output_root,
        );
        assert_eq!(output.status.code(), Some(2), "adapter={adapter}");
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostics.contains(expected_message),
            "adapter={adapter} diagnostics={diagnostics}"
        );
    }
}

#[test]
fn asynchronous_timeout_is_unknown_and_not_a_pass() {
    let root = temp_root("slow");
    let output_root = root.join("bundles");
    fs::create_dir_all(&output_root).unwrap();
    let scenario = scenario_with_adapter(&root, "examples/reference-clean.yml", "reference-slow");
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        scenario.to_str().unwrap(),
        &output_root,
    );
    assert_eq!(
        run.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bundle = fs::read_dir(&output_root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let results = fs::read_to_string(bundle.join("results.json")).unwrap();
    assert!(results.contains(r#""status": "UNKNOWN""#));
    assert!(results.contains("did not report stable state"));
}

#[test]
fn remote_endpoint_requires_explicit_network_permission() {
    let root = temp_root("network-policy");
    let output_root = root.join("bundles");
    fs::create_dir_all(&output_root).unwrap();
    let mut source = fs::read_to_string(project_root().join("examples/mem0.yml")).unwrap();
    source = source.replace(
        "base_url: http://localhost:8888",
        "base_url: http://localhost:8888\n      endpoint_search: https://example.com/search",
    );
    let scenario = root.join("remote-endpoint.yml");
    fs::write(&scenario, source).unwrap();
    let run = run(
        env!("CARGO_BIN_EXE_memoryproof"),
        scenario.to_str().unwrap(),
        &output_root,
    );
    assert_eq!(run.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&run.stderr).contains("endpoint_search"));
}

#[test]
fn network_environment_variables_cannot_bypass_cli_permission() {
    let root = temp_root("network-env-bypass");
    let output_root = root.join("bundles");
    fs::create_dir_all(&output_root).unwrap();
    let mut source = fs::read_to_string(project_root().join("examples/mem0.yml")).unwrap();
    source = source.replace(
        "base_url: http://localhost:8888",
        "base_url: http://localhost:8888\n      endpoint_search: https://example.com/search",
    );
    let scenario = root.join("remote-endpoint.yml");
    fs::write(&scenario, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .env("MEMORYPROOF_ALLOW_NETWORK", "1")
        .env("FORGETPROOF_ALLOW_NETWORK", "1")
        .args([
            "run",
            scenario.to_str().unwrap(),
            "--output",
            output_root.to_str().unwrap(),
            "--allow-network=false",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("endpoint_search"));
}

#[test]
fn doctor_remote_endpoint_requires_explicit_network_permission() {
    let output = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .args([
            "doctor",
            "--adapter",
            "mem0",
            "--config",
            "base_url=https://example.com",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("base_url"));
}

#[test]
fn expand_llm_endpoint_requires_explicit_network_permission() {
    let output_path = temp_root("expand-network").join("scenario.lock.yml");
    let output = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(project_root())
        .env_remove("MEMORYPROOF_LLM_BASE_URL")
        .env_remove("FORGETPROOF_LLM_BASE_URL")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("MEMORYPROOF_ALLOW_NETWORK")
        .env_remove("FORGETPROOF_ALLOW_NETWORK")
        .env("MEMORYPROOF_LLM_BASE_URL", "https://example.com")
        .args([
            "expand",
            "examples/reference-clean.yml",
            "-o",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("LLM base URL"));
}

#[test]
fn adapters_can_be_loaded_from_explicit_runtime_root() {
    let output_root = temp_root("adapter-root");
    let scenario = project_root().join("examples/reference-clean.yml");
    let python_root = project_root().join("python");
    let output = Command::new(env!("CARGO_BIN_EXE_memoryproof"))
        .current_dir(&output_root)
        .env("MEMORYPROOF_ADAPTER_ROOT", python_root)
        .env_remove("PYTHONPATH")
        .args([
            "run",
            scenario.to_str().unwrap(),
            "--output",
            output_root.join("bundles").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
