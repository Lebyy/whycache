use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use crate::{
    cli::Cli,
    error::{Error, Result},
    model::{BaselineCaptured, SummaryPair, SummarySource, portable_path},
    parser::{parse_bytes, parse_file},
};

const OWN_BASELINE: &str = ".whycache/last-summary.json";

pub struct Discovery {
    pub root: PathBuf,
}

pub enum LoadResult {
    Ready(Box<SummaryPair>),
    BaselineCaptured(BaselineCaptured),
}

impl Discovery {
    pub fn new(start: Option<&Path>) -> Result<Self> {
        let start = match start {
            Some(path) => path.to_owned(),
            None => env::current_dir()?,
        };
        let root = find_repo_root(&start).ok_or_else(|| Error::RepoNotFound(start.clone()))?;
        Ok(Self { root })
    }

    pub fn load(&self, cli: &Cli) -> Result<LoadResult> {
        let mut warnings = Vec::new();
        let summaries = summary_files(&self.root)?;
        let own_baseline = self.root.join(OWN_BASELINE);

        if let Some(against) = &cli.against {
            let baseline = if against == "-" {
                read_stdin_summary()?
            } else {
                source_from_path(resolve(&self.root, against))?
            };
            let current = newest_source(&summaries)?;
            if same_file(&baseline.path, &current.path) {
                return Err(Error::SameSummary(current.path));
            }
            collect_schema_warning(&baseline, &mut warnings);
            collect_schema_warning(&current, &mut warnings);
            return Ok(LoadResult::Ready(Box::new(SummaryPair {
                baseline,
                current,
                warnings,
            })));
        }

        if summaries.len() >= 2 {
            let current = source_from_path(summaries.last().unwrap().clone())?;
            let baseline_path = summaries[..summaries.len() - 1]
                .iter()
                .rev()
                .find_map(|path| {
                    parse_file(path)
                        .ok()
                        .filter(|summary| summary.successful())
                        .map(|_| path.clone())
                })
                .unwrap_or_else(|| summaries[summaries.len() - 2].clone());
            let baseline = source_from_path(baseline_path)?;
            collect_schema_warning(&baseline, &mut warnings);
            collect_schema_warning(&current, &mut warnings);
            return Ok(LoadResult::Ready(Box::new(SummaryPair {
                baseline,
                current,
                warnings,
            })));
        }

        let tasks = match cli.task.as_deref() {
            Some(task) => vec![task.to_owned()],
            None => configured_tasks(&self.root)?,
        };
        if !tasks.is_empty() {
            let dry = run_dry_summary(&self.root, &tasks)?;
            let current = SummarySource {
                path: PathBuf::from("<turbo --dry=json>"),
                summary: parse_bytes(&dry, "turbo --dry=json")?,
            };

            let baseline = if let Some(path) = summaries.last() {
                source_from_path(path.clone())?
            } else if own_baseline.is_file() {
                source_from_path(own_baseline.clone())?
            } else {
                save_baseline(&own_baseline, &dry)?;
                let next_command = cli
                    .task
                    .as_deref()
                    .map_or_else(|| "whycache".to_owned(), |task| format!("whycache {task}"));
                return Ok(LoadResult::BaselineCaptured(BaselineCaptured {
                    schema_version: "1",
                    status: "baseline_captured",
                    path: relative_display(&self.root, &own_baseline),
                    task_count: current.summary.tasks.len(),
                    message: "No historical summary existed, so WhyCache captured the current inputs. A past cache miss cannot be reconstructed without a baseline.".to_owned(),
                    next_command,
                }));
            };

            save_baseline(&own_baseline, &dry)?;
            warnings.push(
                "Compared a fresh `turbo --dry=json` snapshot with the latest saved baseline."
                    .to_owned(),
            );
            collect_schema_warning(&baseline, &mut warnings);
            collect_schema_warning(&current, &mut warnings);
            return Ok(LoadResult::Ready(Box::new(SummaryPair {
                baseline,
                current,
                warnings,
            })));
        }

        Err(Error::NoSummaries(self.root.join(".turbo/runs")))
    }
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let start = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    start.ancestors().find_map(|candidate| {
        (candidate.join("turbo.json").is_file()
            || candidate.join(".turbo/runs").is_dir()
            || package_mentions_turbo(&candidate.join("package.json")))
        .then(|| candidate.to_owned())
    })
}

fn package_mentions_turbo(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|contents| contents.contains("turbo"))
        .unwrap_or(false)
}

fn summary_files(root: &Path) -> Result<Vec<PathBuf>> {
    let directory = root.join(".turbo/runs");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = fs::read_dir(&directory)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let modified = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (modified, path)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    Ok(entries.into_iter().map(|(_, path)| path).collect())
}

fn newest_source(summaries: &[PathBuf]) -> Result<SummarySource> {
    summaries
        .last()
        .cloned()
        .ok_or_else(|| Error::NoSummaries(PathBuf::from(".turbo/runs")))
        .and_then(source_from_path)
}

fn source_from_path(path: PathBuf) -> Result<SummarySource> {
    let summary = parse_file(&path)?;
    Ok(SummarySource { path, summary })
}

