---
id: CSP-072
title: 'Prune empty `[declared]` sections after the last declared link is removed'
status: Done
assignee: []
created_date: '2026-05-16 16:15'
labels:
  - p5-fu
milestone: m-8
dependencies: []
ordinal: 78000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when `remove_declared_link` brings the link list to zero,
  delete the `[declared]` table entirely (and the file when no
  other top-level sections remain) so a fresh `declared list` from
  that store prints nothing instead of showing a dangling header.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`write_declared_links_if_changed` now removes the
`declared` table from the `toml_edit::DocumentMut` when no
links remain, and the new `write_document` helper deletes the
config file outright when no other top-level keys are left.
Files with sibling sections (e.g. `[session]`) keep their other
content and just lose the `[declared]` header. Added two unit
tests in `src/declared.rs` for the file-deleted and
sibling-preserved cases.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-FU-001`
