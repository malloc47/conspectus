---
id: CSP-493
title: Write the consolidated mux-attribution architecture note
status: To Do
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 178000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: the attribution rules span ADRs 0006, 0028, 0046, 0047, 0048,
  0071, 0072, and 0077 plus resolver evidence-string weights; no single
  document states the evidence hierarchy end-to-end. Write one
  architecture note (no new decisions) describing what evidence exists,
  what outranks what, and how ambiguity is preserved and surfaced, with
  links back to the ADRs. Natural moment: alongside `CSP-476`, which
  moves the per-harness halves of this logic onto the adapter.
- Tests: docs-only; `git diff --check`.
- Blockers: none hard; pairs with `CSP-476`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-ADR-004`
