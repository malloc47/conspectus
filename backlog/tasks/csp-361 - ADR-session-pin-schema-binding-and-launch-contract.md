---
id: CSP-361
title: 'ADR: session pin schema, binding, and launch contract'
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies: []
ordinal: 294000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: record the schema (`[[pins.entries]]` TOML sibling to
  `[declared]` and `[aliases]`), the mux-anchored binding rules,
  cwd as launch-parameter-not-discriminator, the four pin-specific
  diagnostics (`PinUnbound` / `PinStaleMux` / `PinAmbiguous` /
  `PinDrift` plus `PinDuplicate` safety net), launch via
  `TmuxRunner::new_session` + exec-replace attach, lockstep rename
  contract, operator escape hatches (`bind` / `rebind` / `adopt`),
  optional `mux.socket_name` for tmux `-L`, identity encoding
  `tmux:<name>` for default socket and `tmux:<socket>:<name>` for
  non-default, prior art comparison, and the hooks-future-proofing
  guarantee.
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0057 captures the decision and is promoted to
Accepted now that the v1 schema, resolver, CLI, launch, TUI, and
closeout slices are in place.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-001`
