use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    Cause, CauseKind, Classification, Evidence, Report, RunSummary, SummaryMetadata, SummaryPair,
    TaskDiagnosis, TaskSummary, UnchangedSummary, changed_keys, portable_path,
};

pub fn analyze(pair: SummaryPair, task_filter: Option<&str>) -> Report {
    let baseline_tasks = task_map(&pair.baseline.summary);
    let current_tasks = task_map(&pair.current.summary);
    let mut tasks = current_tasks
        .values()
        .filter(|task| {
            task_filter.is_none_or(|filter| task.matches(filter))
                && (task_filter.is_some()
                    || matches!(
                        task.cache.status(),
                        crate::model::CacheStatus::Miss | crate::model::CacheStatus::Unknown
                    ))
        })
        .map(|current| {
            diagnose_task(
                baseline_tasks.get(&current.identity()).copied(),
                current,
                &pair.baseline.summary,
                &pair.current.summary,
                &baseline_tasks,
                &current_tasks,
            )
        })
        .collect::<Vec<_>>();
    let topological_order = topological_order(&current_tasks);
    tasks.sort_by(|left, right| {
        classification_rank(left.classification)
            .cmp(&classification_rank(right.classification))
            .then_with(|| {
                topological_order
                    .get(&left.task_id)
                    .cmp(&topological_order.get(&right.task_id))
            })
            .then_with(|| left.task_id.cmp(&right.task_id))
    });

    Report {
        schema_version: "1",
        baseline: metadata(&pair.baseline),
        current: metadata(&pair.current),
        tasks,
        warnings: pair.warnings,
    }
}

fn diagnose_task(
    baseline: Option<&TaskSummary>,
    current: &TaskSummary,
    baseline_run: &RunSummary,
    current_run: &RunSummary,
    baseline_tasks: &BTreeMap<String, &TaskSummary>,
    current_tasks: &BTreeMap<String, &TaskSummary>,
) -> TaskDiagnosis {
    let task_id = current.identity();
    let Some(baseline) = baseline else {
        return TaskDiagnosis {
            task_id: task_id.clone(),
            package: current.package.clone(),
            task: current.task.clone(),
            cache_status: current.cache.status(),
            baseline_hash: None,
            current_hash: current.hash.clone(),
            classification: Classification::NewTask,
            causes: vec![Cause {
                kind: CauseKind::NewTask,
                summary: "Task is not present in the baseline run".to_owned(),
                confidence: 100,
                evidence: vec![Evidence {
                    source: task_id,
                    before: None,
                    after: current.hash.clone(),
                    detail: Some("No baseline task with the same id was found.".to_owned()),
                }],
            }],
            unchanged: unchanged_summary(None, current, baseline_run, current_run),
            hints: vec![
                "Verify that the same package and task graph were selected in both runs."
                    .to_owned(),
            ],
            git_stats: BTreeMap::new(),
        };
    };

    let mut causes = Vec::new();
    add_environment_causes(&mut causes, baseline, current, baseline_run, current_run);
    add_input_causes(&mut causes, baseline, current);
    add_global_causes(&mut causes, baseline_run, current_run);
    add_configuration_causes(&mut causes, baseline, current);
    add_upstream_causes(
        &mut causes,
        baseline,
        current,
        baseline_tasks,
        current_tasks,
    );
    add_version_cause(&mut causes, baseline_run, current_run);

    let same_hash = baseline.hash.is_some() && baseline.hash == current.hash;
    let is_miss = matches!(current.cache.status(), crate::model::CacheStatus::Miss);
    if same_hash && is_miss {
        causes.insert(
            0,
            Cause {
                kind: CauseKind::CacheUnavailable,
                summary: "The task hash is unchanged, but the cached artifact was unavailable"
                    .to_owned(),
                confidence: 99,
                evidence: vec![Evidence {
                    source: "task hash".to_owned(),
                    before: baseline.hash.clone(),
                    after: current.hash.clone(),
                    detail: Some(cache_detail(current)),
                }],
            },
        );
    }

    causes.sort_by(|left, right| {
        right
            .confidence
            .cmp(&left.confidence)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.summary.cmp(&right.summary))
    });

    let has_direct = causes.iter().any(|cause| {
        !matches!(
            cause.kind,
            CauseKind::UpstreamTask | CauseKind::CacheUnavailable
        )
    });
    let has_upstream = causes
        .iter()
        .any(|cause| cause.kind == CauseKind::UpstreamTask);
    let classification = if same_hash && is_miss {
        Classification::CacheUnavailable
    } else if baseline.hash == current.hash && causes.is_empty() {
        Classification::Unchanged
    } else if !has_direct && has_upstream {
        Classification::Cascade
    } else if !causes.is_empty() {
        Classification::RootCause
    } else {
        Classification::Unexplained
    };

    let unchanged = unchanged_summary(Some(baseline), current, baseline_run, current_run);
    let hints = build_hints(current, classification, &causes);
    TaskDiagnosis {
        task_id,
        package: current.package.clone(),
        task: current.task.clone(),
        cache_status: current.cache.status(),
        baseline_hash: baseline.hash.clone(),
        current_hash: current.hash.clone(),
        classification,
        causes,
        unchanged,
        hints,
        git_stats: BTreeMap::new(),
    }
}

