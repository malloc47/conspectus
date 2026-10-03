---
id: CSP-068
title: Implement link and unlink commands
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-065
  - CSP-066
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add write commands that create and remove active declared
  relationships between supported endpoint types (`AgentSession`,
  `MuxSession`, `ForgePr`, `Workspace`, `Repo`, `Checkout`,
  `Branch`, and `Fork`), using nearest-store selection by default.
  Link creation should not delete discovered evidence.
- Tests: CLI integration tests for session↔mux, branch↔PR,
  workspace/repo/checkout/fork relationships, global orphan links,
  unlink by declared ID, unlink idempotency, and graph output after
  link/unlink.
- Manual checks: create a manual mux/session link, rerun graph JSON,
  confirm the declared link wins resolution and discovered candidates
  remain visible, then unlink and confirm resolution returns to
  discovered evidence.
- Blockers: `CSP-065`, `CSP-066`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus declared create` builds a declared link with
state=Active, picks the target store via
`select_store_for_declaration` (auto), `--store {project|user}`
(explicit), or rejects `--store all`; `conspectus declared remove`
walks project (nearest, per `--scan-root` walk) then user stores
and removes the first match, reporting a clear error when no
store holds the id. Eight new CLI smoke tests cover repo-rooted
write to project config, orphan write to user config, explicit
`--store user` override, `--store all` rejection, idempotent
re-create (wrote → unchanged), remove from project config,
"no declared link" error path, and graph JSON showing the
newly created `local_declared` candidate.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-009`
