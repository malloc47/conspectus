---
id: CSP-561
title: 'Remove dead code hidden by `#[allow(dead_code)]`…'
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 570000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove dead code hidden by `#[allow(dead_code)]`: an unused `UiEvent` enum, four uncalled test reset functions, a write-only field, `_selection_display`, and the unused `proptest` / `rstest` dev-dependencies (`b1d93eb`).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RUST-008`
