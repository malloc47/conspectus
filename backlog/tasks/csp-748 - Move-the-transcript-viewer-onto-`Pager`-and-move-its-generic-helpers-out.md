---
id: CSP-748
title: Move the transcript viewer onto `Pager` and move its generic helpers out
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 21:27'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
references:
  - CSP-342
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0052-native-session-viewer.md
priority: high
type: enhancement
ordinal: 834000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The transcript viewer (`src/viewer/`, about 5.9k lines with parsers) holds both domain code (the model, parsers, gutter chips, tool phrases) and generic code the TUI already imports: `word_wrap`, `find_ci`, `snippet_around`, `FindPrompt`, and `paint`. That is a layering inversion. The viewer has its own `Theme` that borrows domain tokens (`harness_color`, `cwd_mark`, `pr_merged`) and 12 raw `Modifier::DIM` sites that bypass `placeholder` and `divider` (inventory slice 05). CSP-342 plans to lift the viewer into its own crate on the same workspace; the generic pager now goes to the framework and the transcript model stays.

Scope: Implement `PagerContent` for the transcript (turn units, gutter chips, the `t`, `T`, `I`, and `H` toggles, the hidden-match count, reveal-on-open). Move the generic helpers to `text` and `LineEdit`. Point the viewer's colours at generic tokens. Narrow ADR 0052's extractable-viewer boundary and re-scope CSP-342 in the final assessment.

TUI changes:
- Consistency wins: the transcript viewer and the pane viewer behave alike; the viewer follows the theme
- Regressions or behavior changes: the viewer's Esc and `q` follow the shared rule; `Ctrl-G` and any alias that the key-rule decision drops stop working

Tests and checks: Port the viewer tests (`viewer_tests.rs` and the widget tests). `--snapshot` of the viewer with a long transcript at 80x24 and 40x10, with find and with hidden matches.

