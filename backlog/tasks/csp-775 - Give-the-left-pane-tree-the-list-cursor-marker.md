---
id: CSP-775
title: Give the left pane tree the list cursor marker
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 19:16'
updated_date: '2026-10-30 06:20'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-751
priority: medium
ordinal: 861000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
House rule 1 of ADR 0151 puts the `marks::POINTER` marker in a 2-cell gutter before the cursor row of every list, so the cursor stays visible without color. The selection extraction gave every menu, form, picker, search list, checklist, Messages, and the right pane that gutter, but kept the left pane's tree as it was: its rows start with the disclosure and kind-glyph columns that `docs/tui-columns.md` documents, and a gutter moves every column and every documented frame by two cells. ADR 0159 decision 6 leaves the gutter to the tree's column schema, where the tree's columns are decided and its frames are regenerated anyway. Until then the tree is the one list whose cursor row is shown by its highlight alone (reversed and bold focused, bold unfocused).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The tree draws its rows through `tuikit::selection::row` (or the tree control built on it), so the cursor row starts with the pointer and every other row with two blank cells
- [x] #2 No column moves when the cursor moves, and every view keeps its documented column grid one gutter in
- [x] #3 The frames in `docs/tui-columns.md` are regenerated with `just tui-columns` and its prose says where the marker sits
- [x] #4 The consistency suite checks the tree like the other lists: one marked row, the marker alone finding it with the selection pair emptied
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames of every view (first/middle/last cursor, folded/unfolded, focus on/off, 160/110/80/60/40 x 30) into /tmp/tuifw-baseline/frames-775-before.
2. tuikit::columns: the measure pass records the least lead it saw, and a schema whose lined-up columns absorb the lead gives way as the shallowest measured row would, so a lead every row shares (the cursor gutter) is not absorbed and floors hold. Unit tests and docs.
3. tuikit::tree: TreeView draws each row through selection::row (pending and ghost looks on the producer's spans, then the gutter and the highlight); RowCtx.width excludes the gutter; RowCtx::row replaces apply_looks for a host drawing one row alone; TreeColumns measures and draws with the gutter plus the indent as the schema's lead, so shares and row steps keep the whole row. Tests, rustdoc, tree_view.md, columns.md, selection.md.
4. Host: render_left_row and lay_out follow; the mux name steps (by the cells left) move by the gutter so the name steps at the same pane widths; update tests that read left-pane columns; consistency suite: one marked row per view in the tree's gutter, marker alone finds it with the selection pair emptied, no column moves when the cursor moves, unfocused keeps the marker.
5. Regenerate docs/tui-columns.md frames, edit its prose (where the marker sits, the cursor glyph row, the mux steps), review and accept insta snapshots with reasons.
6. ADR for the tree's gutter and the shared-lead rule; pointers from ADRs 0159/0182/0183; operator-review rows.
7. After frames, compare by category; full checks; commit in slices; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Commits: fba3a0c2 (columns: a shared lead is not absorbed), c4bb87bc (the tree draws the gutter; host, snapshots, tui-columns frames), 80d5ff9e (consistency checks), f1ceb038 (ADR 0186, design, changelog, operator review).
Frames: /tmp/tuifw-baseline/frames-775-{before,after} (420 left-pane frames: 7 listings x folded/unfolded x first/middle/last x focus on/off x 160/110/80/60/40 by 30, plus 7 screens at 110) and frames-775-after-plus2 (42/62/82). Every after frame has exactly one marked row, on the before frame's cursor row; cell styles after the gutter are unchanged where the text is; the screens change only inside the left pane. Rows: 5424 identical after the gutter, 1348 differ only in the fill, 414 narrow a cell by at most two cells, 206 lose a cell at 40/60/110 by the documented order. At the stacked widths every row other than hit, pane, and pane-hit rows and mux names reads in a pane two cells wider exactly as it read before.
Checks: cargo fmt --check; clippy --workspace --all-targets --all-features -D warnings; full isolated suite 4474 passed, 0 failed, 1 ignored; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps; cargo build --examples; git diff --check; tuikit independence guard.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed the list cursor marker in the left pane's tree (ADR 0186).

- `tuikit::tree::TreeView` draws every row through `selection::row`: `►` and a space in a 2-cell gutter on the cursor row, focused or not, two blank cells on every other row. The producer is told the width after the gutter; `RowCtx::row(line)` replaces `apply_looks`; of two copies of an identity only the cursor's is the cursor row.
- `TreeColumns` passes the gutter plus the indent as each row's schema lead, so shares and row steps count the whole row as in the search modal. `tuikit::columns` keeps a lead every row shares out of an absorbing column (`Measure::least_lead`), so header and Pinned label floors hold.
- Host: the mux name's steps moved by the gutter (37/71/103) so every pane width keeps its name width; no other threshold moved. `docs/tui-columns.md` frames regenerated with `just tui-columns`; the gutter is cell 1 of the common row prefix, and the selection, glyph, room, and mux-step prose say so.
- Consistency suite: in every view at every cursor position, focused and not, one row carries the marker and the highlight, the marker alone finds it with the selection pair emptied, and the text after the gutter never moves.
- 17 insta snapshots re-accepted, each for the gutter shift and marker (Pinned 70/110/160, by project and kind, History flat and by project, the history and pin row menus, six PR states); ungrouped Pinned at 110 also drops its content (under 7 cells). Five checks follow the gutter (listed in the operator review).

Verified with `conspectus tui --snapshot` at 160/110/80/60/40 x 30 (420 frames before and after, plus 42/62/82), fmt, clippy -D warnings, the full isolated suite (4474 passed, 0 failed, 1 ignored), rustdoc -D warnings, `cargo build --examples`, `git diff --check`; just check's steps run individually.

**Regressions**
- Every left-pane row moves two cells right; each row's fill (preview, snippet, content, PR title, directory) ends two cells sooner.
- Narrow panes give way two cells sooner, in each row's documented order: at 40 columns the Mux view drops its ages (names widen), the Sessions view its keys and header paths, Pinned labels narrow by two; at 110 ungrouped Pinned content and a Search view flat project can leave and hit labels narrow; at 130 a nested pane row drops its age.
- The Search header line and the Pinned banner stay flush with the border, two cells left of the rows.

**Consistency wins**
- The tree's cursor row starts with `►` like every other list, focused and unfocused, so it shows without color or reverse video; the highlight looks are unchanged.
- No column moves when the cursor moves; a row listed twice is marked and highlighted once.
- Text snapshots and the reference frames show where the cursor is; mux name widths and the hit rows' modal cells hold at every pane width.
<!-- SECTION:FINAL_SUMMARY:END -->
