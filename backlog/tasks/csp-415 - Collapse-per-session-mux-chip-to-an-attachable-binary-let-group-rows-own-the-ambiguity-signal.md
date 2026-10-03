---
id: CSP-415
title: >-
  Collapse per-session mux chip to an attachable-binary; let group rows own the
  ambiguity signal
status: Done
assignee: []
created_date: '2026-06-17 15:15'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 355000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed under ADR 0072
(`docs/adr/0072-mux-indicator-attachable-binary.md`). The
row chip now reads `◉` only when a single definitive
`LinkedToMux` candidate exists; `Ambiguous { .. }` and
`Unmuxed` both render as `◯`. Group rows drop the
per-bucket `◉ a ◐ b ◯ c` summary in favor of `(N total)`
plus a single `⚠` (theme `warning`) when any descendant
session is in the `Ambiguous` state. Filter modal keeps
three buckets; `MuxIndicator::Ambiguous { candidate_count }`
stays on the model so status-bar hints, header counts, and
the ADR 0071 group-detail catalog continue to work.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-001`
