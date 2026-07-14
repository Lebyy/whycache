#!/usr/bin/env node

"use strict";

const { spawnSync } = require("node:child_process");

const targets = {
  "darwin-arm64": "whycache-darwin-arm64",
  "darwin-x64": "whycache-darwin-x64",
  "linux-arm64": "whycache-linux-arm64",
  "linux-x64": "whycache-linux-x64",
  "win32-x64": "whycache-win32-x64"
};

const target = `${process.platform}-${process.arch}`;
const packageName = targets[target];

if (!packageName) {
  console.error(`whycache does not provide a binary for ${target}.`);
  process.exit(1);
}

let binary;
try {
  binary = require.resolve(`${packageName}/bin/whycache${process.platform === "win32" ? ".exe" : ""}`);
} catch {
  console.error(
    `The ${packageName} optional package is missing. Reinstall whycache without disabling optional dependencies.`
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`Could not start whycache: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
