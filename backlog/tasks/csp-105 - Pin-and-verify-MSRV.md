---
id: CSP-105
title: Pin and verify MSRV
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
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
