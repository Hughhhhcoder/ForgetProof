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
    let manifest = fs::read_to_string(bundle.join("manifest.json")).unwrap();
    assert!(manifest.contains("memoryproof.bundle/v1"));
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
