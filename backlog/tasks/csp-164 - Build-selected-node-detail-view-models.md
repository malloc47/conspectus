---
id: CSP-164
title: Build selected-node detail view-models
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 391000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/tui/detail.rs` exposes `NodeDetail` with
per-kind `header_fields`, candidate-link summaries (outgoing +
incoming), resolved relationships, and diagnostics — same
content categories as `render_node_show` but as plain data.
Agent-session header rows are the locked five (harness, cwd,
title-when-set, mux, pr, lineage); the mux row carries the
ambiguous-candidate count + `⚠` annotation, and the pr row
walks checkout → branch → PR in the resolved graph to surface
the immediate-stage label. Mux/PR/fork detail will gain richer
fields as the enrichment stories land.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-005`
