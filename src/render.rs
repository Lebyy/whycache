use std::fmt::Write;

use crate::{
    cli::Cli,
    error::Result,
    model::{
        BaselineCaptured, CacheStatus, Cause, CauseKind, Classification, Report, TaskDiagnosis,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Human,
    Json,
    Markdown,
}

impl Format {
    pub fn from_cli(cli: &Cli) -> Self {
        if cli.json {
            Self::Json
        } else if cli.md {
            Self::Markdown
        } else {
            Self::Human
        }
    }
}

pub fn render(report: &Report, format: Format, colored: bool) -> Result<String> {
    match format {
        Format::Human => Ok(render_human(report, colored)),
        Format::Json => Ok(format!("{}\n", serde_json::to_string_pretty(report)?)),
        Format::Markdown => Ok(render_markdown(report)),
    }
}

pub fn baseline_captured(captured: &BaselineCaptured, format: Format) -> Result<String> {
    match format {
        Format::Json => Ok(format!("{}\n", serde_json::to_string_pretty(captured)?)),
        Format::Markdown => Ok(format!(
            "## WhyCache baseline captured\n\n{}\n\n- Baseline: `{}`\n- Tasks: {}\n- Next: `{}`\n",
            captured.message, captured.path, captured.task_count, captured.next_command
        )),
        Format::Human => Ok(format!(
            "WhyCache baseline captured\n\n{}\n\n  Baseline  {}\n  Tasks     {}\n  Next      {}\n",
            captured.message, captured.path, captured.task_count, captured.next_command
        )),
    }
}

fn render_human(report: &Report, colored: bool) -> String {
    let mut output = String::new();
    let title = style("WhyCache", "1;36", colored);
    let _ = writeln!(output, "{title} — Turborepo cache diagnosis\n");
    let _ = writeln!(output, "  Baseline  {}", report.baseline.path);
    let _ = writeln!(output, "  Current   {}", report.current.path);
    if let (Some(before), Some(after)) = (
        report.baseline.turbo_version.as_deref(),
        report.current.turbo_version.as_deref(),
    ) {
        let _ = writeln!(output, "  Turbo     {before} → {after}");
    }
    let root_causes = report
        .tasks
        .iter()
        .filter(|task| {
            matches!(
                task.classification,
                Classification::RootCause
                    | Classification::CacheUnavailable
                    | Classification::NewTask
            )
        })
        .count();
    let cascades = report
        .tasks
        .iter()
        .filter(|task| task.classification == Classification::Cascade)
        .count();
    let _ = writeln!(
        output,
        "  Result    {root_causes} root cause(s), {cascades} cascade(s), {} task(s)",
        report.tasks.len()
    );

    if report.tasks.is_empty() {
        let _ = writeln!(output, "\nNo cache misses or matching tasks were found.");
    }
    for task in &report.tasks {
        render_human_task(&mut output, task, colored);
    }
    if !report.warnings.is_empty() {
        let _ = writeln!(output, "\n{}", style("Warnings", "1;33", colored));
        for warning in &report.warnings {
            let _ = writeln!(output, "  ! {warning}");
        }
    }
    output
}

fn render_human_task(output: &mut String, task: &TaskDiagnosis, colored: bool) {
    let classification = classification_label(task.classification);
    let status = cache_status_label(task.cache_status);
    let color = match task.classification {
        Classification::Unchanged => "1;32",
        Classification::Cascade => "1;33",
        Classification::CacheUnavailable | Classification::RootCause => "1;31",
        Classification::NewTask | Classification::Unexplained => "1;35",
    };
    let _ = writeln!(
        output,
        "\n{}  {}  {}",
        style(&task.task_id, "1", colored),
        style(status, color, colored),
        style(classification, color, colored)
    );
    let _ = writeln!(
        output,
        "  Hash      {} → {}",
        short_hash(task.baseline_hash.as_deref()),
        short_hash(task.current_hash.as_deref())
    );
    for (index, cause) in task.causes.iter().enumerate() {
        let _ = writeln!(
            output,
            "\n  {}. {}  {}% confidence",
            index + 1,
            cause.summary,
            cause.confidence
        );
        for evidence in &cause.evidence {
            let change = match (&evidence.before, &evidence.after) {
                (Some(before), Some(after)) => {
                    format!("{} → {}", short_hash(Some(before)), short_hash(Some(after)))
                }
                (None, Some(after)) => format!("added ({})", short_hash(Some(after))),
                (Some(before), None) => format!("removed ({})", short_hash(Some(before))),
                (None, None) => String::new(),
            };
            let _ = writeln!(output, "     • {}  {change}", evidence.source);
            if let Some(stat) = task.git_stats.get(&evidence.source) {
                let _ = writeln!(
                    output,
                    "       Git: +{} −{} lines",
                    stat.added_lines, stat.removed_lines
                );
            }
        }
    }
    if let Some(culprit) = likely_culprit(task) {
        let _ = writeln!(output, "\n  💡 Likely culprit: {culprit}");
    }
    let _ = writeln!(output, "\n  UNCHANGED  {}", unchanged_text(&task.unchanged));
    for hint in &task.hints {
        let _ = writeln!(output, "  Hint: {hint}");
    }
}

fn render_markdown(report: &Report) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "## WhyCache report\n");
    let _ = writeln!(output, "| Summary | Source | Turbo | Commit |");
    let _ = writeln!(output, "|---|---|---|---|");
    let _ = writeln!(
        output,
        "| Baseline | `{}` | {} | {} |",
        escape_table(&report.baseline.path),
        optional(&report.baseline.turbo_version),
        optional_short(&report.baseline.commit_sha)
    );
    let _ = writeln!(
        output,
        "| Current | `{}` | {} | {} |",
        escape_table(&report.current.path),
        optional(&report.current.turbo_version),
        optional_short(&report.current.commit_sha)
    );
    for task in &report.tasks {
        let _ = writeln!(output, "\n### `{}`\n", task.task_id);
        let _ = writeln!(
            output,
            "**{} · {}** — hash `{}` → `{}`\n",
            cache_status_label(task.cache_status),
            classification_label(task.classification),
            short_hash(task.baseline_hash.as_deref()),
            short_hash(task.current_hash.as_deref())
        );
        if task.causes.is_empty() {
            let _ = writeln!(output, "No input differences were detected.\n");
        }
        for cause in &task.causes {
            render_markdown_cause(&mut output, cause, task);
        }
        if let Some(culprit) = likely_culprit(task) {
            let _ = writeln!(output, "**Likely culprit:** {culprit}\n");
        }
        let _ = writeln!(
            output,
            "**Unchanged:** {}\n",
            unchanged_text(&task.unchanged)
        );
        if !task.hints.is_empty() {
            let _ = writeln!(output, "**Next checks**\n");
            for hint in &task.hints {
                let _ = writeln!(output, "- {hint}");
            }
            let _ = writeln!(output);
        }
    }
    if !report.warnings.is_empty() {
        let _ = writeln!(output, "### Warnings\n");
        for warning in &report.warnings {
            let _ = writeln!(output, "- {warning}");
        }
    }
    let content_length = output.trim_end_matches('\n').len();
    output.truncate(content_length);
    output.push('\n');
    output
}

