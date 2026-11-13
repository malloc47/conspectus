---
id: CSP-749
title: Extract `richtext` and the preview-pane scroll region
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 22:42'
labels:
  - tui-framework
  - tuifw-reading
milestone: m-25
dependencies:
  - CSP-745
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0106-preview-pane-wrap-modes.md
priority: medium
type: enhancement
ordinal: 835000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The preview pane combines a generic scroll region (anchoring at the top, the bottom, or a match) and ANSI wrap modes with domain sources and loaders. The markdown, table, and syntax rendering (`tui-markdown`, `syntect`, `comfy-table`) and the GFM segmenter sit in `src/viewer/render.rs` and `table.rs`. ADR 0152 puts the heavy dependencies behind a cargo feature (inventory slice 05).

Scope: Create the `richtext` feature with the ANSI captured-pane layout and wrap modes (`preview_wrap.rs`), markdown and table rendering, and syntax highlighting, and a `ScrollRegion` for the preview pane with Top, Bottom, and Match anchoring. Preview sources, loaders, the store, and the banner stay in Conspectus, which enables the feature.

TUI changes: none expected. The preview wrap modes and the transcript and pane previews render as before.

Tests and checks: The `preview_wrap` tests and the preview-pane tests pass. A build without the `richtext` feature compiles and tests the rest of the framework.

Blockers: `Pager` and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The `richtext` feature owns ANSI layout, wrap modes, markdown, tables, and syntax highlighting, and `syntect`, `tui-markdown`, and `comfy-table` are optional dependencies
- [x] #2 The preview pane's scroll region lives in the framework with Top, Bottom, and Match anchoring
- [x] #3 `tuikit` builds and passes its tests with the feature off
- [x] #4 Preview rendering is unchanged: the preview tests pass without edits to expectations
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
1. Capture before frames of the preview pane (pane capture in smart, plain, and no wrap; a session's preview; a scrolled one; one opened on a search match; PR, repo, and other loaded previews; a failure banner) at 100x30 and 60x20 into /tmp/tuifw-baseline/frames-749-before.
2. tuikit `richtext` feature (off by default): `ansi-to-tui`, `tui-markdown` (with its syntect highlighting), and `comfy-table` become optional dependencies. Module `tuikit::richtext`: the ANSI shim (`ansi_text`), the captured-output layout and its wrap modes (`layout_capture`, `WrapMode`, moved from `src/tui/preview_wrap.rs`), Markdown rendering with GFM tables and highlighted fenced code (`markdown`, `code`, moved from `src/viewer/render.rs`), and the table segmenter and renderer (moved from `src/viewer/table.rs`), with their tests. The table border glyphs move from the host's vetted registrations to the framework's table.
3. Host: `crate::tui::preview_wrap` keeps the `[tui] preview_wrap` tokens and the controls labels over `WrapMode`; `preview_pane.rs` and the viewer call the framework; the host's direct rich-text dependencies go and `tuikit` is enabled with `features = ["richtext"]`; the viewer dependency manifest follows.
4. `tuikit::scroll::ScrollRegion` with `Anchor::{Top, Bottom, Mark}`: an offset counted from the anchor, `scroll_by` stopping at the anchor's own end, placement clamped inside the content and kept by value, a lines draw (wrapped to fit, scrolled with a leading banner) and a cut draw (a host-cut window under a fixed banner), the scrollbar, and `rows_under` for a banner. Decide against the pager's Embedded mode and record why.
5. Host: the App keeps one `ScrollRegion` for the Preview instead of `preview_scroll` and `preview_match_offset`; `J`/`K` route to it with the source's anchor; the zone draws through it; tests keep their expectations.
6. Docs: `crates/tuikit/docs/controls/richtext.md`, the region in `scroll.md`, architecture and README rows, pager Ports, ADR 0181 (and amendment notes on 0025, 0051, 0054, 0106, 0152, 0177, 0180), the ADR index, design.md, operator review.
7. After frames, compare byte for byte; fmt, clippy (workspace all features, and tuikit without the feature), tuikit tests with and without the feature, the independence guard, the isolated full suite, rustdoc, examples, diff check; close the story.

8. As built: a failure banner cannot be reached in a snapshot (seeded messages carry no target), so the frames cover the banner-above paths with Related-row headers (over a transcript's cut window, and over a git status's scrolled lines), and the banner's unit tests cover the failure banner itself.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in seven commits: tuikit `richtext` (feature, module, tests, table glyphs), `scroll::ScrollRegion`, the host on richtext, the host on the region, framework docs, ADR 0181, host docs. The two host slices were split from one working tree and each verified (the richtext slice built and passed the preview, viewer, settings, consistency, and glyph tests on its own).

Decision: a `ScrollRegion` rather than the pager's `Embedded` mode (ADR 0181 decision 6). The anchor at key time comes from the App (`preview_anchor`), at draw time from the source; a different anchor starts at it. Mark offsets are now clamped by each placement like the other two. A `code` entry point covers syntax highlighting outside Markdown. `syntect` is not named in the framework manifest (it comes with `tui-markdown`'s `highlight-code`).