fn add_environment_causes(
    causes: &mut Vec<Cause>,
    baseline: &TaskSummary,
    current: &TaskSummary,
    baseline_run: &RunSummary,
    current_run: &RunSummary,
) {
    let before = baseline.environment_variables.fingerprints();
    let after = current.environment_variables.fingerprints();
    let mut changed = changed_keys(&before, &after);
    let global_before = baseline_run
        .global_cache_inputs
        .environment_variables
        .fingerprints();
    let global_after = current_run
        .global_cache_inputs
        .environment_variables
        .fingerprints();
    changed.extend(changed_keys(&global_before, &global_after));
    if changed.is_empty() {
        return;
    }
    let evidence = changed
        .iter()
        .map(|name| Evidence {
            source: name.clone(),
            before: before
                .get(name)
                .or_else(|| global_before.get(name))
                .cloned()
                .flatten(),
            after: after
                .get(name)
                .or_else(|| global_after.get(name))
                .cloned()
                .flatten(),
            detail: Some("Only the variable name and Turborepo fingerprint are shown; values are never recorded by WhyCache.".to_owned()),
        })
        .collect::<Vec<_>>();
    causes.push(Cause {
        kind: CauseKind::Environment,
        summary: format!("{} environment fingerprint(s) changed", evidence.len()),
        confidence: 98,
        evidence,
    });
}

