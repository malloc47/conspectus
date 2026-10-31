---
id: CSP-766
title: Assess the milestone's impact on the existing backlog
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:28'
updated_date: '2026-10-31 08:19'
labels:
  - tui-framework
  - tuifw-close
milestone: m-25
dependencies:
  - CSP-764
  - CSP-762
  - CSP-763
documentation:
  - docs/plans/tui-framework/control-set.md
  - docs/plans/tui-framework/gap-analysis.md
priority: high
type: task
ordinal: 852000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The extraction moves modules, renames concepts, removes views and keys, and changes shared behavior (cursor, close keys, confirmation, theme keys). Many open stories name `src/tui/widgets/*`, `ui/left_panel.rs`, `Controls`, `Picker`, or `LaunchSpecFormState`; some now solve a problem the framework solved; some conflict with a house rule. At planning time there were 124 open stories. Reviewing them once, at the end, with the framework in its final shape, is cheaper and more accurate than guessing now (`docs/plans/tui-framework/control-set.md`, section 11, lists the overlaps already visible).

Scope: List every story that is open at the time of the review and every In Progress story. Classify each as unaffected, needs a path or terminology update, superseded or obsolete, needs re-scoping, or newly blocked or unblocked. Record the result in `docs/plans/tui-framework/backlog-impact.md` and apply it through the `backlog` CLI: update descriptions, dependencies, and labels, and close superseded stories with a final summary that points at the story that solved them. Give an explicit outcome to the known overlaps: CSP-342 (the viewer crate), CSP-390, 391, 392, 393, and 394 (command palette and minibuffer), CSP-430 (`tui-textarea`), CSP-432 (spinners), CSP-434, 455, and 455.01 (path field), CSP-435 (`tui-tree-widget`), CSP-269 and 270 (theme presets and density), CSP-319, 190, 180, and 183 (responsive layout, frame matrix, snapshot coverage), CSP-188 and 191 (labels and density), CSP-616.09, CSP-687, CSP-702, and the m-24 stories. Map each Tier 1 and Tier 2 entry in the gap analysis to an existing story, a new story, or an explicit "not planned". If the runtime evaluation was done, fold its outcome in.

TUI changes: none.

Tests and checks: A final listing of open stories shows every one with a recorded classification. `backlog doctor` passes after the dependency changes.

Blockers: Last in the milestone: it depends on the documentation and separation stories so it sees the final shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every story open when the review starts is classified in `docs/plans/tui-framework/backlog-impact.md`
- [x] #2 Each story needing a change is updated through the CLI, and each superseded story is closed with a final summary that names its replacement
- [x] #3 Each known overlap named in the scope has an explicit recorded outcome
- [x] #4 Each Tier 1 and Tier 2 gap-analysis entry maps to an existing story, a new story, or an explicit decision not to plan it
- [x] #5 `backlog doctor` passes
- [x] #6 A summary of counts by classification is written in the final summary
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Snapshot every open story (128 incl. this one and CSP-644 In Progress) with `backlog task list --exclude-status Done --json`; dump each `task view --plain` to /tmp/tuifw-baseline/766/ and grep for removed/moved identifiers and TUI-area words to order the reading; read every story.
2. Verify each removed/moved name against the tree (widgets/{form,multi_select,row_menu,mux_menu,popup_frame,toast,input}.rs, cursor.rs, detail*.rs, ui/text.rs gone; picker.rs and clean_up.rs remain as thin adapters; LaunchSpecFormState, TextInputState, FindPrompt, ViewerState, the three shims, Union/Forks, GroupAlign/PrAlign/MixedAlign gone; ControlsContext remains).
3. Classify each story (unaffected / path or terminology update / superseded or obsolete / re-scoping / newly blocked or unblocked) with reason and evidence; give every named overlap an explicit outcome; map each Tier 1 and Tier 2 gap to a story, a new story, or not planned; list the milestone's open follow-ups.
4. Write docs/plans/tui-framework/backlog-impact.md (counts, table, overlaps, gap map, follow-ups); record judgment calls in operator-review.md; commit.
5. Apply through the CLI: dated "TUI framework update" notes, retitles (CSP-430, CSP-432), CSP-342 scope rewrite, dependency changes (CSP-104, CSP-394), closes (CSP-435, CSP-455.01), the NO_COLOR/colour-depth story; comment only on CSP-644 if needed; `backlog doctor`; commit as chore(backlog).
6. `git diff --check`; finalize CSP-766.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-748 (ADR 0180), for the assessment of CSP-342: the transcript viewer now runs on tuikit's Pager, and src/viewer keeps only the transcript (model, parsers, rendering, the pager content with its find unit, hidden-match count, toggles, and opening at a search result). src/viewer imports nothing from crate::tui and lists tuikit in docs/transcript-viewer-deps.md. Lifting src/viewer into a workspace member therefore carries tuikit as a dependency and no reader of its own; its clap binary would draw Pager<Transcript>. Re-scope CSP-342 to that.

