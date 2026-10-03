---
id: CSP-425
title: Adopt `ratatui-macros` for `Span` / `Line` / `Text` / layout boilerplate
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies: []
ordinal: 364000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `ratatui-macros` 0.7 ships `span!` / `line!` /
  `text!` / `constraints!` / `vertical!` / `horizontal!` / `row!`
  macros that retire the `Span::raw / Span::styled / Line::from(
  vec![…]) / Style::default().add_modifier(…)` chains that
  dominate dense renderer code. It is already in `Cargo.lock`
  transitively (via `tui-markdown`); the ratatui 0.30 facade
  re-exports it as `ratatui::macros` behind the `macros` feature
  flag, which the current Conspectus configuration
  (`default-features = false, features = ["crossterm"]`) does
  *not* enable. This is the lowest-risk forcing function for the
  workstream — pure cleanup, zero new direct deps, zero new
  runtime behavior.
- Scope:
    - Add `"macros"` to the ratatui feature list in `Cargo.toml`.
    - Sweep `src/tui/ui.rs` (242 Span/Line/Style call sites in
      6287 LOC), `src/tui/widgets/pins.rs` (56 sites in 1733
      LOC), `src/tui/widgets/search.rs` (24/756),
      `src/tui/widgets/help.rs` (16/598), and
      `src/tui/widgets/controls.rs` (16/1142) — the five files
      carrying ~95% of the renderer boilerplate. Smaller files
      (`badge.rs`, `toast.rs`, `multi_select.rs`,
      `value_modal.rs`, `input.rs`) opportunistic only.
    - Land in reviewable slices: one focused widget file per
      commit so snapshot regeneration is per-file, not a single
      sprawling diff.
    - Where a style combines `Style::default().fg(c)
      .add_modifier(BOLD)`, build the Style outside the macro
      and pass it as `span!(style; "…")` — macro syntax only
      cleanly accepts a single Color/Modifier/Style.
    - Use `line!` / `text!` for static keymaps and chip
      compositions; keep manual `Line::from` for spans built in
      loops where the macro buys nothing.
    - Use `vertical!` / `horizontal!` / `constraints!` where
      layout call sites are dense; the explorer detail pane
      (`src/tui/detail.rs`, 2679 LOC) and `ui.rs` outer panel
      split are the obvious candidates.
- Tests: the existing `cargo nextest run --all-targets
  --all-features` corpus must stay green. Ratatui buffer
  snapshots regenerate where rendering changed; insta should
  show byte-identical output for pure mechanical conversions and
  only meaningful diffs where a style consolidation altered the
  output. Snapshot review is the visual proof that no semantic
  drift slipped in.
- Open questions:
    - Whether the macros should be re-exported under a project
      prelude (`crate::tui::macros::*`) to localize style and
      survive a future ratatui re-org. Recommend yes once two or
      more files import the same macro set.
    - Whether `row!` (Table rows) is worth adopting; Conspectus
      builds tables programmatically via `comfy-table`, not
      ratatui Tables, so probably not.
- Progress (2026-06-19): the five target files in scope are
  fully converted. `widgets/help.rs` (−9 LOC),
  `widgets/controls.rs` (−8 LOC), `widgets/search.rs` (−18 LOC),
  `widgets/pins.rs` (−21 LOC), and `src/tui/ui.rs` (−148 LOC
  across two slices — header / status / breadcrumb first, then
  row renderers + explorer link rows + detail-pane preview
  builders) all use `span!` at every dense site. The 33
  remaining `Span::raw(CONST)` calls in `ui.rs` stay as-is —
  they accept `&'static str` directly while `span!("...")`
  would allocate via `format!`. All 1670 tests stay green per
  slice; snapshots regenerated where rendering changed. Net
  workstream-wide LOC reduction: **−312** across the six
  commits.
- Follow-on landings (2026-07-05):
    - `f022014`: `widgets/badge.rs` + `widgets/value_modal.rs`
      cleaned with `span!` / `line!`. `input.rs` skipped (its
      two `Line::from(some_string.clone())` sites are the same
      no-gain pattern as the ui.rs `Span::raw(CONST)` sites);
      `toast.rs` and `multi_select.rs` no longer carry any
      Span/Line/Text call sites per the audit.
    - `93308b9`: `src/tui/ui.rs` layout hot spots migrated to
      `vertical!` / `horizontal!` — four call sites (outer draw
      split, responsive body panel split with narrow-vertical /
      wide-horizontal branch, and two detail-pane header
      splits). `Constraint` / `Direction` / `Layout` imports
      retire from `ui.rs`. `detail.rs` has no
      `Layout::default()` construction today (uses Frame area
      directly), so its story-scope layout work is a no-op.
- Blockers: none. Land before the other tiers so the new code
  written for swaps lands in the macro idiom from day one.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-001`