fn add_input_causes(causes: &mut Vec<Cause>, baseline: &TaskSummary, current: &TaskSummary) {
    let changed = changed_keys(&baseline.inputs, &current.inputs);
    if changed.is_empty() {
        return;
    }
    let lockfiles = changed
        .iter()
        .filter(|path| is_lockfile(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !lockfiles.is_empty() {
        causes.push(file_cause(
            CauseKind::DependencyGraph,
            "A dependency lockfile changed",
            96,
            &lockfiles,
            &baseline.inputs,
            &current.inputs,
        ));
    }
    let config = changed
        .iter()
        .filter(|path| is_task_config(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !config.is_empty() {
        causes.push(file_cause(
            CauseKind::TaskConfiguration,
            "Task or workspace configuration changed",
            94,
            &config,
            &baseline.inputs,
            &current.inputs,
        ));
    }
    let ordinary = changed
        .difference(&lockfiles)
        .filter(|path| !config.contains(*path))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !ordinary.is_empty() {
        let confidence = if ordinary.len() == 1 { 95 } else { 91 };
        causes.push(file_cause(
            CauseKind::InputFile,
            &format!("{} task input file(s) changed", ordinary.len()),
            confidence,
            &ordinary,
            &baseline.inputs,
            &current.inputs,
        ));
    }
}

fn add_global_causes(causes: &mut Vec<Cause>, baseline: &RunSummary, current: &RunSummary) {
    let before = &baseline.global_cache_inputs;
    let after = &current.global_cache_inputs;
    let changed_files = changed_keys(&before.files, &after.files);
    if !changed_files.is_empty() {
        causes.push(file_cause(
            CauseKind::GlobalInput,
            &format!("{} global dependency file(s) changed", changed_files.len()),
            93,
            &changed_files,
            &before.files,
            &after.files,
        ));
    }
    if before.root_key != after.root_key
        || before.hash_of_external_dependencies != after.hash_of_external_dependencies
        || before.hash_of_internal_dependencies != after.hash_of_internal_dependencies
        || before.engines != after.engines
    {
        causes.push(Cause {
            kind: CauseKind::DependencyGraph,
            summary: "The global dependency graph or runtime engines changed".to_owned(),
            confidence: 92,
            evidence: vec![Evidence {
                source: "globalCacheInputs".to_owned(),
                before: before
                    .hash_of_external_dependencies
                    .clone()
                    .or_else(|| before.root_key.clone()),
                after: after
                    .hash_of_external_dependencies
                    .clone()
                    .or_else(|| after.root_key.clone()),
                detail: engines_detail(&before.engines, &after.engines),
            }],
        });
    }
    if before.root_pipeline != after.root_pipeline {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The repository-wide Turborepo pipeline changed".to_owned(),
            confidence: 95,
            evidence: vec![Evidence {
                source: "globalCacheInputs.rootPipeline".to_owned(),
                before: None,
                after: None,
                detail: Some("The raw pipeline is omitted because arbitrary configuration can contain sensitive values.".to_owned()),
            }],
        });
    }
}

fn add_configuration_causes(
    causes: &mut Vec<Cause>,
    baseline: &TaskSummary,
    current: &TaskSummary,
) {
    if baseline.resolved_task_definition != current.resolved_task_definition {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The resolved Turborepo task definition changed".to_owned(),
            confidence: 95,
            evidence: vec![Evidence {
                source: "resolvedTaskDefinition".to_owned(),
                before: None,
                after: None,
                detail: Some("The raw task definition is omitted because arbitrary configuration can contain sensitive values.".to_owned()),
            }],
        });
    }
    if baseline.command != current.command {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The task command changed".to_owned(),
            confidence: 93,
            evidence: vec![Evidence {
                source: "command".to_owned(),
                before: baseline.command.as_deref().map(redact_command),
                after: current.command.as_deref().map(redact_command),
                detail: None,
            }],
        });
    }
    if baseline.hash_of_external_dependencies != current.hash_of_external_dependencies {
        causes.push(Cause {
            kind: CauseKind::DependencyGraph,
            summary: "The task's external dependency fingerprint changed".to_owned(),
            confidence: 94,
            evidence: vec![Evidence {
                source: "hashOfExternalDependencies".to_owned(),
                before: baseline.hash_of_external_dependencies.clone(),
                after: current.hash_of_external_dependencies.clone(),
                detail: None,
            }],
        });
    }
    if baseline.directory != current.directory {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The task package directory changed".to_owned(),
            confidence: 89,
            evidence: vec![Evidence {
                source: "directory".to_owned(),
                before: baseline.directory.clone(),
                after: current.directory.clone(),
                detail: None,
            }],
        });
    }
    if baseline.dependents != current.dependents || baseline.dependencies != current.dependencies {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The task graph edges changed".to_owned(),
            confidence: 91,
            evidence: vec![Evidence {
                source: "task graph".to_owned(),
                before: Some(format!(
                    "dependencies=[{}], dependents=[{}]",
                    baseline.dependencies.join(", "),
                    baseline.dependents.join(", ")
                )),
                after: Some(format!(
                    "dependencies=[{}], dependents=[{}]",
                    current.dependencies.join(", "),
                    current.dependents.join(", ")
                )),
                detail: None,
            }],
        });
    }
    if baseline.outputs != current.outputs || baseline.excluded_outputs != current.excluded_outputs
    {
        causes.push(Cause {
            kind: CauseKind::TaskConfiguration,
            summary: "The declared task outputs changed".to_owned(),
            confidence: 91,
            evidence: vec![Evidence {
                source: "outputs".to_owned(),
                before: Some(baseline.outputs.join(", ")),
                after: Some(current.outputs.join(", ")),
                detail: None,
            }],
        });
    }
}

