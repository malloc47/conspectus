---
id: CSP-211
title: Aider transcript parser for the native viewer (deferred)
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 199000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: ADR 0112 renders session previews and the full-screen transcript through the native viewer, which parses Claude Code, Codex, and opencode but not aider, so aider sessions show "transcript unavailable".
- Deferred for the same reasons as `CSP-159`: aider writes free-form markdown without a stable assistant-turn delimiter, and its input history is user-only and would mislead. Reopen when aider gains a structural marker or a stable fixture corpus exists.
- Blockers: same as `CSP-159`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-007`
