---
id: CSP-024
title: Add cwd and configured scan-root discovery inputs
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-021
  - CSP-023
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: discover from the current working directory and from explicitly
  configured scan roots without recursively walking `$HOME` by default.
- Tests: unit tests for scan-root normalization, duplicate-root handling, and
  missing/non-git roots.
- Manual checks: verify running outside a git repo still returns a valid
  sparse graph document.
- Blockers: `CSP-021`, `CSP-023`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added current-directory and explicit scan-root context builders,
canonicalization and deduplication for existing roots, missing-root errors,
and local discovery over non-git roots without recursive scanning.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-004`