fn same_file(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn read_stdin_summary() -> Result<SummarySource> {
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    Ok(SummarySource {
        path: PathBuf::from("<stdin>"),
        summary: parse_bytes(&bytes, "stdin")?,
    })
}

fn configured_tasks(root: &Path) -> Result<Vec<String>> {
    let path = root.join("turbo.json");
    let contents = fs::read(&path).map_err(|source| Error::Read {
        path: path.clone(),
        source,
    })?;
    let config: serde_json::Value =
        serde_json::from_slice(&contents).map_err(|source| Error::Config {
            path: path.clone(),
            source,
        })?;
    let tasks = config
        .get("tasks")
        .or_else(|| config.get("pipeline"))
        .and_then(serde_json::Value::as_object)
        .map(|tasks| tasks.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if tasks.is_empty() {
        return Err(Error::NoConfiguredTasks(path));
    }
    Ok(tasks)
}

fn run_dry_summary(root: &Path, tasks: &[String]) -> Result<Vec<u8>> {
    let (program, prefix) = turbo_program(root);
    let command = format!("{} run <tasks> --dry=json", program.display());
    let output = Command::new(&program)
        .current_dir(root)
        .args(prefix)
        .arg("run")
        .args(tasks)
        .arg("--dry=json")
        .output()
        .map_err(|source| Error::Spawn {
            command: command.to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(Error::Command {
            command: command.to_owned(),
            status: output.status.code().map_or_else(
                || "terminated by signal".to_owned(),
                |code| code.to_string(),
            ),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

fn turbo_program(root: &Path) -> (PathBuf, Vec<PathBuf>) {
    if let Some(configured) = env::var_os("TURBO_BINARY_PATH") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return (path, Vec::new());
        }
    }

    if let Some(binary) = local_turbo_candidates(root, env::consts::OS, env::consts::ARCH)
        .into_iter()
        .find(|path| path.is_file())
    {
        return (binary, Vec::new());
    }

    let launcher = root.join("node_modules/turbo/bin/turbo");
    if launcher.is_file() {
        return (PathBuf::from("node"), vec![launcher]);
    }

    (PathBuf::from("turbo"), Vec::new())
}

fn local_turbo_candidates(root: &Path, os: &str, arch: &str) -> Vec<PathBuf> {
    let platform = match os {
        "macos" => "darwin",
        "windows" => "windows",
        other => other,
    };
    let architecture = if arch == "x86_64" { "64" } else { arch };
    let executable = if os == "windows" {
        "turbo.exe"
    } else {
        "turbo"
    };
    let scoped = format!("{platform}-{architecture}");
    let legacy = format!("turbo-{platform}-{architecture}");
    [
        root.join("node_modules/@turbo")
            .join(&scoped)
            .join("bin")
            .join(executable),
        root.join("node_modules/turbo/node_modules/@turbo")
            .join(&scoped)
            .join("bin")
            .join(executable),
        root.join("node_modules")
            .join(&legacy)
            .join("bin")
            .join(executable),
        root.join("node_modules/turbo/node_modules")
            .join(&legacy)
            .join("bin")
            .join(executable),
    ]
    .into_iter()
    .collect()
}

fn save_baseline(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::SaveBaseline {
            path: path.to_owned(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| Error::SaveBaseline {
        path: path.to_owned(),
        source,
    })
}

fn resolve(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

fn collect_schema_warning(source: &SummarySource, warnings: &mut Vec<String>) {
    if source
        .summary
        .version
        .as_deref()
        .is_some_and(|version| !matches!(version, "0" | "1"))
    {
        warnings.push(format!(
            "{} uses run-summary schema {}; parsed in compatibility mode.",
            source.path.display(),
            source.summary.version.as_deref().unwrap_or("unknown")
        ));
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    portable_path(path.strip_prefix(root).unwrap_or(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_root_from_nested_directory() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("turbo.json"), "{}").unwrap();
        let nested = temp.path().join("packages/app");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_repo_root(&nested).unwrap(), temp.path());
    }

    #[test]
    fn sorts_summaries_deterministically() {
        let temp = tempfile::tempdir().unwrap();
        let runs = temp.path().join(".turbo/runs");
        fs::create_dir_all(&runs).unwrap();
        fs::write(runs.join("b.json"), "{}").unwrap();
        fs::write(runs.join("a.json"), "{}").unwrap();
        assert_eq!(summary_files(temp.path()).unwrap().len(), 2);
    }

    #[test]
    fn resolves_windows_native_turbo_without_a_shell() {
        let root = Path::new("C:/repo");
        let candidates = local_turbo_candidates(root, "windows", "x86_64");
        assert_eq!(
            candidates[0],
            root.join("node_modules")
                .join("@turbo")
                .join("windows-64")
                .join("bin")
                .join("turbo.exe")
        );
    }

    #[test]
    fn discovers_current_and_legacy_task_configuration() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("turbo.json"),
            r#"{"tasks":{"test":{},"build":{}}}"#,
        )
        .unwrap();
        assert_eq!(configured_tasks(temp.path()).unwrap(), ["build", "test"]);

        fs::write(
            temp.path().join("turbo.json"),
            r#"{"pipeline":{"lint":{}}}"#,
        )
        .unwrap();
        assert_eq!(configured_tasks(temp.path()).unwrap(), ["lint"]);
    }

    #[test]
    fn warns_only_for_unknown_summary_schemas() {
        let summary = parse_bytes(
            br#"{"version":"0","tasks":[{"taskId":"app#build"}]}"#,
            "schema-zero",
        )
        .unwrap();
        let mut source = SummarySource {
            path: PathBuf::from("summary.json"),
            summary,
        };
        let mut warnings = Vec::new();
        collect_schema_warning(&source, &mut warnings);
        assert!(warnings.is_empty());

        source.summary.version = Some("99".to_owned());
        collect_schema_warning(&source, &mut warnings);
        assert_eq!(warnings.len(), 1);
    }
}
