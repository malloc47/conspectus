---
id: CSP-178
title: >-
  Collapse duplicate repo group rows when a project appears across multiple
  checkout buckets in the TUI sessions row tree
status: Done
assignee: []
created_date: '2026-05-19 19:33'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 406000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`emit_checkout_bucket` now tracks the most recent
workspace + repo keys and skips re-emitting headers when
they're unchanged across adjacent checkout buckets (buckets
are already ordered by the BTreeMap so siblings are
adjacent). The existing
`two_checkouts_in_same_repo_show_checkout_level` test now
asserts exactly one repo row.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-001`
