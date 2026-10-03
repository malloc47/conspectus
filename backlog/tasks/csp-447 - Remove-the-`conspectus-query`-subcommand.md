---
id: CSP-447
title: Remove the `conspectus query` subcommand
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 506000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: 25 fewer tests overall (the cli_query + query
  regression binaries) and 2 pin-invariant tests pruned
  to drop the query-command assertions. `cargo nextest
  run --all-targets --all-features` green at 1753 tests;
  `cargo fmt -- --check` and `cargo clippy --all-targets
  --all-features -- -D warnings` clean.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Clean removal — the `Query(QueryArgs)` enum
variant on `cli::Command`, the dispatch arm, the
`QueryArgs` struct, `QueryFormatFlag`, the
`From<QueryFormatFlag> for conspectus::query::OutputFormat`
impl, the `impl QueryArgs::run`, and the
`resolve_query_width` helper are all gone from
`src/cli.rs`. The clap "unknown subcommand" error is the
operator-facing response (no stub-with-hint cycle —
nothing in the wild was scripting against this yet).
`tests/cli_query.rs` and `tests/query_regression.rs`
delete; the `cli_pin_invariants.rs` tests that ran
`["query", "SELECT ..."]` to assert read-only pin
invariants drop the now-impossible cases (the parallel
`graph` / `table` / `pin list` / `pin show` /
`node show` assertions still cover the same read-side
read-only property). `docs/query-guide.md` and
`docs/vector-search.md` (ADR 0042's surface, no
operator-facing remnant after the subcommand drop)
delete. `README.md`'s read-command list drops `query`.
`src/query/` stays intact in this story — the TUI's
`GraphDb::from_snapshot` materialization and the 14
`query::read_snapshot(...)` call sites still consume
it. Those get ripped out in CSP-448 alongside the
`rusqlite` drop.
`design.md`'s "Conspectus Query Surface" section keeps
its live reference for now; CSP-449's rewrite covers
that and the broader ADR supersession.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-010`
