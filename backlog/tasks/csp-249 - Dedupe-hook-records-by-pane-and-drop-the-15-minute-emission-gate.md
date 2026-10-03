---
id: CSP-249
title: Dedupe hook records by pane and drop the 15-minute emission gate
status: Done
assignee: []
created_date: '2026-05-23 04:22'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 266000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: in-app `/resume` between two Claude Code sessions in the
  same tmux pane leaves both sessions linked to the mux. Each
  session's `SessionStart` / `Resume` hook writes its own record;
  both records fall inside the 15-minute `ACTIVE_TTL_SECONDS`
  window, both resolve to the same mux, and the current
  `apply_hook_sidecars` pass emits an Active `LinkedToMux` for each
  rather than letting the freshest pane observation win. The 15-min
  TTL also causes legitimate live links to drop off a working
  session as soon as the operator idles longer than the window.
- Scope: change `apply_hook_sidecars` in
  `src/discovery/hook_sidecar.rs` to group records by
  `(resolved_mux.id, record.tmux.pane_id)` after mux resolution,
  pick the record with the highest `observed_epoch` per group as
  the Active winner, and emit older same-pane records as
  `LinkState::Overridden { by: winner_link_id, reason: "superseded
  by fresher hook sidecar record for same pane" }`. Records with
  no `pane_id` fall back to keying by `(resolved_mux.id, None)`
  (one winner per mux for pane-less records), which conservatively
  dedupes cwd-only and pid-only matches. Drop the
  `ACTIVE_TTL_SECONDS` filter from the emission gate; the constant
  stays in `src/hook.rs` for higher-layer freshness signals (e.g.
  the live-session advisory planned for `CSP-243`). Update
  ADR 0028 to record the new emission semantics.
- Tests: replace the existing
  `stale_hook_record_does_not_link_active_mux` test with one
  asserting an old hook record still links when no fresher record
  supersedes it. Add `fresher_hook_record_overrides_older_hook_for
  _same_pane` asserting the older link state flips to `Overridden`.
  Add `hook_records_for_different_panes_in_same_mux_both_remain_
  active` asserting per-pane independence. Existing demotion tests
  for launch-argv and cwd evidence stay green.
- Manual checks: reproduce the in-app `/resume` scenario from a
  real Claude Code tmux pane and verify
  `conspectus graph --format json` and `conspectus tui` show
  exactly one Active `LinkedToMux` per pane (the freshest), with
  the older link visible as Overridden in diagnostic output.
- Related: `CSP-227` (the original in-process `/resume`
  drift fix scope), ADR 0028 (hook sidecar attribution).
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`apply_hook_sidecars` now sorts hook records newest
first, groups candidates by `(resolved_mux.id, pane_id)`, keeps
the freshest record active, and marks older same-pane records
`Overridden` with the planned reason. Records no longer expire
solely because they are older than the former 15-minute TTL; old
records still link when no fresher same-pane record supersedes
them. Pane-command and mux-created-after-observation guards still
ignore clearly stale records. ADR 0028 now documents the pane
dedupe semantics. Unit tests cover old-record retention,
fresher-same-pane override, and different-pane independence.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-018`
