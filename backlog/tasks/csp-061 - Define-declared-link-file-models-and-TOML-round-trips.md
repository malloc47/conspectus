---
id: CSP-061
title: Define declared-link file models and TOML round trips
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-060
  - CSP-049
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `src/config.rs` or add a focused declared-link module
  with serializable structs for project-local and user-level declared
  links, ignored links, overrides, reasons, optional labels, and schema
  versioning. Preserve unknown config sections and keep session
  projection loading compatible with existing config files.
- Tests: unit tests for TOML decode/encode round trips, missing
  sections, unknown keys, malformed declared-link tables, duplicate
  declared IDs, and backwards-compatible files containing only
  `[session]`.
- Manual checks: inspect representative `.conspectus.toml` and
  user-config TOML snippets for readable shape.
- Blockers: `CSP-060`, `CSP-049`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `src/declared.rs` with ADR 0014 file models for
`[declared]`, `[[declared.links]]`, typed endpoints, link state,
optional reasons/labels, and schema validation. Added TOML
round-trip and validation coverage for session-only config,
unknown keys, all endpoint shapes, malformed TOML, unsupported
schema versions, duplicate IDs, and overridden links missing
`overridden_by`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-002`
