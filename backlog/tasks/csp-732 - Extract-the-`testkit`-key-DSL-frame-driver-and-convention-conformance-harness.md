---
id: CSP-732
title: >-
  Extract the `testkit`: key DSL, frame driver, and convention conformance
  harness
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-29 02:49'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-719
  - CSP-720
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/adr/0067-tui-snapshot-mode-for-agent-iteration.md
priority: high
type: enhancement
ordinal: 818000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The test and preview machinery is reusable and partly duplicated: 23 test files define their own key helper, there are four copies of `strip_ansi`, and the modal registry is kept by hand in four places. `tui --snapshot` cannot drive 10 of the 16 overlay key actions, and insta goldens drop styles, so colour regressions are invisible to the suite. About 30 of the 204 consistency tests lift directly into helpers, about 55 more are Conspectus instances of generic rules, and about 115 are pure domain (inventory slice 06, section 6).

Scope: Create `tuikit::testkit` with the vim-style key DSL, buffer inspection helpers (token-column scraping, cell-style probes), the ANSI-preserving one-frame driver behind `tui --snapshot`, one `strip_ansi`, a style-aware golden format that keeps colour, the overlay and key conformance harness lifted from `consistency_tests/{layout,keys,help}.rs` as `assert_conventions(&Registry)` (nothing a modal draws escapes its frame at 5 widths by 2 heights, selection moves no text, no word is cut, key tables match the convention, footers are generated), and the source-scan helpers from `w_scan`. Conspectus keeps its domain tests and calls the kit. Extend the snapshot driver so it can drive every overlay key action. Keep the existing `tui-pantry` preview example working; add no new gallery or coverage rule.

TUI changes: none expected in the product. `--snapshot` gains the ability to drive every overlay key action, a developer-tool improvement.

Tests and checks: The ported conformance checks pass for the existing modals and fail for a deliberately broken fixture modal. The style-aware golden fails when only a colour changes.

