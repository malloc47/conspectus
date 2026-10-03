---
id: CSP-004
title: Add the Rust package skeleton
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-001
  - CSP-002
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `Cargo.toml`, `rust-toolchain.toml`, `src/lib.rs`, and
  `src/main.rs` for a Rust 2024 library-first CLI crate.
- Tests: `cargo check` succeeds.
- Manual checks: `cargo run -- --help`.
- Blockers: `CSP-001`, `CSP-002`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-001`
