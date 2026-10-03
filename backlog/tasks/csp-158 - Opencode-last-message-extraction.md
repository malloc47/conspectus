---
id: CSP-158
title: Opencode last-message extraction
status: Done
assignee: []
created_date: '2026-05-19 03:32'
labels:
  - h-preview
milestone: m-11
dependencies:
  - CSP-155
ordinal: 189000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Blockers: `CSP-155`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/discovery/harness/opencode.rs` populates
`last_message_preview` by reading the modern `part` table
alongside the existing `session` query. A
`read_last_message_previews` helper runs a single
`ROW_NUMBER()`-windowed SQL query that pulls the most recent
`type: "text"` row per session via `json_extract`, ordered by
`(time_created DESC, id DESC)`, filtered to non-empty text. The
`bundled` rusqlite feature guarantees JSON1 is available;
schemas without the `part` table (or any row that fails to
parse) degrade to `None` so legacy stores still discover their
sessions without lineage or preview. Each non-empty text passes
through `normalize_last_message_preview` for whitespace
collapse and the 200-char cap. Five new SQLite-backed unit
tests cover (a) the most-recent-text-part wins, (b) empty/
missing-text rows are skipped, (c) per-session attribution,
(d) absent-`part`-table degrades to `None`, (e) long text is
capped via the shared helper. All 440 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-004`
