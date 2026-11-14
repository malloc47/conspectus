---
id: CSP-884
title: >-
  Match open transcripts in a moved Claude Code or Codex directory as fd
  evidence
status: To Do
assignee: []
created_date: '2026-11-14 23:33'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 961000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Process-tree attribution reads a harness process's open files and matches them against fixed path fragments in each adapter's `RuntimeSignature::fd_path_patterns` (`/.claude/projects/`, `/.claude/tasks/`, `/.codex/sessions/`, `/.codex/tmp/`). Since CSP-838 and CSP-844, discovery reads Claude Code under `CLAUDE_CONFIG_DIR` and Codex under `CODEX_HOME` (ADR 0239), and `CONSPECTUS_<HARNESS>_STATE` could already move them, but a process holding a transcript in such a directory gives no fd evidence, so a session there loses that signal for session-to-pane binding (hooks, pane commands, and cwd still attribute it). The patterns are `&'static` and `session_key_evidence_from_fd_paths` (`src/discovery/cross_link.rs`) has no access to the configured state roots. Found while doing CSP-838.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A harness process with a transcript open under a moved directory (CLAUDE_CONFIG_DIR, CODEX_HOME, or the Conspectus override) yields the same fd session evidence as one under the home default
- [ ] #2 Tests cover a moved directory for each harness and leave the home-default matching unchanged
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
