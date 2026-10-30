---
id: CSP-790
title: Extract `Breadcrumb` and one rule for pieces of a line that give way
status: To Do
assignee: []
created_date: '2026-10-30 08:09'
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
- [ ] #1 `tuikit::text` has one rule that keeps the pieces of a line by rank and marks each run that gave way, and `tuikit::breadcrumb::Breadcrumb` draws a chain with it, as the explorer design specifies
- [ ] #2 The right pane's title draws its chain with `Breadcrumb`; the kind glyphs, their colors, the tags, and the duplicate suffix come from a Conspectus adapter
- [ ] #3 The six breadcrumb tests in `explorer_tests.rs` are ported (the generic ones to the framework, the kind color one to the adapter) and none is dropped
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
