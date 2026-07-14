use std::{fs, path::Path};

use crate::{
    error::{Error, Result},
    model::RunSummary,
};

pub fn parse_file(path: &Path) -> Result<RunSummary> {
    let bytes = fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    parse_bytes(&bytes, &path.display().to_string())
}

pub fn parse_bytes(bytes: &[u8], label: &str) -> Result<RunSummary> {
    let summary: RunSummary = serde_json::from_slice(bytes).map_err(|source| Error::Parse {
        path: label.to_owned(),
        source,
    })?;
    if summary.tasks.is_empty() {
        return Err(Error::EmptySummary(label.to_owned()));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_unknown_fields_and_missing_optional_fields() {
        let summary = parse_bytes(
            br#"{"version":"1","futureField":{"nested":true},"tasks":[{"taskId":"app#build","future":42}]}"#,
            "fixture",
        )
        .unwrap();
        assert_eq!(summary.tasks[0].identity(), "app#build");
    }

    #[test]
    fn parses_current_turborepo_fixture() {
        let summary = parse_bytes(
            include_bytes!("../tests/fixtures/turbo-2.10.json"),
            "turbo-2.10.json",
        )
        .unwrap();
        assert_eq!(summary.version.as_deref(), Some("1"));
        assert_eq!(summary.turbo_version.as_deref(), Some("2.10.5"));
        assert_eq!(summary.tasks[0].identity(), "@fixture/app#build");
        assert!(summary.global_cache_inputs.engines.is_empty());
        assert!(summary.tasks[0].excluded_outputs.is_empty());
    }

    #[test]
    fn rejects_summaries_without_tasks() {
        let error = parse_bytes(br#"{"version":"1","tasks":[]}"#, "empty").unwrap_err();
        assert!(error.to_string().contains("does not contain any tasks"));
    }

    #[test]
    fn parses_turborepo_1_9_fixture() {
        let summary = parse_bytes(
            include_bytes!("../tests/fixtures/turbo-1.9.json"),
            "turbo-1.9.json",
        )
        .unwrap();
        assert_eq!(summary.turbo_version.as_deref(), Some("1.9.9"));
        assert_eq!(summary.tasks[0].identity(), "@fixture/app#build");
        assert!(
            summary.tasks[0]
                .environment_variables
                .fingerprints()
                .contains_key("VERCEL_ANALYTICS_ID")
        );
    }

    #[test]
    fn handles_every_truncated_fixture_without_panicking() {
        let fixture = include_bytes!("../tests/fixtures/turbo-1.9.json");
        for length in 0..fixture.len() {
            let _ = parse_bytes(&fixture[..length], "truncated");
        }
        assert!(parse_bytes(fixture, "complete").is_ok());
    }

    #[test]
    fn rejects_invalid_utf8_and_wrong_field_types() {
        assert!(parse_bytes(b"{\"tasks\":[\xff]}", "invalid-utf8").is_err());
        assert!(parse_bytes(br#"{"tasks":"not-an-array"}"#, "wrong-type").is_err());
        assert!(parse_bytes(br#"["not-an-object"]"#, "wrong-root").is_err());
    }
}
