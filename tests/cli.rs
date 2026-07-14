use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

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

#[test]
fn all_renderers_match_golden_reports() {
    let cases = [
        (&["--no-color"][..], include_str!("golden/report.txt")),
        (&["--md"][..], include_str!("golden/report.md")),
        (&["--json"][..], include_str!("golden/report.json")),
    ];
    for (format, expected) in cases {
        let mut args = vec![
            "build",
            "--repo",
            "tests/fixtures/repo",
            "--against",
            ".turbo/runs/a-baseline.json",
        ];
        args.extend(format);
        let output = Command::new(env!("CARGO_BIN_EXE_whycache"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn invalid_task_and_self_comparison_fail_clearly() {
    let missing = Command::new(env!("CARGO_BIN_EXE_whycache"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "missing-task",
            "--repo",
            "tests/fixtures/repo",
            "--against",
            ".turbo/runs/a-baseline.json",
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("was not present"));

    let same = Command::new(env!("CARGO_BIN_EXE_whycache"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--repo",
            "tests/fixtures/repo",
            "--against",
            ".turbo/runs/z-current.json",
        ])
        .output()
        .unwrap();
    assert_eq!(same.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&same.stderr).contains("same file"));
}

#[test]
fn reads_external_baseline_from_stdin() {
    let baseline = fs::read(fixture_repo().join(".turbo/runs/a-baseline.json")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_whycache"))
        .args([
            "web#build",
            "--json",
            "--repo",
            fixture_repo().to_str().unwrap(),
            "--against",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&baseline).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["baseline"]["path"], "<stdin>");
    assert_eq!(report["tasks"][0]["taskId"], "web#build");
}
