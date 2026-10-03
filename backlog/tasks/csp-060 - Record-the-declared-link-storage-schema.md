---
id: CSP-060
title: Record the declared-link storage schema
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-057
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add an ADR for durable declared relationship state in
  `.conspectus.toml` and user config, covering link identity, endpoint
  encoding, relation kinds, link state (`active`, `ignored`,
  `overridden`), local-vs-global precedence, write ownership, and
  compatibility rules for future schema changes.
- Tests: docs-only; `git diff --check`.
- Manual checks: review the schema against `docs/design.md`, ADR
  0002, ADR 0012, and the Phase 5 implementation plan.
- Blockers: `CSP-057`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0014 defines the `[declared]` TOML schema,
`[[declared.links]]` entries, typed inline endpoint tables,
active/ignored/overridden states, local-vs-global provenance from
config location, nearest-store write ownership, and compatibility
behavior for unknown fields and schema versions.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-001`
