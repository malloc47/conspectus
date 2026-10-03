---
id: CSP-542
title: Add installation paths beyond `--path`
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies:
  - CSP-533.02
priority: medium
ordinal: 613000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: once public, document `cargo install --locked --git
  https://github.com/malloc47/conspectus`. The flake exposes only
  `devShells.default`; add `packages.default` / `apps.default` so
  `nix run github:malloc47/conspectus` works (a distribution surface,
  so record it in an ADR). Verify a release build without
  `--features snapshot` on a clean machine.
- Blockers: `CSP-533.02`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-011`
