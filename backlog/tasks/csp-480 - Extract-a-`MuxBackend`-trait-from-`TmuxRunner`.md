---
id: CSP-480
title: Extract a `MuxBackend` trait from `TmuxRunner`
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-473
ordinal: 162000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03 (ADR 0089). `TmuxRunner` trait renamed
  to `MuxBackend`. New required method
  `backend_key(&self) -> &'static str` returns the stable
  string every consumer keys off (pin `mux.backend`,
  `MuxSessionNode.backend`, provider stamps). `SystemTmux`
  is the first impl, returning `"tmux"` (aliased to
  `providers::TMUX`). Existing capability methods
  (`capture_pane`, `rename_session`, `new_session`,
  `attach_session`, `send_keys`) keep their `Unsupported`
  defaults so a new backend implements only what it
  supports; CSP-481 completes the capability-gate
  migration by replacing the pre-existing
  `backend == "tmux"` string checks in
  `src/tui/actions.rs` and `src/pins.rs` with
  outcome-based gating.
  `LocalDiscoveryConfig.tmux_runner:
  Option<Box<dyn TmuxRunner>>` migrates to
  `mux_backends: Vec<Box<dyn MuxBackend>>`. New builders:
  `with_mux_backend(runner)` pushes; deprecated
  `with_tmux_runner(runner)` alias keeps existing test
  call sites compiling. `without_tmux()` retained;
  semantics migrated to "clear entries whose
  `backend_key() == "tmux"`." New accessors
  `mux_backend_by_key(key)` (reference) and
  `take_mux_backend_by_key(key)` (consuming). Discovery
  path pulls the tmux backend via
  `config.take_mux_backend_by_key(TMUX_BACKEND)`.
  Trait rename rippled through all 23+ `&dyn TmuxRunner` /
  `Box<dyn TmuxRunner>` call sites (CLI, TUI runtime,
  preview, effect executor). Impls of `MuxBackend` gain
  a `backend_key` method: `SystemTmux` → `"tmux"`,
  `FakeTmux` → `"tmux"`, `Box<dyn MuxBackend>` delegates
  to inner, test-only `ReadOnlyRunner` / `MinimalRunner`
  → `"test-*"`.
  Outcome enums (`TmuxOutcome`, `TmuxCaptureOutcome`,
  etc.) keep their `Tmux`-prefixed names for now; their
  variant shapes are already backend-neutral so the
  rename is a mechanical follow-up orthogonal to the
  trait-shape work here. `socket_name → namespace`
  generalization and the `TmuxDiscovery`-wrapper collapse
  are also deferred to land alongside CSP-482.
  ADR 0089 records the shape, alternatives, and
  consequences. Added to the Decisions catalog.
  All 25 suites (1516 lib tests) pass byte-identically;
  fmt / clippy clean.
- Blockers: `CSP-473` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-008`
