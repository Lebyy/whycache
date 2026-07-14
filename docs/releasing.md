# Releasing

Releases are tag-driven and must start from a clean `main` branch.

1. Update `CHANGELOG.md` with the user-visible changes.
2. Verify each direct crate against crates.io, update `Cargo.toml`, and refresh the complete compatible graph:

   ```sh
   cargo update
   cargo update --dry-run --verbose
   cargo audit
   ```

   The dry run must resolve zero updates. If a newest crate raises its MSRV, decide explicitly whether the release should raise `rust-version`; never let the lockfile make that policy decision silently.

3. Synchronize Cargo and every npm manifest:

   ```sh
   scripts/sync-version.sh 0.1.0
   ```

4. Run the complete local gate:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets --all-features
   cargo +1.85.0 test --all-targets --all-features
   cargo package --locked
   cmp README.md npm/whycache/README.md
   node --check npm/whycache/bin/whycache.js
   npm pack --dry-run ./npm/whycache
   ```

5. Commit the version change, create the matching `vX.Y.Z` tag, and push the commit and tag.

The release workflow builds five native archives, generates checksums, smoke-tests the npm launcher with the Linux binary, creates the GitHub release, and publishes Cargo and npm packages from the protected `release` environment. Configure npm trusted publishing or `NPM_TOKEN`, plus `CARGO_REGISTRY_TOKEN`, before pushing the first tag.

Never rerun a partially published version. Correct the failure and publish a new patch version because npm, crates.io, and GitHub release artifacts are immutable distribution records.
