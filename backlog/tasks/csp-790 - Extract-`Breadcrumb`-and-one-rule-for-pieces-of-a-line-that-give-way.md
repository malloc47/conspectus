---
id: CSP-790
title: Extract `Breadcrumb` and one rule for pieces of a line that give way
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 08:09'
updated_date: '2026-10-30 09:48'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-755
documentation:
  - docs/plans/tui-framework/explorer-design.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 876000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The right pane's title draws the drill chain with `render_breadcrumb_chain` (about 140 lines in `src/tui/explorer.rs`): a fixed ladder of four forms (every hop, `first › … › last`, `… › last`, the last hop alone) under an 8-cell floor that lets the pane border clip the title. The tab strip (`tab_strip_fit`, CSP-716) and the status bar's priority drop fit their pieces with their own code. The inventory (slice 07, sections 3.7 and 8) and CSP-758 call this one fit problem. The explorer design (docs/plans/tui-framework/explorer-design.md, section 4.1) specifies one rule (pieces with ranks give way lowest first, each run that gave way leaves one mark, and a piece that cannot give way is cut last) and a `Breadcrumb` built on it. This is step 1 of that design's migration; `TabBar` and `PriorityLine` (CSP-758) adopt the same rule.

Scope: Add the rule to `tuikit::text` and the separator mark to `tuikit::glyphs::marks`; add `tuikit::breadcrumb` (hops with an optional glyph, a tag, and a suffix shown only when two tags repeat) with its control page. The right pane's title draws its chain through it. Conspectus keeps the hop data: the kind glyph and its color, the tag from the node's short label, the id suffix, and the title's other parts (focus marker, kind label, back mark, depth).

TUI changes:
- Consistency wins: the chain keeps as many hops as fit, dropping the oldest middle hops first, instead of jumping from the whole chain to `first › … › last`; the last hop is cut with `…` when even it does not fit, instead of the border clipping the title
- Regressions or behavior changes: chains between their full width and the old elided form read differently (more hops shown); the 8-cell floor goes

Tests and checks: framework tests for the rule (kept pieces, ranks, one mark per run, mark at a gap or at the end, the last-resort cut, widths in cells) and for `Breadcrumb`; the six `render_breadcrumb_chain` tests in `explorer_tests.rs` ported; `conspectus tui --snapshot` of a two- and three-hop drill at 160, 100, 60, and 40 columns.

Blockers: the design (CSP-755).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::text` has one rule that keeps the pieces of a line by rank and marks each run that gave way, and `tuikit::breadcrumb::Breadcrumb` draws a chain with it, as the explorer design specifies
- [x] #2 The right pane's title draws its chain with `Breadcrumb`; the kind glyphs, their colors, the tags, and the duplicate suffix come from a Conspectus adapter
- [x] #3 The six breadcrumb tests in `explorer_tests.rs` are ported (the generic ones to the framework, the kind color one to the adapter) and none is dropped
- [x] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Framework rule: `crates/tuikit/src/text/elide.rs` with `Piece`, `MarkAt`, `Elided` (plus `Item` and `items()`/`width()` helpers), and `elide`, re-exported from `tuikit::text`; unit tests and fixed-seed property tests (kept pieces, rank order, one mark per run, gap or end, last-resort cut, widths in cells) in `text/elide_tests.rs`; `text.md` section.
2. Marks: `marks::CRUMB` (`›`) and `marks::SPACED_CRUMB` (` › `) in `glyphs::marks` and `marks::ALL`; the vetted table's typed `›` entry goes through the constant; host guard `guarded_marks` gains `CRUMB`; `glyphs.md` table row.
3. Framework control: `crates/tuikit/src/breadcrumb.rs` with `Hop` (glyph span, tag, suffix shown only when two tags repeat) and `Breadcrumb` (theme's `inert` by default, `.style`, `.line(room)`), ranks per the design (last unranked, first above the middle, oldest middle lowest), `MarkAt::Gap`; tests porting the five generic `render_breadcrumb_chain` tests plus rule tests; page `docs/controls/breadcrumb.md`; module map, README, architecture rows.
4. Host: replace `render_breadcrumb_chain` and helpers with a `breadcrumb_hops(&[BreadcrumbHop], &Theme) -> Vec<Hop>` adapter; `right_panel_title` measures its fixed parts from the spans it draws, gets the cells the border leaves (width - 2), and gives the chain the rest with no 8-cell floor; kind-colour test stays as an adapter test.
5. Frames before and after (two-, three-, five-hop drill with a repeated tag) at 160/100/60/40 into /tmp/tuifw-baseline/frames-790-*; CHANGELOG, design.md/ADR 0188 status lines, operator-review rows.
6. fmt, clippy, full isolated suite, doc, examples, diff --check; final summary with Regressions and Consistency wins.

7. As built: the title's trailing pad gives way before any hop (drawn only when the chain leaves a cell), so a chain that used to fit only by losing the pad to the corner keeps its hops; Breadcrumb::new takes the theme.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: `text::elide` (d4a88764), `Breadcrumb` and `marks::CRUMB` (c64d22cf), the host adapter and title (97d32177), docs (2e8d18e3).
Found while measuring: `right_panel_title` was handed the pane's whole width, not the cells its corners leave, so a chain that filled its budget lost the title's pad and the depth's digit to the corner. Fixed with the inner width.
Frames before/after in /tmp/tuifw-baseline/frames-790-before and -after (two-, three-, and five-hop drills at 160/100/60/40, plus dup-180, dup-32, two-28). Only dup-160, dup-180, dup-32, and two-28 differ in text; every drilled frame differs in ANSI only by the space after each hop glyph, now in the chain's inert color.
Checks: fmt, clippy (workspace, all targets, all features, -D warnings), full isolated suite 4532 passed / 0 failed / 1 ignored, tuikit doctests 103 passed, rustdoc -D warnings, cargo build --examples, git diff --check: all pass.
Filed CSP-793 (a non-bare repo's hop reads `.git`, and so does its suffix).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tuikit::text::elide` (`Piece`, `MarkAt`, `Elided`, `Item`): pieces measured in cells give way lowest rank first (the later of a tie), each run that gave way leaves one mark in its gap or once at the end, and when only unranked pieces are left the marks go and the last piece is cut to the room the others leave. Added `marks::CRUMB` (`›`) and `marks::SPACED_CRUMB`; the vetted table's typed `›` now goes through the constant and the host's mark guard covers it. Added `tuikit::breadcrumb::{Breadcrumb, Hop}` on that rule (last hop never gives way, first outlasts the middle, middle hops oldest first, `MarkAt::Gap`), with its page `crates/tuikit/docs/controls/breadcrumb.md`, the module map row, and README. The right pane's title draws its chain through `Breadcrumb`, with `breadcrumb_hops` in `src/tui/explorer.rs` supplying the kind glyph in its color, the tag without `kind:`, and the last four characters of the id; the title keeps its marker, kind glyph and label, ` ◄ `, and ` · depth N`, gets the cells its corners leave, and gives the chain the rest with no floor. The trailing pad gives way before any hop.

