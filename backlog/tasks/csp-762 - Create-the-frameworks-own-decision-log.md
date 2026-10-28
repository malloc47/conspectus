---
id: CSP-762
title: Create the framework's own decision log
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-separation
milestone: m-25
dependencies:
  - CSP-761
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
  - docs/adr/README.md
priority: medium
type: docs
ordinal: 848000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Decisions that govern framework code live in Conspectus's ADR directory, mixed with decisions about graph resolution and discovery. When the framework leaves the repository, its rationale has to go with it. ADRs 0150 to 0152 and older ADRs on the theme, text input, clipboard, scrollbar, snapshot mode, MVU overlay stack, and overlay key tables each explain a framework design choice (the ADR index groups them under Interactive TUI).

Scope: Write a framework-scoped decision log in `crates/tuikit/docs/decisions/`: one short record per framework-owned decision, each stating the decision and its rationale without Conspectus vocabulary, and linking its origin ADR in Conspectus. Mark the origin ADRs in `docs/adr/README.md` as framework-owned. State the policy: until separation the Conspectus ADRs are canonical for the origin, and after separation the framework log is canonical.

TUI changes: none.

Tests and checks: The independence guard's vocabulary scan passes on the decision log. Every framework-owned origin ADR is linked from the log and marked in the index.

Blockers: Complete documentation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `crates/tuikit/docs/decisions/` has an index and one record per framework-owned decision, written without Conspectus vocabulary
- [ ] #2 Each record links its origin ADR, and the Conspectus ADR index marks the framework-owned ADRs
- [ ] #3 The canonical-source policy before and after separation is written down
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
