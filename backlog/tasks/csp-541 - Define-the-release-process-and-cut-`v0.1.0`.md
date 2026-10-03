---
id: CSP-541
title: Define the release process and cut `v0.1.0`
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
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
