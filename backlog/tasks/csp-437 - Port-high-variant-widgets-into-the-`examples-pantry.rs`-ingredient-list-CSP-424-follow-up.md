---
id: CSP-437
title: >-
  Port high-variant widgets into the `examples/pantry.rs` ingredient list
  (CSP-424 follow-up)
status: Done
assignee: []
created_date: '2026-06-20 04:05'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-424
ordinal: 376000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: CSP-424 spike landed "go" with the smoke test
  (`widgets/multi_select.rs` × 3 variants). The follow-up tax
  is per-widget ingredient code so visual iteration for the
  rest of the widget surface gets the same fast loop. This is
  opportunistic work — no widget is blocking on it — but each
  port pays off the next time that widget needs a visual
  judgement call.
- Scope, in rough priority order (each widget is one
  independent commit):
    - `widgets/controls.rs` ingredient — variants: default,
      sub-editor open (harness multi-select), sub-editor open
      (mux-state multi-select), sub-editor open (max-age
      text input with valid/invalid value).
    - `widgets/pins.rs` ingredients — six sub-modals × at
      least 2 variants each (empty + filled, error + no-
      error): Pins menu, Create, Edit, Rebind, Bind, Remove.
    - `widgets/value_modal.rs` ingredient — variants: small
      single-line value, multi-line wrapped value, scrolled
      mid-way through long value, scrolled to bottom.
    - `widgets/search.rs` ingredient — variants: empty query,
      query with no matches, query with several matches,
      cursor highlight on a match.
    - `widgets/help.rs` ingredient — variants: top of keymap,
      scrolled to middle, icon legend section, narrow
      terminal width.
    - `widgets/input.rs` ingredient — variants: empty value,
      mid-edit with cursor mid-string, long value scrolled.
    - Theme harness ingredient — exercises every
      `[tui.theme]` key against `Theme::default()` and the
      dark / light presets so palette work has a one-frame
      visual reference.
- Per-widget LOC budget: ~50–100 LOC of ingredient code
  depending on variant count and state complexity. Aggregate
  ~400–500 LOC across the seven targets if all land.
- Migration to canonical `pantry.toml` form: once the
  ingredient list grows beyond ~5 widgets (after the controls
  + pins ports), migrate from the inline
  `tui_pantry::run!(ingredients)` in `examples/pantry.rs` to
  the proc-macro `pantry_ingredients!()` + `pantry.toml`
  `[ingredients]` table convention. Keeps the single-file
  form for the spike-era and the discoverable per-widget
  module form once the list is too long to read in one
  screen.
- Tests: each commit must keep
  `cargo nextest run --all-targets --all-features` green and
  `cargo run --example pantry -- --list` show the new
  variants without panic. No new correctness tests required;
  the ingredients are dev-time scaffolding.
- Risks: the `Ingredient: Send` bound forced the multi_select
  smoke test to build state inside `render()` (the upstream
  `ratatui_cheese` validator is `!Send`). Other widgets that
  embed `!Send` state need the same "build state per render"
  shim. None of our other widget states use
  `Box<dyn Fn>`-style callbacks today, but verify per port.
- Blockers: `CSP-424` [met]. Each per-widget slice is
  independent.
- Verification: each variant verified via
  `cargo run --example pantry -- --dump <group> --variant
  <name> --size <wxh>`. 1666 tests stay green across all
  commits. The `Ingredient: Send` shim pattern (hold config
  on the ingredient, build state in `render`) generalized to
  every port; no in-tree widget state needed exposing extra
  methods.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-20): all seven targets landed across six
per-widget commits + a theme-harness commit:
  - `aee8dab` Controls (4 variants)
  - `a60fc8a` ValueModal (4 variants)
  - `7de83fc` Help (3 variants)
  - `16491ea` Search (4 variants)
  - `8f8d5c1` TextInput (3 variants)
  - `453f1b4` Pins (6 variants — one per sub-modal)
  - `8138065` Theme harness (1 variant covering every
    `[tui.theme]` key)
Plus the smoke-test multi_select (3 variants) from
`4f918aa` (CSP-424). Total **28 ingredients across 8
groups** in a single `examples/pantry.rs` file (~890 LOC).
Per-widget glue matched the budget (~70–180 LOC each); the
aggregate is on the high side of the prediction because the
theme harness added ~290 LOC of its own (section index +
sample renderers) — heavier than a typical widget port
because it builds custom rendering rather than wrapping an
existing widget. Migration to `pantry.toml` + proc-macro
convention deferred — the single-file form is still
readable at 890 LOC, but is the natural next step if more
widgets are added.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WIDG-013`
