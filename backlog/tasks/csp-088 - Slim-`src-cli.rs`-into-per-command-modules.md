---
id: CSP-088
title: Slim `src/cli.rs` into per-command modules
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies:
  - CSP-083
  - CSP-084
ordinal: 88000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Landed 2026-07-05 across 12 waves** (~85%
  reduction). `cli.rs` split into a `src/cli/` directory
  module with 12 per-command submodules; `cli/mod.rs`
  dropped from 5006 → 758 lines.
- Wave landing shape:
  * Wave 1 (`388aa8d`): `cli.rs` → `cli/mod.rs` directory
    module; `columns` extracted as a proof-of-shape.
  * Wave 2 (`07023bc`): `rename` subtree + 3 helpers.
  * Wave 3 (`4319e7f`): `dev` subtree
    (`#![cfg(debug_assertions)]`).
  * Wave 4 (`7c3e6b8`): `hook` subtree — largest single
    wave (~788 lines) with 30+ hook-specific helpers.
  * Waves 5-6 (`6cc4f58`): `node` + `graph`.
  * Wave 7 (`ba45720`): `serve` + `refresh` + `status`
    into `cli/lifecycle.rs`.
  * Wave 8 (`d4604ff`): `table` subtree + 3 helpers.
  * Wave 9 (`17afcb6`): `alias` subtree +
    `format_alias_endpoint` (shared with declared).
  * Wave 10 (`f1ba88c`): `pin` subtree — 10 subcommands +
    all pin-specific helpers (~1150 lines).
  * Wave 11 (`7811154`): `declared` subtree +
    `resolve_write_store` + `DeclaredEndpointArg` parser
    + `run_confirm_or_ignore` shared helper.
  * Wave 12 (`3667e9d`): `tui` subtree + `SnapshotPaneFlag`
    / `SessionsGroupingFlag` value enums + duration
    parsers.
- `cli/mod.rs` retained: `Cli`/`Command` clap enum, shared
  flag enums (`ColorFlag`, `InclusionFlag`, `LayoutFlag`,
  `OutputFormat`, `SortFlag`, `ViewFlag`, `FilterArgs`,
  `DeclaredStoreFlag`), daemon-fallback discovery helpers
  (`try_daemon_snapshot`, `warm_start_discover_and_resolve`,
  `cache_resolved_snapshot`, `current_unix_epoch_for_table`),
  pager plumbing (`PagerOptions`,
  `pager_candidates_with_env`), and the store-selection
  helpers shared across pin, declared, and rename
  (`resolve_alias_store`, `discover_for_store_selection`,
  `candidate_store_paths`, `effective_scan_roots`,
  `project_store_path`, `store_label`, `provenance_label`).
- Every wave: `cargo fmt --check`, `cargo clippy
  --all-targets --all-features -- -D warnings`, and
  `cargo test --all-targets --all-features` clean, with
  the pre-H-REF-006 test suite passing byte-identically.
- Blockers: `CSP-083` (landed), `CSP-084` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-006`
