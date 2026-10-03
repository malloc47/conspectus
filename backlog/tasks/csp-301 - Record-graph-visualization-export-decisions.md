---
id: CSP-301
title: Record graph visualization export decisions
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - gv
milestone: m-16
dependencies:
  - CSP-312
ordinal: 488000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: write an ADR covering graph visualization outputs: Graphviz
  DOT for static inspection and an HTML output for interactive,
  navigable graph inspection. Decide how the HTML renderer loads its
  JavaScript graph library (vendored asset, CDN, or generated
  self-contained bundle), the minimum feature set, and how large
  graphs should degrade.
- Tests: none; docs-only decision.
- Manual checks: review the ADR against `docs/design.md` and update
  the design doc if the exported graph shape or CLI surface becomes
  part of the product contract.
- Blockers: `CSP-312`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0050 settles both formats, picks inlined Cytoscape.js
as the HTML library, locks provider-neutral `NodeKind` /
`RelationKind` / `Provenance` visual encoding, treats candidate
vs. resolved as toggleable views in HTML (distinctly styled
together in DOT), defaults `RuntimeProcess` and unresolved-
endpoint stubs to visible-but-filterable, commits to
deterministic emission, and introduces a shared `[theme]` table
with `[tui.theme]` / `[html.theme]` overrides. Bespoke
navigation chrome (focus, N-depth, upstream/downstream) wraps
the Cytoscape API directly. Live server-hosted HTML view is
flagged as a follow-up. `docs/design.md` gains a Graph
Visualization Exports subsection and a Decisions entry.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `GV-001`
