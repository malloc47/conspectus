---
id: CSP-822
title: Record real payloads and verify live state against each harness
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
labels:
  - live-state
  - ls-docs
  - needs-operator
milestone: m-26
dependencies:
  - CSP-811
  - CSP-815
  - CSP-816
  - CSP-817
documentation:
  - docs/plans/live-state/research/01-claude-code.md
  - docs/plans/live-state/research/02-codex.md
  - docs/plans/live-state/research/03-opencode.md
  - docs/plans/live-state/research/04-pi.md
priority: medium
type: task
ordinal: 908000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 fixtures are synthetic, built from documentation, and every research note lists claims it could not verify: whether `PermissionRequest` fires for questions, what `SessionEnd` does on SIGHUP, the order of compaction events, whether Codex hooks run silently until trusted, whether OpenCode 2.x emits `session.status`, how long Pi's `session_shutdown` takes on `kill-pane`. Until a real session confirms them, the reducers encode guesses.

Scope: This story needs a real session of each harness, so it is the operator's call when to run it (it uses their accounts). Build a redacting recorder (every non-enum string replaced by a placeholder) that runs a harness in a throwaway project and private tmux server with a settings override, so the operator's own settings are never touched. Run the manual checks listed in each note, record the payload shapes as fixtures, replace the synthetic fixtures, fix any reducer that a real sequence contradicts, and update each note's UNVERIFIED appendix.

TUI changes: none.

Tests and checks: Each note's manual-check list is answered in the final summary; the sequence tests run against recorded payloads.

Blockers: The four harness stories, and the operator running real sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every manual check in the four harness notes is answered or explicitly deferred
- [ ] #2 Recorded, redacted payloads replace the synthetic fixtures where they differ
- [ ] #3 Reducers match the recorded sequences
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
