---
id: CSP-421
title: >-
  Renderer-side fallback so candidate fan-out flags the explorer group as
  ambiguous even when no `ResolvedRelationship`…
status: Done
assignee: []
created_date: '2026-06-17 19:04'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 361000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Renderer-side fallback so candidate fan-out flags the explorer group as ambiguous even when no `ResolvedRelationship` exists.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`build_relationship_group`
(`src/tui/explorer.rs`) now derives `ambiguous` from the
candidate set's distinct target count when
`resolved_for` returns `None`, so the suppressed-LinkedToMux
case (the showcase ambig sessions) renders a `⚠` glyph on
the group header instead of looking like a clean fan-out.
Lets the showcase reproduce the same explorer ambiguity
signal the live TUI shows. Tracked properly at the resolver
layer by `CSP-420`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-007`
