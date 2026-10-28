---
id: CSP-722
title: Move the vetted glyph table to `glyphs` and retire duplicate glyph literals
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 11:49'
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
- [x] #1 `tuikit::glyphs` owns the marks, the vetted-glyph table, the width checks, and the scanner; the scanner covers `tuikit` and `conspectus` sources
- [x] #2 The unused marks are either used or removed, and the duplicate literals are replaced
- [x] #3 Conspectus registers its node-kind and state glyphs through an adapter
- [x] #4 The operator-preference exceptions are preserved and documented
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
1. Framework: add `tuikit::glyphs` (marks incl. pointer, back, disclosure, ok, fail, warning, bullet, ellipsis, no-value, rule, spinner; `cell_width`; `glyph_risk`; `VettedGlyphs` table with registration; a pure lexing scanner that reports non-ASCII characters drawn in code outside comments and `#[cfg(test)]` items). Move `scripts/glyph-widths.py` to `crates/tuikit/scripts/`, made directory-generic. Framework guard test: no glyph literal in framework source outside `glyphs`, and every glyph in `glyphs` is vetted. Control page `crates/tuikit/docs/controls/glyphs.md`; flip the module-map row; raise `FRAMEWORK_SRC` minimum.
2. Host adapter (`src/tui/icons/vetted.rs`): registers node-kind, state, untracked, viewer-table, and the few non-terminal glyphs into the framework table; `icons.rs` drops `VETTED`, `glyph_risk`, `marks`; `parse_icon_override` measures with `cell_width`. Host scan test walks `src` and `crates/tuikit/src` through `tests/support/source_tree.rs` and requires every drawn glyph to be vetted.
3. Replace duplicate literals with the marks at each site, keeping the glyph each site shows (`►` stays `►`, `▶` stays `▶`); use `POINTER`, `BACK`, `FAIL`; add a host guard against re-introducing a mark's glyph as a literal. Capture `tui --snapshot` frames before and after (normal and narrow) and confirm byte-identical.
4. Docs: ADR for the glyph split and scan scope, ADR 0073 status note, `docs/tui-columns.md` (exceptions), operator-review rows, README/architecture/contributing in the framework.
5. Checks: fmt, clippy, full isolated suite, rustdoc, `git diff --check`; finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: framework `glyphs` module and guard (9347fac); host adapter and the whole-`src` vetting scan (21d42bb); mark sites and the typed-mark guard (03a8069); ADR 0154 and docs (4233b5f).
Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, and `cargo test -p tuikit --test independence` all pass. The isolated full suite passed 3632, failed 0, ignored 1 (no flakes this run). `conspectus tui --snapshot` frames (106: left pane in sessions, mux, PRs, pinned, history; right pane, drills with the back mark, the Other zone, forms with the path check mark, help, Messages, search, menus, viewer; at 100x30 and 60x24) are byte-identical before and after (/tmp/tuifw-baseline/frames-722-before and -after). The Messages log-level glyphs and the multi-select bullet are not reachable in snapshot mode; their sites changed constant only and their unit tests pass.
Judgment calls are in docs/plans/tui-framework/operator-review.md.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tuikit::glyphs` now owns the marks, the vetted-glyph table with registration, the cell-width and emoji-risk checks, and a pure source scanner; Conspectus registers its node-kind and state glyphs from `tui::icons::register_glyphs`. Every site that typed a mark's character names the constant instead, and a host guard plus a framework guard keep it that way. `marks::POINTER`, `BACK`, and `FAIL` are used; `ELLIPSIS` and `RULE` were added and used. `scripts/glyph-widths.py` moved to `crates/tuikit/scripts/` and takes directories. ADR 0154 records the decisions and amends ADR 0073.

Scope notes: the vetting scan walks the whole `src` of both crates (it was `src/tui`), so 18 glyphs the old scan never saw (the viewer's table borders and `↳`, the export's `★`) are now registered; `é` and `§` left the table (test code only). `▶` (the collapsed-row arrow, not the pin marker) and the Braille spinner are kept and documented as the two operator-preference exceptions. The `"> "` cursor sites are untouched; house rule 1 (CSP-727) unifies the cursor.

**Regressions**
- None. Default frames are byte-identical.

**Consistency wins**
- One definition per shared mark: 10 `▲`, 4 `►`, `◄`, 2 `√`, 2 `×`, the `▼`/`▶` pair, 6 `─` rules, 33 `—` placeholders, and 22 truncation `…` now come from `tuikit::glyphs::marks`, so a glyph cannot drift between its uses.
- The chrome's glyphs (focus marker, breadcrumb back mark, Other header arrows, ambiguity chips, rule cells) are covered by the same width and vetting checks as the left panel.
- The glyph scan skips comments and test items instead of whole lines and now reaches the viewer and export glyphs.

Verification: fmt, clippy (`--workspace --all-targets --all-features -D warnings`), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `git diff --check`, and the independence guard pass; the isolated full suite passed 3632, failed 0, ignored 1. 106 `tui --snapshot` frames at 100x30 and 60x24 (every left-pane view, right pane, drills, Other zone, forms, help, Messages, search, menus, viewer) are byte-identical before and after. The log-level glyphs and the multi-select bullet are not reachable in snapshot mode; their unit tests pass.

For the operator's review: the rows in docs/plans/tui-framework/operator-review.md (the two preference exceptions, the cursor sites left for CSP-727, the widened scan and its 18 registrations, ELLIPSIS/RULE and the guard's exceptions, `◌` moved to `state`).
<!-- SECTION:FINAL_SUMMARY:END -->
