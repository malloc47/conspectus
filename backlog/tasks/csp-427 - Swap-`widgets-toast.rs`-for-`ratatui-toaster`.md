---
id: CSP-427
title: Swap `widgets/toast.rs` for `ratatui-toaster`
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
ordinal: 366000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: in-tree toast surface is 235 LOC carrying
  info/success/warning/error variants, positioning, and a small
  engine. `ratatui-toaster` 0.1.3 (Unlicense OR MIT, ratatui
  0.30) covers the same shape with a builder API. Low risk;
  small surface to bridge.
- Story-vs-reality calibration: the motivation paragraph above
  overstated the in-tree surface. The actual in-tree was
  ~80 LOC of code (rest tests) carrying **one variant**
  (success), **one position** (bottom-centered), and a polled
  `is_expired()` field on `ToastState`. The "info/success/
  warning/error variants and positioning" framing came from
  looking at the upstream we were swapping *to*, not the source
  we were replacing. Recording the gap so future Tier A
  motivations are written from the in-tree side.
- Known limitation: hardcoded `Success → Color::Green` mapping
  upstream means operators who override `[tui.theme] success`
  will see the toast stay green while other "success" surfaces
  honor the override. Conspectus's `theme.success` default is
  `Color::Green` so the default-palette visual is byte-
  identical. Acceptable for a 1.5-second blip; an upstream
  `border_fg: Option<Color>` override would close the gap and
  is the natural follow-up (PR-worthy or fork-and-replace).
- Tests: 5 shim unit tests (no-toast, show+tick clears,
  immediately visible, dismiss_all, paints borders/label) +
  1 App-level replacement test rewritten for the queue model
  (`queue_len()` + `current_message()` instead of `posted_at`
  ordering). Net test count delta across the swap: −3.
- Blockers: `CSP-425`. [met]
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-19): landed as `aa1adeb` after a spike that
eliminated `ratatui-toaster` 0.1.3 in favor of
`ratatui-comfy-toaster` 0.4.2. The Open Question's
"recommend toaster first" path failed at first contact: both
crates hardcode the border color via `From<ToastType> for
Color`, but `ratatui-toaster` additionally only supports
`Borders::LEFT|RIGHT` (vs Conspectus's `Borders::ALL`), has
no per-toast `expires_at` without the tokio feature, and uses
a different border glyph set. `ratatui-comfy-toaster`
exposes `ToastBorderMode::Full`, per-toast `expires_at`, a
`tick()` retirement method, and `ToastPosition::Center` +
`offset(0, i16::MAX)` to approximate bottom-anchoring via
the upstream's offset clamp. Net source delta: `+237 / -214`
across 7 files; +1 direct dep.
The App's prior `Option<ToastState>` field becomes
`ToastEngine<()>` wrapped in a small `ToastEngineHolder`
newtype so `#[derive(Debug)]` on `App` still derives. The
runtime calls `prepare_toast_for_render(area)` before each
`terminal.draw` so `set_area` follows the frame on resize
and `tick` retires expired toasts. `engine_dismiss_all`
runs before each `show_toast` to preserve the in-tree
"newer toast replaces older" semantic. Snapshot mode also
calls `prepare_toast_for_render` for parity.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WIDG-003`
