---
id: CSP-233
title: 'ADR: TUI text-input primitive'
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies: []
ordinal: 282000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: resolve ADR 0024's deferred `tui-input` decision now that three
  callers exist (rename, `CSP-193` search overlay, `CSP-175` mux-picker).
  Settle hand-rolled vs crate, locked key semantics (`Enter` confirm,
  `Esc` cancel, `Tab` suspended while overlay is open), overlay
  placement (centered modal, 60-col width cap), and module boundary
  (`src/tui/widgets/input.rs`). Record as ADR 0030.
- Tests: docs-only; `git diff --check`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-002`
