---
id: CSP-580
title: >-
  Preview pane hides quiet panes' output and wraps agent UI decorations (ADR
  0106)
status: Done
assignee: []
created_date: '2026-10-02 19:04'
labels:
  - h-preview-wrap
milestone: m-18
dependencies: []
ordinal: 550000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom: a `conspectus serve` pane previewed as empty because the
  preview kept the capture's last N lines, which were the blank rows
  below the server's output. Agent panes' full-width rules and box
  borders wrapped into extra rows of fragments in the narrower
  preview.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-10-02): the preview lays the capture out into rows
that fit the preview width, drops trailing blank lines, then keeps
the bottom rows that fit (shrinking below a failure banner). Three
`PreviewWrap` modes: smart (the default) truncates
decoration-only overflow, squeezes padding, and word-wraps content
with a hanging indent; plain wraps everything; none re-wraps at the
pane width that `capture_pane` now reports via `#{pane_width}` and
clips. The mode is set with `[tui] preview_wrap`, a "Preview wrap"
section in the controls overlay, and is persisted in
`tui-state.json`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-WRAP-001`
