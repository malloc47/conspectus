---
id: CSP-489
title: Write the provider-adapter contributor guide
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-474
  - CSP-480
  - CSP-484
  - CSP-486
ordinal: 171000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New `docs/provider-adapter-guide.md`
  (250 lines) with per-family checklists for harness / mux
  / forge / orchestrator adapters: required trait
  methods, optional methods with their defaults,
  registration steps, worked examples pointing at
  existing adapters (codex, zellij, GitHub / GitLab
  skeleton, agent_deck), what the registry provides for
  free downstream, what needs fixtures, what needs an
  ADR. Hook-plugin contract cross-referenced to
  `plugins/opencode-hook/README.md`. Closing "Reading
  order for a new adapter" section lists the reading
  order: this guide → family ADR → worked example →
  conformance suite → registration point.
- Blockers: `CSP-474` (landed), `CSP-480` (landed),
  `CSP-484` (landed), `CSP-486` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-017`
