---
id: CSP-720
title: Stand up the framework documentation skeleton and contribution rules
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-workspace
milestone: m-25
dependencies:
  - CSP-717
documentation:
  - docs/adr/0150-tui-framework-workspace-crate.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: docs
ordinal: 806000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The framework is internal-reuse only, but it must stand alone: someone in a different repository has to be able to learn what it is, how it is layered, how to add a control, and which rules keep it independent without reading Conspectus. Writing the skeleton now means each extraction story fills in its own control's page as it lands, so the documentation is never a final-week scramble.

Scope: Create under `crates/tuikit/`: `README.md` (purpose, layers, status as internal-reuse, how to depend on it); `docs/architecture.md` (the layers and module map, the one-way dependency rule, the host message type `M` and context type `Cx`, intent outcomes instead of effects, what the framework never does); `docs/contributing.md` (independence rules and their guards, how to add a control, the house rules summarized, how to catalog TUI changes, how to test with the test kit); `docs/controls/_template.md` (purpose, usage example, key table, theme tokens, house rules followed, seams left open); and `docs/decisions/README.md` (an index of framework-scoped decisions, populated later). Decide and enforce a documentation lint for the public API: module docs required, and public types documented, so a missing page fails the build.

TUI changes: none.

Tests and checks: `RUSTDOCFLAGS='-D warnings' cargo doc -p tuikit --no-deps` passes. The doc lint fails on an undocumented public item in a scratch change.

Blockers: The workspace conversion.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `crates/tuikit` has a README, an architecture guide, a contribution guide, a per-control page template, and a decisions index
- [ ] #2 The architecture guide names the layers, the dependency direction, and the `M` and `Cx` parameters
- [ ] #3 A documentation lint is enforced for the public API and fails the build when a public module or type lacks docs
- [ ] #4 The contribution guide describes the independence guards and how a story catalogs its TUI changes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
