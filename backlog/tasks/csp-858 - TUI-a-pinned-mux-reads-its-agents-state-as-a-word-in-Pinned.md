---
id: CSP-858
title: 'TUI: a pinned mux reads its agents state as a word in Pinned'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 22:42'
updated_date: '2026-11-14 14:47'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 943000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In Pinned a running session pin reads `waiting`, `working`, `ready`, or `error` (ADR 0209 decision 4), but a mux pin keeps `live`, which the filled pin marker already says. ADR 0209 left a rolled-up word for a mux pin to the mux rollup work; CSP-824 carried out the operator's decision 2 for mux rows and group headers only, which does not cover Pinned words, and decision 7 (CSP-829) names session words. Whether a mux pin reads its agents' rolled-up state (by the same order as its row's mark) is a display choice for the operator.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Operator-approved 2026-11-13: in Pinned, a pinned mux with agents in live states reads the word of its rolled-up mark (waiting, error, ready, working, by the precedence of its Mux-view mark) in place of live, in that mark's style
- [x] #2 A mux with no agent state (none counted, only unknown, or only exited) keeps live
- [x] #3 One mux shows one state in its Mux-view mark and its Pinned word; the consistency suite checks it
- [x] #4 docs/tui-columns.md (state words), operations.md, CHANGELOG, and an ADR amending ADR 0209 decision 4 describe it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. columns.rs live_word_sign: a running mux row stands for LiveSign::of_mux, so its word (and quiet/unseen style) is the mark's, by the same mapping as a session's.
2. Tests first: consistency suite's expected() reads a mux pin's word from the mux's dominant state, plus a test that sets a mux's Mux-view mark beside its Pinned word for every state; unit test of the drawn word and style.
3. Frames before/after at 170 and 70 columns; just tui-columns regenerates the state-word frames.
4. ADR amending ADR 0209 decision 4; docs/tui-columns.md state words; operations.md; CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision 2026-11-13: approved. The word is the one LiveSign::of_mux draws, so it follows the mux row's mark exactly, including the dimmed and bold styles of ADRs 0233 and 0214; a mux whose mark is `?` or `·` (or has no agent state) keeps `live`. Frames at 170 and 70 columns: only the Pinned view changes (`project-main` reads `working`, dim green, where it read `live`); docs/tui-columns.md frames regenerated with `just tui-columns` (four lines). Validation: consistency suite extended (a mux pin's expected word comes from the model's counts; a test sets the mux's agents through every state and compares the Mux-view mark with the Pinned word; a style test); `just check` green (5746 passed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
In Pinned (and History), a running mux pin whose agents' mark is `!`, `×`, `›`, or `»` now reads `waiting`, `error`, `ready`, or `working` in place of `live`, in that mark's style (ADR 0234, amending ADR 0209 decision 4); a mux with no agent state, or only can't-tell or exited agents, keeps `live`. `live_word_sign` in `src/tui/ui/left_panel/columns.rs` now also takes a mux row's `LiveSign::of_mux`, the same sign that draws the Mux view's mark, so the two cannot disagree and the word is dimmed or bold as the mark is. The consistency suite derives each mux pin's expected word from the model's counts, and new tests set a mux's agents through every state and compare its Mux-view mark with its Pinned word, and check the drawn style. docs/tui-columns.md (state-word table, prose, regenerated frames), operations.md, CHANGELOG, and the ADR index describe it. Verified with the consistency tests (red before the renderer change), before/after frames at 170 and 70 columns, and `just check` (5746 passed).
<!-- SECTION:FINAL_SUMMARY:END -->
