---
id: CSP-722
title: Move the vetted glyph table to `glyphs` and retire duplicate glyph literals
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
references:
  - docs/adr/0073-node-kind-visual-identity.md
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
priority: medium
type: enhancement
ordinal: 808000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`icons.rs` mixes a generic glyph table (marks, width tests, the "every glyph is vetted" scanner) with Conspectus node-kind and state glyphs. About 25 glyph literals bypass it (13 `▲`, 4 `►`, 3 `√`, 2 `×`), `marks::FAIL`, `POINTER`, and `BACK` are never used, and `NO_VALUE` is used 4 times against about 65 literal `—`. The row cursor marker is `"> "` at 10 sites and `►` at 4, and the chrome has 9 hard-coded glyph sites (inventory slices 01 and 07).

Scope: Create `tuikit::glyphs` with the marks (pointer, disclosure, ok, fail, back, ellipsis, no-value), the vetting table, the cell-width checks, and the scanner that fails when a source file contains an unvetted glyph. Move `scripts/glyph-widths.py` next to it. Conspectus registers its node-kind and state glyphs into the table from an adapter. Replace the duplicate literals with the marks. Preserve the two operator-preference exceptions (`▶` and the Braille spinner).

TUI changes:
- Consistency wins: one source for every glyph, so a glyph cannot drift between its uses; the chrome's glyphs are covered by the width test
- Regressions or behavior changes: none expected

Tests and checks: The glyph-width scanner covers both crates' sources and passes. A test fails on a literal glyph in framework source that is not from `glyphs`.

Blockers: The independence guard and the documentation skeleton.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::glyphs` owns the marks, the vetted-glyph table, the width checks, and the scanner; the scanner covers `tuikit` and `conspectus` sources
- [ ] #2 The unused marks are either used or removed, and the duplicate literals are replaced
- [ ] #3 Conspectus registers its node-kind and state glyphs through an adapter
- [ ] #4 The operator-preference exceptions are preserved and documented
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
