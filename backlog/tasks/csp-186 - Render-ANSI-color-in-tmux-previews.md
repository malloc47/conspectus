---
id: CSP-186
title: Render ANSI color in tmux previews
status: Done
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 412000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`SystemTmux::capture_pane` now passes `-e` so tmux
emits the pane's escape sequences alongside the visible text.
The right-panel preview consumes those via `ansi-to-tui`
(ADR 0025) and renders them as styled `Text<'static>` —
agent output keeps the colours operators see in the source
pane. `--color=never` flattens the parsed text back to plain
via `Text::to_string`, and malformed escape bytes fall back
to a `Text::raw` so a single bad byte doesn't lose pane
content. Three unit tests cover the colour, no-colour, and
malformed-input paths.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-010`
