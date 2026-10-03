---
id: CSP-487
title: Add an orchestrator mutation-capability seam (deferred)
status: To Do
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-486
  - CSP-451
ordinal: 169000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Officially deferred pending CSP-451 demand.**
  CSP-486's `OrchestratorDescriptor` is the shape this
  story would extend (with an optional `mutation:
  Option<&'static dyn OrchestratorMutation>` field or
  equivalent). Because no shipping mutation feature needs
  this today, landing the seam speculatively would encode
  guesses about `owns_mux()` / rename-routing semantics that
  the first consumer would then have to renegotiate. Story
  stays open in the backlog and lands alongside the first
  concrete consumer.
- Tests: TBD with the first consumer.
- Blockers: `CSP-486` (landed), `CSP-451` demand.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-015`
