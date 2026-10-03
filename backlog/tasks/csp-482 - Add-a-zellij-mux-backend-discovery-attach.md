---
id: CSP-482
title: Add a zellij mux backend (discovery + attach)
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-ext
milestone: m-11
dependencies:
  - CSP-480
  - CSP-481
ordinal: 164000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-03. New `discovery::zellij` module carries
  `SystemZellij` (implements `MuxBackend` with
  `backend_key = "zellij"`; shells out to
  `zellij list-sessions --no-formatting` and `zellij attach`),
  `parse_zellij_sessions` (fixture-driven parser handling
  both `--no-formatting`-short and human-formatted outputs
  with `[Created ...]` metadata, `(current)`, and
  `(EXITED - attach to resurrect)` markers), and
  `ZellijDiscovery` (a `DiscoveryProvider` that wraps a
  backend and emits `MuxSessionNode`s stamped with the
  zellij backend key).
  Registrations:
  * `discovery::providers::ZELLIJ` const + entry in the
    `REGISTRY` (class `Mux`, alongside tmux).
  * `discovery::tmux::KNOWN_MUX_BACKENDS` extended to
    `[TMUX_BACKEND, ZELLIJ]` — pin validation and attach
    dispatch accept `mux.backend = "zellij"` automatically.
  * `LocalDiscoveryConfig::from_env` pushes a `SystemZellij`
    unless `CONSPECTUS_DISABLE_ZELLIJ` is set. Missing
    `zellij` binary surfaces as
    `TmuxOutcome::Unavailable(BinaryNotFound)` and
    `ZellijDiscovery` degrades to an empty fragment.
  * `discover_local_warm_with` pulls the zellij backend via
    `take_mux_backend_by_key(ZELLIJ_BACKEND)` and wraps it
    in `ZellijDiscovery` alongside the tmux path.
  Rename / new_session / capture_pane / send_keys stay as
  their `MuxBackend` trait defaults (`Unsupported`) — zellij
  doesn't have a first-class rename op, capture-pane
  requires a plugin, and send-keys semantics are untested.
  Follow-up stories can promote each capability as needed.
  A pre-existing `pins.rs::unsupported_mux_backend_is_rejected`
  test migrates from asserting zellij is unregistered
  (which is now false) to using a synthetic `"screen-notreal"`
  key so the rejection-path assertion still fires.
  Zero edits outside the new module + the three registry
  entries. Acceptance test for CSP-480 satisfied. All 25
  suites (1521 lib tests: +5 from the new zellij tests)
  pass; fmt / clippy clean.
- Blockers: `CSP-480` (landed), `CSP-481` (landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-EXT-010`
