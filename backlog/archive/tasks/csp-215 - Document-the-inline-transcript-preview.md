---
id: CSP-215
title: Document the inline transcript preview
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: archived. ADR 0112 renders session previews through the native viewer (ADR 0052), which already parses Claude Code, Codex, and opencode transcripts, so this recent-turns path is superseded. `CSP-585.02` implements the session preview.
<!-- SECTION:NOTES:END -->
