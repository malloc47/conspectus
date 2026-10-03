---
id: CSP-403
title: >-
  Demote `LinkedToMux` candidates whose source `AgentSession` is materially
  stale compared to a fresher candidate for the…
status: Done
assignee: []
created_date: '2026-06-08 02:02'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 269000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Demote `LinkedToMux` candidates whose source `AgentSession` is materially stale compared to a fresher candidate for the same mux.

- Problem: `resolve_links` (`src/resolve/mod.rs:303`) buckets
  `LinkedToMux` candidates by `(source, relation, target)` and
  picks one winner *per source*. When two sessions each produce a
  `LinkedToMux` candidate to the same mux, the resolver does not
  cross-compare them — each is bucketed alone and both resolve as
  independent relationships. The TUI mux row builder
  (`src/tui/rows/mux.rs:653`) then shows every agent with an
  Active `LinkedToMux` to the mux as attached, with no preference
  among them. When a tmux pane's `active_pane_start_command`
  carries a stale `--resume <UUID>` whose `AgentSession` has been
  untouched for days, the stale session is treated as equally
  "attached" to the mux as the live session writing the same pane.
  In the live `agentdeck_-local-command-caveat-…-573ac208` case
  the stale source had `last_active_epoch 1779653903` (~14 days
  behind the mux), while the correct source (`7f01dbdf-…`) had
  `last_active_epoch 1780881982` within minutes of the mux's
  `activity_epoch`.
- Scope: add a pre-resolver pass on `snapshot.candidate_links`
  that groups Active `LinkedToMux` candidates by target mux,
  selects the freshest source per mux (by source `AgentSession.
  last_active_epoch` closeness to the mux's `activity_epoch`), and
  marks materially-older same-mux candidates as `LinkState::
  Overridden { by, reason }`. Only run when the mux's
  `activity_epoch` is present and the freshest candidate's source
  is itself within a `FRESH_WINDOW` of the mux (don't penalize on
  weak signals). Skip Declared and Pin provenance entirely — user
  intent always wins. Run from `resolve_snapshot` between
  `apply_pin_bindings` and `resolve_links` so the demoted state
  is visible to the bucketing pass and to SQLite materialization.
- Tests: resolver-fixture test where one stale and one fresh
  source both link to the same mux; assert the stale candidate is
  Overridden and the resolver returns only the fresh relationship.
  Regression where the stale source is the only candidate; assert
  it stays Active. Regression where a `LocalPin` candidate is
  stale but a fresher `StrongDiscovered` competes; assert the pin
  is not demoted. Regression where neither source is fresh against
  the mux; assert no demotion (don't penalize on weak signals).
  Regression where the mux's `activity_epoch` is missing; assert
  no demotion.
- Manual checks: replay the `agentdeck_-local-command-caveat-…`
  snapshot through `conspectus graph --format json` and confirm
  `7f01dbdf-…` is attributed to the mux instead of `c1901a9e-…`.
- Related: `CSP-227`, `CSP-402`, ADR 0006
  (resolver ordering), ADR 0028.
- Blockers: none. Land alongside or after `CSP-402` so the
  fresh hook-sidecar candidates are reaching `snapshot.candidate_
  links` Active before the freshness pass has to differentiate.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`demote_stale_source_mux_candidates` in
`src/resolve/mod.rs` runs from `resolve_snapshot` between
`apply_pin_bindings` and `resolve_links`. Groups Active
`LinkedToMux` candidates by target mux, selects the freshest
candidate per mux (by source `AgentSession.last_active_epoch`
closeness to the mux's `activity_epoch`), and marks others
`LinkState::Overridden` when the source-epoch gap to the winner
is ≥ `STALE_SOURCE_GAP_SECONDS` (24h) and the winner is itself
within `FRESH_MUX_BIND_WINDOW_SECONDS` (6h) of the mux. Skips
Pin / Declared provenance entirely (user intent always wins),
skips when the mux's `activity_epoch` is missing, and skips
when no candidate is itself fresh against the mux. Unit tests
cover all five paths. Live caveat-mux fix once CSP-402
starts producing Active hook records.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-021`
