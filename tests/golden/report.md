## WhyCache report

| Summary | Source | Turbo | Commit |
|---|---|---|---|
| Baseline | `tests/fixtures/repo/.turbo/runs/a-baseline.json` | 2.9.15 | aaaaaaaaaaaa |
| Current | `tests/fixtures/repo/.turbo/runs/z-current.json` | 2.9.15 | bbbbbbbbbbbb |

### `ui#build`

**MISS · root cause** — hash `upstream-a` → `upstream-b`

#### 1 task input file(s) changed · 95% confidence

| Evidence | Before | After | Git |
|---|---:|---:|---:|
| `packages/ui/src/button.tsx` | `button-a` | `button-b` | — |

**Likely culprit:** packages/ui/src/button.tsx changed between runs.

**Unchanged:** 0 file(s), 1 environment variable(s), turbo.json, task configuration


### `web#build`

**MISS · root cause** — hash `111111111111` → `222222222222`

#### 1 environment fingerprint(s) changed · 98% confidence

| Evidence | Before | After | Git |
|---|---:|---:|---:|
| `NODE_ENV` | `env-a` | `env-b` | — |

#### 1 task input file(s) changed · 95% confidence

| Evidence | Before | After | Git |
|---|---:|---:|---:|
| `apps/web/src/index.ts` | `source-a` | `source-b` | — |

#### 1 upstream task hash(es) changed · 90% confidence

| Evidence | Before | After | Git |
|---|---:|---:|---:|
| `ui#build` | `upstream-a` | `upstream-b` | — |

**Likely culprit:** NODE_ENV changed between runs.

**Unchanged:** 0 file(s), 1 environment variable(s), turbo.json, task configuration

**Next checks**

- Keep task-affecting variables in `env` or `globalEnv`; avoid broad wildcard inputs when possible.