Blockers: The scan extension and the independence guard.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::testkit` provides the key DSL, buffer helpers, the ANSI-preserving frame driver, one `strip_ansi`, and a style-aware golden format
- [x] #2 `assert_conventions(&Registry)` runs the lifted layout, key, and footer checks and fails for a deliberately broken fixture modal
- [x] #3 The 23 per-file key helpers and four `strip_ansi` copies are replaced by the kit
- [x] #4 `tui --snapshot` can drive every overlay key action
- [x] #5 The `tui-pantry` preview example still builds and runs
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (/tmp/tuifw-baseline/frames-732-before): representative views, every kind of modal, explorer, viewers at 120x40 and 60x20, plus the overlay picks and submits the snapshot driver used to skip.
2. tuikit: add a `testkit` cargo feature (off by default; compiled for the crate's own tests). Modules: `keys` (the `--snapshot-keys` DSL parser moved from src/tui/snapshot.rs, plus event constructors), `render` (one-frame TestBackend driver generic over a draw callback), `buffer` (text, token columns, cell-style probes, find text, frame rect, inner rows), `ansi` (the ANSI writer moved from snapshot.rs, adding the blink, hidden, and crossed-out modifiers the theme grammar allows), `golden` (a style-aware text format with a legend, compare, and a file check whose accept step is an environment variable and a host-supplied writer, since the framework writes no files), `conformance` (`Registry`, `Rig`, `Case`, `check_conventions`/`assert_conventions`, `same_chrome`), and `source` (one Rust lexer shared with the glyph scanner: sanitize, test-item stripping, string literals, word-start matching, a read-only file walk).
3. `text::strip_ansi` becomes the one ANSI stripper (production code needs it for captured pane text); testkit re-exports it.
4. Conformance checks lifted from consistency_tests/{layout,keys,help,registry}.rs: drawn, contained at 5 widths x 2 heights, edges are frame glyphs, one title form, selection moves no text, no word cut at the edge, key convention by role plus the table checks, footer generated from its table, Ctrl-C/Esc/q close rules and silent close. Unit tests with a good fixture modal and a deliberately broken one (overflows, cuts words, hand-writes its footer, Enter leaves).
5. Host adoption: dev-dependency with the feature and `snapshot = ["tuikit/testkit"]`; snapshot.rs uses the kit (parser, driver, writer); the consistency suite builds one registry and calls assert_conventions; rendering/state/styling use kit tokens and probes; help.rs uses same_chrome; replace the per-file key helpers and the four strip_ansi copies; ui.rs/viewer buffer_to_string and render_to_buffer delegate to the kit; text.rs and overlay_keymap_hygiene use kit source helpers; framework guards share the lexer; pantry uses kit key constructors.
6. Snapshot extension: every message an overlay sends runs through the reducer; effects that reach outside (exec, mux, store writes) are skipped with a stderr note naming them; test that every registered modal's keys are driven.
7. Docs: crates/tuikit/docs/controls/testkit.md, architecture/README/contributing updates, raw-style guard exception for the ANSI writer, ADR 0164 + index, design.md line, dev-scenarios note, operator-review judgment calls.
8. After frames, byte-identical check for the representative states; fmt, clippy, full isolated suite, cargo doc -D warnings, cargo build --examples, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in six commits: the framework's `testkit` module behind a `testkit` feature (key scripts, `FrameDriver`, buffer probes, ANSI writer, style-aware goldens, the conformance suite, source helpers on the crate's one lexer), snapshot mode on the kit with every overlay message through the reducer, one `strip_ansi` in `tuikit::text`, the host's key helpers on the kit, the host conformance registry (`consistency_tests/conventions.rs`), and ADR 0164.
Frames: 46 before/after `--snapshot` captures (23 states at 120x40 and 60x20, in /tmp/tuifw-baseline/frames-732-*): 44 byte-identical; the two pane-viewer `a` frames changed on purpose (the attach key now reaches the reducer). stderr notes name the skipped effect (`skipped a store write`) instead of `skipped a pick or submit`.
Checks: cargo fmt --check, clippy --workspace --all-targets --all-features -D warnings, the isolated suite (3948 passed, 0 failed, 1 ignored), RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps (and with --all-features), cargo build --examples, pantry --list and --dump, git diff --check: all pass. Each slice was checked with fmt, clippy, the framework suite, the host lib tests, and the scan tests before it was committed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::testkit` behind a `testkit` cargo feature (off by default; the host enables it from its dev-dependencies and its `snapshot` feature; the framework depends on itself by path for its own tests, which the manifest guard now accepts). It holds the key-script notation `--snapshot-keys` used (moved from `src/tui/snapshot.rs`, same syntax and error messages) and press constructors; `FrameDriver`, the one-frame driver over a draw callback; buffer probes (text, token columns, the cell and style under text, matching-cell counts, a modal's frame rectangle and inner rows, repainted rows); the ANSI writer (byte-for-byte, plus blink, hidden, crossed out); style-aware goldens (text rows, style rows, legend; a color-only change fails; `assert_golden` with `TUIKIT_ACCEPT_GOLDENS=1` and a host writer); the conformance suite (`assert_conventions(&Registry)` over a host `Rig`: drawn, contained at 5x2 sizes, title form, selection moves no text, no word cut, keys by role plus the table checks, generated footers, close keys), with a broken fixture modal that fails seven checks; and `testkit::source` on the crate's one lexer, shared with the glyph scanner. `tuikit::text::strip_ansi` replaces the four copies. Twenty-six host test files (and two framework ones) lost their key helpers; `consistency_tests/conventions.rs` registers every modal once and runs the suite; the framework guards, the host tree walk, and the text, mark, list-cursor, and overlay-keymap scans use the kit. Snapshot mode runs every overlay message through the reducer and skips only outside effects with a note naming them. ADR 0164; operator-review rows for the judgment calls.

**Regressions**
- None in the product: 44 of 46 before/after `--snapshot` frames (23 states at 120x40 and 60x20) are byte-identical, and the other two are the snapshot-mode change below.
- Snapshot mode (dev tool only): an outside effect a pager or the search used to refuse there with the "that isn't available in this mode" toast is now skipped with a stderr note, so that toast no longer appears in such a frame.

**Consistency wins**
- None in the product.
- Snapshot mode (dev tool): every overlay key action is drivable. Picks and submits reach the reducer (the pane viewer's `a` closes it and selects the mux, as live; a row-menu pin and a rename submit run their reducer arms), and the note names the skipped effect (`skipped a store write`, `skipped an attach`) instead of `skipped a pick or submit`. The writer also encodes the blink, hidden, and crossed-out modifiers a theme may set.

Verified with: cargo fmt --check; clippy --workspace --all-targets --all-features -D warnings; the isolated suite (3948 passed, 0 failed, 1 ignored); cargo nextest; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps (also with --all-features); cargo build --examples and pantry --list/--dump; git diff --check; the frame captures above.
<!-- SECTION:FINAL_SUMMARY:END -->
