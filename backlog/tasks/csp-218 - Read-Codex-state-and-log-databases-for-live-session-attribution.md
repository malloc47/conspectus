---
id: CSP-218
title: Read Codex state and log databases for live session attribution
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 255000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: per ADR 0048, add two read-only Codex slices under the
  existing harness state-root discovery. The state-reader slice
  globs `state_*.sqlite`, selects the highest numeric suffix, and
  emits one `AgentSession` per `threads` row populating the
  existing sparse `AgentSessionNode` shape only: `id`, `cwd`,
  `title` with capped/normalized `first_user_message` fallback,
  `last_active_epoch` from millisecond timestamps, and previews via
  the existing rollout-tail extractor. The richer threads columns
  (`rollout_path`, `git_*`, `model`, `cli_version`, `agent_*`,
  `archived_at`) are queried but not stored on the node; promoting
  them needs a separate ADR. The state reader also emits one
  intra-harness `parent_session` candidate per `thread_spawn_edges`
  row with `lineage_kind = "spawn"`, distinct from the rollout
  reader's existing `forked_from_id` candidates which keep
  `lineage_kind = "fork"`. The
  log-linker slice reads `logs_*.sqlite` only to resolve the
  freshest `thread_id` for each live Codex pid by parsing
  `process_uuid` as `pid:<os_pid>:<uuid>` and querying within a
  15-minute `ts` floor matching ADR 0028. The resulting
  `LinkedToMux` candidate ranks above
  `active_pane_command_session_match` and
  `active_pane_fd_session_match` for Codex and demotes stale
  command-session candidates for the same mux, mirroring the
  hook-sidecar override rule. Fresh log evidence may synthesize a
  sparse `AgentSession` when state has not yet observed the thread,
  matching the ADR 0028 synthesis path. Both readers use
  `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` plus
  `PRAGMA query_only = ON`, never select `feedback_log_body`, and
  cap/normalize `first_user_message` like opencode previews.
- Tests: temp sqlite fixtures for the current `state_5` and
  `logs_2` schemas; missing-database and unknown-schema
  degradation; column-probe behavior when a known column is absent;
  parent-lineage candidate emission and self/empty-parent skipping;
  log-linker fixture proving the freshest `thread_id` wins per pid;
  pid-reuse disambiguation via the trailing UUID; 15-minute ts
  floor rejecting stale rows; resolver tests proving codex
  log-derived candidates rank above command and fd evidence and
  override stale `active_pane_command_session_match`; synthesized
  sparse session round-tripping into the state-backed node when
  state catches up.
- Manual checks: run against the live Codex state/log stores while
  a session is active; reproduce the in-process resume drift case
  (launch argv names session A, in-process switch to session B)
  and confirm `conspectus graph --format json` and `conspectus tui`
  link the mux to B without consulting argv. Confirm no write-ahead
  log churn from Conspectus reads.
- Related: ADR 0048; ADR 0028 (sidecar TTL and demotion rule
  reused); ADR 0046 (process-tree provides the live Codex pid set);
  ADR 0018 (parent_session shape); `CSP-227` (Claude analogue
  of the drift case this closes for Codex); `CSP-219` /
  `CSP-220` (Codex `remote_control_enrollments` belongs to
  the control-plane audit, not here).
- Blockers: none. ADR 0048 supplies the persistent schema-dependency
  decision the original blocker required.
- **audit slice landed**: ADR 0048 records the May 2026 audit of
  opencode `opencode.db`, Codex `state_5.sqlite`, and Codex
  `logs_2.sqlite`. opencode's slice of 004 closes as a no-op
  because the schema carries no live process/server binding beyond
  what the existing reader already extracts; live opencode↔mux
  attribution remains the responsibility of `CSP-221` /
  `CSP-229`. Codex `jobs`, `agent_jobs`, `thread_goals`, and
  `stage1_outputs` were empty on the audit machine and are deferred
  until in-the-wild usage justifies coverage.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-004`
