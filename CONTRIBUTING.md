# Contributing

Thanks for improving WhyCache. Small, evidence-backed changes are preferred over broad refactors.

## Setup

Install stable Rust 1.85 or newer, then run:

```sh
cargo test --all-targets --all-features
```

## Before opening a pull request

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Add or update a sanitized run-summary fixture for parser changes. Diagnosis changes need a test that proves the ranking, classification, and evidence. Never commit environment values, access tokens, private repository paths, or proprietary source fingerprints.

Keep comments for invariants, protocol quirks, and safety decisions that are not obvious from the code. Do not narrate straightforward code.

## Compatibility changes

When Turborepo changes its summary shape:

1. add a minimal fixture from the new version;
2. document the source version;
3. preserve old fixtures;
4. add fields only when they contribute to a diagnosis or compatibility decision;
5. verify JSON output remains backward-compatible or increment its schema version.

## Commits and pull requests

Use a focused subject in the imperative mood. Explain the user-visible problem, the evidence for the fix, and the validation performed. Pull requests must pass every supported operating-system job.

By contributing, you agree that your contribution is licensed under MIT OR Apache-2.0.
