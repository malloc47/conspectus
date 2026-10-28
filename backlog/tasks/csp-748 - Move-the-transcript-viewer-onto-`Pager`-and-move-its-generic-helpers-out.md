---
id: CSP-748
title: Move the transcript viewer onto `Pager` and move its generic helpers out
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
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
- [ ] #1 The transcript viewer is a `PagerContent` adapter on `Pager` with its toggles, hidden-match count, and reveal-on-open preserved
- [ ] #2 `src/tui` no longer imports generic helpers from `src/viewer`
- [ ] #3 The viewer's colours come from generic theme tokens and it has no raw `Modifier::DIM`
- [ ] #4 ADR 0052's boundary is narrowed in the index and the effect on CSP-342 is noted for the closing assessment
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
