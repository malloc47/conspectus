---
id: CSP-759
title: Extract `StatusBar` and generate its hints from the key tables
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-728
  - CSP-730
  - CSP-723
  - CSP-731
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 845000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The status bar's hint string, the help section, and `remap_for_focus` each describe the right-pane keys by hand, and the wording of `e` differs across them. Four status hints in `runtime.rs` duplicate or drift from the generated modal footers, and `tests/overlay_keymap_hygiene.rs` never scans `runtime.rs`. Three modal-opening functions copy footer text into the status bar. The status line paints every message in the warning colour (inventory slices 06 and 07).

Scope: Create `StatusBar` and `KeyHintBar` generated from the key tables with priority-drop fitting, using the feedback severities for message styling. Replace the hand-written hints in `runtime.rs` and the modal-open copies. Keep Conspectus's contextual segment producers and `default_action_status_hint` as annotations on table rows. Extend the hygiene guard to `runtime.rs`.

TUI changes:
- Consistency wins: a hint cannot advertise a key that does not resolve; status messages are styled by severity
- Regressions or behavior changes: hand-tuned contextual hint strings are regenerated, so their wording changes

Tests and checks: A test asserts that every advertised key resolves in the current mode. The extended `overlay_keymap_hygiene` passes. `--snapshot` of the status bar in each view and focus.

Blockers: The key tables, help generation, and text.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `StatusBar` and `KeyHintBar` are generated from key tables and fit by priority
- [ ] #2 The hand-written hints in `runtime.rs` and the modal-open copies are removed
- [ ] #3 `overlay_keymap_hygiene` scans `runtime.rs`
- [ ] #4 A test fails when an advertised key does not resolve
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
