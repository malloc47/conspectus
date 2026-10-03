---
id: CSP-054
title: Add the `conspectus session` CLI subcommand
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-049
  - CSP-051
  - CSP-052
  - CSP-053
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `session` to the CLI with a
  `--projection {agent|mux|union}` flag that defaults to the value
  from the loaded config (or `agent` when no config is present). The
  command runs the existing local discovery + resolver and renders the
  chosen projection. Reuse the env-based isolation used by the graph
  command (`HOME`, `CONSPECTUS_DISABLE_TMUX`, future
  `CONSPECTUS_DISABLE_FORGE`).
- Tests: CLI integration tests for the default projection, each
  explicit flag value, invalid values, config-file defaulting, and
  deterministic output across repeated runs.
- Manual checks: `cargo run -- session`,
  `cargo run -- session --projection agent`,
  `cargo run -- session --projection mux`,
  `cargo run -- session --projection union`.
- Blockers: `CSP-049`, `CSP-051`, `CSP-052`, `CSP-053`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the `session` subcommand with an optional
`--projection {agent|mux|union}` flag and `--scan-root` re-using
the graph command's options. Without `--projection`, the CLI
loads `.conspectus.toml` / user config via
`config::ConfigLoader::from_env()` and falls back to `agent`.
Config diagnostics print to stderr but do not abort the run.
Six new CLI smoke tests cover the default projection,
`--projection {agent|mux|union}`, an invalid value, project
config defaulting to union, and deterministic output across
repeated runs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-010`
