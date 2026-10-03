---
id: CSP-366
title: 'Atomic write helpers for `[pins]`'
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-365
ordinal: 299000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: read-modify-write upsert/remove for project and user
  config `[pins]` sections. Preserve unrelated TOML sections, sort
  entries deterministically (by `id`), replace duplicates by id,
  create parent directories only on writes, temp-file-and-rename
  atomicity, leave malformed pre-existing files untouched.
- Tests: unit tests for upsert / remove / preserve-others / sort /
  duplicate-replacement / malformed-file refusal.
- Blockers: `CSP-365`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pin upsert/remove helpers preserve sibling TOML
sections, sort deterministically, reject malformed inputs, and
write through the shared atomic config path.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-006`
