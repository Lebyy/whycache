<p align="center">
  <img src="https://raw.githubusercontent.com/Lebyy/whycache/main/assets/whycache.svg" width="148" alt="WhyCache logo">
</p>

<h1 align="center">WhyCache</h1>

<p align="center">
  <strong>Stop guessing why Turborepo missed the cache.</strong><br>
  Compare two runs, isolate the changed evidence, and find the first task that invalidated the graph.
</p>

<p align="center">
  <a href="https://github.com/Lebyy/whycache/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Lebyy/whycache/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/Lebyy/whycache/actions/workflows/codeql.yml"><img alt="CodeQL" src="https://github.com/Lebyy/whycache/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://github.com/Lebyy/whycache/actions/workflows/audit.yml"><img alt="RustSec" src="https://github.com/Lebyy/whycache/actions/workflows/audit.yml/badge.svg"></a>
  <a href="https://crates.io/crates/whycache"><img alt="crates.io" src="https://img.shields.io/crates/v/whycache.svg"></a>
  <a href="https://www.npmjs.com/package/whycache"><img alt="npm" src="https://img.shields.io/npm/v/whycache.svg"></a>
  <img alt="Rust 1.85 or newer" src="https://img.shields.io/badge/rust-1.85%2B-f74c00.svg">
  <a href="https://github.com/Lebyy/whycache/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-2563eb.svg"></a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/whycache">npm</a> ·
  <a href="https://crates.io/crates/whycache">crates.io</a> ·
  <a href="https://github.com/Lebyy/whycache/releases">release binaries</a> ·
  <a href="https://github.com/Lebyy/whycache/issues">issues</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/Lebyy/whycache/main/assets/terminal.gif" width="1000" alt="Animated WhyCache diagnosis from a real deterministic fixture">
</p>

WhyCache is a small, fully local Rust CLI for answering one expensive question:

> **Why did this task miss when I expected a cache hit?**

It reads the run summaries Turborepo already produces, compares a known-good baseline with the current run, removes unchanged noise, ranks the remaining causes, and attaches every conclusion to before/after fingerprints. It understands direct invalidation, downstream cascades, and the important case where the hash did not change but the cached artifact was unavailable.

No source upload. No account. No API key. No telemetry. No AI. 🛡️

## ✨ Why it is useful

A Turborepo task hash can change because of source files, environment fingerprints, lockfiles, global inputs, task configuration, dependency hashes, an upstream task, or even a Turborepo version change. Raw run summaries contain the evidence, but they are large and graph-shaped.

WhyCache turns that evidence into an investigation order:

```text
web#build  MISS  root cause
  Hash      111111111111 → 222222222222

  1. 1 environment fingerprint(s) changed  98% confidence
     • NODE_ENV  env-a → env-b

  2. 1 task input file(s) changed  95% confidence
     • apps/web/src/index.ts  source-a → source-b

  3. 1 upstream task hash(es) changed  90% confidence
     • ui#build  upstream-a → upstream-b

  💡 Likely culprit: NODE_ENV changed between runs.
```

The example above is generated from the checked-in compatibility fixture and protected by exact golden-output tests.

<p align="center">
  <img src="https://raw.githubusercontent.com/Lebyy/whycache/main/assets/diagnosis.png" width="1000" alt="Static WhyCache terminal report showing ranked environment and file evidence">
</p>

## 🚀 Install

### npm — recommended

```sh
npm install --global whycache
```

The npm launcher selects the native package for your operating system and architecture. The JavaScript launcher has no third-party runtime libraries.

### Cargo

```sh
cargo install whycache --locked
```

### Release binary

