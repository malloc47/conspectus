---
id: CSP-071
title: Verify the Phase 5 end state
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-063
  - CSP-067
  - CSP-068
  - CSP-069
  - CSP-070
ordinal: 67000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: run the full Phase 5 automated and manual check set and
  record follow-up tasks instead of expanding Phase 5 scope.
- Tests: `just check`.
- Manual checks: run the Phase 5 plan's read-only invariant check,
  create a manual mux/session link, rerun graph/session output,
  confirm declared precedence and evidence preservation, then unlink
  and confirm the generated TOML returns to the expected state.
- Blockers: `CSP-063`, `CSP-067`, `CSP-068`, `CSP-069`, `CSP-070`.
- Follow-up: `declared remove` leaves an empty `[declared]\n
  schema_version = 1` section behind when it strips the last
  declared link. The file remains schema-valid and re-adding a link
  repopulates the section, but a future task should prune empty
  sections so removed declarations don't leave dangling headers.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`nix develop --command just check` passed with 276 tests.
Manual smoke from a fresh temp git repo with isolated `$HOME`
confirmed: (a) `graph --format json` runs read-only and creates
no config files; (b) `declared create --store project --scan-root
.` writes a well-formed `.conspectus.toml`; (c) the new
`local_declared` candidate appears in graph JSON and the agent
session table renders cleanly; (d) `declared remove` strips the
link; (e) graph output returns to its pre-declare candidate set
(only the git-discovered links remain). Discovery stayed
read-only throughout.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-012`
