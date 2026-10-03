---
id: CSP-371
title: '`HarnessAdapter::launch_argv` defaults'
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-361
ordinal: 304000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a `launch_argv(&self) -> Vec<OsString>` method to
  `HarnessAdapter`. Default implementations: codex `["codex"]`,
  claude-code `["claude"]`, opencode `["opencode"]`, aider
  `["aider"]`. Override via `pin.launch.argv` flows through the
  launch primitive (CSP-372).
- Tests: per-adapter unit tests for the default; integration test
  that the launch primitive prefers `pin.launch.argv` when set.
- Blockers: none beyond `CSP-361`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Harness adapters expose default launch argv, and launch
flows prefer per-pin argv overrides when configured.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-011`
