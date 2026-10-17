---
id: CSP-615
title: Document Pi support for operators
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
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
- [ ] #1 README and the harness list in operator docs include Pi
- [ ] #2 Operator docs cover `CONSPECTUS_PI_STATE`, how `PI_CODING_AGENT_DIR` and `PI_CODING_AGENT_SESSION_DIR` are honored, and `conspectus hook init pi`
- [ ] #3 Known limits are documented: custom `--session-id` values outside UUID grammar are not extracted from commands, `--no-session` runs are invisible, and a project-local `sessionDir` setting is not followed
- [ ] #4 `docs/provider-adapter-guide.md` lists the Pi extension alongside the other hook plugins
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