fn likely_culprit(task: &TaskDiagnosis) -> Option<String> {
    let cause = task.causes.first()?;
    let source = cause
        .evidence
        .first()
        .map(|evidence| evidence.source.as_str());
    Some(match (cause.kind, source) {
        (CauseKind::Environment, Some(name)) => format!("{name} changed between runs."),
        (CauseKind::InputFile | CauseKind::GlobalInput, Some(path)) => {
            format!("{path} changed between runs.")
        }
        (CauseKind::UpstreamTask, Some(task)) => {
            format!("{task} changed first and cascaded into this task.")
        }
        (CauseKind::CacheUnavailable, _) => {
            "The cache key is unchanged, but its artifact was unavailable.".to_owned()
        }
        (CauseKind::TurboVersion, _) => "The Turborepo version changed.".to_owned(),
        (CauseKind::NewTask, _) => "The task did not exist in the baseline run.".to_owned(),
        (_, Some(source)) => format!("{}: {source}.", cause.summary),
        (_, None) => format!("{}.", cause.summary),
    })
}

fn unchanged_text(unchanged: &crate::model::UnchangedSummary) -> String {
    let mut parts = vec![
        format!("{} file(s)", unchanged.files),
        format!(
            "{} environment variable(s)",
            unchanged.environment_variables
        ),
    ];
    if unchanged.lockfile == Some(true) {
        parts.push("lockfile".to_owned());
    }
    if unchanged.turbo_json == Some(true) {
        parts.push("turbo.json".to_owned());
    }
    if unchanged.task_configuration {
        parts.push("task configuration".to_owned());
    }
    parts.join(", ")
}

