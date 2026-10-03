---
id: CSP-231
title: Add `conspectus hook init` installer UX
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-230
ordinal: 265000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add an idempotent hook installer for supported harnesses,
  starting with Claude Code. It should merge with existing harness
  settings, preserve unrelated user hooks, install a hook command
  that invokes `conspectus hook write`, and support dry-run/status/
  remove flows before mutating external config. Candidate commands:
  `conspectus hook init claude-code --scope user|project`,
  `conspectus hook status claude-code`, and
  `conspectus hook remove claude-code`. Treat this as a write to
  external tool configuration, distinct from read-only discovery.
- Tests: fixture-backed settings merge tests for empty settings,
  existing unrelated hooks, existing Conspectus hook, malformed
  settings, dry-run output, status detection, and removal without
  deleting unrelated entries.
- Manual checks: install into a temporary Claude Code settings file,
  run a hook-enabled session, verify sidecar emission, then remove
  and confirm the settings file returns to the expected state.
- Blockers: `CSP-230`; ADR/design update if the command mutates
  any persistent convention not already covered by ADR 0028.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `conspectus hook init/status/remove claude-code`
with user/project scope support. The installer merges with existing
Claude settings, preserves unrelated hooks, and installs a command
that invokes `conspectus hook write claude-code`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-017` (the 2026-05-23 story; the ID was used twice)
