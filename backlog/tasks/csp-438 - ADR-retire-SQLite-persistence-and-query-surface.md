---
id: CSP-438
title: 'ADR: retire SQLite persistence and query surface'
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 497000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted as ADR 0082. The ADR settles the
architectural pivot — daemon-as-source-of-truth, single
on-disk artifact for daemonless reads, no SQL surface, no
schema migrations, no rotation/backups. Names the cluster
of ADRs it supersedes (0036, 0037, 0039, 0040, 0042, 0043,
0044) and the partial-supersession of ADR 0038 (the
writer-fallback fork goes away; the socket gains a
`snapshot` read command). Implementation lands across the
remaining P11 stories.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-001`
