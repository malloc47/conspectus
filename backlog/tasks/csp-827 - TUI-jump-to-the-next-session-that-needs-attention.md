---
id: CSP-827
title: 'TUI: jump to the next session that needs attention'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 03:40'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 913000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The cheapest high-value control in the surveyed tools is a key that moves to the next waiting agent from any view, then attach (the existing gesture). The order (longest-waiting first or most recent first) and whether it includes finished-unseen turns are the operator's.

Scope: Per the operator's decision: a key in the scoped keymap with a generated hint, the order, and the wrap rule.

TUI changes: A new key and status hint.

Tests and checks: Keymap tests, generated help, cursor movement across views.

Blockers: Operator decisions 5 and 6, and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 W (the waiting mark ! already opens Messages) jumps from either pane, with no conflict in the scoped keymap, to the next session waiting on you: longest-waiting first by when the wait began, wrapping, waiting only; from a selection that is itself a waiting session it goes to the one after it, otherwise to the longest-waiting one
- [x] #2 In the current view the cursor lands on the session's row (or a pin on it; in the Mux view its mux's row when the session has no row of its own), with the groups above it expanded; an agent in a pane no session matches is reached through its mux's row
- [x] #3 When the current view lists no row for it (for example Pinned or History, or a filter hides it) the jump switches to Sessions (Mux for a pane-only agent) and the status line says so; Backspace returns; a session no view lists is skipped
- [x] #4 With nothing waiting, or only the selected session waiting, or every waiting session hidden, W leaves the cursor and says so as a short status-line warning; a jump says which of how many waiting it reached and since when
- [x] #5 The status bar names the key only while an agent waits, and its help-page row comes from the keymap binding; the help legend names the key through the keymap; no hint or help row is written by hand; attach stays the existing gesture
- [x] #6 Keymap tests and the consistency suite cover the key in every view, its feedback, its hint, and its help row; no new effect is added
- [x] #7 The showcase has waiting sessions in several groups with different since times; docs/tui-columns.md, docs/operations.md (how to find what waits), the help, CHANGELOG, design.md and the plan describe the key; frames at 170, 100, 70 and 50 before and after the key are reviewed
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0216: the jump key (`W`, since `!` opens Messages), the order (longest waiting first by the wait's start, ties by entity, wrapping, waiting only, pane-only agents included through their mux), which row stands for a waiting agent in each view (its own row, a pin on it, in Mux its mux's row), the fallback to Sessions (Mux for a pane-only agent) with a Backspace way back, the status-line messages, no new effect. Index it; list it among the host records the framework log leaves out.
2. `RowTree.live` gives the waiting agents in order; `App::jump_to_next_waiting` finds the selection's place, the next target in this view or the fallback, expands the groups above it (`TreeState::select_at`), and says where it went; `Msg::NextWaiting`, `Command::NextWaiting` bound in the scoped keymap with a verb, rank, and `available_when` so the status bar names it only while something waits; the help row is the binding's.
3. Help legend names the key from the keymap; the showcase gets waiting sessions in other groups with different since times (spliced fixture, frames regenerated).
4. Tests: keymap (key reaches the binding from both panes, no conflict), app tests (order, wrap, current selection, expand, fallback views, mux row, pane agent, nothing waiting, filtered out), consistency (every view lands on the longest-waiting session; feedback severity; status hint only while waiting; help lists it).
5. Docs: tui-columns.md, operations.md (how to find what waits), CHANGELOG, design.md, plan; frames before/after at 170/100/70/50 and after the key.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 5: jump to the next waiting session from any view, longest-waiting first, waiting only, wrapping; the key comes from the free keys of the scoped keymap and its hint is generated.

Key: `W` (`!`, the waiting mark, opens the Messages log and is named by its unseen chip and failure messages; `w` is the worktree menu). Rank DEFAULT_ACTION so the hint outlasts the sort and grouping settings on a narrow bar while something waits.
Validation: checks.sh exit 0; run-tests.sh exit 0, 5530 passed, 0 failed (also on the code commit alone). Frames: wframes.sh (showcase, 170/100/70/50) before (CSP-826 binary, old fixture) and after, plus W, WW, ... WWWWW in Sessions, W and WW in Mux, W in Pinned and History, W then Backspace in History: the order walks Ungrouped (2h), project's pin (1h), repo-a's fork (30m, parent opened), project (1m), then wraps; Pinned and History switch to Sessions and say so; Backspace returns.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed the operator's decision 5 (ADR 0216). `W` (the waiting mark `!` already opens Messages; `w` is the worktree menu) moves the cursor, from any view and either pane, to the agent waiting on you longest (by when its wait began), each press to the next, wrapping; from a waiting agent's row it goes to the one after. It lands on the row standing for the agent in the view on screen (its session row, a pin on it, in Mux its mux's row when the mux has no row for it, a pane-only agent on its mux) via `TreeState::select_at`, opening the groups above; otherwise it switches to Sessions (Mux for a pane-only agent) as a reveal does, with Backspace back. The status line says `2 of 4 waiting, since 1h ago` (plus `switched to Sessions: Pinned doesn't list it`), or warns that nothing waits, nothing else does, or the filters hide them. The binding is the scoped keymap's `waiting.next`: help row from the binding, the legend names its key via the keymap, the status bar shows `W next waiting` only while something waits. No new effect. Showcase: the orphan (Ungrouped, 2h) and the codex fork (repo-a, 30m) now wait.

Verified: checks.sh exit 0; run-tests.sh exit 0, 5530 passed; keymap tests (W from both panes, no conflict, `!`/`w` keep their bindings, unavailable when nothing waits); `rows::live` order test; consistency `jump` (every view, order and wrap, opened groups, Mux rows, pane agent, content search hiding, nothing/only/hidden warnings, status hint, help) and `feedback`; frames before and after at 170/100/70/50.

**Regressions**
- While an agent waits the status bar adds `W next waiting`, and at 100 columns `sort:` (Sessions, PRs, History) or `group:` (Mux) gives way to it.
- After a jump the status line shows where it went instead of the key hints until the cursor moves; at 50 columns a switch message is cut (`switched to Sessio…`).
- Showcase frames: the header reads `! 4` (was `! 2`), `repo-a` and Ungrouped headers gain `! 1`, the fork and the orphan rows show `!` (README screenshots will when next rendered).

**Consistency wins**
- The key's status hint, help row, and the legend's key name all come from its keymap binding.
- It visits exactly the agents the header's `! N` counts, pane-only agents included.
- It lands where reveal and quick select land (a view's own listing, else the entity's view if that view lists it) and returns with Backspace like a reveal.
- Its no-op messages are status-line warnings like `o` and Backspace with nothing to act on.
<!-- SECTION:FINAL_SUMMARY:END -->
