---
id: CSP-436
title: >-
  Opportunistic `ratatui-macros` sweep across the remaining small widgets and
  the layout helpers
status: Done
assignee: []
created_date: '2026-06-19 14:45'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-426
  - CSP-427
  - CSP-428
ordinal: 375000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Absorbed 2026-07-05 into the CSP-425 closure** (`f022014`
  + `93308b9`). Scope was byte-identical to CSP-425's parked
  remainders:
    - Small widget sweep — `badge.rs` and `value_modal.rs`
      swept; `input.rs` skipped per the no-gain convention;
      `toast.rs` retired entirely by CSP-427; `multi_select.rs`
      retired by CSP-426 (both landed pre-2026-07-05).
    - Layout-macro sweep — `ui.rs`'s four `Layout::default()`
      chains migrated to `vertical!` / `horizontal!`;
      `detail.rs` audit confirmed no `Layout::default()`
      construction (uses Frame area directly).
  Retained here as `[x]` rather than deleted so the story ID
  stays discoverable; canonical writeup lives on CSP-425.
- Motivation: `CSP-425` closed the five files in the critical
  path (help / controls / search / pins / ui — `−204 LOC` net),
  but left two opportunistic remainders parked: the five small
  widgets that carry a handful of `Span`/`Line`/`Style` sites
  each, and the layout helpers that build `Layout::default()
  .constraints(...)` arrays imperatively. Filing them as a
  bundled follow-on so the workstream does not lose track of
  them while CSP-426..433 progress.
- Scope:
    - Small widget sweep (target: 1 commit, ~15 LOC delta):
      - `src/tui/widgets/badge.rs` (2 sites)
      - `src/tui/widgets/toast.rs` (2 sites — but this file
        retires entirely if `CSP-427`'s `ratatui-toaster`
        swap lands first; skip if so)
      - `src/tui/widgets/multi_select.rs` (the shim from
        `CSP-426` already uses the macros in its bordered
        modal frame — no remaining sites)
      - `src/tui/widgets/value_modal.rs` (5 sites — same
        retirement caveat under `CSP-428`'s `tui-popup`
        swap)
      - `src/tui/widgets/input.rs` (3 sites)
    - Layout-macro sweep (target: 1 commit):
      - Apply `vertical!` / `horizontal!` / `constraints!` to
        the obvious hot spots: `ui.rs`'s outer header / left /
        right / status split (the `Layout::default()
        .direction(...).constraints(...)` chains around the
        frame), and `src/tui/detail.rs`'s detail-pane vertical
        stack. Skip layouts that build their constraint arrays
        conditionally; the macros only help when the slice is
        literal.
- Tests: existing `cargo nextest run --all-targets
  --all-features` corpus stays green per slice; snapshot
  regeneration as needed.
- Open questions:
    - Should the workstream wait for the Tier A swap stories
      (`CSP-426` / `CSP-427` / `CSP-428`) to land before sweeping the
      small widgets whose file might retire? Recommend yes for
      `toast.rs`, `multi_select.rs`, `value_modal.rs` — the
      retirement absorbs the macro work. `badge.rs` and
      `input.rs` are safe to sweep anytime.
    - Whether the layout macros warrant a separate prelude
      re-export alongside `span!` / `line!`. Decide alongside
      the prelude question still open on `CSP-425`.
- Blockers: ideally lands *after* Tier A swap decisions on
  `CSP-426` / `CSP-427` / `CSP-428` so the file-retirement caveats
  resolve cleanly. `badge.rs` and `input.rs` slices unblocked
  today.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-012`
