---
id: CSP-264
title: Replay recent MUXPROC drift and stale-evidence bugs
status: Done
assignee: []
created_date: '2026-05-24 20:27'
labels:
  - test
milestone: m-11
dependencies:
  - CSP-262
  - CSP-263
ordinal: 275000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: encode the recent bugfix history as replay scenarios:
  launch argv names session A while stronger hook/fd evidence names
  B; multiple same-pane hook records where the freshest wins; stale
  Claude hook records left behind after the pane starts running
  Codex; hook records whose transcript path is missing; Codex argv
  naming an older resumed thread while open fd evidence names the
  current rollout. Assert both graph evidence state and TUI-visible
  row projection.
- Tests: scenario snapshots or structured assertions proving there
  is exactly one preferred mux indicator per active pane, weaker
  launch/cwd evidence is overridden or ignored rather than deleted,
  phantom sessions are not synthesized from missing transcripts,
  and Codex open-fd evidence remains preferred over stale argv.
- Manual checks: none required once scenarios are replayable; live
  checks remain useful only when adding a new real-world failure to
  the corpus.
- Blockers: `CSP-262`; benefits from `CSP-263`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added two replay scenarios in `tests/testing_replay.rs`.
`same_pane_hook_supersession_freshest_wins_and_tui_shows_active`
replays a Claude Code pane where hook record A is superseded by
fresher hook record B; asserts exactly one Active hook-sidecar
link, one Overridden, and the sessions row tree shows B as
`Attached`. `codex_fd_evidence_beats_stale_argv_and_tui_follows
_current_rollout` replays a Codex pane where the launch
`start_command` references a stale session but injected fd evidence
names the current rollout; asserts `active_pane_fd_session_match`
exists, the resolver prefers it, and the row projection attaches
the current session. Also added a CSP-265-style invariant helper
`assert_at_most_one_active_hook_link_per_mux_pane` that verifies
at most one Active hook-sidecar `LinkedToMux` per `(mux, pane_id)`,
called from the hook supersession test.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-003`
