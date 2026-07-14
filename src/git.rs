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