Tests: 13 `elide` tests (one a 20,000-case fixed-seed property test) and 12 `breadcrumb` tests. The six `render_breadcrumb_chain` tests are ported, none dropped. Five generic ones are framework tests: join, none when empty, middle elision (now also checks that at 44 cells the newer middle hop stays), the narrow fallback (at 5 cells the last hop is now cut to `* he…` where it used to be shown whole), and repeated tags. The kind color test stays in `explorer_tests.rs` as an adapter test, next to a new tag-and-suffix test. A new `ui_tests.rs` title test checks the fit at every width and that the corner clips nothing.

Verified with `conspectus tui --snapshot` of the right pane for a two-hop, a three-hop, and a five-hop drill with a repeated tag, at 160, 100, 60, and 40 columns, plus 180, 32, and 28 (before and after in /tmp/tuifw-baseline/frames-790-*). Checks: fmt, clippy -D warnings, full isolated suite (4532 passed, 0 failed, 1 ignored), tuikit doctests, rustdoc -D warnings, `cargo build --examples`, `git diff --check`, independence guard.

**Consistency wins**
- A chain keeps as many hops as fit, oldest middle hops first, instead of jumping to `first › … › last` (five-hop drill at 160: `● claude › … › ◆ .git·.git` became `● claude › … › ▦ showcase-deck › ◆ .git·.git`; the same at 180) (house rule 14, one fit rule).
- A last hop too wide for the room is cut with `…`, and the title stays inside its corners, so ` · depth N` is never clipped (32 columns: `◆ .git·.git ·┐` became `◆ … · depth 5┐`; 28: `◇ project · de┐` became `◇ p… · depth 2┐`). The 8-cell floor is gone, and the title measures the kind glyph instead of assuming two cells (rule 14).

**Regressions**
- On the narrowest titles the last hop's tag is cut to keep the depth whole; the old title kept the tag and the corner cut the depth instead.
- Two hops of different kinds with the same tag (a session and a mux both `abc`) now both carry the `·xxxx` suffix: the rule compares the tags drawn, not `kind:tag`.
- No visible change: the space after each hop's glyph now takes the chain's `inert` color (ANSI only). The two- and three-hop drills at all four widths, and the five-hop drill at 100, 60, and 40, read as before. The three-hop drill at 100 keeps `● claude › … › ◆ .git · depth 3┐` because the pad gives way before a hop.

Judgment calls for review are in operator-review.md: the theme-taking constructor and builder hops, repeats by drawn tag, the pad rule, and the strict give-way order CSP-758 inherits. Filed CSP-793.
<!-- SECTION:FINAL_SUMMARY:END -->
