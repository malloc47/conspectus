---
id: CSP-065
title: Add atomic declared-link write helpers
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-064
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement read-modify-write helpers for local
  `.conspectus.toml` and user config declared-link sections, creating
  parent directories only for explicit write commands, preserving
  unrelated config, sorting entries deterministically, and writing
  atomically enough to avoid partial files on failure.
- Tests: unit tests for create, update, remove, preserve-unrelated
  sections, deterministic ordering, duplicate replacement, malformed
  existing TOML behavior, and global config parent creation.
- Manual checks: inspect generated TOML and verify read-only
  commands still do not call these helpers.
- Blockers: `CSP-064`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added explicit upsert/remove helpers that read and validate
existing config, preserve unrelated TOML sections, replace duplicate
declared IDs, sort links deterministically, create parent
directories only on writes, and replace config files via
temp-file-and-rename writes while leaving malformed files untouched.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-006`
