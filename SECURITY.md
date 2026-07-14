# Security policy

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting feature for `Lebyy/whycache`. Do not open a public issue for a vulnerability or include secrets and private build metadata in a report.

Include the affected version, operating system, reproduction steps, impact, and any suggested mitigation. You should receive an acknowledgement within seven days. A coordinated disclosure date will be agreed after validation.

Only the newest released minor version receives security fixes before 1.0. After 1.0, the newest major version is supported.

## Security boundaries

WhyCache parses untrusted JSON with Serde, reads repository-local files, and can invoke local `turbo` and `git` executables. It never invokes a shell. Paths and arguments are passed directly to child processes. Reports should still be treated as build metadata and reviewed before public sharing.
