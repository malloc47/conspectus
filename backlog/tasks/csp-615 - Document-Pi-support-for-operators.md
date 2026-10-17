---
id: CSP-615
title: Document Pi support for operators
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 14:42'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-610
  - CSP-612
  - CSP-614
documentation:
  - README.md
  - docs/operations.md
  - docs/provider-adapter-guide.md
priority: low
type: docs
ordinal: 664000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When the Pi adapter, attribution, and hook extension land, the operator-facing docs still list four harnesses and don't mention Pi's env vars, hook install, or limits. This story brings README, operations docs, and the provider adapter guide in line so a Pi user can set things up without reading the ADR.

Coordinate with CSP-546 (consolidate harness-hook docs): if it has landed, add Pi to the consolidated page instead of creating a new one.

Manual checks: follow the docs from scratch on a machine with Pi installed and confirm Pi sessions appear and attribute correctly.

Blockers: the discovery adapter, runtime attribution, and hook extension stories.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 README and the harness list in operator docs include Pi
- [x] #2 Operator docs cover `CONSPECTUS_PI_STATE`, how `PI_CODING_AGENT_DIR` and `PI_CODING_AGENT_SESSION_DIR` are honored, and `conspectus hook init pi`
- [x] #3 Known limits are documented: custom `--session-id` values outside UUID grammar are not extracted from commands, `--no-session` runs are invisible, and a project-local `sessionDir` setting is not followed
- [x] #4 `docs/provider-adapter-guide.md` lists the Pi extension alongside the other hook plugins
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Add Pi to README (harness lists, hook row, viewer list, write surface), docs/operations.md (state-root table, Pi subsection with precedence and known limits, hook init/status/remove pi, CLI surface), and the adapter guide Hook plugins section. CSP-546 has not landed, so no consolidated page yet.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Operator docs now cover Pi. CSP-546 (harness-hook consolidation) hasn't landed, so Pi was added next to the existing hook docs rather than onto a consolidated page.
- README: Pi in the intro, the architecture diagram, the "What it discovers" table (harness and hook rows), and the viewer list. The write-surface list now says Pi's hook is one extension file.
- docs/operations.md:
  - a `CONSPECTUS_PI_STATE` row and a Pi subsection with the state-root precedence (`PI_CODING_AGENT_SESSION_DIR`, absolute global `sessionDir`, `PI_CODING_AGENT_DIR`) and both layouts,
  - the known limits: `--no-session` is invisible, project-level, relative, and per-run session dirs aren't followed, and custom non-UUID `--session-id` values aren't matched in pane commands,
  - `hook init/status/remove pi` at both scopes,
  - the CLI surface line.
- docs/provider-adapter-guide.md: the Hook plugins section lists the Pi extension and its README.

Verification: `just check` passes, `git diff --check` is clean, and `conspectus hook init --help` lists `pi`. The documented commands are covered by the CSP-614 `cli_smoke` tests.

Pi is not installed on this machine, so following the docs from scratch with a live Pi was not done.
<!-- SECTION:FINAL_SUMMARY:END -->
