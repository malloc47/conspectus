---
id: CSP-260
title: Help-overlay docs
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-253
  - CSP-254
ordinal: 456000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the `?` help overlay with the new keymap
  (`v`, `1`–`5`, `]`/`[`, `f`, `F`, grouping-cycle), a one-line
  description of the controls overlay, the v1 filter dimensions,
  and an example CLI invocation. Document the menu-first
  discovery rule.
- Tests: snapshot test for the help overlay's new layout.
- Blockers: `CSP-253`, `CSP-254`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/tui/widgets/help.rs` — the help
overlay documents the controls overlay (`f`), clear-filters
(`F`), direct view switches (`1`–`5`), view cycling, the
grouping-cycle binding, and the v1 filter dimensions. The
CSP-416 icon legend layered in alongside as the
discoverability surface for kind glyphs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-011`
