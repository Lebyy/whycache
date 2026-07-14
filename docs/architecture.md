# Architecture

WhyCache is a single Rust executable with no runtime services.

## Data flow

1. Repository discovery walks upward to find `turbo.json`, a Turborepo dependency, or `.turbo/runs`.
2. Summary discovery orders `.turbo/runs/*.json` by modification time and path. The newest run is current; the nearest earlier successful run is the default baseline.
3. The parser deserializes known fields and tolerates unknown fields. Required diagnostic structure is validated after parsing.
4. The comparison engine builds task maps, diffs direct inputs, and then follows task dependency edges.
5. The ranking engine assigns stable confidence scores based on signal specificity.
6. Renderers consume one versioned report model for human, JSON, and Markdown output.

No renderer performs diagnosis. This keeps every format semantically identical.

## Root causes and cascades

A task is a root cause when its own inputs, environment, configuration, dependency graph, or global inputs changed. It is a cascade when its own inputs are stable but a direct upstream task hash changed. The upstream evidence points to the first task to inspect.

When baseline and current hashes match but the current summary reports a miss, the input comparison cannot explain the miss because the cache key did not change. WhyCache reports cache unavailability and recommends checking eviction, authentication, team selection, and artifact retention.

## Confidence model

Confidence is a deterministic priority, not a statistical probability:

- 98: an environment fingerprint changed;
- 95–96: one exact input or lockfile changed;
- 91–95: task configuration or dependency graph changed;
- 90: a direct upstream task hash changed;
- 84: only the Turborepo version changed.

Multiple causes remain visible. Ranking does not discard lower-confidence evidence.

## Secret handling

Turborepo summaries expose environment entries as names and hashes. WhyCache splits on the first `=` and stores only the name and fingerprint. It never reads the live environment to resolve a value. Git integration accepts only commit SHAs and already identified file paths, passed as separate process arguments.

## Compatibility

Run-summary schema version `1` is the native contract. Serde defaults allow fields absent from older Turborepo releases, and unknown fields are ignored. A different declared schema version produces an explicit compatibility warning. New schema fixtures should be added before a support claim changes.
