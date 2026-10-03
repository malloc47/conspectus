---
id: CSP-104
title: Complete `Cargo.toml` metadata for crates.io
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-dist
milestone: m-11
dependencies:
  - CSP-103
ordinal: 137000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `Cargo.toml` is missing `authors`, `repository`, `homepage`,
  `documentation`, `keywords`, `categories`, `readme`, and an
  `exclude`/`include` pattern. Fill in for the first publish and verify
  `cargo publish --dry-run` succeeds.
- Tests: `cargo publish --dry-run` in CI on tagged releases.
- Blockers: `CSP-103`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DIST-002`
