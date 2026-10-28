---
id: CSP-732
title: >-
  Extract the `testkit`: key DSL, frame driver, and convention conformance
  harness
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
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
- [ ] #1 `tuikit::testkit` provides the key DSL, buffer helpers, the ANSI-preserving frame driver, one `strip_ansi`, and a style-aware golden format
- [ ] #2 `assert_conventions(&Registry)` runs the lifted layout, key, and footer checks and fails for a deliberately broken fixture modal
- [ ] #3 The 23 per-file key helpers and four `strip_ansi` copies are replaced by the kit
- [ ] #4 `tui --snapshot` can drive every overlay key action
- [ ] #5 The `tui-pantry` preview example still builds and runs
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
