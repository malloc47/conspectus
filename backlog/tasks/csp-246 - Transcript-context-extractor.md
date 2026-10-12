---
id: CSP-246
title: Transcript context extractor
status: To Do
assignee: []
created_date: '2026-05-23 02:59'
updated_date: '2026-10-12 13:12'
labels:
  - h-ai-naming
milestone: m-11
dependencies:
  - CSP-245
ordinal: 348000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: reuse extractors from the `H-TRANSCRIPT-*` workstream
  (currently 0/12). Coordination dependency: this story either waits
  on `CSP-207` (recent-history adapter API) and the per-
  harness extractors, or pulls them forward.
- Tests: per-harness fixture tests showing extracted context is bounded
  and transcript-stable.
- Blockers: `CSP-245`, `CSP-207`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AI-NAMING-002`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: `CSP-207` (recent-history adapter API) is archived by ADR 0112. Take transcript context from the native viewer's parser in `src/viewer/parser/` (Claude Code, Codex, opencode) instead.
<!-- SECTION:NOTES:END -->
