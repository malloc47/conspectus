---
id: CSP-230
title: Add `conspectus hook write` sidecar writer
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-224
  - CSP-225
  - CSP-226
ordinal: 264000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: move the sidecar write path into the Conspectus binary so
  schema validation, root selection, atomic writes, permissions, and
  future migrations live in Rust beside the reader. Treat the
  subcommand as the compatibility boundary between harness hook
  configuration and Conspectus storage: v1 may continue writing
  per-event JSON files, but the command should encapsulate that choice
  so a later SQLite state backend or daemon ingest path does not
  require users to reinstall hooks. The first supported input should be
  Claude Code hook JSON on stdin, producing the same schema-v1 records
  currently written by `scripts/conspectus-claude-hook-sidecar.py`.
  Keep the script only as a compatibility shim or remove it once the
  CLI path is documented. Candidate command shape:
  `conspectus hook write claude-code`, with room to add `--format` if
  future harnesses need multiple payload forms.
- Design notes: ADR 0028 keeps hook observations as rebuildable local
  state and calls SQLite a plausible next backend once this command
  owns migrations, busy handling, retention, and fallback behavior.
  Keep that separate from ADR 0029 session aliases, which are
  user-authored durable intent even if a future implementation reuses
  SQLite machinery for both.
- Tests: CLI tests for valid Claude payloads, missing `session_id`,
  malformed JSON, sidecar-root precedence, user-only file
  permissions where supported, and atomic write behavior. Reader /
  writer compatibility tests should assert the discovery provider
  accepts records emitted by the subcommand.
- Manual checks: configure a Claude Code hook to call the subcommand
  directly, then confirm `graph --format json` shows
  `hook_session_match` / `hook_session_path_match` evidence without
  relying on the Python emitter.
- Blockers: `CSP-224`, `CSP-225`, `CSP-226`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `conspectus hook write claude-code`, which reads
Claude Code hook JSON from stdin and writes schema-v1 observations
to `hooks.sqlite3` under the hook state root. Discovery reads the
SQLite store plus legacy per-event JSON records.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-016` (the 2026-05-23 story; the ID was used twice)
