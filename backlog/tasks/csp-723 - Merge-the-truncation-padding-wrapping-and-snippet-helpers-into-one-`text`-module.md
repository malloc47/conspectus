---
id: CSP-723
title: >-
  Merge the truncation, padding, wrapping, and snippet helpers into one `text`
  module
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
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: high
type: enhancement
ordinal: 809000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI has 13 truncate/ellipsize/pad implementations (6 count `char`s instead of terminal cells), 6 word-wrap implementations, 2 snippet croppers, and 7 match painters. `tui::widgets` imports `crate::viewer::render::word_wrap`, a layering inversion. Wide characters misalign wherever a char-counting helper runs, and each fix has to be made in several places (inventory slices 01 and 05).

Scope: Create `tuikit::text` with cell-width measure, end and middle truncation, pad, fit-spans, word and character wrap, snippet fit (with the backtick stripping as a parameter), `find_ci`, and one match painter. Migrate all call sites. Keep the domain `compact_path_*` and `compact_mux_*` helpers in Conspectus. Record whether the CLI table renderer's `fit_to_width` shares the module.

TUI changes:
- Consistency wins: CJK and emoji no longer misalign where a helper counted chars; one ellipsis form (`…`) and one middle-ellipsis rule
- Regressions or behavior changes: truncation points move by a cell in places that counted chars; a few rows that wrapped differently from their siblings now wrap alike

Tests and checks: Property tests: output width never exceeds the budget for ASCII, CJK, emoji, and combining marks; truncation is idempotent. Snapshots at 80 and 40 columns for the left pane, forms, and search.

Blockers: The independence guard and the documentation skeleton.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::text` provides measure, truncate, middle-truncate, pad, fit, wrap, snippet fit, `find_ci`, and one match painter, all measuring terminal cells
- [ ] #2 All 13 truncation, 6 wrap, 2 snippet, and 7 match-paint sites use the module; the old helpers are removed
- [ ] #3 `tui::widgets` no longer imports anything from `crate::viewer` for text helpers
- [ ] #4 Property tests cover ASCII, CJK, emoji, and combining characters
- [ ] #5 The decision on sharing the module with the CLI table renderer is recorded
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
