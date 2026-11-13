---
id: CSP-756
title: 'Extract `Explorer`, `Breadcrumb`, and `StackedMasterDetail` per the design'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 12:10'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-755
  - CSP-729
  - CSP-789
  - CSP-790
  - CSP-791
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/plans/tui-framework/explorer-design.md
priority: medium
type: enhancement
ordinal: 842000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
This is the last and riskiest extraction, gated by the design spike. The explorer is the part of the TUI most entangled with the graph model, and extracting its generic core completes the property-grid story by giving the right pane a reusable host (inventory slice 07).

Scope: Implement the design: a generic zoned cursor with stable row keys and reseating, the drill stack with an armed back, an outcome-typed key handler, `Breadcrumb` with elision, and `StackedMasterDetail` layout (content-sized list zone, divider, remainder with a minimum detail height). The node builders and the `impl App` wiring that adapt Conspectus stay behind. If the spike said no-go, close this task with that decision instead.

TUI changes:
- Consistency wins: the right pane's keys are declared in one table; the breadcrumb elides instead of clipping; the explorer is testable without an `App`
- Regressions or behavior changes: `h`, `l`, Page keys, and Esc in the right pane may gain meanings that the design chooses, each listed

Tests and checks: The explorer tests (`explorer_tests.rs`, 1,690 lines) are ported to the generic core and the adapter. `--snapshot` of drilling in, going back, and armed back on the showcase fixture.

Blockers: The design spike and the overlay contract.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The generic explorer core, breadcrumb, and stacked master-detail layout are in the framework as the design specifies, or the task is closed on the design's no-go with the reason recorded
- [x] #2 The explorer is driven through an outcome type, not `Msg` variants plus runtime branching
- [x] #3 The right pane's key meanings are declared in one table
- [x] #4 Explorer tests are ported and none is dropped
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Follow explorer-design.md section 5, steps 3 to 6 (steps 0 to 2 landed as CSP-789, CSP-790, CSP-791), one or more commits per step, each green.

