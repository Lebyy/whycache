use std::{path::PathBuf, process::Command};

fn fixture_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repo")
}

#[test]
fn json_report_identifies_environment_and_file_causes() {
    let output = Command::new(env!("CARGO_BIN_EXE_whycache"))
        .args([
            "build",
            "--json",
            "--repo",
            fixture_repo().to_str().unwrap(),
            "--against",
            ".turbo/runs/a-baseline.json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schemaVersion"], "1");
    let web = report["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["taskId"] == "web#build")
        .unwrap();
    assert_eq!(web["classification"], "root_cause");
    assert_eq!(web["causes"][0]["kind"], "environment");
    assert!(
        web["causes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cause| cause["kind"] == "input_file")
    );
}

#[test]
fn markdown_report_is_ready_for_step_summary() {
    let output = Command::new(env!("CARGO_BIN_EXE_whycache"))
        .args([
            "build",
            "--md",
            "--repo",
            fixture_repo().to_str().unwrap(),
            "--against",
            ".turbo/runs/a-baseline.json",
        ])
        .output()
        .unwrap();
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.starts_with("## WhyCache report"));
    assert!(markdown.contains("95% confidence"));
    assert!(!markdown.contains("production-secret-value"));
}
