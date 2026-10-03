---
id: CSP-190
title: Make the status bar contextual to the selected row
status: Done
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 437000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: existing `contextual_status_offers_enter_*`,
  `status_hint_for_*_pin_*`, plus four new coverage entries —
  `contextual_status_offers_ambiguous_attach_hint_with_choose_affordance`,
  `contextual_status_for_group_row_advertises_expand_collapse_folding`,
  `status_bar_renders_stale_chip_when_refresh_failure_recorded`,
  `status_bar_renders_provider_error_chip_for_unavailable_tmux`,
  `contextual_status_surfaces_disabled_attach_reason_for_current_tmux_session`.
- Follow-ups: provider/freshness chip *content* (richer wording,
  chip ordering polish, "tmux:off" vs "tmux:unavailable" nuance)
  stays with `CSP-180`; this story closes on the contextual-left-zone
  deliverable and the right-zone wire-up.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed. The status bar's left zone is fully contextual
via `contextual_status_text` / `default_action_status_hint` in
`src/tui/ui.rs`: attachable rows show
`Enter/a attach <mux-display>`, ambiguous rows show
`Enter/a attach preferred <…> · m choose`, un-muxed agent rows
show `Enter/v view <session> · S resume` (or the bare view hint
when the harness has no registered resume), pin rows surface
their per-binding-state hint, and group rows show
`Enter/l expand · h collapse`. Disabled-attach rows fall through
to `attach_disabled_reason` (e.g. "refusing to attach current
tmux session `…`"). The right chip zone renders provider error
chips (`tmux:<reason>` / `gh:<reason>`) and a `stale` chip when
a refresh failure is recorded.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-014`
