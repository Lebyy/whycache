use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub turbo_version: Option<String>,
    #[serde(default)]
    pub global_cache_inputs: GlobalCacheInputs,
    #[serde(default)]
    pub execution: Option<Execution>,
    #[serde(default)]
    pub tasks: Vec<TaskSummary>,
    #[serde(default)]
    pub scm: Option<serde_json::Value>,
}

impl RunSummary {
    pub fn successful(&self) -> bool {
        self.execution
            .as_ref()
            .and_then(|execution| execution.exit_code)
            .map_or_else(
                || {
                    self.tasks.iter().all(|task| {
                        task.execution
                            .as_ref()
                            .and_then(|execution| execution.exit_code)
                            .is_none_or(|code| code == 0)
                    })
                },
                |code| code == 0,
            )
    }

    pub fn commit_sha(&self) -> Option<&str> {
        let scm = self.scm.as_ref()?;
        ["sha", "commitSha", "commit"]
            .into_iter()
            .find_map(|key| scm.get(key).and_then(serde_json::Value::as_str))
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalCacheInputs {
    #[serde(default)]
    pub root_key: Option<String>,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub hash_of_external_dependencies: Option<String>,
    #[serde(default)]
    pub hash_of_internal_dependencies: Option<String>,
    #[serde(default)]
    pub environment_variables: EnvironmentVariables,
    #[serde(default)]
    pub engines: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub hash_reason: Option<String>,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub hash_of_external_dependencies: Option<String>,
    #[serde(default)]
    pub cache: CacheSummary,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub excluded_outputs: Vec<String>,
    #[serde(default)]
    pub log_file: Option<String>,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub dependents: Vec<String>,
    #[serde(default)]
    pub resolved_task_definition: Option<serde_json::Value>,
    #[serde(default)]
    pub environment_variables: EnvironmentVariables,
    #[serde(default)]
    pub execution: Option<Execution>,
}

impl TaskSummary {
    pub fn identity(&self) -> String {
        if !self.task_id.is_empty() {
            self.task_id.clone()
        } else if !self.package.is_empty() && !self.task.is_empty() {
            format!("{}#{}", self.package, self.task)
        } else {
            self.task.clone()
        }
    }

    pub fn matches(&self, filter: &str) -> bool {
        self.identity() == filter
            || self.task == filter
            || self.identity().ends_with(&format!("#{filter}"))
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSummary {
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub remote: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub time_saved: Option<u64>,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub dirty_hash: Option<String>,
}

impl CacheSummary {
    pub fn status(&self) -> CacheStatus {
        match self
            .status
            .as_deref()
            .map(str::to_ascii_uppercase)
            .as_deref()
        {
            Some("HIT") => CacheStatus::Hit,
            Some("MISS") => CacheStatus::Miss,
            Some("SKIP") | Some("SKIPPED") | Some("DISABLED") => CacheStatus::Skipped,
            _ if self.local || self.remote => CacheStatus::Hit,
            _ => CacheStatus::Unknown,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde(default)]
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariables {
    #[serde(default)]
    pub specified: SpecifiedEnvironmentVariables,
    #[serde(default)]
    pub configured: Vec<String>,
    #[serde(default)]
    pub inferred: Vec<String>,
    #[serde(default)]
    pub passthrough: Option<Vec<String>>,
}

impl EnvironmentVariables {
    pub fn fingerprints(&self) -> BTreeMap<String, Option<String>> {
        let mut result = BTreeMap::new();
        for entry in self.configured.iter().chain(&self.inferred) {
            let (name, hash) = split_env_fingerprint(entry);
            result.insert(name, hash);
        }
        for name in self
            .specified
            .env
            .iter()
            .chain(self.specified.pass_through_env.iter().flatten())
            .chain(self.passthrough.iter().flatten())
        {
            result.entry(name.clone()).or_insert(None);
        }
        result
    }
}

fn split_env_fingerprint(entry: &str) -> (String, Option<String>) {
    match entry.split_once('=') {
        Some((name, hash)) => (name.to_owned(), Some(hash.to_owned())),
        None => (entry.to_owned(), None),
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecifiedEnvironmentVariables {
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub pass_through_env: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStatus {
    Hit,
    Miss,
    Skipped,
    Unknown,
}

#[derive(Debug)]
pub struct SummarySource {
    pub path: PathBuf,
    pub summary: RunSummary,
}

#[derive(Debug)]
pub struct SummaryPair {
    pub baseline: SummarySource,
    pub current: SummarySource,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: &'static str,
    pub baseline: SummaryMetadata,
    pub current: SummaryMetadata,
    pub tasks: Vec<TaskDiagnosis>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryMetadata {
    pub path: String,
    pub id: Option<String>,
    pub schema_version: Option<String>,
    pub turbo_version: Option<String>,
    pub commit_sha: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDiagnosis {
    pub task_id: String,
    pub package: String,
    pub task: String,
    pub cache_status: CacheStatus,
    pub baseline_hash: Option<String>,
    pub current_hash: Option<String>,
    pub classification: Classification,
    pub causes: Vec<Cause>,
    pub hints: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub git_stats: BTreeMap<String, GitStat>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    RootCause,
    Cascade,
    CacheUnavailable,
    Unchanged,
    NewTask,
    Unexplained,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cause {
    pub kind: CauseKind,
    pub summary: String,
    pub confidence: u8,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseKind {
    Environment,
    InputFile,
    DependencyGraph,
    GlobalInput,
    TaskConfiguration,
    UpstreamTask,
    TurboVersion,
    CacheUnavailable,
    NewTask,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStat {
    pub added_lines: u64,
    pub removed_lines: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineCaptured {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub path: String,
    pub task_count: usize,
    pub message: String,
    pub next_command: String,
}

pub fn changed_keys<V: PartialEq>(
    before: &BTreeMap<String, V>,
    after: &BTreeMap<String, V>,
) -> BTreeSet<String> {
    before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}
