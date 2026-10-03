---
id: CSP-136
title: Implement the process-tree linker as a discovery source
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-135
ordinal: 245000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: build the linker per the `CSP-135` ADR. Walk every
  discovered pane's process tree, match descendant commands
  against the harness binary set, and emit candidate links between
  the matching `AgentSession` (when a corresponding session is
  already in the graph) and the pane's `MuxSession`. When no
  matching session exists, emit an unresolved-endpoint candidate
  link carrying the harness key, pane id, and shell PID so the
  evidence survives until later discovery (or a re-run) resolves
  it. Best-effort: missing `/proc` access, an unreadable PID, or
  an unrecognized binary degrade silently. Gate the provider
  behind a `CONSPECTUS_DISABLE_PROCTREE` env var mirroring the
  existing tmux / forge toggles.
- Tests: fixture-backed unit tests using an injected
  process-snapshot trait (mirroring the `TmuxRunner` /
  `GhRunner` seam) covering a direct match, a nested
  shell-then-agent match, an unknown binary, a missing pid, and
  a permission-denied path. Resolver tests confirming the new
  evidence raises confidence on existing session ↔ mux candidates
  rather than producing duplicate winning relationships.
- Manual checks: `cargo run -- graph --format json` inside a
  tmux session running an agent; confirm the new evidence on the
  `LinkedToMux` candidate links.
- Blockers: `CSP-135`.
- **slice landed**: tmux discovery now records active-pane
  process hints available directly from tmux format variables
  (`pane_current_command`, `pane_pid`, `pane_current_path`, and
  `pane_start_command`) on `MuxSessionNode`. Cross-link inference
  inspects the pane process' open file descriptors for known
  harness session paths such as Codex rollout JSONL files and
  Claude task files, then falls back to known session keys in the
  active pane's start command. Evidence is labeled as
  `active_pane_fd_session_match`,
  `active_pane_fd_command_session_match`, or
  `active_pane_command_session_match` depending on the strongest
  available signal. If active-pane evidence names a resumed parent
  session and a discovered child session has a `parent_session`
  link to that parent, the child is treated as active and the
  parent is not linked. Once a mux session has active-pane session
  evidence, cwd-only matches to other sessions are suppressed for
  that mux; this turns the previous many-sessions-to-one-single-pane
  ambiguity into a graph-level refinement instead of a TUI-only
  picker problem.
- Tests: `cargo test active_pane --all-targets`.
- **ranking slice landed**: resolver ordering now distinguishes
  `LinkedToMux` `match_kind` evidence. Fresh current-session
  evidence such as hooks, control-plane responses, and active-pane
  fd matches ranks above `active_pane_command_session_match`, so
  argv / start-command session ids remain useful launch evidence
  without being treated as definitive current-session truth.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The remaining process-tree slice is now implemented per
ADR 0046. Cross-link inference can walk a Linux `/proc` snapshot
from each tmux active-pane PID, match supported harness binaries
through nested shell children, emit `active_pane_process_match`
`LinkedToMux` candidates for exact process command session keys or
a single discovered same-harness/same-cwd session, preserve
unresolved agent evidence when no session node exists yet or
same-cwd matches are ambiguous, and skip the linker with
`CONSPECTUS_DISABLE_PROCTREE`. Tests cover direct and nested
process matches, exact command-key matches, ambiguous same-cwd
suppression, fd evidence suppressing conflicting stale process
resume ids, process-cardinality gating for multi-session mux
attribution, unknown binaries, missing process data, unresolved
evidence, and resolver ranking above launch argv.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-002`
