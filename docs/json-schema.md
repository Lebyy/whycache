# JSON output

`whycache --json` emits a single JSON object. `schemaVersion` is currently `"1"`.

Top-level fields:

- `baseline` and `current`: summary source, run id, schema, Turborepo version, and optional commit SHA;
- `tasks`: diagnoses ordered by task id;
- `warnings`: compatibility and enrichment warnings in discovery order.

Each task contains its cache status, hashes, classification, ranked causes, hints, and optional Git statistics. Every cause includes a stable `kind`, a human summary, integer confidence from 0 through 100, and concrete evidence.

Consumers should ignore unknown fields and branch on `schemaVersion` before relying on changed semantics.
