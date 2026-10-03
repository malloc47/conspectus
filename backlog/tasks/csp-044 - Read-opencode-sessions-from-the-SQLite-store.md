---
id: CSP-044
title: Read opencode sessions from the SQLite store
status: Done
assignee: []
created_date: '2026-05-16 01:59'
labels:
  - p3-fu
milestone: m-10
dependencies: []
ordinal: 82000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: modern opencode (≥ ~0.5) keeps sessions in
  `~/.local/share/opencode/opencode.db` (table `session` with
  `id`, `directory`, `title`, `time_created`, `time_updated`,
  `parent_id`, etc.) rather than the legacy
  `storage/session/<id>/info.json` layout the current adapter expects.
  The legacy parser stays useful for older installs but finds nothing
  on modern setups.
- Blockers: requires an ADR for the new SQLite read dependency
  (`rusqlite` or similar) before introducing it; CLAUDE.md forbids
  dependency additions without one.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0013 records the `rusqlite` dependency decision.
The opencode adapter now opens `opencode.db` read-only, reads
`session.id`, `directory`, and `title` rows into `AgentSession`
nodes, preserves the legacy `storage/session/<id>/info.json`
parser, and lets SQLite rows win on duplicate session ids. Added
tests for SQLite discovery, duplicate precedence, and malformed
database degradation.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-FU-002`
