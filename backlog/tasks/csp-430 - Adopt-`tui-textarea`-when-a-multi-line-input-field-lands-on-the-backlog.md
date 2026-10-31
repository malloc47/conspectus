---
id: CSP-430
title: Adopt `ratatui-textarea` when a multi-line input field lands on the backlog
status: To Do
assignee: []
created_date: '2026-06-19 00:59'
updated_date: '2026-10-31 08:15'
labels:
  - h-widg
milestone: m-11
dependencies: []
ordinal: 369000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `widgets/input.rs` shells over `tui-input` for
  single-line entry. Multi-line input is unbuilt today; future
  candidates: pin `reason` notes, richer alias editing,
  PR-comment composer for forge surfaces. `tui-textarea` 0.7
  (MIT, ratatui 0.29+, pure state machine, 96% docs) is the
  canonical drop-in.
- Scope: deferred until a story names the surface that needs
  multi-line. Adopt the moment one does; do not preempt.
- Blockers: a downstream story that demands multi-line input.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-006`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): retargeted from `tui-textarea` to `ratatui-textarea`. `tui-textarea` 0.7 declares `ratatui ^0.29` and does not resolve against the workspace's ratatui 0.30; `ratatui-textarea` (ratatui organization) is the maintained engine (gap analysis, Tier 2; `crates/tuikit/docs/roadmap.md`). `src/tui/widgets/input.rs` is gone: single-line entry is `tuikit::text_edit::LineEdit` over `tui-input`, drawn by `TextField` (ADR 0165). A multi-line area would be a framework control beside `LineEdit` (`crates/tuikit/docs/controls/text_edit.md`, "Seams left open"), so adopting the crate needs a record in the framework's decision log, an entry in its dependency allowlist (`crates/tuikit/tests/independence.rs`), and a host ADR (ADR 0152 item 6). A multi-line paste must still not submit (house rule 15). Still deferred until a story names the surface.
<!-- SECTION:NOTES:END -->