fn add_upstream_causes(
    causes: &mut Vec<Cause>,
    baseline: &TaskSummary,
    current: &TaskSummary,
    baseline_tasks: &BTreeMap<String, &TaskSummary>,
    current_tasks: &BTreeMap<String, &TaskSummary>,
) {
    let dependencies = baseline
        .dependencies
        .iter()
        .chain(&current.dependencies)
        .collect::<BTreeSet<_>>();
    let evidence = dependencies
        .into_iter()
        .filter_map(|dependency| {
            let before = baseline_tasks.get(dependency.as_str())?.hash.clone();
            let after = current_tasks.get(dependency.as_str())?.hash.clone();
            (before != after).then(|| Evidence {
                source: dependency.clone(),
                before,
                after,
                detail: Some(
                    "This dependency changed first; the current task inherited its hash change."
                        .to_owned(),
                ),
            })
        })
        .collect::<Vec<_>>();
    if !evidence.is_empty() {
        causes.push(Cause {
            kind: CauseKind::UpstreamTask,
            summary: format!("{} upstream task hash(es) changed", evidence.len()),
            confidence: 90,
            evidence,
        });
    }
}

fn add_version_cause(causes: &mut Vec<Cause>, baseline: &RunSummary, current: &RunSummary) {
    if baseline.turbo_version != current.turbo_version {
        causes.push(Cause {
            kind: CauseKind::TurboVersion,
            summary: "The Turborepo version changed".to_owned(),
            confidence: 84,
            evidence: vec![Evidence {
                source: "turboVersion".to_owned(),
                before: baseline.turbo_version.clone(),
                after: current.turbo_version.clone(),
                detail: Some("Hashing behavior can change across Turborepo versions.".to_owned()),
            }],
        });
    }
}

