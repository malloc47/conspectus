---
id: CSP-278
title: Vector search via `sqlite-vec` (deferred)
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies: []
ordinal: 470000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: settle embedding sources, dim budget, ingestion lifecycle,
  and `sqlite-vec` integration. Add an embedding overlay table,
  expose `conspectus query --similar-to <node-id>`, and keep actual
  embedding computation outside Conspectus.
- Blockers: ADR-G, now accepted as ADR 0042.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted ADR 0042. The schema now includes an
`embeddings` overlay table, the query runner can load a SQLite
extension with `--load-extension`, and `conspectus query
--similar-to <node-id>` performs a built-in cosine-distance
nearest-neighbor scan over imported embedding blobs. Runtime
distribution of `sqlite-vec` and embedding ingestion remain
follow-ups rather than normal discovery behavior.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-008`
