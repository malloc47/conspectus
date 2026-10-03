---
id: CSP-576
title: Pin placeholder rows preview their live mux
status: Done
assignee: []
created_date: '2026-10-01 18:04'
labels:
  - h-pin-fix
milestone: m-11
dependencies: []
ordinal: 344000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: a sessions-view pin placeholder whose pin has a live mux
  (`StaleMux`, e.g. a `conspectus serve` pin, which never realizes
  an agent session) targets that mux for preview capture and `a`;
  other placeholders preview the pin's diagnostics instead of
  repeating the cwd.
- Tests: `tui::ui` stale-mux capture, unbound diagnostics,
  contested-session text.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`resolve_attach_target` maps `RowId::Pin` through
`pin_live_mux`; `preview_text_for_selection` routes pin rows
through `pin_placeholder_preview`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-FIX-003`