Review done on 2026-10-31 at `5fbd6154`: every open story's text was dumped and scanned for the removed or moved names and TUI words (scripts kept outside the repository), each name was checked against the tree, and every story was read. Results in `docs/plans/tui-framework/backlog-impact.md` (`f3dd2731`); CLI edits in `ffeb77a5`. The CSP-748 note on CSP-342 was applied as a rewritten description.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Reviewed the 128 stories open at the start (127 To Do, 1 In Progress; every milestone and none) against the framework's final shape, recorded each in `docs/plans/tui-framework/backlog-impact.md`, and applied the result through the CLI (`f3dd2731` the review, `ffeb77a5` the backlog edits).

**Counts by classification:** unaffected 74; needs a path or terminology update 38; needs re-scoping 13; newly blocked or unblocked 1; superseded or obsolete 2 (total 128).

**Changed (dated "TUI framework update" note, author's text kept):** CSP-105, CSP-172, CSP-175, CSP-180, CSP-183, CSP-185, CSP-188, CSP-191, CSP-192, CSP-193, CSP-247, CSP-268, CSP-269, CSP-294, CSP-318, CSP-318.01, CSP-322, CSP-323, CSP-345, CSP-346, CSP-354, CSP-355, CSP-392, CSP-393, CSP-413, CSP-455, CSP-494, CSP-543, CSP-544, CSP-547, CSP-551, CSP-616.09, CSP-655 (references updated too), CSP-687, CSP-689, CSP-699, CSP-700, CSP-702.

**Re-scoped:** CSP-171.02, CSP-179, CSP-270, CSP-319, CSP-342 (description rewritten for the viewer on `Pager`, original kept), CSP-344, CSP-390, CSP-391, CSP-394 (dependency on CSP-193 removed), CSP-430 (retitled to `ratatui-textarea`), CSP-432 (retitled: the framework spinner, for the loading placeholders), CSP-434, CSP-541.

**Newly blocked:** CSP-104 now depends on CSP-541: the package depends on `tuikit` by path and `tuikit` is `publish = false` (ADR 0150), so `cargo publish` cannot pass until the crates.io choice includes it.

**Closed:** CSP-435 (won't do: `tuikit::explorer`, ADR 0188, and `tuikit::tree`, ADR 0182) and CSP-455.01 (superseded by ADR 0168's one ring with `Enter` submitting from any field; its form is gone).

**Created:** CSP-803 (Medium, bug): the TUI resolves `--color` and `NO_COLOR` into `TuiOptions::color` and never reads it, and nothing downgrades colors to the terminal's depth (the Tier 1 gap with no story).

**Not edited:** the 74 unaffected stories, including CSP-644 (In Progress for another agent).

Every known overlap the scope names has an outcome in the review's overlap table (CSP-190 was already Done). Every Tier 1 and Tier 2 gap maps to a story or a reason: Tier 1 palette CSP-392, availability CSP-776, spinner CSP-432, presets CSP-269, `NO_COLOR` and color depth CSP-803, and bracketed paste, select-all, and style-aware goldens done; Tier 2 multi-line CSP-430, completion CSP-393 and CSP-455, shared schema CSP-179, resizable split CSP-319, mouse CSP-344 and CSP-355, density CSP-270, and validated input, progress bar, light and dark adaptation, and user remapping not planned (reasons in the review). The runtime evaluation's outcome (ADR 0200) is folded into CSP-342, CSP-185, and CSP-644's classification.

Open follow-ups the milestone leaves: CSP-776, CSP-802, CSP-770, and CSP-803; flagged for the operator: CSP-193 and CSP-098 may already be delivered, and CSP-191 closes with CSP-270. Judgment calls are in `docs/plans/tui-framework/operator-review.md`; the plan README and `docs/index.md` point at the review.

**Regressions:** none; no TUI changes. **Consistency wins:** none; no TUI changes.

Verified: a listing of open stories after the edits has a row in the review for every one (the only open story without a row is CSP-803, which the review created and lists under follow-ups); every edited story carries the note (51 notes, CSP-342's rewritten description, two final summaries); `backlog doctor` passes; `git diff --check` passes. Docs and backlog only, so no test suite was run.
<!-- SECTION:FINAL_SUMMARY:END -->
