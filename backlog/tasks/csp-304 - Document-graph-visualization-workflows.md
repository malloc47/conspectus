---
id: CSP-304
title: Document graph visualization workflows
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - gv
milestone: m-16
dependencies:
  - CSP-302
  - CSP-303
ordinal: 495000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `README.md`, `docs/operations.md`, or a focused
  visualization guide with examples for generating DOT and HTML
  outputs, rendering DOT through Graphviz, and using the HTML explorer
  for debugging resolver behavior.
- Tests: docs-only `git diff --check`.
- Manual checks: run each documented command against a fixture or local
  repo before marking complete.
- Blockers: `CSP-302`, `CSP-303`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New `docs/graph-visualization.md` covers `--format dot`
(pipe through Graphviz, recipes for SVG/PDF/PNG, fallback layout
flags for dense graphs), `--format html` (single self-contained
file, how to open) and walks every chrome surface (filter panel,
inspector, search, focus navigation). Includes four debugging
recipes ("why did the resolver pick mux X over Y", "show me
everything reachable from this fork", "approximate the CLI
table view", "what's this RuntimeProcess evidence for").
`docs/operations.md` CLI Surface and Paging notes updated.
`README.md` CLI block updated and `docs/index.md` registers the
new guide. Cross-references all point at ADR 0050 for rationale.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `GV-004`
