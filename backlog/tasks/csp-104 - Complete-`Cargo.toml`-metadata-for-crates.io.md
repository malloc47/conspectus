---
id: CSP-104
title: Complete `Cargo.toml` metadata for crates.io
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
updated_date: '2026-10-31 08:14'
labels:
  - h-dist
milestone: m-11
dependencies:
  - CSP-103
  - CSP-541
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `cargo publish --dry-run` cannot pass as things stand. The repository is a Cargo workspace (ADR 0150), the `conspectus` package depends on the framework crate by path (`tuikit = { path = "crates/tuikit", ... }` in the root `Cargo.toml`), and `crates/tuikit/Cargo.toml` is `publish = false` (ADR 0150 decision 3: internal reuse only). crates.io refuses a package with a dependency it cannot resolve to a published version. Publishing therefore waits on CSP-541's crates.io decision (publish `tuikit` too, which amends ADR 0150; fold the framework into the package; or release by tag and `cargo install --git` only), and this story now depends on CSP-541. Also: `edition` and `license` come from `[workspace.package]`; the metadata this story adds goes on the root `[package]`, or on `[workspace.package]` where both members share it.
<!-- SECTION:NOTES:END -->
