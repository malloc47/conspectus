---
id: CSP-829
title: 'TUI: live state in the detail pane and in pinned and history rows'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-13 00:26'
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
ordinal: 915000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The explorer should say what state a node is in, since when, how old the evidence is, where it came from, and whether it is quiet or stale. Pinned and History use words, so a pinned live session also needs its word decided (replace `in mux`, or compose with it).

Scope: Per the operator's decision: Node fields in the explorer, the glossary entry, and the Pinned and History words.

TUI changes: The detail pane and two views gain fields or words.

Tests and checks: Explorer and view snapshots; glossary tests.

Blockers: Operator decisions 7 and 12, and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 For an agent session with live state, the explorer Node zone lists, after `status`: `state` (led by the row's live mark in its style, then the state and detail in words, e.g. `waiting · permission`, `ready · finished`, `unknown · stale` led by `?`), `since` (when the shown state began on the TUI age ladder, e.g. `5m ago`, previewing the timestamp), `source` (`hook`), and `evidence` (`fresh`, `quiet`, or `stale`); quiet and stale `state` and `evidence` values recede in the muted look
- [x] #2 `subagents` (`N active`, a count only) is listed only while the agent runs one or more and its state is neither `exited` nor `unknown`, the same rule as the row chip
- [x] #3 A session or mux with no live evidence lists none of these fields; every other node kind and field is unchanged
- [x] #4 A mux with live evidence lists `agents` (its counts in rollup order joined by ` · `, e.g. `1 waiting · 1 error`, led by the mux row's mark) and, when agents run in panes no session matches, `pane_agents` (`%3 working · generating, %4 starting`, previewing one agent per line)
- [x] #5 The detail pane reads the same live values as the rows at the same moment; a store change refreshes the Node fields in place and keeps the explorer cursor on its field
- [x] #6 The glossary explains every state, every detail, each source, and `fresh`/`quiet`/`stale`; tests walk the explorer cursor onto `state`, `since`, and `evidence` and check their previews, and the Related drill and its Preview still work
- [x] #7 Pinned rows keep the CSP-823 words; History lists no sessions and shows no live word; stopped, removed, and missing rows never show one (tests)
- [x] #8 docs/tui-detail-mockup.md, docs/operations.md, docs/design.md, CHANGELOG.md, and a new ADR record the fields; `tui --snapshot` frames of every affected view and the right pane are checked at widths 170, 100, 70, and 50, and every visible change is listed in the final summary
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. ADR 0213 (detail pane live fields): labels, core placement, the mark leading `state`, muted quiet/stale, nothing without evidence, mux `agents` and `pane_agents`; index it and list it in the framework log's detail-pane row.
2. Rows: `LiveCell` carries the source and the shown start (the CLI's rule); `LiveValues` keeps pane-only cells and becomes the tree's `live` field, so the explorer reads the values the rows were filled with.
3. Explorer: `ExplorerInputs`/`GraphSource` take the tree's live values; session core gains state, since, source, evidence, subagents; mux core gains agents and pane_agents; `CoreField` gains a live mark and a receding flag that the grid draws; the overlay refill refreshes the explorer in place.
4. Glossary entries for states, details, sources, and evidence.
5. Tests: explorer builders, grid styling, cursor walks onto the new fields and previews, the drill, refresh on a store change, Pinned/History words and stopped/removed rows.
6. Docs: tui-detail-mockup field reference, operations.md, design.md, CHANGELOG.
7. Validate with frames.sh before/after and snapshots at 170/100/70/50 (left and right panes); checks.sh and run-tests.sh; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 items 7 and 12: Node fields State, Since, Source, and Evidence (fresh, quiet, stale) in the explorer, the glossary entry, and the Pinned and History words from CSP-823. Quiet states show dimmed; stale or unknown shows '?'.

From CSP-823 (ADR 0209): the Pinned words landed there. A running session's Pinned row (a session pin or a session nested under a pin) whose agent is waiting, working (compacting, starting), ready, or failed reads `waiting`, `working`, `ready`, or `error` in place of `in mux` or `resumable` (both link words, since an agent that reports a state is running), in its mark's style, dimmed when quiet; unknown, stale, exited, and no evidence keep the link word. Quiet states already draw dimmed and stale ones `?` in the live cell. Left for this story: the explorer's State, Since, Source, and Evidence fields and the glossary entry.

Verified: tests in src/tui/explorer/live_tests.rs (fields, quiet/stale receding, subagent rule, no-evidence, mux agents and pane_agents, grid styling), src/tui/consistency_tests/live_detail.rs (every view's rows agree with the detail pane, store change refreshes in place with the cursor kept, cursor walk onto state/since/evidence previews, Related drill and Backspace, mux pane agents, dim Pinned/History rows never spell a live word), and src/tui/preview/value_tests.rs (every live word has a glossary entry). Full suite 5468 passed / 0 failed / 1 ignored; checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) and just tuikit-features pass. Snapshots before/after at 170/100/70/50: left pane, header, and status bar byte-identical in all six views; only the right pane changes. Follow-up CSP-859 (key-script examples in CLAUDE.md, AGENTS.md, README, dev-scenarios) and a note on CSP-855.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The detail pane now says what a node's agents are doing (ADR 0213). A session with live state lists, after `status`: `state` (led by its row's live mark in the row's style, then `waiting · permission`, `ready · finished`, `unknown · stale`...), `since` (the shown state's start on the age ladder, the CLI's rule), `source` (`hook`), `evidence` (`fresh`/`quiet`/`stale`; quiet and stale words recede), and `subagents` (`N active`, a count, by the row chip's rule). A mux lists `agents` (counts in rollup order, led by its row's mark) and `pane_agents` (first pane agent and `(+N)`, previewing all). No evidence, no fields. The values are the ones the rows were filled with (`RowTree::live`), and the overlay's refill reloads the explorer in place. The glossary explains every state, detail, source, and evidence word. Pinned and History words are unchanged.

Commits: a6a6cac6 (ADR 0213, ADR 0112 amended, indexes), f4f4dbe4 (code, tests, screenshot key script), 6b748d2d (docs, changelog, plan).

**Regressions**
- Right pane, session with live state: four or five more Node rows, so its Preview shows up to five fewer lines (the showcase's pinned session loses its transcript's first question at 150x34); in the stacked layout at width 50 the Related rule drops below the fold.
- Right pane, mux with live state: one or two more Node rows (`agents`, `pane_agents`), so two fewer pane-capture lines in the Mux view's project mux.
- `<Tab>` plus five `j` no longer reaches a live session's first Related row (screenshot recipe updated; prose examples are CSP-859).
- `evidence` now names two things in the pane: live freshness in Node fields, a link's producers in a Related Preview.

**Consistency wins**
- A node's live fields can never disagree with its row: both read one set of values at one moment (tested across all six views).
- `state`/`agents` lead with the exact mark and style of the row (`?` stale, `·` exited, dimmed when quiet).
- `since` follows `conspectus node show`'s shown-since rule; mux counts use the rollup's precedence.
- `pane_agents` reads like other list fields (first plus `(+N)`).
- Left pane, header, and status bar byte-identical before/after in every view at 170/100/70/50.

Checks: run-tests.sh 5468 passed / 0 failed / 1 ignored; checks.sh pass; just tuikit-features pass.
<!-- SECTION:FINAL_SUMMARY:END -->
