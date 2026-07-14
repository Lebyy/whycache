use std::path::PathBuf;

use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(
    name = "whycache",
    version,
    about = "Explain why a Turborepo task missed the cache",
    after_help = "Examples:\n  whycache build\n  whycache test --against .turbo/runs/previous.json\n  whycache build --json\n  whycache lint --md > $GITHUB_STEP_SUMMARY"
)]
pub struct Cli {
    /// Limit the report to a task name or task id.
    pub task: Option<String>,

    /// Compare the newest run with this summary. Use - to read it from stdin.
    #[arg(long, value_name = "SUMMARY")]
    pub against: Option<String>,

    /// Emit stable machine-readable JSON.
    #[arg(long, conflicts_with = "md")]
    pub json: bool,

    /// Emit GitHub-flavored Markdown.
    #[arg(long, conflicts_with = "json")]
    pub md: bool,

    /// Add Git diff statistics when both summaries contain commit SHAs.
    #[arg(long)]
    pub git: bool,

    /// Repository to inspect. Defaults to the current directory.
    #[arg(long, value_name = "PATH", hide = true)]
    pub repo: Option<PathBuf>,

    /// Disable ANSI styling in human output.
    #[arg(long, action = ArgAction::SetTrue, hide = true)]
    pub no_color: bool,
}
