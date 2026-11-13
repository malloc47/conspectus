---
id: CSP-828
title: 'TUI: tell a finished turn you have seen from one you have not'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 01:28'
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
ordinal: 914000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Six surveyed tools split "ready" into new and seen, which drives the attention count down as the operator works. Where "seen" lives is open: viewer memory, derived read-only from the mux's last attach time, or a persisted observation (a write under ADR 0143, and a flicker source in agent-deck).

Scope: Per the operator's decision, with `since` already kept accurate by the model.

TUI changes: Ready rows gain a seen distinction.

Tests and checks: Seen/unseen transitions on attach and on view; no flicker on refresh.

Blockers: Operator decision 6 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A `ready` turn of a session whose preferred mux is known reads unseen until that mux shows a client attached at or after the second the turn ended (its last attach time), or a snapshot loaded after the turn ended shows a client attached to it; a pane-only agent follows the same rule with its pane's mux
- [x] #2 Once seen, a turn stays seen in this TUI instance until a later turn ends: a later refresh that shows the client detached never turns it back to unseen (no unseen, seen, unseen flicker), tested across an attach and a detach
- [x] #3 A `ready` turn of a session with no mux reads unseen until the cursor rests on a row standing for that session (any view, a Pinned row included) for at least one second in this TUI instance; a cursor that passes through sooner does not count, and a turn that ends after that reads unseen again
- [x] #4 An unseen ready mark `›` and an unseen Pinned `ready` word draw bold in the ready (success) color, a seen one plain, with the same glyph; a mux row whose mark is `›` is bold while any of its ready agents' turns is unseen; nothing else about a row changes
- [x] #5 The detail pane's `state` field reads `ready · finished · unseen` or `ready · finished · seen` (each word previews its glossary entry), and a mux's `agents` field says how many ready turns are unseen (`2 ready (1 unseen)`)
- [x] #6 Nothing is persisted and no effect is added: the seen memory lives only in the running TUI, is not in tui-state.json, and resets when the TUI restarts
- [x] #7 The showcase world has an unseen ready session and a seen ready one; docs/tui-columns.md (glyph table, live cell, weight rule, regenerated frames), docs/operations.md, the help legend, CHANGELOG.md, and a new ADR describe the rule; the consistency suites cover it; `tui --snapshot` frames are checked at widths 170, 100, 70, and 50 and every visible change is listed in the final summary
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0214: the derived rule (mux last attach or a client attached after the turn ended), the per-turn in-memory latch that stops flicker, the one-second rest rule for sessions with no mux, weight as the additive cue, the detail pane's words, and why it is view state rather than an effect; index it and list it in the framework log.
2. rows/seen.rs: the agent key, the in-memory memory of seen turns, the judge (host mux by the preferred-mux helper, last attach, attached at load), the rest constant; LiveValues judges every ready cell, LiveCell gains `unseen`, a mux row carries its unseen count (MuxLive).
3. LiveSign gains `unseen`: bold over the ready style; the mux mark follows its ready agents.
4. App: the memory latches every turn found seen after each annotate (set_data, overlay refill) and the rest check runs each loop pass (live and static modes); nothing persisted.
5. Explorer: `ready · finished · unseen|seen`, mux `N ready (K unseen)`; glossary entries; help legend.
6. Showcase: the deck launcher's turn ends before its mux's last attach (seen) and a no-mux session ready (unseen); regenerate fixture and tui-columns frames.
7. Tests: judge unit tests, latch/no-flicker across attach and detach, rest rule, bold vs plain in every view, detail words, no persistence; consistency suites.
8. Docs: tui-columns.md, operations.md, design.md, CHANGELOG, plan; snapshots 170/100/70/50; checks; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 6: derived read-only. A ready turn is unseen until a tmux client has been attached to its mux after the turn ended (the mux's last attach time), or, for a session with no mux, until the operator selects it in this TUI instance (memory only). Unseen ready is bold, seen is plain; the shape is the same. Nothing is persisted.

Verified: src/tui/rows/seen_tests.rs (last attach at, before, and in the turn's second; a client attached in a listing loaded after vs before the turn; pane agents through their pane's mux; a mux the snapshot does not list counts as none; the memory keeps the latest turn and a later one is unseen), src/tui/app/seen_tests.rs (unseen, seen after the attach's refresh, still seen after the detach's refresh, unseen again for a later turn; last-attach path; the one-second rest on a session with no mux; a passing cursor restarts the rest; resting on a mux session does nothing; build_persisted_state unchanged and rows and selection unchanged; a new App remembers nothing), src/tui/consistency_tests/seen.rs (bold vs plain with the same glyph, color, and line text in Sessions and Pinned; the Search view's rest turns Pinned and Sessions plain; a mux's › bold until its mux is attached; one judgment per entity), and the existing one-value-per-entity suite now requires every listing to carry the same judgment. Full suite 5485 passed / 0 failed / 1 ignored; checks.sh and just tuikit-features pass; commit 14f3ac31 alone passes the doc-frame and showcase tests. Follow-up CSP-860 (a turn watched while attached).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A finished turn the operator has not seen now draws its `›` bold (ADR 0214), derived read-only. For an agent in a mux (the session's preferred mux, or a pane-only agent's pane mux) the turn is seen once the mux's last attach time is at or after the turn's end, or a snapshot loaded after the turn ended lists a client attached. For a session with no mux it is seen once the cursor has rested on one of its rows (session row, search hit, or pin) for one second. The App remembers every turn its rows find seen, per agent, until a later turn ends, so a refresh after a detach never turns it bold again; nothing is persisted and no effect is emitted. A mux's `›` is bold while any of its ready agents' turns is unseen; the Pinned `ready` word follows its mark. The detail pane's `state` ends `· unseen` or `· seen`, a mux's `agents` says `N ready (K unseen)`, and the glossary and help legend (a new "Ready, unseen" row) explain it. The showcase's agent-deck mux was last attached after its launcher's turn (seen) and the codex parent, in no mux, finished a turn nobody looked at (unseen).

Commits: 02a9f7bd (ADR 0214, amendments, indexes), 14f3ac31 (rows/seen.rs, LiveSign weight, App memory and rest, detail words, glossary, help, tests), cec62a5b (showcase world and fixture, regenerated tui-columns frames and weight rule), b182be02 (operations, design, changelog, plan).

**Regressions**
- On the cursor row, which the highlight already draws bold, unseen and seen `›` look alike; only the detail pane's words tell them apart there.
- A turn that ends while you watch it in an attached client can read unseen after you return, if no refresh listed your client while it was there (CSP-860).
- A ready session with no mux reads unseen again after every TUI restart (memory only, by decision).
- The help page's Agent state section is one line longer; the detail pane's `state` value is one word longer for ready sessions.
- Showcase data: the codex parent's mark changes from `·` to `›` in every frame that lists it; the agent-deck mux's last attach moves from 45 min to 30 s before the anchor (only visible with the "last attached" mux sort).

**Consistency wins**
- One judgment per entity in every view (tested), shared by the session cell, the mux rollup, the Pinned word, and the detail pane, all through `LiveSign`.
- Weight is purely additive: shape and color never change. Rendering the same fixture with the binaries before and after this story gives identical text in every left-pane frame at 170/100/70/50; only the bold attribute on the unseen `›` and the detail pane's words differ.
- Seeing is view state like the cursor and folds: no effect, no lifecycle entry, no write path.

Checks: run-tests.sh 5485 passed / 0 failed / 1 ignored; checks.sh pass; just tuikit-features pass.
<!-- SECTION:FINAL_SUMMARY:END -->
