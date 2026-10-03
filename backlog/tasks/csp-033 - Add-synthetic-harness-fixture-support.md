---
id: CSP-033
title: Add synthetic harness fixture support
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-032
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add test helpers for creating provider state directories and session
  records for `claude-code`, `opencode`, `codex`, and `aider` without reading
  the user's real harness state.
- Tests: fixture self-checks for generated paths, timestamps, cwd/root
  fields, and malformed records.
- Manual checks: verify fixtures live under temporary directories and do not
  depend on local home-directory state.
- Blockers: `CSP-032`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a `discovery::harness::fixtures` module with a
`HarnessFixture` builder and standalone writers for Codex, Claude Code,
opencode, and aider state layouts plus a malformed-record helper, all rooted
at a caller-supplied temp directory; covered paths, optional fields, cwd
encoding, opencode time fields, aider marker files, and malformed records.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-002`
