---
id: CSP-490
title: Restate the payload-privacy tenet precisely
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-adr
milestone: m-11
dependencies: []
ordinal: 175000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New standalone tenet ADR 0086 grades
  payload access into three tiers by reader purpose:
  Tier 1 (attribution / identity readers, never read
  payload), Tier 2 (operator-facing content features like
  the sessions preview column and the native transcript
  viewer — may read payload subject to capping /
  normalization / an operator gesture and an ADR entry),
  Tier 3 (hook sidecars and rebuildable state records, stay
  payload-free). ADR 0048's §Lock and privacy hygiene section
  was scoped to cite the new tenet and place its readers
  explicitly (state reader Tier 2, log reader Tier 3).
  `docs/design.md`'s state-persistence section replaces the
  sweeping "never selects privacy-sensitive payload
  columns" wording with a citation of ADR 0086. ADR 0086
  added to the Decisions catalog.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-ADR-001`