Blockers: `Pager`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The transcript viewer is a `PagerContent` adapter on `Pager` with its toggles, hidden-match count, and reveal-on-open preserved
- [x] #2 `src/tui` no longer imports generic helpers from `src/viewer`
- [x] #3 The viewer's colours come from generic theme tokens and it has no raw `Modifier::DIM`
- [x] #4 ADR 0052's boundary is narrowed in the index and the effect on CSP-342 is noted for the closing assessment
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
1. Framework helper: `tuikit::text::paint_matches` (find_all_ci + paint after a skipped gutter), used by `Find::paint`, the viewer's turn painter, and the preview's pane rows; document it on the text page.
2. Theme: the viewer draws from `tuikit::theme::Theme` (drop the `src/viewer/theme.rs` re-export of the host theme). Generic tokens where the role is generic (user chip `accent`, cwd `accent` + `muted`, current match `selection_active`); named styles where it is domain: the title's harness color by a role name the bridge hands the content, and the compaction chip as a viewer-declared role (`transcript_compaction`, magenta) the host registers. Default frames unchanged.
3. Content adapter: `Transcript<M>` (document, toggles, layout cache, title role) implements `PagerContent`: cached layout and rows, header, toggle chips, turn find over the kinds shown, `match_rows` from the turn starts, paint after the gutter, the hidden-match count as `find_detail`, `describe` for the ends and find, the unavailable banner, and `t`/`T`/`I`/`H` as `TranscriptKey` (`TRANSCRIPT_KEYS`, help entries) returning `Relayout`. `open` (at the end) and `open_at` (reveal the toggles that show the hit, `land`, note). Tests.
4. Preview: hold the `Transcript` with the search's marks instead of `ViewerState`/`FindState`.
5. Switch the TUI: `Modal::Viewer(Box<TranscriptViewer>)` (`Pager<Transcript<Msg>>`), the bridge builds it; delete `Msg::Viewer`/`Msg::ViewerFind` and their reducer arms, the entries impl, `VIEWER_KEYS`/`VIEWER_FIND_KEYS`, `viewer_help.rs`, `ViewerState`, `FindState`, the reducers, chrome, footer, and find prompt; port the viewer, bridge, app, runtime, key-table, and consistency tests.
6. Docs: ADR 0180 (narrows ADR 0052's boundary; amends 0161, 0162, 0177, 0178), the ADR index, pager.md Ports, transcript-viewer-deps.md, the viewer module docs, operations.md, CHANGELOG, operator-review rows; note the CSP-342 effect for CSP-766.
7. Before/after `--snapshot` frames (long transcript, find, hidden matches, toggles, help, reveal, narrow) at 100x30, 60x20, 40x10; full checks; close the story.

8. As built: Modal::Viewer holds the pager unboxed (boxing it tripped clippy's large-variant check), and the generic composition left in src/viewer (paint_after) became tuikit::text::paint_matches.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in seven commits: `text::paint_matches` (one row painter for the pager's find, the viewer, and the Preview); the viewer on `tuikit::theme` (user chip `accent`, cwd `accent`+`muted`, compaction chip the new host role `transcript_compaction`, title by the role the bridge hands it, current match `selection_active`; the selection scan's last `REVERSED` exception went); `Transcript<M = ()>` implementing `PagerContent` with `TranscriptKey`/`TRANSCRIPT_KEYS` in `src/viewer/keys.rs`; the Preview onto `TranscriptPreview` (content plus `Marks`); the switch (`Modal::Viewer(Pager<Transcript<Msg>>)`, deleting `ViewerState`, `FindState`, both reducers, `Msg::Viewer`/`Msg::ViewerFind`, the entries impl, `VIEWER_KEYS`, `VIEWER_FIND_KEYS`, `viewer_help`, and the viewer's chrome); and docs (ADR 0180, the deps manifest, operations, CHANGELOG, design, pager.md Ports, operator review).

Generic helpers: `word_wrap`, `find_ci`, `snippet_around`, `FindPrompt`, and `paint` had already moved (CSP-723/733); the last generic composition left in `src/viewer` (`paint_after`) became `tuikit::text::paint_matches`, and `draw_find_prompt` went with the chrome. `src/tui` now imports only domain items from `src/viewer` (model, search types, the content, its keys and marks).

`Modal::Viewer` is unboxed: boxing it made the search overlay the largest variant by 400 bytes and clippy refused. A preview commit's doc link to a private item failed rustdoc; it was folded into that commit before the docs landed, and every story commit was rebuilt with `RUSTDOCFLAGS='-D warnings' cargo doc` (all pass).

Frames: 120 before/after captures (40 key scripts at 100x30, 60x20, 40x10 over a long Claude transcript with thinking, tools, an aborted exchange, a compaction, a table, and "needle" in shown and hidden turns), plus 40 at 80x24, in /tmp/tuifw-baseline/frames-748-*. 17 (5 at 80x24) are byte-identical; 99 (32) in all, identical ones included, differ at most in the footer row with header and body byte-identical, styles included; the rest differ in the body by the stick-to-end rule after a toggle at the end (aborted, think, tools-sum, all-on, find-shown) and in the help page (section order, `Esc` and `G` words). Classes in frames-748-classes.txt, rows in frames-748-diff.txt.

Checks: fmt clean; clippy (workspace, all targets, all features, -D warnings) clean; full isolated suite 4366 passed, 0 failed, 1 ignored (13 fewer than 4379: the generic reducer tests went with the reducer and the interim help adapter's tests with it; the pager's own tests cover them); rustdoc -D warnings clean at every commit; `cargo build --examples` clean; `git diff --check` clean; tuikit independence and raw-style guards pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The transcript viewer is the framework's `Pager` (Screen mode) over the viewer's `Transcript<M>` content (ADR 0180). The content lays out the turns behind their gutter chips, finds shown turns, counts matches the toggles hide (`N in hidden turns`), names its toggles in chips, rewords `g`/`G`/`/` on the help page through `describe`, and takes `t`, `T`, `I`, and `H` as `TRANSCRIPT_KEYS` (Relayout). Opening at a search result reveals what hides the hit, lands on it, and notes it. `ViewerState`, `FindState`, both reducers, `Msg::Viewer`/`Msg::ViewerFind`, `VIEWER_KEYS`, `VIEWER_FIND_KEYS`, the interim `viewer_help`, and the viewer's own chrome are gone. The viewer draws from `tuikit::theme` (no raw styles, no `crate::tui` import, `src/viewer/theme.rs` names its two roles), one painter (`tuikit::text::paint_matches`) serves the pager, the viewer, and the Preview, and the Preview holds the content with its marks. ADR 0052's boundary is narrowed and marked in the index; the effect on CSP-342 is noted on CSP-766.

**Regressions**
- A toggle pressed at the end keeps the view at the end (the pager's rule), so a find confirmed after it starts from the end and may wrap to the first match; the old viewer let go of the end on any key but `G`.
- Hiding the turn the find is on leaves the find on no match (`find "q" 3 matches`) until `n`/`N`; the old viewer silently picked the first match at the top.
- A narrow footer drops the toggle chips from the last (at 40 columns only the position, find chip, and `Esc/q close` remain); the old footer kept the tools and thinking chips and whichever of aborted or hidden filtered something.
- The help page lists Finding before Showing more or less and uses the pager's `Esc` words (`close this help`); `G` reads `Jump to the end, where the viewer opens, and stay there`.
- A theme that set `harness.codex`, `cwd_mark`, or `pr_merged` no longer recolors user turns, the cwd, or compaction summaries; `accent` and the new `transcript_compaction` do. Defaults are byte-identical.
- Aliases the key rule dropped (`Ctrl-G` and the other character chords) do nothing, as before this story.

**Consistency wins**
- The transcript viewer and the pane viewer behave alike: `Esc` steps back (help, find, viewer), `q`/`Ctrl-C` leave from anywhere, a find always lands one row below the top, paging moves by the rows shown, and both show `[ n/m ]` and the same generated footer (`/ find · ? help · Esc/q close` after the chips, `j/k/PgDn/PgUp scroll` when it fits).
- The footer always keeps `Esc/q close` (at 40 columns the old one lost its close hint mid-chip).
- The note from opening at a search result (`showing full tool output`) is a chip of its own in its severity's tone, and a gone query's note is a warning.
- A transcript whose every turn is hidden says `every turn here is hidden` instead of an empty body.
- The viewer follows the theme throughout, and the current match takes `selection_active`.

Verified with 160 `conspectus tui --snapshot` frames before and after (long transcript at the top, scrolled, paged, find with matches, stepping and wrapping, hidden-only matches, the prompt, toggles on and off, the help page, close keys, reveal-on-open from Search view hits, an unavailable transcript; 100x30, 80x24, 60x20, 40x10): 131 of 160 are identical or differ only in the footer (their header and body byte-identical, styles included), and the rest differ by the end-stick rule after a toggle and the help page's order and words. Tests: the viewer, find, highlight, help, bridge, app, runtime, key-table, conventions, help-parity, scrolling, and text-entry tests drive the pager; ten viewer snapshots changed only in the footer and three help-page snapshots moved to the viewer. Full isolated suite 4366 passed, 0 failed, 1 ignored; fmt, clippy -D warnings, rustdoc -D warnings (at every commit), `cargo build --examples`, `git diff --check`, and the tuikit independence guard pass.
<!-- SECTION:FINAL_SUMMARY:END -->
