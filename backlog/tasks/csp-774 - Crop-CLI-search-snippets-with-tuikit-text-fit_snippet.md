---
id: CSP-774
title: 'Crop CLI search snippets with tuikit::text::fit_snippet'
status: To Do
assignee: []
created_date: '2026-10-28 13:57'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-723
priority: low
type: enhancement
ordinal: 860000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`conspectus search` crops its table's snippet column with `cli/search.rs` `crop_around`, a third snippet cropper next to the one the TUI now uses (`tuikit::text::fit_snippet`, ADR 0155). `crop_around` grows the window one character at a time on alternating sides, so it centers the match, then moves each cut to a word edge; the framework favors the text after the match (two thirds of the spare cells) and cuts on word edges. The CLI already shares the framework's cell measure and end cut (`output::render::display_width` and `truncate_to_width` delegate to `tuikit::text`).

Scope: replace `crop_around` with `tuikit::text::fit_snippet` (no dropped characters, or decide whether Markdown ticks should go as they do in the TUI), keep `fit_snippet`'s bold highlight in `cli/search.rs`, and port or retire the `crop_around` tests in `cli/search_tests.rs`.

This changes what `conspectus search` prints when a snippet is cut: the window moves toward the text after the match. Decide whether that is wanted for the CLI before landing it; if the CLI should keep a centered crop, record why and close this.

Tests and checks: the `cli/search_tests.rs` width and floor tests pass; a snippet cut at several widths keeps its match and fits; `tests/cli_search.rs` passes.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The CLI search table crops snippets with tuikit::text::fit_snippet, or the reason it keeps its own crop is recorded
- [ ] #2 cli/search_tests.rs covers the crop at several widths, with wide characters
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
