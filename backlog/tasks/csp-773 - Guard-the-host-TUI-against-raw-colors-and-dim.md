---
id: CSP-773
title: Guard the host TUI against raw colors and dim
status: To Do
assignee: []
created_date: '2026-10-28 10:57'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 859000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-721 replaced every raw `Color::` and `Modifier::DIM` in the TUI production code (`src/tui`, `src/viewer`) with theme tokens, so `[tui.theme]` reaches the forms, previews, and viewer. Only the framework crate has a guard (`crates/tuikit/tests/raw_styles.rs`); nothing stops a new host widget from hard-coding a color again, which would silently ignore the operator's theme. The framework guard shares its Rust sanitizer through `crates/tuikit/tests/support/source_scan.rs`; the test kit (CSP-732) is meant to provide such source-scan helpers to the host.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A host test fails on a raw `Color::` or `Modifier::DIM` in non-test code under `src/tui` and `src/viewer`, ignoring comments, string literals, and `#[cfg(test)]` items
- [ ] #2 The palette and grammar adapter (`src/tui/theme.rs`) and the ANSI encoder in `src/tui/snapshot.rs` are the only reviewed exceptions, each with a reason, and a stale exception fails
- [ ] #3 The scan names its trees through `tests/support/source_tree.rs` and has a planted-violation test for each tree
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
