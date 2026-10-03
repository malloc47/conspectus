---
id: CSP-176
title: >-
  Surface session `title` in the sessions row tree when it uniquely
  distinguishes siblings
status: Done
assignee: []
created_date: '2026-05-19 19:05'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 404000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: four new row-tree unit tests in
  `src/tui/rows/sessions.rs` covering each backlog case —
  `title_disambiguation_off_for_single_session_per_harness`,
  `title_disambiguation_on_for_same_harness_siblings_with_distinct_titles`,
  `title_disambiguation_only_flags_the_session_with_a_title`,
  `title_disambiguation_flips_deterministically_on_refresh_without_reordering`.
  Existing renderer test
  `session_display_label_is_truncated_in_left_row` flipped its
  fixture to `title_disambiguates: true` to keep the width-cap
  assertion exercised.
- Follow-ups: none — the canonical title surface remains the
  right-pane header, and the alias-overlay precedence in
  `display_label` is preserved.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped. `AgentSessionRow` gained a
`title_disambiguates: bool` field plus a new `tree_label()`
helper that returns `alias > (title if title_disambiguates) >
None`. The sessions row-tree builder
(`src/tui/rows/sessions.rs`) computes the flag per project-group
bucket via `title_disambiguating_sessions`: a session qualifies
only when its `title` is non-empty *and* the bucket holds at
least one other session sharing the same rendered harness label.
Lineage children and the cross-view rows (mux, prs, forks,
union) all pass `false` since the disambiguation rule is
sessions-view bucket-scoped.

The left-tree renderer
(`render_session_spans` in `src/tui/ui.rs`) consults
`tree_label()` instead of `display_label()`, so titles only
surface in row labels when they disambiguate. Right pane,
search index (`search.rs`), status hints, and the pin-create
flow keep using `display_label()` so the title is never hidden
from surfaces where it carries diagnostic value. Behavior is
deterministic across refreshes: the flag is derived from the
bucket membership at build time, never re-keys the sort, and
flips on cleanly the moment a sibling appears.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-015`
