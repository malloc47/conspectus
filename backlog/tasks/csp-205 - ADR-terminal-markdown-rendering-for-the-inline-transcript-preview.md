---
id: CSP-205
title: 'ADR: terminal markdown rendering for the inline transcript preview'
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 193000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0051 selects `tui-markdown 0.3.7` with
`default-features = false` (no `syntect`, no second
`ansi-to-tui` path) for the inline transcript preview.
Alternatives evaluated: `termimad` (crossterm-only, requires
bridging) and a roll-your-own renderer over
`pulldown-cmark` (same long-tail surface ADR 0025 rejected
for SGR). Integration target is a new
`src/tui/transcript_preview.rs` per `CSP-213`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TRANSCRIPT-001`
