---
id: CSP-130
title: Resolve table row identifiers in `conspectus node show`
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 122000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented together with CSP-094. The `node show <id>`
resolver accepts (a) the short content-addressed prefix from the
session table's `ID` column, prefix-matched (floor 4 hex chars),
(b) the full `NodeId` `Display` form, and (c) the harness/mux
label when it uniquely identifies one node. Ambiguous prefixes
error with the matching candidates listed. `docs/operations.md`
documents the accepted forms; CLI integration tests round-trip a
short id from `conspectus session --wide` through `node show`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-005`
