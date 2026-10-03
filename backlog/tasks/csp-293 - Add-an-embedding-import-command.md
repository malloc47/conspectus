---
id: CSP-293
title: Add an embedding import command
status: To Do
assignee: []
created_date: '2026-05-28 02:52'
labels:
  - p9-fu
milestone: m-14
dependencies: []
ordinal: 471000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement the ADR 0042 import path for external embedding
  pipelines: read JSON Lines from stdin with `node_id`,
  `source_field`, `model`, and `vector` fields, validate dimensions,
  and insert/update rows in the `embeddings` table through the
  writer path. Keep embedding computation out of Conspectus.
- Tests: CLI tests for valid imports, malformed JSON, unknown node
  IDs, mixed dimensions, duplicate replacement, and subsequent
  `--similar-to` results.
- Blockers: none; future server writer routing may refine the
  mutation path.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P9-FU-001`
