<p align="center">
  <img src="https://raw.githubusercontent.com/Lebyy/whycache/main/assets/whycache.svg" width="136" alt="WhyCache logo">
</p>

<h1 align="center">WhyCache</h1>

<p align="center">
  Explain exactly why a Turborepo task missed the cache.
</p>

<p align="center">
  <a href="https://github.com/Lebyy/whycache/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Lebyy/whycache/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://crates.io/crates/whycache"><img alt="crates.io" src="https://img.shields.io/crates/v/whycache.svg"></a>
  <a href="https://www.npmjs.com/package/whycache"><img alt="npm" src="https://img.shields.io/npm/v/whycache.svg"></a>
  <a href="https://github.com/Lebyy/whycache/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg"></a>
</p>

WhyCache compares two Turborepo run summaries, isolates the inputs that changed, and ranks the most likely cause. It distinguishes direct root causes from downstream cascades and catches the especially confusing case where the task hash stayed identical but the cached artifact was unavailable.

It runs locally, produces deterministic output, and never sends source, environment data, or build metadata anywhere.

```text
WhyCache — Turborepo cache diagnosis

  Baseline  .turbo/runs/01.json
  Current   .turbo/runs/02.json
  Turbo     2.9.15 → 2.9.15

web#build  MISS  root cause
  Hash      111111111111 → 222222222222

  1. 1 environment fingerprint(s) changed  98% confidence
     • NODE_ENV  env-a → env-b

  2. 1 task input file(s) changed  95% confidence
     • apps/web/src/index.ts  source-a → source-b
```

## Install

Once the first public release is available:

```sh
cargo install whycache
```

or:

```sh
npm install --global whycache
```

Release binaries will be published for Linux x86-64/ARM64, macOS Intel/Apple Silicon, and Windows x86-64. Until then, build from source with Rust 1.85 or newer:

```sh
cargo install --path .
```

## Use

First, ask Turborepo to retain run summaries:

```sh
turbo run build --summarize
```

After a later run, inspect a task:

```sh
whycache build
```

WhyCache normally selects the newest summary as the current run and the most recent earlier successful summary as the baseline.

```sh
# Choose the baseline explicitly
whycache build --against .turbo/runs/previous.json

# Read the baseline from standard input
cat previous.json | whycache build --against -

# Stable automation formats
whycache build --json
whycache build --md >> "$GITHUB_STEP_SUMMARY"

# Add line counts from Git when both summaries contain commit SHAs
whycache build --git
```

If no saved history exists, `whycache build` runs a Turborepo dry summary and stores it at `.whycache/last-summary.json`. It tells you that a baseline was captured and waits for the next comparison. WhyCache does not invent a cause for a past miss it cannot observe.

## What it explains

| Signal | Diagnosis |
|---|---|
| Task input fingerprints | Added, removed, and changed source files |
| Environment fingerprints | Changed variable names without exposing values |
| Lockfiles and dependency hashes | Dependency graph changes |
| `turbo.json` and resolved task definitions | Pipeline, output, and command changes |
| Global inputs and engine constraints | Repository-wide invalidation |
| Upstream task hashes | Root cause versus cascade |
| Same task hash plus a miss | Eviction, remote-cache access, or artifact availability |
| Turborepo version | Possible hashing behavior changes |

Findings include confidence scores and concrete before/after fingerprints. Environment values are never requested or printed.

## Output contract

`--json` emits schema version `1`. Its keys and ordering are deterministic for the same two summaries. `--md` produces GitHub-flavored Markdown designed for `$GITHUB_STEP_SUMMARY` and pull-request comments. Human output uses color only when writing to a terminal.

Unknown run-summary fields are ignored. Unknown schema versions are parsed in compatibility mode with a visible warning. Fixtures cover Turborepo 1.9 and the current v2 summary shape.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See the [contribution guide](https://github.com/Lebyy/whycache/blob/main/CONTRIBUTING.md) for test and release expectations, and the [architecture notes](https://github.com/Lebyy/whycache/blob/main/docs/architecture.md) for the diagnosis model.

## Privacy and scope

WhyCache is offline and has no telemetry, accounts, API keys, hosted service, or AI features. It reads Turborepo summaries already present in your repository. `--git` invokes only your local Git executable and passes paths as separate arguments.

WhyCache is not affiliated with or endorsed by Vercel. Turborepo is a trademark of its respective owner.

## License

Licensed under either [MIT](https://github.com/Lebyy/whycache/blob/main/LICENSE-MIT) or [Apache License 2.0](https://github.com/Lebyy/whycache/blob/main/LICENSE-APACHE), at your option.
