use std::{collections::BTreeSet, path::Path, process::Command};

use crate::model::{GitStat, Report};

pub fn enrich_with_git(root: &Path, report: &mut Report) {
    let (Some(before), Some(after)) = (
        report.baseline.commit_sha.as_deref(),
        report.current.commit_sha.as_deref(),
    ) else {
        report.warnings.push(
            "`--git` was requested, but both summaries do not contain commit SHAs.".to_owned(),
        );
        return;
    };

    for task in &mut report.tasks {
        let paths = task
            .causes
            .iter()
            .flat_map(|cause| &cause.evidence)
            .map(|evidence| evidence.source.as_str())
            .filter(|source| looks_like_path(source))
            .collect::<BTreeSet<_>>();
        if paths.is_empty() {
            continue;
        }

        let range = format!("{before}..{after}");
        let output = Command::new("git")
            .current_dir(root)
            .arg("diff")
            .arg("--numstat")
            .arg(range)
            .arg("--")
            .args(&paths)
            .output();
        let Ok(output) = output else {
            report
                .warnings
                .push("Could not start Git to collect diff statistics.".to_owned());
            return;
        };
        if !output.status.success() {
            report.warnings.push(format!(
                "Git diff statistics were unavailable: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            return;
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let mut fields = line.splitn(3, '\t');
            let (Some(added), Some(removed), Some(path)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let (Ok(added_lines), Ok(removed_lines)) = (added.parse(), removed.parse()) else {
                continue;
            };
            task.git_stats.insert(
                path.to_owned(),
                GitStat {
                    added_lines,
                    removed_lines,
                },
            );
        }
    }
}

fn looks_like_path(source: &str) -> bool {
    source.contains('/') || source.contains('.')
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use crate::model::{
        CacheStatus, Cause, CauseKind, Classification, Evidence, SummaryMetadata, TaskDiagnosis,
        UnchangedSummary,
    };

    use super::*;

    #[test]
    fn adds_numstat_for_changed_evidence_paths() {
        let temp = tempfile::tempdir().unwrap();
        git(temp.path(), &["init", "--quiet"]);
        git(temp.path(), &["config", "user.name", "WhyCache Test"]);
        git(
            temp.path(),
            &["config", "user.email", "whycache@example.invalid"],
        );
        fs::write(temp.path().join("input.txt"), "before\n").unwrap();
        git(temp.path(), &["add", "input.txt"]);
        git(temp.path(), &["commit", "--quiet", "-m", "before"]);
        let before = git(temp.path(), &["rev-parse", "HEAD"]);

        fs::write(temp.path().join("input.txt"), "after\nsecond line\n").unwrap();
        git(temp.path(), &["add", "input.txt"]);
        git(temp.path(), &["commit", "--quiet", "-m", "after"]);
        let after = git(temp.path(), &["rev-parse", "HEAD"]);

        let mut report = Report {
            schema_version: "1",
            baseline: metadata("before", before),
            current: metadata("after", after),
            tasks: vec![TaskDiagnosis {
                task_id: "app#build".to_owned(),
                package: "app".to_owned(),
                task: "build".to_owned(),
                cache_status: CacheStatus::Miss,
                baseline_hash: Some("a".to_owned()),
                current_hash: Some("b".to_owned()),
                classification: Classification::RootCause,
                causes: vec![Cause {
                    kind: CauseKind::InputFile,
                    summary: "input changed".to_owned(),
                    confidence: 95,
                    evidence: vec![Evidence {
                        source: "input.txt".to_owned(),
                        before: Some("a".to_owned()),
                        after: Some("b".to_owned()),
                        detail: None,
                    }],
                }],
                unchanged: UnchangedSummary {
                    files: 0,
                    environment_variables: 0,
                    lockfile: None,
                    turbo_json: None,
                    task_configuration: true,
                },
                hints: vec![],
                git_stats: BTreeMap::new(),
            }],
            warnings: vec![],
        };
        enrich_with_git(temp.path(), &mut report);
        let stat = &report.tasks[0].git_stats["input.txt"];
        assert_eq!(stat.added_lines, 2);
        assert_eq!(stat.removed_lines, 1);
    }

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn metadata(path: &str, commit_sha: String) -> SummaryMetadata {
        SummaryMetadata {
            path: path.to_owned(),
            id: None,
            schema_version: Some("1".to_owned()),
            turbo_version: None,
            commit_sha: Some(commit_sha),
        }
    }
}
