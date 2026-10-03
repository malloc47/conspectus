---
id: CSP-423
title: Persist last-active view across TUI restarts
status: Done
assignee: []
created_date: '2026-06-18 23:17'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 458000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: `App::config().default_view` is in-memory only,
  seeded from `[tui].default_view` config. After view switching
  (`v` / `1`..`5` / `]`/`[`), the next `conspectus tui` invocation
  drops the operator back at the configured default — costing a
  keystroke every cold start and breaking the "pick up where you
  left off" expectation that pins, scenarios, and the sessions
  tree otherwise reinforce. A persisted handoff is the smallest
  surface that closes the loop.
- Scope:
    - Introduce a TUI state file at
      `$XDG_STATE_HOME/conspectus/tui-state.json` (sibling to the
      hook state-root already under `$XDG_STATE_HOME/conspectus/`;
      rebuildable, not authoritative — same posture as the
      pin-binding sidecar). Schema v1 carries
      `{ schema_version: 1, last_view: "sessions" | "mux" |
      "union" | "prs" | "forks" }`. Atomic write helpers (tempfile
      + rename) and a `skip-on-unchanged` comparison so quiet
      sessions produce no mtime churn, mirroring
      `CSP-395` / `CSP-397`.
    - Write on view switch (the `App::switch_view` seam at
      `src/tui/app.rs:551` is the natural single funnel — every
      accelerator and overlay-driven switch goes through it).
      Best-effort write; failures log at debug and do not abort
      the switch.
    - Read at `conspectus tui` startup, after config load and
      before runtime init. Precedence: explicit `--view <name>`
      CLI flag wins → persisted `last_view` wins → config
      `[tui].default_view` → built-in `View::Sessions`. Add a
      `--no-resume-view` opt-out for scripts and snapshot tests
      that need a deterministic starting view independent of
      prior runs.
    - The `--snapshot` dev-only path (ADR 0067) must default to
      `--no-resume-view` semantics so snapshot regeneration is
      not influenced by whatever view the operator last touched
      outside the test fixture.
    - Read-only invariant: nothing other than `conspectus tui`
      reads or writes the state file. `graph` / `table` / `query`
      / `node show` / `pin *` paths never touch it. Mirror the
      CSP-379 / CSP-400 fingerprinting test pattern so
      regressions get caught.
- Tests:
    - Unit tests on the state-file helpers covering atomic
      write, skip-on-unchanged, malformed-JSON fallback (treat
      as absent, do not panic), and forward-compatible unknown
      fields (preserve through round-trip so future schema bumps
      do not strand old writes).
    - Reducer test pinning that `switch_view` schedules a write
      through the same seam the runtime uses, and that the
      startup-precedence resolver picks the right source under
      each combination of flag / file / config.
    - Integration test: launch the TUI, switch to `mux`, exit;
      relaunch and assert the initial view is `mux`. Re-run with
      `--no-resume-view` and assert the initial view falls back
      to the configured default.
    - Read-only invariant test mirroring
      `tests/cli_pin_resume_invariants.rs`: assert that
      `graph`, `table <rows>`, `query`, `node show`, and every
      `pin` subcommand leave the state file byte-for-byte
      untouched (content + mtime fingerprints) and do not create
      the file when absent.
- Open questions:
    - Format: JSON (mirrors the pin sidecar's serde + skip-on-
      unchanged pattern verbatim) vs TOML (matches every other
      Conspectus config surface). Recommend JSON to share the
      cache helpers; flag during impl.
    - Scope creep: CSP-252 covers in-session per-view state
      (filters / grouping / selection / expansion / scroll).
      Should this story persist *only* `last_view`, or extend
      the schema to carry the CSP-252 `ViewStates` map? Recommend
      scope-tight v1 (just the view enum) and a follow-up after
      CSP-252 lands; document the schema bump path so the
      forward-compat fixture stays honest.
    - ADR threshold: a single persisted enum probably does not
      clear the ADR bar, but the XDG location + read-only
      invariant + precedence rules are durable conventions worth
      memorializing in `docs/operations.md` §"Caches" (or a new
      §"TUI state") regardless. Decide during impl whether the
      cross-surface impact warrants a short ADR.
- Blockers: none structurally — `App::switch_view` and
  `default_view` already exist. Coordinate with `CSP-252` so the
  schema can grow without a breaking migration; coordinate with
  `CSP-254` so the new view-switch accelerators all funnel through
  the same persistence seam.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped. State file at
`$XDG_STATE_HOME/conspectus/tui-state.json` with schema v1
(`schema_version` + `last_view`). Persistence module
`src/tui_state.rs` mirrors the pin-binding sidecar pattern
(atomic write, skip-on-unchanged, forward-compat unknown
fields). `App::switch_to_view` writes best-effort through an
optional cache that the runtime enables in `event_loop` /
`static_event_loop` only — snapshot mode (ADR 0067)
deliberately skips enabling the cache so snapshots stay
deterministic. Startup precedence in `src/cli.rs`: explicit
`--view` → persisted → built-in `Sessions` (config
`default_view` integration is the CSP-252 follow-up).
`--no-resume-view` opt-out shipped; `--snapshot` implies it.
Read-only invariant covered by
`tests/cli_tui_state_invariants.rs`. Operations docs updated
under §"TUI state". Open question on `ViewStates` schema
extension stays scoped to CSP-252 as recommended.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-013`
