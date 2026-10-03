---
id: CSP-565
title: Narrow the public surface
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 574000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: every module in `lib.rs` is `pub`, including `tui`,
  `server`, `viewer`, `tui_state`, and `pin_bindings`, because the
  binary imports them through `conspectus::`. The curated
  `conspectus::api` facade (ADR 0015) is therefore not the real
  contract.
- Plan: move the CLI into the library (`conspectus::cli::run` called
  from a three-line `main.rs`), then make internal modules
  `pub(crate)`. That also removes the need for `pub
  #[doc(hidden)]` test helpers.
- ADR: ADR 0015 amendment. Pairs with `CSP-553`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0015 amended. The CLI moved into the library
(`conspectus::cli::run`, three-line `main.rs`). `server`, `viewer`,
`tui_state`, `pins`, `pin_bindings`, and `pin_store_registry` are
`pub(crate)`. `cli`, `tui`, `snapshot`, `hook`, `filter`, and
`dev_scenarios` stay `pub` but `#[doc(hidden)]`, because integration
tests and `examples/pantry.rs` use them. Making them `pub(crate)`
needs those tests moved into the crate first. Six dead or test-only
items surfaced and were removed or gated.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-012`
