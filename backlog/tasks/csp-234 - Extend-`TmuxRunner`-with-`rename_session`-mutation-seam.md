---
id: CSP-234
title: Extend `TmuxRunner` with `rename_session` mutation seam
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies: []
ordinal: 283000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: first non-read-only tmux call. Add `rename_session(target,
  new_name) -> TmuxOutcome` to the trait in `src/discovery/tmux/mod.rs`
  with a default impl returning `Unsupported` so future backends (zellij
  per `CSP-111`) don't break. `SystemTmux` runs
  `tmux rename-session -t <native_id> <new_name>`. `FakeTmux` records
  calls for assertion.
- Tests: per-impl tests for success, target-missing, binary-missing, and
  name-collision (tmux rejects duplicates). `FakeTmux` recording
  assertions.
- Blockers: none (parallel to ADRs).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-003`