fn file_cause(
    kind: CauseKind,
    summary: &str,
    confidence: u8,
    paths: &BTreeSet<String>,
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Cause {
    Cause {
        kind,
        summary: summary.to_owned(),
        confidence,
        evidence: paths
            .iter()
            .map(|path| Evidence {
                source: path.clone(),
                before: before.get(path).cloned(),
                after: after.get(path).cloned(),
                detail: match (before.contains_key(path), after.contains_key(path)) {
                    (false, true) => Some("File was added to the task inputs.".to_owned()),
                    (true, false) => Some("File was removed from the task inputs.".to_owned()),
                    _ => Some("File content fingerprint changed.".to_owned()),
                },
            })
            .collect(),
    }
}

fn task_map(summary: &RunSummary) -> BTreeMap<String, &TaskSummary> {
    summary
        .tasks
        .iter()
        .map(|task| (task.identity(), task))
        .collect()
}

fn topological_order(tasks: &BTreeMap<String, &TaskSummary>) -> BTreeMap<String, usize> {
    let mut indegree = tasks
        .iter()
        .map(|(id, task)| {
            let dependencies = task
                .dependencies
                .iter()
                .filter(|dependency| tasks.contains_key(dependency.as_str()))
                .count();
            (id.clone(), dependencies)
        })
        .collect::<BTreeMap<_, _>>();
    let mut downstream = BTreeMap::<String, BTreeSet<String>>::new();
    for (id, task) in tasks {
        for dependency in &task.dependencies {
            if tasks.contains_key(dependency) {
                downstream
                    .entry(dependency.clone())
                    .or_default()
                    .insert(id.clone());
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = BTreeMap::new();
    while let Some(id) = ready.pop_first() {
        let position = order.len();
        order.insert(id.clone(), position);
        for dependent in downstream.get(&id).into_iter().flatten() {
            let degree = indegree.get_mut(dependent).unwrap();
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    for id in tasks.keys() {
        if !order.contains_key(id) {
            order.insert(id.clone(), order.len());
        }
    }
    order
}

fn classification_rank(classification: Classification) -> u8 {
    match classification {
        Classification::RootCause | Classification::CacheUnavailable | Classification::NewTask => 0,
        Classification::Cascade => 1,
        Classification::Unexplained => 2,
        Classification::Unchanged => 3,
    }
}

fn unchanged_summary(
    baseline: Option<&TaskSummary>,
    current: &TaskSummary,
    baseline_run: &RunSummary,
    current_run: &RunSummary,
) -> UnchangedSummary {
    let Some(baseline) = baseline else {
        return UnchangedSummary {
            files: 0,
            environment_variables: 0,
            lockfile: None,
            turbo_json: None,
            task_configuration: false,
        };
    };
    let files = baseline
        .inputs
        .iter()
        .filter(|(path, hash)| {
            !is_lockfile(path) && !is_task_config(path) && current.inputs.get(*path) == Some(*hash)
        })
        .count();
    let mut before_env = baseline.environment_variables.fingerprints();
    before_env.extend(
        baseline_run
            .global_cache_inputs
            .environment_variables
            .fingerprints(),
    );
    let mut after_env = current.environment_variables.fingerprints();
    after_env.extend(
        current_run
            .global_cache_inputs
            .environment_variables
            .fingerprints(),
    );
    let environment_variables = before_env
        .iter()
        .filter(|(name, fingerprint)| after_env.get(*name) == Some(*fingerprint))
        .count();
    let lockfile = path_unchanged(is_lockfile, baseline, current, baseline_run, current_run);
    let turbo_json = path_unchanged(
        |path| path.rsplit('/').next() == Some("turbo.json"),
        baseline,
        current,
        baseline_run,
        current_run,
    );
    let task_configuration = baseline.resolved_task_definition == current.resolved_task_definition
        && baseline.command == current.command
        && baseline.outputs == current.outputs
        && baseline.excluded_outputs == current.excluded_outputs
        && baseline.directory == current.directory
        && baseline.dependencies == current.dependencies
        && baseline.dependents == current.dependents;
    UnchangedSummary {
        files,
        environment_variables,
        lockfile,
        turbo_json,
        task_configuration,
    }
}

fn path_unchanged(
    predicate: impl Fn(&str) -> bool,
    baseline: &TaskSummary,
    current: &TaskSummary,
    baseline_run: &RunSummary,
    current_run: &RunSummary,
) -> Option<bool> {
    let before = baseline
        .inputs
        .iter()
        .chain(&baseline_run.global_cache_inputs.files)
        .filter(|(path, _)| predicate(path))
        .collect::<BTreeMap<_, _>>();
    let after = current
        .inputs
        .iter()
        .chain(&current_run.global_cache_inputs.files)
        .filter(|(path, _)| predicate(path))
        .collect::<BTreeMap<_, _>>();
    if before.is_empty() && after.is_empty() {
        None
    } else {
        Some(before == after)
    }
}

fn metadata(source: &crate::model::SummarySource) -> SummaryMetadata {
    SummaryMetadata {
        path: portable_path(&source.path),
        id: source.summary.id.clone(),
        schema_version: source.summary.version.clone(),
        turbo_version: source.summary.turbo_version.clone(),
        commit_sha: source.summary.commit_sha().map(str::to_owned),
    }
}

fn is_lockfile(path: &str) -> bool {
    matches!(
        path.rsplit('/').next().unwrap_or(path),
        "bun.lock" | "bun.lockb" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock"
    )
}

fn is_task_config(path: &str) -> bool {
    matches!(
        path.rsplit('/').next().unwrap_or(path),
        "turbo.json" | "package.json" | "tsconfig.json" | "tsconfig.base.json"
    )
}

fn cache_detail(task: &TaskSummary) -> String {
    let mut facts = Vec::new();
    if let Some(source) = &task.cache.source {
        facts.push(format!("source={source}"));
    }
    if task.cache.remote {
        facts.push("remote=true".to_owned());
    }
    if task.cache.local {
        facts.push("local=true".to_owned());
    }
    if let Some(sha) = &task.cache.sha {
        facts.push(format!("artifact={sha}"));
    }
    if let Some(dirty_hash) = &task.cache.dirty_hash {
        facts.push(format!("dirtyHash={dirty_hash}"));
    }
    if let Some(hash_reason) = &task.hash_reason {
        facts.push(format!("hashReason={hash_reason}"));
    }
    if let Some(time_saved) = task.cache.time_saved {
        facts.push(format!("timeSaved={time_saved}ms"));
    }
    if facts.is_empty() {
        "The summary reports a miss for an identical hash.".to_owned()
    } else {
        facts.join(", ")
    }
}

fn redact_command(command: &str) -> String {
    command
        .split_whitespace()
        .map(|part| match part.split_once('=') {
            Some((name, _)) if sensitive_name(name) => format!("{name}=<redacted>"),
            _ => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sensitive_name(name: &str) -> bool {
    let normalized = name
        .trim_start_matches('-')
        .to_ascii_uppercase()
        .replace('-', "_");
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "AUTH",
        "CREDENTIAL",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn engines_detail(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Option<String> {
    let changed = changed_keys(before, after);
    (!changed.is_empty()).then(|| {
        format!(
            "Changed engine constraints: {}",
            changed.into_iter().collect::<Vec<_>>().join(", ")
        )
    })
}

fn build_hints(
    current: &TaskSummary,
    classification: Classification,
    causes: &[Cause],
) -> Vec<String> {
    let mut hints = Vec::new();
    if classification == Classification::CacheUnavailable {
        hints.push("Check remote-cache authentication, cache retention, and whether the artifact was evicted.".to_owned());
        hints.push(
            "Confirm that every runner uses the same remote-cache team and signature settings."
                .to_owned(),
        );
    }
    if causes
        .iter()
        .any(|cause| cause.kind == CauseKind::Environment)
    {
        hints.push("Keep task-affecting variables in `env` or `globalEnv`; avoid broad wildcard inputs when possible.".to_owned());
        let volatile = causes
            .iter()
            .filter(|cause| cause.kind == CauseKind::Environment)
            .flat_map(|cause| &cause.evidence)
            .map(|evidence| evidence.source.as_str())
            .filter(|name| volatile_environment_name(name))
            .collect::<BTreeSet<_>>();
        if !volatile.is_empty() {
            hints.push(format!(
                "Per-run variable(s) {} commonly invalidate every build; remove them from task hashing unless their values affect outputs.",
                volatile.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    if causes
        .iter()
        .any(|cause| cause.kind == CauseKind::DependencyGraph)
    {
        hints.push(
            "Review the lockfile diff and workspace dependency ranges before clearing caches."
                .to_owned(),
        );
    }
    if current.outputs.is_empty() {
        hints.push("This task declares no outputs; add `outputs` if its artifacts should be restored from cache.".to_owned());
    }
    let output_inputs = current
        .inputs
        .keys()
        .filter(|input| {
            current
                .outputs
                .iter()
                .any(|output| path_matches_output(input, output))
        })
        .collect::<Vec<_>>();
    if !output_inputs.is_empty() {
        hints.push(format!(
            "Generated output {} is also hashed as an input; exclude generated directories from inputs and version control.",
            output_inputs[0]
        ));
    }
    let log_inputs = current
        .inputs
        .keys()
        .filter(|path| {
            path.ends_with(".log")
                || path.contains(".turbo/")
                || current.log_file.as_ref().is_some_and(|log_file| {
                    log_file.ends_with(path.as_str()) || path.ends_with(log_file)
                })
        })
        .collect::<Vec<_>>();
    if !log_inputs.is_empty() {
        hints.push(format!(
            "Generated log {} is part of the task inputs; ignore or exclude it to prevent self-invalidating builds.",
            log_inputs[0]
        ));
    }
    let direct_kinds = causes
        .iter()
        .filter(|cause| cause.kind != CauseKind::UpstreamTask)
        .map(|cause| cause.kind)
        .collect::<BTreeSet<_>>();
    if direct_kinds == BTreeSet::from([CauseKind::DependencyGraph])
        && causes
            .iter()
            .flat_map(|cause| &cause.evidence)
            .any(|evidence| is_lockfile(&evidence.source))
    {
        hints.push("Only the lockfile changed; inspect resolved versions and workspace links before clearing the cache.".to_owned());
    }
    hints.sort();
    hints.dedup();
    hints
}

fn volatile_environment_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    [
        "BUILD_TIME",
        "BUILD_TIMESTAMP",
        "CI_COMMIT_SHA",
        "GITHUB_RUN_ID",
        "GITHUB_RUN_NUMBER",
        "GITHUB_SHA",
        "RUN_ID",
        "SOURCE_DATE_EPOCH",
        "TIMESTAMP",
    ]
    .iter()
    .any(|candidate| name == *candidate)
}

fn path_matches_output(input: &str, output: &str) -> bool {
    let output = output.trim_start_matches('!').trim_start_matches("./");
    let prefix = output
        .split('*')
        .next()
        .unwrap_or(output)
        .trim_end_matches('/');
    !prefix.is_empty() && (input == prefix || input.starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{model::SummarySource, parser::parse_bytes};

    use super::*;

    fn source(label: &str, json: &str) -> SummarySource {
        SummarySource {
            path: PathBuf::from(label),
            summary: parse_bytes(json.as_bytes(), label).unwrap(),
        }
    }

    #[test]
    fn ranks_environment_change_above_file_change() {
        let baseline = source(
            "before",
            r#"{"tasks":[{"taskId":"app#build","task":"build","hash":"one","inputs":{"a.ts":"a"},"environmentVariables":{"configured":["NODE_ENV=aaa"]}}]}"#,
        );
        let current = source(
            "after",
            r#"{"tasks":[{"taskId":"app#build","task":"build","hash":"two","inputs":{"a.ts":"b"},"environmentVariables":{"configured":["NODE_ENV=bbb"]},"cache":{"status":"MISS"}}]}"#,
        );
        let report = analyze(
            SummaryPair {
                baseline,
                current,
                warnings: vec![],
            },
            Some("build"),
        );
        assert_eq!(report.tasks[0].causes[0].kind, CauseKind::Environment);
        assert_eq!(report.tasks[0].classification, Classification::RootCause);
    }

    #[test]
    fn detects_same_hash_cache_miss() {
        let baseline = source(
            "before",
            r#"{"tasks":[{"taskId":"app#build","hash":"same"}]}"#,
        );
        let current = source(
            "after",
            r#"{"tasks":[{"taskId":"app#build","hash":"same","cache":{"status":"MISS"}}]}"#,
        );
        let report = analyze(
            SummaryPair {
                baseline,
                current,
                warnings: vec![],
            },
            None,
        );
        assert_eq!(
            report.tasks[0].classification,
            Classification::CacheUnavailable
        );
    }

    #[test]
    fn redacts_sensitive_command_assignments() {
        assert_eq!(
            redact_command("deploy --api-token=plain REGION=iad"),
            "deploy --api-token=<redacted> REGION=iad"
        );
    }

    #[test]
    fn orders_root_cause_before_downstream_cascade() {
        let baseline = source(
            "before",
            r#"{"tasks":[
                {"taskId":"ui#build","task":"build","hash":"ui-a","inputs":{"button.ts":"a"},"cache":{"status":"HIT"}},
                {"taskId":"web#build","task":"build","hash":"web-a","inputs":{"index.ts":"same"},"dependencies":["ui#build"],"cache":{"status":"HIT"}}
            ]}"#,
        );
        let current = source(
            "after",
            r#"{"tasks":[
                {"taskId":"ui#build","task":"build","hash":"ui-b","inputs":{"button.ts":"b"},"cache":{"status":"MISS"}},
                {"taskId":"web#build","task":"build","hash":"web-b","inputs":{"index.ts":"same"},"dependencies":["ui#build"],"cache":{"status":"MISS"}}
            ]}"#,
        );
        let report = analyze(
            SummaryPair {
                baseline,
                current,
                warnings: vec![],
            },
            None,
        );
        assert_eq!(report.tasks[0].task_id, "ui#build");
        assert_eq!(report.tasks[0].classification, Classification::RootCause);
        assert_eq!(report.tasks[1].task_id, "web#build");
        assert_eq!(report.tasks[1].classification, Classification::Cascade);
    }

    #[test]
    fn emits_curated_hints_for_known_self_invalidation_patterns() {
        let baseline = source(
            "before",
            r#"{"tasks":[{"taskId":"web#build","task":"build","hash":"a","inputs":{"pnpm-lock.yaml":"lock-a"},"outputs":[".next/**"],"environmentVariables":{"configured":["GITHUB_SHA=sha-a"]}}]}"#,
        );
        let current = source(
            "after",
            r#"{"tasks":[{"taskId":"web#build","task":"build","hash":"b","inputs":{"pnpm-lock.yaml":"lock-b",".next/cache/data":"generated",".turbo/build.log":"log"},"outputs":[".next/**"],"logFile":".turbo/build.log","environmentVariables":{"configured":["GITHUB_SHA=sha-b"]},"cache":{"status":"MISS"}}]}"#,
        );
        let report = analyze(
            SummaryPair {
                baseline,
                current,
                warnings: vec![],
            },
            None,
        );
        let hints = report.tasks[0].hints.join("\n");
        assert!(hints.contains("Per-run variable(s) GITHUB_SHA"));
        assert!(hints.contains("Generated output .next/cache/data"));
        assert!(hints.contains("Generated log .turbo/build.log"));
    }

    #[test]
    fn diagnoses_global_dependency_config_and_version_changes() {
        let baseline = source(
            "before",
            r#"{
              "turboVersion":"1.9.9",
              "globalCacheInputs":{"files":{"turbo.json":"a"},"hashOfExternalDependencies":"deps-a"},
              "tasks":[{"taskId":"app#build","task":"build","hash":"a","command":"old","resolvedTaskDefinition":{"cache":true},"cache":{"status":"HIT"}}]
            }"#,
        );
        let current = source(
            "after",
            r#"{
              "turboVersion":"2.9.15",
              "globalCacheInputs":{"files":{"turbo.json":"b"},"hashOfExternalDependencies":"deps-b"},
              "tasks":[{"taskId":"app#build","task":"build","hash":"b","command":"new","resolvedTaskDefinition":{"cache":false},"cache":{"status":"MISS"}}]
            }"#,
        );
        let report = analyze(
            SummaryPair {
                baseline,
                current,
                warnings: vec![],
            },
            None,
        );
        let kinds = report.tasks[0]
            .causes
            .iter()
            .map(|cause| cause.kind)
            .collect::<BTreeSet<_>>();
        assert!(kinds.contains(&CauseKind::GlobalInput));
        assert!(kinds.contains(&CauseKind::DependencyGraph));
        assert!(kinds.contains(&CauseKind::TaskConfiguration));
        assert!(kinds.contains(&CauseKind::TurboVersion));
    }
}