fn render_markdown_cause(output: &mut String, cause: &Cause, task: &TaskDiagnosis) {
    let _ = writeln!(
        output,
        "#### {} · {}% confidence\n",
        cause.summary, cause.confidence
    );
    let _ = writeln!(output, "| Evidence | Before | After | Git |");
    let _ = writeln!(output, "|---|---:|---:|---:|");
    for evidence in &cause.evidence {
        let git = task.git_stats.get(&evidence.source).map_or_else(
            || "—".to_owned(),
            |stat| format!("+{} / −{}", stat.added_lines, stat.removed_lines),
        );
        let _ = writeln!(
            output,
            "| `{}` | `{}` | `{}` | {} |",
            escape_table(&evidence.source),
            short_hash(evidence.before.as_deref()),
            short_hash(evidence.after.as_deref()),
            git
        );
    }
    let _ = writeln!(output);
}

fn classification_label(classification: Classification) -> &'static str {
    match classification {
        Classification::RootCause => "root cause",
        Classification::Cascade => "cascade",
        Classification::CacheUnavailable => "cache unavailable",
        Classification::Unchanged => "unchanged",
        Classification::NewTask => "new task",
        Classification::Unexplained => "unexplained",
    }
}

fn cache_status_label(status: CacheStatus) -> &'static str {
    match status {
        CacheStatus::Hit => "HIT",
        CacheStatus::Miss => "MISS",
        CacheStatus::Skipped => "SKIPPED",
        CacheStatus::Unknown => "UNKNOWN",
    }
}

fn short_hash(value: Option<&str>) -> &str {
    value.map_or("—", |value| value.get(..12).unwrap_or(value))
}

fn optional(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("—")
}

fn optional_short(value: &Option<String>) -> &str {
    short_hash(value.as_deref())
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|")
}

fn style(value: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\u{1b}[{code}m{value}\u{1b}[0m")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Classification, Report, SummaryMetadata, TaskDiagnosis, UnchangedSummary};
    use std::collections::BTreeMap;

    #[test]
    fn json_output_is_stable_and_versioned() {
        let report = Report {
            schema_version: "1",
            baseline: metadata("before"),
            current: metadata("after"),
            tasks: vec![],
            warnings: vec![],
        };
        let output = render(&report, Format::Json, false).unwrap();
        assert!(output.contains("\"schemaVersion\": \"1\""));
    }

    #[test]
    fn human_output_has_no_ansi_when_disabled() {
        let report = Report {
            schema_version: "1",
            baseline: metadata("before"),
            current: metadata("after"),
            tasks: vec![TaskDiagnosis {
                task_id: "app#build".to_owned(),
                package: "app".to_owned(),
                task: "build".to_owned(),
                cache_status: CacheStatus::Miss,
                baseline_hash: Some("aaa".to_owned()),
                current_hash: Some("bbb".to_owned()),
                classification: Classification::Unexplained,
                causes: vec![],
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
        assert!(
            !render(&report, Format::Human, false)
                .unwrap()
                .contains('\u{1b}')
        );
    }

    fn metadata(path: &str) -> SummaryMetadata {
        SummaryMetadata {
            path: path.to_owned(),
            id: None,
            schema_version: Some("1".to_owned()),
            turbo_version: None,
            commit_sha: None,
        }
    }
}
