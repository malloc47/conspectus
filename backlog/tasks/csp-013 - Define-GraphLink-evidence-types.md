---
id: CSP-013
title: Define GraphLink evidence types
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-011
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement `GraphLink`, relation kinds, provenance, confidence,
  freshness, source metadata, unresolved endpoint evidence, and ignored or
  overridden state.
- Tests: unit tests for relation-kind serialization, round trips, unresolved
  endpoints, ignored links, and overridden links.
- Manual checks: inspect serialized candidate links for readable relation and
  provenance names.
- Blockers: `CSP-011`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-003`