Frames: 82 Preview frames (world749, with a recorded pane exercising every wrap rule) at 100x30 and 60x20 and 120 viewer frames (CSP-748's script) byte-identical. Full suite 4393/0/1 (one rerun after the CSP-770 "Text file busy" flake). Filed CSP-785 for a feature-off framework check in CI.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The framework owns rich text behind a `richtext` cargo feature, and the Preview scrolls in a framework `ScrollRegion` (ADR 0181).

Framework: `tuikit::richtext`, off by default, with `ansi-to-tui`, `tui-markdown` (its `highlight-code` brings `syntect`), and `comfy-table` as optional dependencies: `ansi_text` (the ANSI shim and its fallbacks), `layout_capture` and `WrapMode` (moved from `src/tui/preview_wrap.rs`), `markdown` (GFM-aware, tables and highlighted code), the table segmenter and `render_table` (moved from `src/viewer/table.rs`), and `code`, a highlighting entry point. The table border glyphs are now the framework's vetted glyphs (`glyphs::TABLE_BORDERS`). `tuikit::scroll::ScrollRegion` with `Anchor::{Top, Bottom, Mark}`: steps stop at the anchor's own end, `place` clamps, `draw_lines` (a banner scrolls with the lines) and `draw_cut` (rows the host cut under a banner that stays put), the shared scrollbar, `rows_under`, kept by value. A region, not the pager's `Embedded` mode: the Preview draws from `&App` over a source the host rebuilds, counts from its end or a match, and paints the host's search (recorded in ADR 0181 and on the pager page). Pages: `docs/controls/richtext.md`, the region on `scroll.md`.

Host: `tuikit = { features = ["richtext"] }` and no direct rich-text crates (no lockfile package added); `crate::tui::WrapMode` re-exports the framework's, and `crate::tui::preview_wrap` keeps the `[tui] preview_wrap` token and the Controls label; the viewer renders bodies with `richtext::markdown`; the App keeps one `ScrollRegion` instead of `preview_scroll` and `preview_match_offset`; sources, loaders, store, banners, and search derivation stay.

Verified: 82 Preview frames (pane capture in smart, plain, and no wrap, both focuses; transcript bottom, scrolled, past both ends; opened on a match, moved, past the end; a hidden-turn hit; PR, git, listing, repo, each scrolled; Related rows over a transcript and a git status; an unavailable transcript) at 100x30 and 60x20, and the 120 transcript-viewer frames of CSP-748, are byte-identical before and after, styles included (`/tmp/tuifw-baseline/frames-749-*`). Preview, wrap, table, and ANSI tests pass with expectations unchanged. tuikit with the feature off: clippy -D warnings, `cargo test -p tuikit --no-default-features` (952 tests and doctests); with it, 998; `cargo tree` shows no `syntect` without it. Full isolated suite 4393 passed, 0 failed, 1 ignored (first run hit the known "Text file busy" forge flake, CSP-770; the rerun was clean); fmt, workspace clippy, rustdoc -D warnings, `cargo build --examples`, `git diff --check`, independence, glyph, and raw-style guards pass.

**Regressions**
- On one row, a search shown or cleared after scrolling a transcript Preview starts the Preview at the new anchor (the match, or the newest turns) instead of restoring the offset kept from before.
- A loading or unavailable transcript's placeholder taller than the zone draws from its top; it used to be scrolled by the transcript's count from the bottom.

**Consistency wins**
- Scrolled past the end of a transcript opened at a search match (or past its top), the first `J`/`K` back moves the Preview, as the top and bottom offsets already did (house rule 12); a match on the last page moves on the first `K`. Both show in live use only: a snapshot loads the Preview after its keys.
- No frame changed.

Follow-up filed: CSP-785 (check the framework with its features off in CI and `just check`).
<!-- SECTION:FINAL_SUMMARY:END -->
