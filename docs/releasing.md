# Releasing

Releases are tag-driven and must start from a clean `main` branch.

1. Update `CHANGELOG.md` with the user-visible changes.
2. Synchronize Cargo and every npm manifest:

   ```sh
   scripts/sync-version.sh 0.1.0
   ```

3. Run the complete local gate:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets --all-features
   cargo package --locked
   npm pack --dry-run ./npm/whycache
   ```

4. Commit the version change, create the matching `vX.Y.Z` tag, and push the commit and tag.

The release workflow builds five native archives, generates checksums, smoke-tests the npm launcher with the Linux binary, creates the GitHub release, and publishes Cargo and npm packages from the protected `release` environment. Configure npm trusted publishing or `NPM_TOKEN`, plus `CARGO_REGISTRY_TOKEN`, before pushing the first tag.

Never rerun a partially published version. Correct the failure and publish a new patch version because npm, crates.io, and GitHub release artifacts are immutable distribution records.
