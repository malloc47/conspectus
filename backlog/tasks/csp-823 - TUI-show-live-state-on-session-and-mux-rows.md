---
id: CSP-823
title: 'TUI: show live state on session and mux rows'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 21:41'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-814
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 909000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The point of the feature is to see state at a glance. Where it appears (a new column beside the mux-link cell, replacing the session kind glyph, or a word in mixed views), which shapes mean what, and whether `working` animates are the operator's to choose: the common `●◐○` set collides with session kind, PR, and refresh marks, and the braille spinner already means "refresh pending".

Scope: Per the operator's decision in `docs/plans/live-state/tui-options.md`: the column schema entry, vetted one-cell glyphs (ADR 0154) and words (ADR 0145), theme roles for each state (ADR 0153), the density modes, and the `docs/tui-columns.md` rows, glyph table, state-word table, and regenerated frames. Reuses the row models filled by the plumbing story.

TUI changes: Adds the cell to Sessions, Mux, PRs, Pinned, and History as decided; list every frame change under Regressions and Consistency wins.

Tests and checks: `conspectus tui --snapshot` at normal and narrow widths in each view, `tests/tui_columns_doc.rs`, the consistency suites for glyphs, state, text, and freshness.

Blockers: Operator decisions 1, 7, and 10 and the plumbing story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Session rows in every view that lists them (Sessions, agents nested under a mux in Mux, under a PR in PRs, and Search) draw an always-filled one-cell live mark right after the age: ! waiting, » working (also compacting and starting), › ready, × error (the shared failure mark), ? unknown or stale (the existing can't-tell mark), · no evidence or exited
- [x] #2 Mux rows, running and stopped, draw one rolled-up mark in the same cell from the mux's live counts by the model's default precedence (waiting, error, ready, working, then the rest), and · when nothing is counted
- [x] #3 A quiet state draws the same glyph dimmed; marks take the theme's warning, info, success, error, and muted tokens, which [tui.theme] configures; nothing animates
- [x] #4 Pinned and History rows for a session whose live state is waiting, working (compacting, starting), ready, or error show that word in the state-word column in place of the link word, styled like its glyph; unknown, stale, exited, and no evidence keep today's word
- [x] #5 The live cell never gives way on a narrow row and the key, chip, and age keep their order; a row awaiting a refresh keeps its live mark and shows the spinner in the link cell; docs/tui-columns.md says both
- [x] #6 » is a vetted glyph; docs/tui-columns.md's glyph table, state-word table, row tables, give-way table, and frames describe the cell and the words, the frames regenerated from a showcase fixture whose live state covers every mark, a quiet one, a mux with mixed agents, and a pinned live session
- [x] #7 The ? help page lists the six marks and what each means
- [x] #8 Consistency tests assert one entity shows one mark in every view and the matching word in Pinned, the live cell lines up on every row kind at every width, and the mark stays legible on the cursor row focused and unfocused
- [x] #9 docs/operations.md, docs/design.md, CHANGELOG.md, and a new ADR describe what the TUI shows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Marks: a host `icons::live` vocabulary (! » › × ? ·), `»` registered as a vetted glyph, the marks added to `all_glyphs`; a pure mapping from a session's LiveCell or a mux's LiveCounts (default precedence) to mark, word, and style (warning/info/success/error/muted tokens, quiet adds muted). No new theme roles: the framework tokens fit and are configurable already.
2. Schema and rendering: `Col::Live` after `Col::Age` on the session schema and `Col::MuxLive` after `Col::MuxAge` on the mux schema (fixed 1, never gives way); cells read `AgentSessionRow.live`, `MuxSessionRow.live`, and a stopped mux's `MixedLive::Mux`; the live cell keeps its mark while the row awaits a refresh (the spinner stays in the link cell).
3. Words: Pinned/History rows for a session with waiting/working/ready/error show the live word in place of `in mux`/`resumable`, styled like the glyph; `words::ALL` gains the four words.
4. Help: a legend section for the six marks on the `?` page.
5. Fixture: the showcase world gets a live-state store covering every mark (waiting, working, quiet working, ready, error, stale, exited, none, a mux with mixed agents via the ambiguous mux, a pane-only agent, a pinned live session); regenerate tests/fixtures/showcase.json (only the live tables change), then `just tui-columns`.
6. Docs: docs/tui-columns.md prose and tables, docs/operations.md, docs/design.md, CHANGELOG, new ADR 0209 (amends 0145 and 0183), framework log's left-out row.
7. Tests: replace the 'changes no frame' test with same-mark-in-every-view, Pinned word, line-up (columns grid with the live cell), cursor-row legibility, narrow give-way; update goldens and snapshots.
8. Validate with frames.sh before/after and snapshots at 70 and 50 columns in every view; checks.sh and run-tests.sh.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 items 1, 7, 10: Option A. A new always-filled one-cell column right after the age on session rows, mux rows (rolled up), nested agent rows, and the agent rows of the PRs view. Glyphs: '!' waiting, '»' working (also compacting and starting), '›' ready, '×' error (the shared failure mark), '?' unknown or stale (the existing can't-tell mark), '·' no evidence or exited. A quiet state dims the same glyph; shape never depends on color; colors are theme roles (ADR 0153); every glyph is vetted (ADR 0154). No animation. Pinned and History use words (waiting, working, ready, error), replacing 'in mux' when live state exists (CSP-829 owns the detail pane). Narrow widths and the density modes: decide the give-way order with the existing narrow-row rules and say so in docs/tui-columns.md. Unseen/seen styling and the header chip are separate stories.

Design choices the decision left open, recorded in ADR 0209 and docs/tui-columns.md: the live column is lined up and never gives way (key, chip, age leave first; labels, projects, previews leave three cells sooner); while a row awaits a hand-off refresh the spinner stays in the link cell and the live mark keeps its value (dimmed with the row); a mux's mark is never dimmed because LiveCounts carries no confidence; both link words (`in mux` and `resumable`) give way to the live word; a mux pin keeps `live`; colors are the framework's warning/info/success/error tokens and muted (no new roles), so working and ready are both green by default; `›` is the breadcrumb separator's character, excused in the shared-marks guard.

Showcase: the world writes its own live-state store through `LiveStore::apply_snapshot` (no pids, so states hold 15 minutes from the anchor; the real-clock scenario TUI shows them stale). Only the `node_live`/`pane_live` tables were spliced into tests/fixtures/showcase.json, because a full regen also rewrites every wall-clock `freshness_epoch`.

Validation: full suite via run-tests.sh exit 0 (5416 passed, 0 failed, 1 ignored); checks.sh exit 0 (fmt, clippy -D warnings, rustdoc -D warnings, diff --check); tui_columns_doc, showcase_scenario, showcase_screenshots, cli_tui_state_invariants, tuikit doc_sync pass. Frames: frames.sh before/after plus snapshots of every view at 170, 70, and 50 columns (cursor rows, unfocused pane, expanded PR and pin); only the left pane changed, History is identical, right panes, header, and status line unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Session rows (Sessions, agents under a mux or a PR, Search) and mux rows (running and stopped) draw an always-filled one-cell live-state mark right after the age: `!` waiting, `»` working/compacting/starting, `›` ready, `×` error (shared failure mark), `?` unknown or stale (can't-tell mark), `·` none or exited; quiet dims the same mark; a mux rolls its agents up by the model's default precedence and is never dimmed; nothing animates. Pinned spells a running session's live state as `waiting`/`working`/`ready`/`error` in place of `in mux`/`resumable`. Colors are the theme's warning/info/success/error tokens and muted look (no new roles); `»` is vetted; the `?` help gains an "Agent state" legend naming `hook init`. The column never gives way, and a pending refresh keeps the mark while the spinner takes the link cell. The showcase world records live state for every mark, and its fixture carries the bound tables.

Commits: 0c08b58b (ADR 0209), ee79ca77 (marks), 883ff4ac (showcase), fcd9d6db (cell), 60367b59 (Pinned words), 9b275db4 (docs).

Verified: run-tests.sh exit 0 (5416 passed, 0 failed); checks.sh exit 0; `just tuikit-features` exit 0; tui_columns_doc frames regenerated with `just tui-columns` and passing; new consistency tests (one mark per entity in every view, snapshot table and overlay; quiet, stale, exited; live state changes only live cells; cursor row focused and unfocused; store change redrawn in place; line-up grid with the live mark), ui tests for narrow give-way and pending refresh, LiveSign unit tests, help legend test; snapshots at 170/70/50 columns in every view compared before and after. Follow-up: CSP-855 (README screenshots); notes left on CSP-824, CSP-829, CSP-270.
<!-- SECTION:FINAL_SUMMARY:END -->
