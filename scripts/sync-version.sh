#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/sync-version.sh <version>" >&2
  exit 2
fi

version="${1#v}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid semantic version: $version" >&2
  exit 2
fi

perl -0pi -e "s/^version = \"[^\"]+\"/version = \"$version\"/m" Cargo.toml

while IFS= read -r manifest; do
  node -e '
    const fs = require("node:fs");
    const path = process.argv[1];
    const version = process.argv[2];
    const value = JSON.parse(fs.readFileSync(path, "utf8"));
    value.version = version;
    if (value.optionalDependencies) {
      for (const dependency of Object.keys(value.optionalDependencies)) {
        value.optionalDependencies[dependency] = version;
      }
    }
    fs.writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
  ' "$manifest" "$version"
done < <(find npm -name package.json -type f | sort)
