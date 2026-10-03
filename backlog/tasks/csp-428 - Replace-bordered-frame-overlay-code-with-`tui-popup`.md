---
id: CSP-428
title: Replace bordered-frame overlay code with `tui-popup`
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
ordinal: 367000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: seven overlays (rename, controls, pins, search,
  help, value, viewer) each carry their own centered-bordered-
  box framing math. The framing layer alone is ~400–600 LOC of
  near-duplication across the widget files. `tui-popup` 0.7.6
  (MIT/Apache, ratatui 0.30) is a `Popup` widget that handles
  auto-sizing, centering, borders, and optional reposition via
  `PopupState`. Logic state machines stay in-tree; only the
  bordered frame leaves.
- Story-vs-reality calibration: the motivation paragraph above
  listed "seven overlays" but the actual count is **nine
  popup-shape surfaces** once you count the five pin sub-editors
  (Pins menu + Create + Edit + Rebind + Bind + Remove) and the
  multi-select sub-editor's bordered frame. The viewer modal is
  not popup-shaped (full-screen reducer surface) and stays
  out of scope. Also: tui-popup's `PopupState.area` is
  `pub(crate)` — there's no public API to pre-set the popup
  rect, so the adoption shape is per-modal body wrappers that
  report `width / height = cap - 2` via the `KnownSize` trait,
  leaving the upstream auto-sizing to reproduce the in-tree
  `centered_modal_rect` output. That's identical to our
  formula (`area.centered(Length(body.width + border_w), ...)`).
- Followups: drag-to-reposition is not enabled (the workstream
  chose not to expose it — overlays stay where they spawn).
  The `PinsOverlayWidget` constructor now requires a `&Theme`,
  plumbed through `app.theme()` at the ui.rs call site; the
  five pin sub-editor widgets gained a `theme` field plumbed
  through `render_sub_editor`. `ControlsOverlayWidget`'s
  sub-editor dispatch passes the theme to `TextInputWidget` via
  the new `.theme()` builder.
- Tests: 1666 tests pass byte-identical across all 9 conversions
  (no snapshot regeneration was triggered since the popup
  auto-sizing reproduces the legacy rect bit-for-bit).
- Blockers: `CSP-425`. [met]
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-19): landed as `40c6f83`. Introduces
`widgets/popup_frame.rs` with `themed_popup(body, title,
&Theme)` + a reusable `LinesBody` wrapper for the common
Vec<Line>-as-Paragraph case. Every centered-bordered-modal in
the TUI now routes through `tui_popup::Popup`. Theme glue
baked in per the Tier A policy: `border_style(theme.
panel_focus_accent)` so every modal border picks up the
project accent (visible cyan in the default palette).
`TextInputWidget` and `MultiSelectWidget` keep a
`.theme()`-less fallback for call sites that don't have a
theme handy.
Net source delta: `+423 / -245` (LOC-positive because the
per-modal wrappers (~10 LOC each × 9 modals) approximately
cancel the framing dedup, exactly as predicted in the pre-
swap finding). The real value is centralized framing through
a single primitive + automatic theme glue + reusable
`LinesBody` for any future modal that's just text.
Visual verification via
`cargo run --features snapshot -- tui --snapshot
--snapshot-keys '?'` (help) and `... 'p'` (pins) confirmed
cyan-themed borders render and body content is unchanged.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WIDG-004`
