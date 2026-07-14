use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not find a Turborepo root from {0}")]
    RepoNotFound(PathBuf),

    #[error(
        "no run summaries found in {0}; run `turbo run <task> --summarize` first or pass a task so WhyCache can capture a baseline"
    )]
    NoSummaries(PathBuf),

    #[error("{0} does not declare any tasks; pass a task name explicitly")]
    NoConfiguredTasks(PathBuf),

    #[error("task `{0}` was not present in the current run summary")]
    TaskNotFound(String),

    #[error("baseline and current summary resolve to the same file: {0}")]
    SameSummary(PathBuf),

    #[error("failed to parse task names from {path}: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("run summary {0} does not contain any tasks")]
    EmptySummary(String),

    #[error("failed to run `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("`{command}` exited with status {status}: {stderr}")]
    Command {
        command: String,
        status: String,
        stderr: String,
    },

    #[error("failed to save the WhyCache baseline at {path}: {source}")]
    SaveBaseline {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoSummaries(_)
            | Self::NoConfiguredTasks(_)
            | Self::TaskNotFound(_)
            | Self::SameSummary(_)
            | Self::RepoNotFound(_) => 2,
            _ => 1,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
