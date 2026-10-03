---
id: CSP-564
title: Give the library typed errors
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 573000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: 66 public functions in library modules (`snapshot`,
  `discovery`, `declared`, `pins`, `config`, and others) return
  `anyhow::Result`, so consumers can't match on failure kinds.
  `thiserror` is already a dependency but used in only three files;
  `snapshot.rs` defines `SnapshotError` and then still returns
  `anyhow`. About 27 functions return `Result<_, String>`.
- Plan: keep `anyhow` in the binary. Start with the
  `conspectus::api` facade and `snapshot`, converting
  `Result<_, String>` parsers to small error enums as touched.
- Decision needed: scope (facade only, or every `pub` function).
  Pairs with `CSP-553`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0101, scoped to what `conspectus::api` exposes. The
discovery entry points return `DiscoveryError`. Its `Provider` variant
names the failing provider's keys and boxes the source error; the
`DiscoveryProvider` trait keeps `anyhow` for implementors.
`render_graph_json` returns `serde_json::Error`, `Projection::parse`
returns `UnknownProjection`, and `load_from_cwd` returns `io::Error`.
Modules outside the contract (ADR 0015 amendment) stay on `anyhow`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-011`