Download the archive for your platform from [GitHub Releases](https://github.com/Lebyy/whycache/releases), verify it against `SHA256SUMS`, and place `whycache` on your `PATH`.

### Build from source

```sh
git clone https://github.com/Lebyy/whycache.git
cd whycache
cargo install --path . --locked
```

WhyCache requires Rust 1.85 or newer when building from source.

| Operating system | Architecture | npm package | Release asset |
|---|---:|---|---|
| Linux | x86-64 | `@whycache/linux-x64` | `linux-x64` |
| Linux | ARM64 | `@whycache/linux-arm64` | `linux-arm64` |
| macOS | Apple Silicon | `@whycache/darwin-arm64` | `darwin-arm64` |
| macOS | Intel | `@whycache/darwin-x64` | `darwin-x64` |
| Windows | x86-64 | `@whycache/win32-x64` | `win32-x64` |

## ⚡ Quick start

### Compare saved Turborepo runs

Ask Turborepo to retain a summary when you run the task:

```sh
turbo run build --summarize
```

After a later run misses, diagnose every missed task:

```sh
whycache
```

Or focus on one task name or fully qualified task id:

```sh
whycache build
whycache web#build
```

By default, WhyCache chooses the newest summary as the current run and the nearest earlier successful summary as the baseline.

### Start without saved history

You can run WhyCache before enabling `--summarize`:

```sh
whycache build
```

WhyCache captures `turbo run build --dry=json` at `.whycache/last-summary.json`. A dry summary calculates inputs without executing the task. The first invocation honestly reports `baseline_captured`; the next invocation compares current inputs against that baseline.

Running `whycache` without a task reads all task names from `turbo.json` and captures them together.

### Choose the baseline yourself

```sh
whycache build --against .turbo/runs/previous.json
```

External systems can stream a baseline over standard input:

```sh
cat previous.json | whycache build --against -
```

## 🔎 What WhyCache compares

| Evidence | What a change means | Reported safely? |
|---|---|---:|
| Task input fingerprints | Source, config, generated input, or declared dependency changed | ✅ Path + hash |
| Environment fingerprints | A configured variable affected the task hash | ✅ Name + fingerprint only |
| Lockfiles | The external dependency graph changed | ✅ Path + hash |
| `turbo.json` | Repository-wide pipeline configuration changed | ✅ Fingerprint |
| Resolved task definition | Command, inputs, outputs, cache policy, or dependencies changed | ✅ Structured diff |
| Global inputs | A root-level file or global environment entry invalidated tasks | ✅ Path/name + hash |
| Upstream task hashes | A dependency changed first and propagated through the graph | ✅ Task id + hash |
| Package dependency hash | External packages or workspace dependency state changed | ✅ Hash only |
| Engine constraints | Node/package-manager constraints changed | ✅ Constraint text |
| Turborepo version | Hashing behavior may differ between the two runs | ✅ Version only |
| Same hash with a miss | The cache key is stable but the artifact was unavailable | ✅ Cache metadata |

Unchanged evidence is summarized instead of repeated, so a large monorepo report stays focused.

## 🧭 How to read a diagnosis

WhyCache classifies each task before ranking its evidence:

| Classification | Meaning | First action |
|---|---|---|
| `root cause` | The task's own inputs or configuration changed | Inspect the highest-confidence evidence |
| `cascade` | Only an upstream task changed | Follow the named upstream task |
| `cache unavailable` | Hash stayed identical but the run missed | Check local/remote cache access and retention |
| `new task` | The task did not exist in the baseline | Confirm the pipeline/package change |
| `unchanged` | No relevant change was found | No cache-key investigation required |
| `unexplained` | The summary lacks enough supported evidence | Review the warning and raw summary version |

Confidence is a deterministic priority—not a probability generated by a model. Exact, task-local evidence ranks above broad or downstream evidence. Multiple plausible causes remain visible.

## 📤 Output formats

### Human terminal report

```sh
whycache build
```

Color is enabled only for an interactive terminal, so redirected output stays clean.

### Stable JSON

```sh
whycache build --json > whycache-report.json
```

JSON output is deterministic for the same inputs and currently uses schema version `1`. Consumers should branch on `schemaVersion` and ignore unknown fields. See the complete [JSON contract](https://github.com/Lebyy/whycache/blob/main/docs/json-schema.md).

```json
{
  "schemaVersion": "1",
  "tasks": [
    {
      "taskId": "web#build",
      "cacheStatus": "miss",
      "classification": "root_cause",
      "causes": [
        {
          "kind": "environment",
          "confidence": 98
        }
      ]
    }
  ],
  "warnings": []
}
```

### GitHub-flavored Markdown

```sh
whycache build --md >> "$GITHUB_STEP_SUMMARY"
```

Markdown uses the same diagnosis model as human and JSON output, including evidence tables, confidence, likely culprit, unchanged signals, and next checks.

### Git line statistics

```sh
whycache build --git
```

When both summaries contain commit SHAs, `--git` adds line counts for already identified evidence paths. WhyCache invokes the local Git executable with argument arrays; it does not execute a shell command assembled from summary data.

## 🤖 GitHub Actions example

Generate and retain summaries in your build job, then write the diagnosis directly to the step summary:

```yaml
- name: Install WhyCache
  run: npm install --global whycache

- name: Explain cache misses
  if: always()
  run: whycache build --md --git >> "$GITHUB_STEP_SUMMARY"
```

For a pull-request bot, store `whycache build --json` as an artifact and post the Markdown output using your existing trusted workflow permissions.

## 🧠 How it works

<p align="center">
  <img src="https://raw.githubusercontent.com/Lebyy/whycache/main/assets/architecture.svg" width="1100" alt="WhyCache discovery, normalization, comparison, diagnosis, and rendering pipeline">
</p>

1. **Discover** — walk upward to the Turborepo root and deterministically order `.turbo/runs/*.json`.
2. **Normalize** — deserialize known Turbo 1.9 and v2 fields, treat historical `null` collections as empty, and ignore unknown additions.
3. **Compare** — diff task-local and global evidence, then build upstream dependency edges.
4. **Diagnose** — separate direct root causes from inherited cascades and rank causes by specificity.
5. **Explain** — render one versioned report model as terminal text, JSON, or Markdown.

Renderers never perform diagnosis, which keeps every output format semantically identical. Read the deeper [architecture notes](https://github.com/Lebyy/whycache/blob/main/docs/architecture.md).

## 🧰 CLI reference

```text
Usage: whycache [OPTIONS] [TASK]
```

| Argument | Purpose |
|---|---|
| `[TASK]` | Match a task name such as `build` or an exact id such as `web#build` |
| `--against <SUMMARY>` | Use a specific baseline; pass `-` to read JSON from stdin |
| `--json` | Emit stable machine-readable JSON |
| `--md` | Emit GitHub-flavored Markdown |
| `--git` | Add Git numstat evidence when both commits are available |
| `-h`, `--help` | Show CLI help and examples |
| `-V`, `--version` | Show the installed WhyCache version |

`--json` and `--md` are mutually exclusive. Invalid usage and missing tasks exit with status `2`; operational and parse failures exit with status `1`.

## 🔐 Privacy and security

- Runs entirely on the local machine or CI runner.
- Never uploads summaries, source paths, hashes, or Git metadata.
- Never reads task environment values to reconstruct secrets.
- Shows environment variable names and Turborepo-provided fingerprints only.
- Uses no network client, telemetry SDK, analytics, AI, or hosted service.
- Pins third-party GitHub Actions to immutable commit SHAs.
- Runs RustSec and CodeQL workflows in addition to the warning-free test matrix.

Please report vulnerabilities privately using the process in [SECURITY.md](https://github.com/Lebyy/whycache/blob/main/SECURITY.md).

## 🧪 Compatibility policy

| Surface | Coverage |
|---|---|
| Turborepo 1.9 | Real schema-0 fixture and scheduled `1.9.9` canary |
| Turborepo 2.0 | Scheduled `2.0.0` canary |
| Current Turborepo v2 | Real fixture and scheduled `latest` canary |
| Linux | Native x86-64 and ARM64 release jobs |
| macOS | Native Intel and Apple Silicon release jobs |
| Windows | Native x86-64 release job |
| Rust | MSRV job on Rust 1.85 plus stable CI |

Unknown run-summary fields are ignored. Declared schema versions other than `0` and `1` are parsed in compatibility mode with a visible warning; WhyCache does not silently claim full support for an untested schema.

## ⚠️ Honest limitations

- WhyCache explains differences present in run summaries; it is not a general-purpose build profiler.
- A historical miss cannot be reconstructed if no baseline or previous summary exists. Automatic mode starts collecting evidence for the next comparison.
- An unchanged hash proves the key stayed stable, but a local summary alone cannot distinguish eviction from remote-cache authentication, team selection, or retention policy.
- Environment values are intentionally unavailable. This protects secrets but means the report names the changed variable rather than printing its plaintext value.
- If Turborepo introduces a new schema, WhyCache warns until that version has a real fixture and compatibility test.

## 🛠️ Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo +1.85.0 test --all-targets --all-features
cargo audit
```

The repository is intentionally small:

```text
src/                    CLI, discovery, parsing, diagnosis, Git, renderers
tests/fixtures/         Sanitized real Turbo schemas and deterministic scenarios
tests/golden/           Exact human, JSON, and Markdown output contracts
npm/                    Cross-platform native npm packages and launcher
.github/workflows/      CI, CodeQL, RustSec, canary, and release automation
assets/                 Logo, real terminal capture, and architecture artwork
docs/                   Architecture, JSON contract, and release procedure
```

Changes must remain warning-free and evidence-backed. Parser changes need a sanitized fixture; diagnosis changes need ranking and classification tests; renderer changes need updated golden files. See [CONTRIBUTING.md](https://github.com/Lebyy/whycache/blob/main/CONTRIBUTING.md).

## 📦 Releases

A `v*` tag builds five native archives, tests each runner, generates checksums, creates the GitHub release, and publishes the matching platform packages, npm installer, and Rust crate. Package versions are synchronized by `scripts/sync-version.sh` before tagging.

The complete checklist is in [docs/releasing.md](https://github.com/Lebyy/whycache/blob/main/docs/releasing.md).

## ❓ FAQ

<details>
<summary><strong>Does WhyCache replace <code>turbo run --summarize</code>?</strong></summary>

No. Turborepo produces the evidence; WhyCache compares and explains it. When history is empty, WhyCache can invoke Turborepo's dry JSON mode to establish a truthful baseline without executing tasks.
</details>

<details>
<summary><strong>Does it send my repository data to an AI model?</strong></summary>

No. There is no AI integration, network client, API key, account, or telemetry path. Diagnosis is a deterministic Rust comparison engine.
</details>

<details>
<summary><strong>Why are confidence scores so exact?</strong></summary>

They are stable priorities. A changed environment fingerprint is direct task evidence, an exact file change is slightly lower, and an upstream hash is ranked below local evidence because it describes propagation. They are not statistical probabilities.
</details>

<details>
<summary><strong>Can it run in a large monorepo?</strong></summary>

Yes. WhyCache compares summary maps in memory, sorts output deterministically, and collapses unchanged signals. Use a task filter when you want the smallest possible report.
</details>

## 🤝 Contributing

Issues and focused pull requests are welcome. Please read the [contribution guide](https://github.com/Lebyy/whycache/blob/main/CONTRIBUTING.md) and [code of conduct](https://github.com/Lebyy/whycache/blob/main/CODE_OF_CONDUCT.md) first.

WhyCache is not affiliated with or endorsed by Vercel. Turborepo is a trademark of its respective owner.

## 📄 License

Licensed under either the [MIT License](https://github.com/Lebyy/whycache/blob/main/LICENSE-MIT) or [Apache License 2.0](https://github.com/Lebyy/whycache/blob/main/LICENSE-APACHE), at your option.
