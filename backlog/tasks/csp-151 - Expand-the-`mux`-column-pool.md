---
id: CSP-151
title: Expand the `mux` column pool
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 128000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`MUX_COLUMNS` gained three opt-in columns:
`attached-count` (number of attached agent sessions, rendered
as `—` when zero), `activity` (relative recency from
`MuxSessionNode::activity_epoch`, reusing `format_relative_age`
from CSP-148), and `created` (relative age from
`created_epoch`). `panes` stays deferred until the mux adapter
records pane counts. Default set is unchanged. Three unit tests
cover `attached-count` (one row with attached agents, one with
none), `activity`/`created` formatting (using
`format_relative_age` and verifying the recency-suffix shape of
the rendered cell), and the no-epoch fallback to `—`. The
`parse_columns_all_resets_to_every_registered_column` regression
test was updated to reflect the larger `all` set. All 377 tests
pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-011`
