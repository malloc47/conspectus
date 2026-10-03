---
id: CSP-456
title: Harness picker with free-form escape hatch
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-454
ordinal: 323000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: make the harness field choose from known harness keys
  discovered in the snapshot plus registered adapter defaults, while
  still allowing an explicit free-form value for future/custom
  harnesses. The selected entry should prefill the harness; blank
  forms should prefer the last-used or most common harness only
  when that choice is visible as a suggestion, not silently hidden.
- Tests: reducer/widget tests for selected-session prefill,
  suggestion navigation, free-form input, unknown-harness warning,
  and confirmation behavior.
- Blockers: `CSP-454`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Researched the Ratatui picker ecosystem before
implementation. No focused single-select/free-form picker was
mature enough to justify a new dependency under the project
dependency policy: the available options were either namespace
placeholders, multi-select-specific, or broader interaction
frameworks. Implemented a scoped in-tree harness picker affordance
instead: registered and discovered harness keys are shown under
the editable field, `Space` cycles known choices, and custom typed
values remain valid with an explicit warning.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-005`
