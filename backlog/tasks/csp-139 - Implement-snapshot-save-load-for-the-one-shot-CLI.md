---
id: CSP-139
title: Implement snapshot save/load for the one-shot CLI
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 379000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: snapshot persist + read round-trips in
  `src/query/persist.rs`; gate algebra unit tests in
  `src/discovery/cache.rs` (8 cases); selective-discovery
  unit tests in `src/discovery/mod.rs`
  (`discover_local_warm_with_*`); CLI integration tests in
  `tests/cli_persist.rs` covering the cold/warm/refresh
  matrix plus the end-to-end fresh-slice carryover.
- Hardening (landed alongside phase 4):
  `load_cached_snapshot` now reads `PRAGMA user_version` up
  front and treats any mismatch — including the
  bare-database `user_version = 0` case — as a cache miss
  so the cold rebuild + post-run persist heals the file.
  The writer probes the existing file before opening and
  moves an unusable cache aside to `.corrupt.<epoch>` for
  forensic inspection so the subsequent `OPEN_CREATE` can
  produce a fresh database. `query::persist::rotate_backup`
  produces `VACUUM INTO 'backups/graph-<epoch>.sqlite'`
  point-in-time copies on every cold rebuild (no warm-start
  cache hit, or `--refresh`) and prunes to the
  `BACKUP_RETENTION = 5` newest per ADR 0037; the TUI's
  continuous-refresh path deliberately does not rotate. The
  forward-only migration chain hinted at in ADR 0037
  remains the eventual answer when schema drift actually
  surfaces user pain; until then "rebuild and overwrite"
  is the safer default.
- Follow-ups: `CSP-091` landed alongside the hardening
  work — every per-emit provider key now lives in
  `src/discovery/providers.rs`, and the per-module
  `HARNESS_KEY` / `ADAPTER_NAME` / `FORGE_ADAPTER` /
  `GITHUB_PROVIDER` / `TMUX_BACKEND` constants re-export
  from there. `cache::provider_class` +
  `cache::MUTATOR_PROVIDERS` reference the consts so a new
  provider that forgets to register the canonical string
  breaks CI rather than silently joining the always-rerun
  bucket. The on-disk cache stays binary-compatible since
  the const string values are unchanged.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in three phases.
- Phase 1 wrote the resolved snapshot to
  `$XDG_DATA_HOME/conspectus/graph.sqlite` after each
  successful `conspectus table` invocation via
  `query::persist::persist_snapshot`. `--no-cache` opts the
  writer out for that run; `--refresh` was reserved as a
  no-op flag ahead of the read path.
- Phase 2 added `query::persist::load_cached_snapshot` (the
  read peer) and `discovery::merge_with_prior` (a backstop
  `merge_fragments`-based union with fresh-wins semantics).
  The CLI overlays the prior on every run unless `--refresh`
  is passed, so prior-only nodes survive but live data wins
  on collisions.
- Phase 3 introduced the freshness gate
  (`discovery::cache::compute_freshness_gate`) per ADR 0079.
  `discover_local_warm_with` evicts stale + always-evict
  slices from the prior, skips heavy providers whose class
  TTL has not expired, re-runs every mutator pass against
  the merged snapshot, and persists back. The CLI's
  `table` command went through this path first. Selective
  eviction relies on the CSP-141 primitive landed alongside.
- Phase 4 generalized the wire-up to every command that
  runs discovery. `node show`, `graph`, and the `tui`
  command now all go through `warm_start_discover_and_resolve`
  (or its TUI peer in `tui::runtime::discover_and_resolve`).
  Each one gained `--no-cache` / `--refresh` flags so the
  whole `conspectus` surface has uniform warm-start
  semantics. The declared/pin store-selection helper reads
  the prior too but deliberately skips the writer — it's a
  transient pre-write probe, not the user's primary
  artifact. The TUI's refresh loop persists on every
  successful cycle so a peer one-shot CLI invocation in
  another shell sees the freshest data.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-003`
