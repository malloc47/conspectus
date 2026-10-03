---
id: CSP-066
title: Define the declared-link CLI surface
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-060
  - CSP-065
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add the CLI command structure and help text for listing,
  creating, removing, confirming, ignoring, and overriding declared
  links without implementing every mutation path. Choose stable flag
  names for source endpoint, relation, target endpoint, reason, and
  store override if needed.
- Tests: CLI smoke tests for `--help`, invalid relation names,
  invalid endpoint syntax, missing required arguments, and no-op list
  output against empty stores.
- Manual checks: `cargo run -- --help` and declared-link subcommand
  help output.
- Blockers: `CSP-060`, `CSP-065`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the `conspectus declared` command group with
`list`, `create`, `remove`, `confirm`, `ignore`, and `override`
subcommands; relation validation uses the existing snake_case graph
relation names, endpoint validation accepts `type:key=value,...`
values using declared TOML field names, and empty `declared list`
succeeds without producing output.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-007`
