---
id: CSP-601
title: Adopt the compass-rose logo
status: Done
assignee:
  - '@claude'
created_date: '2026-10-14 00:43'
updated_date: '2026-10-14 00:44'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 650000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator chose the compass-rose mark from the logo bake-off: a survey reticle recast as a compass, with a small evidence graph at its centre whose links bend slightly.

Scope (ADR 0119):
- add the mark as light and dark SVGs, plus 512 px PNGs for slides, under `docs/images/logo/`;
- show it in the README header, switching with the reader's colour scheme.

Tests: `git diff --check`; README link check.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Light and dark SVG and PNG marks are in docs/images/logo
- [x] #2 The README header shows the mark in both color schemes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in 2a7d497. The compass-rose mark, with its graph links bent slightly (bend 4.5 on the 64-unit grid), is in `docs/images/logo/` as light and dark SVGs and 512 px PNGs. The README header shows it through a `<picture>` that follows the reader's color scheme. ADR 0119 records the bake-off, the color meanings, and the alternatives. Checked the renders at 512, 64, 32, and 16 px. README links resolve, and `git diff --check` is clean.
<!-- SECTION:FINAL_SUMMARY:END -->
