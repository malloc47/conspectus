---
id: CSP-635
title: Fix inverted parent/child labels on session lineage links
status: Done
assignee:
  - '@claude'
created_date: '2026-10-18 06:41'
updated_date: '2026-10-18 11:57'
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
- [x] #1 A focused child session reads "child of" its parent and a focused parent reads "parent of" its child
- [x] #2 Fork-anchored child_session links read correctly from both ends
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm link orientation across adapters and find every surface that labels lineage direction.
2. Swap the ParentSession/ChildSession verbs in the explorer catalog and ADR 0074's table.
3. Add explorer tests from both ends of session and fork lineage links; check a TUI snapshot.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope check: only the explorer's `directional_verb` catalog (and the ADR 0074 table it was built from) was inverted. `conspectus node show` prints raw relation names with outgoing/incoming counts, the DOT export draws plain source→target edges, and the detail header's lineage field, agent output, and fork tables all treat the `parent_session` target as the parent, so none needed changes.
ADR 0074's catalog table is corrected, with a note on the link orientation it follows.
Verification: new explorer tests build real snapshots and read the rendered verb from each end of a session `parent_session` link and of an Atelier-style fork's `parent_session` and `child_session` links; the catalog anchor test now pins the corrected pairs. A TUI snapshot (right pane) of the sandboxed opencode 2.x subagent parent now reads "parent of ● opencode:<child>", where it read "child of" before. `just check` passes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Swapped the explorer's lineage verbs so a focused child reads "child of" its parent and a focused parent reads "parent of" its child, for session `parent_session` links and fork-anchored `parent_session` / `child_session` links alike, and corrected ADR 0074's catalog table. Verified with explorer tests from both ends of each link and a TUI snapshot of an opencode subagent parent.
<!-- SECTION:FINAL_SUMMARY:END -->
