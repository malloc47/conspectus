---
id: CSP-413
title: Docs and snapshot coverage
status: To Do
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies:
  - CSP-410
  - CSP-411
  - CSP-412
ordinal: 525000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` with the icon key — a table
  listing every `NodeKind`, its glyph, its default color, and its
  meaning. Update `Theme` docs in `src/tui/theme.rs` with the new
  per-node-type color fields. Refresh all affected insta snapshots
  (row tree × 5 views, detail panel, graph explorer, JSON, DOT,
  HTML). Add a `NO_COLOR` / `--no-color` snapshot variant proving
  glyphs remain distinguishable without ANSI codes.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest
  run --all-targets --all-features`; `git diff --check`. Insta
  review of every changed snapshot for stable ordering and
  consistent glyph placement.
- Blockers: `CSP-410`, `CSP-411`, `CSP-412`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIS-006`
