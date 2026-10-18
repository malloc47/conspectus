---
id: CSP-635
title: Fix inverted parent/child labels on session lineage links
status: To Do
assignee: []
created_date: '2026-10-18 06:41'
labels:
  - h-hyg
milestone: m-11
dependencies: []
priority: medium
type: bug
ordinal: 693000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every adapter emits lineage as `child --parent_session--> parent` (`src/discovery/harness/{claude_code,codex,opencode,pi}.rs`), and Atelier emits `fork --child_session--> child`. The explorer's `directional_verb` (`src/tui/explorer.rs:345-348`) reads `<focused> <verb> <neighbor>` with Downstream meaning the focused node is the source, yet maps `(ParentSession, Downstream)` to "parent of" and `(ChildSession, Downstream)` to "child of". A focused parent session therefore reads "child of <its child>" and a child reads "parent of <its parent>". `src/tui/explorer_tests.rs:23-24` pins the inverted values. Seen on an opencode 2.x subagent parent while checking CSP-604.

- Scope: swap the four ParentSession/ChildSession verbs and the test's expectations; check the CLI `node show` and graph exports for the same inversion.
- Tests: explorer tests for a child focused on a `parent_session` link ("child of"), its parent ("parent of"), and an Atelier fork's `child_session` link.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A focused child session reads "child of" its parent and a focused parent reads "parent of" its child
- [ ] #2 Fork-anchored child_session links read correctly from both ends
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
