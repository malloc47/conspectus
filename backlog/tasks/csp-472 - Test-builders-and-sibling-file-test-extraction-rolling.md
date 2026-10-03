---
id: CSP-472
title: Test builders and sibling-file test extraction (rolling)
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 103000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Wave 1 landed 2026-07-04**. Ships `AgentSessionNode::new(id,
  harness_key)` + `with_cwd` / `with_title` /
  `with_last_message_preview` / `with_last_active_epoch` /
  `with_session_kind`; `MuxSessionNode::new(id, backend, native_id)`
  + `with_cwd` / `with_active_pane_command` / `with_active_pane_pid`
  / `with_active_pane_current_path` / `with_active_pane_start_command`
  / `with_client_attached` / `with_activity_epoch` / `with_created_epoch`.
  Mirrors the `RepoNode::new` builder shape called out in the story.
- **Rolling migration policy**: existing 83 `AgentSessionNode { … }`
  / 53 `MuxSessionNode { … }` struct literals migrate
  opportunistically per file touched (per the story's explicit
  "not a big-bang" scope). Every future H-HYG / H-EXT / H-TUI wave
  that touches a file with an inline struct literal migrates that
  file's literals as a drive-by.
- **Struct-literal → builder migration (`15f7630`)**: rewrote
  99 `AgentSessionNode { … }` + `MuxSessionNode { … }`
  literals to `Node::new(…).with_*(…)` builder chains across
  24 files. Cases with complex `Some(<expr>)` shapes were left
  intact per the story's "opportunistic" scope.
- **Sibling-file `tests.rs` extraction (rolling wave, 2026-07-05)**:
  landed across every file with ≥100 test lines. Fully
  systematic pass. Files extracted (65 pairs, ~43,000 lines
  of tests moved):
  * Big-3 (CSP-470): `tui/ui.rs` (−3,378),
    `widgets/pins.rs` (−1,496), `output/table.rs` (−2,675).
  * Mid-size: `tui/app.rs` (−3,159), `tui/rows/sessions.rs`
    (−3,002), `discovery/cross_link.rs` (−1,798),
    `resolve/mod.rs` (−1,833), `tui/runtime.rs` (−1,442),
    `tui/explorer.rs` (−1,337), `tui/detail.rs` (−1,306),
    `cli.rs` (−750).
  * Discovery / harness: `discovery/harness/codex.rs`
    (−970), `discovery/harness/claude_code.rs` (−864),
    `discovery/harness/opencode.rs` (−796),
    `discovery/hook_sidecar.rs` (−921), `config.rs`
    (−924), `pins.rs` (−692), `declared.rs` (−684),
    `pin_bindings.rs` (−671), `tui/rows/mux.rs` (−950).
  * Small + parser + widget: 27 additional file pairs.
  Every extraction preserves in-place tests via
  `#[path = "…_tests.rs"] mod tests;`. All 25 test suites
  pass byte-identically after each landing.
  Remaining files with `mod tests` under 100 lines are
  small enough that inline tests don't harm readability;
  the rolling policy stays open — future waves that touch
  those files can extract as a drive-by.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-011`
