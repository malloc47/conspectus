---
id: CSP-541
title: Define the release process and cut `v0.1.0`
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
updated_date: '2026-10-31 08:15'
labels:
  - rel
milestone: m-20
dependencies:
  - CSP-532.01
  - CSP-533.02
  - CSP-539
priority: medium
ordinal: 612000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: advances `CSP-104` / `CSP-105` / `CSP-106`. Decide
  tag + GitHub release now and crates.io now or later (ADR 0016 prefers
  crates.io; the ADR audit's C6 suggests "don't break gratuitously"
  until a consumer exists). Fill `Cargo.toml` metadata (`repository`,
  `homepage`, `readme`, `keywords`, `categories`, `rust-version`).
  Stamp the `[Unreleased]` 0.1.0 entry from `CSP-539` with the version
  and date. Decide whether binary and HTML-export distributions carry
  third-party notices (see `CSP-532.02`). Record the outcome as an ADR
  0016 amendment.
- Blockers: `CSP-532.01`, `CSP-533.02`, `CSP-539`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-010`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the crates.io choice now includes the framework crate. The `conspectus` package depends on `tuikit` by path (root `Cargo.toml`), and `crates/tuikit/Cargo.toml` is `publish = false` (ADR 0150 decision 3: internal reuse, no semver promise, no release pipeline), so `cargo publish` of `conspectus` fails until one of these holds: `tuikit` is published as well (an amendment to ADR 0150 and a release pipeline for it; `crates/tuikit/docs/separation.md` describes moving it to its own repository first), the framework is folded back into the package, or 0.1.0 ships only as a tag, a GitHub release, and `cargo install --locked --git` (which the workspace does not affect). Record the choice in the ADR 0016 amendment; CSP-104 now depends on this story for it. `rust-version` and shared metadata can sit in `[workspace.package]` (see CSP-105).
<!-- SECTION:NOTES:END -->
