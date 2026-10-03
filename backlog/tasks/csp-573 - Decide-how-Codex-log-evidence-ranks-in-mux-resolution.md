---
id: CSP-573
title: Decide how Codex-log evidence ranks in mux resolution
status: Done
assignee: []
created_date: '2026-10-01 03:27'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 582000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: typing the match kinds (`CSP-567`) showed that the
  resolver's string matches had drifted from what adapters emit:
  - `mux_evidence_rank` gives `codex_log_current_thread_match`
    rank 0, below `exact_cwd_match` (20). ADR 0048 says log-derived
    evidence should rank above `active_pane_command_session_match`
    and `active_pane_fd_session_match` for Codex.
  - `has_compatible_session_mux_link` accepted
    `codex_log_thread_match`, which nothing emits; the emitted kind is
    `codex_log_current_thread_match`.
  - The ranks for `control_plane_current_session_match`,
    `harness_state_current_session_match`, and
    `hook_process_session_match` matched nothing, because nothing
    emits them.
- `CSP-567` kept the existing behavior: those three kinds were
  dropped from the vocabulary, and Codex-log links still rank 0.
- Plan: give `CodexLogCurrentThreadMatch` a rank consistent with
  ADR 0048, decide whether it counts as process evidence in
  `identifies_process`, and add resolver tests for a Codex session
  with competing cwd and log-derived candidates. This changes
  attribution results, so confirm the intent first.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (operator decision 2026-10-01: Codex-log evidence ranks
above cwd): `codex_log_current_thread_match` ranks 55, above the
cwd kinds (20/10) and, per ADR 0048, above open-file and hook
evidence (50). It also counts as process evidence in
`identifies_process`, so the resolver no longer derives a duplicate
runtime-process link next to it. ADR 0048 is amended and the
ranking table in `docs/mux-link-resolution.md` updated. Three
resolver tests cover cwd, open-file, and the duplicate case.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-020`
