use std::fs;
use std::path::Path;
use std::process::Command;

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").leak()
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("forgetproof-test-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn clean_backend_passes_and_bundle_verifies() {
    let root = project_root();
    let output = temp_root("clean");
    let run = Command::new(env!("CARGO_BIN_EXE_forgetproof"))
        .current_dir(root)
        .args([
            "run",
            "examples/reference-clean.yml",
            "--output",
            output.to_str().unwrap(),
            "--allow-network=false",
        ])
        .output()
        .unwrap();
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
    let verify = Command::new(env!("CARGO_BIN_EXE_forgetproof"))
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
}

#[test]
fn leaky_backend_fails_with_a_report() {
    let root = project_root();
    let output = temp_root("leaky");
    let run = Command::new(env!("CARGO_BIN_EXE_forgetproof"))
        .current_dir(root)
        .args([
            "run",
            "examples/reference-leaky.yml",
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
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
    assert!(results.contains("\"status\": \"FAIL\""));
    assert!(results.contains("target-derived-after"));
}
