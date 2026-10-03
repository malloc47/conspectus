---
id: CSP-215
title: Document the inline transcript preview
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies:
  - CSP-214
ordinal: 203000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` and the Phase 8
  implementation doc with the new preview behavior,
  `--no-live-preview` semantics for transcript reads, the
  privacy posture (transcript text never leaves the local
  process), and the supported harnesses. Note aider's
  deferred status.
- Tests: `git diff --check`.
- Blockers: `CSP-214`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-011`
