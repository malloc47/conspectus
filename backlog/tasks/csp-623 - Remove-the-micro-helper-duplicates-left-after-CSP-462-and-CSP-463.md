---
id: CSP-623
title: Remove the micro-helper duplicates left after CSP-462 and CSP-463
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 681000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two leftovers from the helper dedupe. `src/tui/rows/mux.rs:727` still defines `mux_indicator`, byte-identical to the shared one in `src/tui/rows/mod.rs:677`. CSP-463 kept `rows/mux.rs`'s `agent_row` because it takes a different input type, but the indicator has no such reason. CSP-462 centralized `current_epoch` in `crate::discovery` but kept re-exports in `hook.rs:138`, `discovery/codex_log.rs:84`, `discovery/hook_sidecar.rs:538`, and `output/render.rs:1280` for source compatibility. The `output::render` one has no callers; the other three serve four call sites (`cli/hook.rs:80`, `cli/pin.rs:1278`, `discovery/mod.rs:774`, `discovery/mod.rs:795`). The `conspectus::api` facade (ADR 0015) does not export `current_epoch`, so none of these paths carry a compatibility promise. Audit findings A2 and A4.

- Scope: delete the duplicate `mux_indicator` and the four re-exports, and point the call sites at `crate::discovery::current_epoch`.
- Tests: `just check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `rows/mux.rs` uses the shared `mux_indicator`
- [ ] #2 `current_epoch` is defined and exported only from `crate::discovery`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
