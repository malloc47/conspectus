---
id: CSP-373
title: CLI `pin bind` (PinAmbiguous override)
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-364
  - CSP-369
ordinal: 306000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: write a `LocalDeclared linked_to_mux` link (per ADR 0014)
  between the named agent session and the pin's mux. Tag the
  declared link with the pin id in `label` or a new
  `bound_by_pin` field (decide during impl; prefer `label =
  "pin:<id>"` to avoid a schema migration). Resolver treats that
  declared link as authoritative when present, suppressing the
  auto-attribution.
- Tests: integration tests for the bind path (resolves
  `PinAmbiguous` deterministically), and a graph-JSON snapshot
  showing the declared link survives alongside the original
  candidate evidence.
- Blockers: `CSP-364`, `CSP-369`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin bind` writes a `pin:<id>` declared
`linked_to_mux` override that resolver precedence treats as the
authoritative ambiguous-binding choice.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-013`
