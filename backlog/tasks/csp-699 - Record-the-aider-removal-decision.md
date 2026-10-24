---
id: CSP-699
title: Record the aider removal decision
status: To Do
assignee: []
created_date: '2026-10-24 03:19'
labels:
  - h-aider-removal
milestone: m-24
dependencies: []
priority: medium
type: docs
ordinal: 775000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Removing aider changes the supported v1 harness set in `docs/design.md`. It also raises questions that are hard to recover from the code later: which seams were aider-only and should go with it, and what happens to an operator config that still sets `harness_aider`. Per the project conventions, a significant decision like this needs an ADR before code changes land.

Earlier ADRs that name aider (among them 0019, 0022, 0032, 0052, 0058, 0112) record history and stay as they are. The new ADR says what changed, and the ADR index marks any decision it narrows.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new ADR under `docs/adr/` records the decision to remove aider support, with the reason (unused), and covers status, context, decision, consequences, and alternatives considered (for example: keep it, or keep it behind a feature flag)
- [ ] #2 The ADR decides, for each aider-only seam, whether it is removed or kept for future harnesses: the viewer external-launch escape hatch, the `cwd` argument on `HarnessAdapter::resume_argv`, and per-repo marker scanning that is not gated by a harness state root
- [ ] #3 The ADR decides how a config that still sets the `harness_aider` theme key is handled (for example: ignored with a warning, or rejected), and how a cached `graph.bin` or history store that holds aider nodes behaves after upgrade
- [ ] #4 `docs/design.md` drops aider from the supported harness set
- [ ] #5 `docs/adr/README.md` lists the new ADR and notes any earlier ADR it amends or supersedes
- [ ] #6 `git diff --check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
