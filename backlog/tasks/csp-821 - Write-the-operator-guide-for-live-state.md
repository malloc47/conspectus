---
id: CSP-821
title: Write the operator guide for live state
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-docs
milestone: m-26
dependencies:
  - CSP-811
  - CSP-813
documentation:
  - docs/plans/live-state/README.md
priority: medium
type: docs
ordinal: 907000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Hooks are opt-in and have known gaps, so the operator needs one page that says what live state is, how to turn it on per harness, what each state means, what is inaccurate by nature, and how to find out why nothing shows.

Scope: A "Live state" section in `docs/operations.md`: enabling it per harness (`hook init`, the Codex `/hooks` review, the OpenCode and Pi plugins), what each state and detail means, the quiet and unknown rules, the known gaps (an Esc interrupt, an approval granted, a killed process) and how each is repaired, the store file and its privacy guarantee, and a troubleshooting list keyed to the `hook status` health line. Update `docs/library-api.md` for the model types, and the README feature list if it lists capabilities.

TUI changes: none.

Tests and checks: `git diff --check`; the doc-sync tests that scan the docs still pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `docs/operations.md` explains enabling, states, gaps, privacy, and troubleshooting
- [ ] #2 `docs/library-api.md` lists the live-state types
- [ ] #3 The known gaps match ADR 0206
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
