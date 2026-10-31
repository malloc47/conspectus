---
id: CSP-105
title: Pin and verify MSRV
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
updated_date: '2026-10-31 08:14'
labels:
  - h-dist
milestone: m-11
dependencies:
  - CSP-103
ordinal: 138000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ADR 0016 says the effective MSRV is the toolchain pinned by
  the Nix dev shell. Make this explicit in `Cargo.toml`
  (`rust-version = "1.85"` or similar) and add a CI job that builds
  against the pinned stable to catch accidental MSRV bumps.
- Tests: dedicated CI job pinning the toolchain.
- Blockers: `CSP-103`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DIST-003`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the repository is a workspace of two members, the root `conspectus` package and `crates/tuikit` (ADR 0150). Put `rust-version` in `[workspace.package]` of the root `Cargo.toml`, where `edition` and `license` already live, and inherit it in both members; the separation rehearsal's export (`scripts/export-tuikit.sh`, ADR 0199) rewrites inherited `[package]` fields, so it travels with the framework crate. CI's Separation rehearsal job already builds the framework alone with the pinned toolchain.
<!-- SECTION:NOTES:END -->