1. Step 3 (framework only, no host change): crates/tuikit/src/explorer.rs with explorer/{state,keys,outcome}.rs: ExplorerModel, ExplorerSource, Zone, Entry, FrameView, Frame, Explorer<M> (open, handle, drill_to, back, seek, disarm, refresh, lines, window/keep_window, footer), Outcome (ten variants), Refusal, Refreshed, ExplorerKey, EXPLORER_KEYS with KeyAction and HelpAction; keys::exact; help::sections_described for host wording. Tests over a toy source: one per rule of 2.5 and per row of table 3.2. Page docs/controls/explorer.md, module map, README.
2. Step 4 (host adopts): src/tui/explorer/source.rs (Graph model, GraphSource with zones from the view model's rows); App.explorer: Option<Explorer<Graph>>; ExplorerState, explorer_window, explorer_back_armed, BreadcrumbHop, LinkRef, strip_relationships go; eight Msg::Explorer* become Msg::Explorer(ExplorerKey) plus Msg::NavBack for the global Backspace; one host function maps each Outcome to its effect and words; Action::ExplorerEnter, OpenValueModal and runtime::explorer_enter go (copy is Effect::Copy; snapshot copy skip posts the toast). Keys still come through remap_for_focus. Port explorer_tests.rs, app_tests, ui_tests, runtime_tests, snapshot_tests, consistency tests mechanically; frames byte-identical (capture before/after at 160x40, 100x30, 60x20). Stop rule: if the 26 app tests or frames cannot stay unchanged with the lending pattern, revert step 3 and take the NO-GO path.
3. Step 5: RIGHT_PANE_KEYS (include EXPLORER_KEYS) resolved first on right focus with no modal; a focus guard drops left-tree actions on right focus; remap_for_focus and the global e row go; help section and right-focus status hint generated from the table; key changes of section 7.1 (PgUp/PgDn page, h/l and arrows fold, Esc back, Alt chords of j/k stop moving the explorer, left Backspace stops popping the drill). Tests pin each key; snapshot frames before/after.
4. Step 6: explorer.md page final, ADR 0188 amendment, docs/design.md, docs/operations.md, CHANGELOG.md, operator-review.md rows; close the task with the ported-test mapping and the visible-change lists.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Step 3 landed (1d2e585d): tuikit::explorer with 47 framework tests over a toy source, keys::exact, help::sections_described, page docs/controls/explorer.md.
Step 4: the host adopts it. src/tui/explorer/source.rs (Graph, GraphSource, zones); App.explorer is Option<Explorer<Graph>>; ExplorerState, explorer_window, explorer_back_armed, BreadcrumbHop, LinkRef, flat_rows, strip_relationships, Action::ExplorerEnter, Action::OpenValueModal, runtime::explorer_enter go; eight Msg::Explorer* become Msg::Explorer(ExplorerKey) plus Msg::NavBack (the global Backspace, which the right arm of remap_for_focus maps to Explorer(Back)); a copy is Effect::Copy; the snapshot's copy skip posts the toast. 1015 before/after snapshot frames (160x40, 100x30, 60x20) byte-identical; full suite 4606 passed, 0 failed, 1 ignored. Stop rule not triggered: the 26 explorer app tests keep their assertions (the copy tests assert Effect::Copy, as the design says); the one changed expectation is CSP-789's gone-frame test, which the design changes (Back skips a gone frame in the same press).

Step 5 (384fc2b3): RIGHT_PANE_KEYS resolved first on right focus with a focus guard; key meanings of design section 7.1; help section and status hint generated. Step 6 (9723b927): docs.
Verification: fmt, clippy --workspace --all-targets --all-features -D warnings, RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps, cargo build --examples, the tuikit independence guard, git diff --check all pass; full isolated suite 4613 passed, 0 failed, 1 ignored; nextest 4613 passed, 1 skipped. Frames: /tmp/tuifw-baseline/frames-756-before, -step4 (byte-identical to before, 1015 captures), -after (150 differ, every one in a listed category; /tmp/tuifw-baseline/frames-756-diff.txt).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
GO carried out (design steps 3 to 6; Breadcrumb and StackedMasterDetail landed in CSP-790 and CSP-791). The stop rule did not apply: the 26 explorer reducer tests kept their assertions and the right pane's frames were byte-identical after the adoption.

Step 3 (1d2e585d): `tuikit::explorer` (`ExplorerModel`, `ExplorerSource`, `Zone`, `Entry`, `FrameView`, `Frame`, `Explorer<M>`, the ten-variant `Outcome`, `Refusal`, `Refreshed`, `ExplorerKey`, `EXPLORER_KEYS`, `hint`), `keys::exact`, `help::sections_described`; 48 framework tests over a toy source (one per rule and per outcome row) plus 2; page `crates/tuikit/docs/controls/explorer.md`.
Step 4 (334d1d37): `src/tui/explorer/source.rs` (`Graph`, `GraphSource`); `App.explorer: Option<Explorer<Graph>>` lent out per call; `ExplorerState`, `explorer_window`, `explorer_back_armed`, `BreadcrumbHop`, `LinkRef`, `flat_rows`, `strip_relationships`, `Action::ExplorerEnter`, `Action::OpenValueModal`, and `runtime::explorer_enter` gone; eight `Msg::Explorer*` are `Msg::Explorer(ExplorerKey)` plus `Msg::NavBack`; a copy is `Effect::Copy`. 1015 snapshot captures byte-identical.
Step 5 (384fc2b3): `RIGHT_PANE_KEYS` (one table, its docs and tests pinning both meanings of each shared key) resolved first with a focus guard; help section and status hint generated. Step 6 (9723b927): ADR 0188 amended (0085, 0160 notes), design.md, operations.md, CHANGELOG.md, operator-review.md (12 rows).

Checks: fmt, clippy -D warnings, rustdoc -D warnings, examples, independence guard, whitespace diff check pass; full isolated suite 4613 passed, 0 failed, 1 ignored; nextest 4613 passed. Frames at 160x40, 100x30, 60x20 in /tmp/tuifw-baseline/frames-756-{before,step4,after}: 49 differ only in the status line, 101 in the key changes and the help page; none else.

Tests ported, none dropped (counts before -> after: app_tests 133 -> 136, explorer_tests 32 -> 32, ui_tests 184, runtime_tests 91 -> 93, snapshot_tests 16, consistency 56, grid_tests 7, help_tests 14 -> 15):

| Old test(s) | Now |
| --- | --- |
| explorer_tests: 29 builder tests, 2 breadcrumb_hops, neighbor_row_key, agreeing_producers | unchanged (adapter over `Frame<Graph>` and the adapter's rows) |
| explorer_tests: flat_rows_splits_validated_and_other_zones | zones_split_validated_and_other_rows (same assertions plus zone shape); framework fold and reseat tests |
| app_tests: 26 explorer tests (home/end, first cursor, drill, 2 mirror, 2 restores, left hint, 3 arm, unwind, edge meta, 2 full detail, Other toggle, clamp, 6 copy/inspect, reset on selection, 2 copy target) | same tests through `Msg::Explorer(..)`; copy tests read `Effect::Copy`; left hint via `Msg::NavBack`; each rule also a framework test |
| app_tests: CSP-789 gone-frame test | backspace_to_a_frame_whose_node_is_gone_skips_it_in_the_same_press, plus backspace_with_every_frame_gone_says_so_and_keeps_the_pane |
| runtime_tests: translate_e and 5 remap_for_focus tests | 6 resolution tests through `action_for_event`, plus the two-meanings and inspect-key tests |
| new | backspace_on_the_left_pane_leaves_the_right_panes_drill_alone, the_right_panes_status_hint_names_what_enter_does_on_the_cursor_row, the_right_panes_section_is_generated_from_its_table |

**Regressions**
- `h`, `l`, `←`, `→` on a row in no section (a field, a link) now warn ("nothing to expand here") where they did nothing.
- `Backspace` past a frame whose node is gone no longer says so when an older frame loads; it warns only when every frame is gone.
- A page key moves by the zone's rows, so with wrapped fields it can land one entry past the last shown.

**Consistency wins**
- `PgUp`/`PgDn` page the right pane by the rows its zone shows (they moved one row).
- `h`/`l` and `←`/`→` close and open the `Other` zone, from its header or a row inside it.
- `Esc` goes back as `Backspace` does.
- Alt and Ctrl chords of `j`, `k`, `g`, `G` (and `h`, `l`) no longer move the explorer or the tree behind it; `Shift-Enter`/`Alt-Enter` still reveal.
- The left pane's `Backspace` no longer pops the right pane's drill; it says there is no reveal to return from.
- The right-focus status hint is generated: `↑/↓ move · Enter copy|follow|fold · e fold · Backspace back` (was `j/k cursor · Enter drill/expand · e group · Backspace back`); shorter, so it now shows at 100 columns, and `r refresh` fits at 160.
- The help page's right-pane section is generated from the table (13 rows plus `E`); the stale `e` text is gone; Navigation's `Enter` row points at it.
- `Backspace` to a gone frame goes on in the same press.
- In `--snapshot`, the value modal's and Messages' copies show their toast as `Enter`'s does; `Enter`'s copy disarms an armed `Backspace`.
- Everything else is byte-identical.

Judgment calls for review are the CSP-756 rows of operator-review.md. No follow-up filed.
<!-- SECTION:FINAL_SUMMARY:END -->
