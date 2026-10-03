# ADR 0048: Codex State and Log Readers for Mux Attribution

## Status

Accepted. Amended 2026-10-01: see the resolver-rank amendment at the end.

## Context

`CSP-218` calls for read-only queries against harness state databases to
strengthen `AgentSession` discovery and `AgentSession -> MuxSession`
attribution. The May 2026 audit covered the three on-disk SQLite stores
Conspectus sees today: `opencode.db`, Codex `state_<N>.sqlite`, and Codex
`logs_<N>.sqlite`.

`opencode.db` carries session metadata, parent lineage, and recency
timestamps. Those fields are already consumed by
`src/discovery/harness/opencode.rs` per ADR 0018. The schema does not record
process id, terminal, or server-binding evidence. Live opencode↔mux binding
therefore belongs to a plugin/server adapter (`CSP-221` /
`CSP-229`), not a state reader. The audit closes opencode's slice of
`CSP-218`.

Codex's stores have not been read yet and contain the strongest live
attribution evidence found anywhere in the audit:

- `state_<N>.sqlite#threads` holds one row per Codex thread/session with
  `rollout_path` (absolute JSONL transcript path), `cwd`, `git_sha`,
  `git_branch`, `git_origin_url`, `model`, `cli_version`, millisecond
  timestamps, archival state, and a privacy-sensitive `first_user_message`.
- `state_<N>.sqlite#thread_spawn_edges` records parent→child subagent
  spawn lineage with a `status` column (`closed` for finished spawns in
  observed data). Audit confirmed these edges are subagent spawns, not
  user-initiated forks: real rows show one parent thread spawning several
  distinct child threads. They are therefore distinct from the rollout
  reader's `forked_from_id` (true user forks) and both can coexist on
  the same `AgentSession`.
- `logs_<N>.sqlite#logs.process_uuid` is encoded as the literal string
  `pid:<os_pid>:<random_uuid>`. Paired with `thread_id` per most-recent `ts`,
  this names the thread a live Codex process is currently writing — including
  cases where launch argv `codex --resume <session A>` is stale because the
  operator switched sessions in-process. This is the Codex equivalent of the
  Claude Code drift case that motivated `CSP-227` and ADR 0028, and the
  audit reproduced it on the maintainer's machine: pid 1770237's launch argv
  named one session while its latest `logs.thread_id` row named another.

Both Codex databases use a numeric filename suffix to express breaking schema
changes (currently `state_5`, `logs_2`), expose explicit
`_sqlx_migrations` cursors, and ship with active WAL files while Codex is
running. The existing opencode reader's pattern of read-only flags plus a
column-probe ladder transfers directly.

`logs.feedback_log_body` may contain prompt or response content. The reader
does not need that column for attribution.

## Decision

Conspectus adds two new read-only Codex discovery slices, both gated on the
existing harness state-root discovery and both following the same read-only
flag set and version-probe ladder as `opencode.rs`.

### Codex state reader

A new harness-state slice reads `state_<N>.sqlite` and emits:

- One `AgentSession` node per `threads` row, scoped by the resolved Codex
  state root. V1 populates only the existing sparse `AgentSessionNode`
  shape: `id`, `cwd`, `title` (with `first_user_message` as a capped and
  normalized fallback when title is empty), `last_active_epoch` from the
  millisecond timestamps, and `last_message_preview` (deferred to the
  existing rollout-tail extractor when state alone cannot produce one).
  Archived rows are still emitted so resolved views can filter them; the
  `archived` flag is carried in link source metadata where needed, not on
  the node. The remaining `threads` columns (`rollout_path`, `git_sha`,
  `git_branch`, `git_origin_url`, `model`, `model_provider`, `cli_version`,
  `agent_role`, `agent_nickname`, `archived_at`, etc.) are present in the
  query result but are not stored on the node because `AgentSessionNode`
  has no slot for them today. Promoting them to first-class fields, or
  routing `git_*` through `Repo` candidate links, is deferred to a
  follow-up ADR; this slice does not introduce a node-side metadata
  channel.
- One intra-harness `parent_session` candidate link per `thread_spawn_edges`
  row using `lineage_kind = "spawn"`. The rollout reader's existing
  `forked_from_id` path continues to emit `lineage_kind = "fork"`. Both
  may coexist on the same `AgentSession` because they describe different
  lineage operations; this matches the operation vocabulary ADR 0018
  established.

The reader globs `state_*.sqlite`, selects the highest numeric suffix as the
active database, and falls back to the next-highest only if the active file
cannot be opened. `_sqlx_migrations` is consulted to record the version
cursor in source metadata for diagnostics. Column probing tolerates unknown
columns by selecting only what the current schema exposes, exactly like
`opencode.rs::session_query`.

### Codex log-derived live-session linker

A second slice reads `logs_<N>.sqlite` strictly to resolve "which thread is
this live Codex process currently writing?" For each `MuxSession` whose
active-pane process tree (per ADR 0046) contains a Codex process with OS
pid `P`, the linker queries:

```
SELECT thread_id
FROM logs
WHERE process_uuid LIKE 'pid:' || P || ':%'
  AND thread_id IS NOT NULL
  AND ts >= :ts_floor
ORDER BY ts DESC, ts_nanos DESC
LIMIT 1
```

`:ts_floor` is `now() - 24 hours` by default, overridable via the
`CONSPECTUS_CODEX_LOG_WINDOW_SECONDS` env var. (Pre-H-EXT-007 this
was also settable through the
`LocalDiscoveryConfig::with_codex_log_window` builder; CSP-479
folded the codex-log knob into
`CodexAdapter::apply_aux_attribution` and dropped the builder as
unused.) Unlike ADR 0028
hook-sidecar records, which arrive asynchronously and can become orphaned
from process lifetimes, codex log evidence is anchored to a live OS pid:
the candidate set comes from `cross_link::active_harness_pids_per_mux`,
which only contains pids currently running codex in the active-pane
process tree. The 24-hour bound therefore serves two purposes that
together justify keeping it nonzero by default:

1. **Query performance.** The `logs` table lacks a `(process_uuid, ts)`
   compound index, so an unbounded `LIKE 'pid:<pid>:%'` scan is expensive
   on heavy users. The `ts` floor lets the planner use `idx_logs_ts` to
   narrow the scan.
2. **Pid-reuse defense.** The candidate pid set guarantees the current
   pid is alive and runs codex, but the log row format
   `pid:<os_pid>:<uuid>` is matched only by prefix. If pid `P` previously
   ran codex `A` (which wrote log rows), exited, and the kernel reused
   `P` for a brand-new codex `B` that has not written any log rows yet,
   `LIKE 'pid:P:%' ORDER BY ts DESC LIMIT 1` would return `A`'s latest
   row and the linker would attribute the wrong thread. The time bound
   confines that misattribution to the window's width.

24 hours balances "long enough that a quiescent codex session resumes
correctly across user idle periods" against "short enough that pid-reuse
risk on typical Linux pid spaces stays low." Users with extreme uptime
or unusually long-idle codex panes can widen via the env var; the
linker can also be skipped entirely via `CONSPECTUS_DISABLE_CODEX_LOG`
when the active-DB read is undesirable.

A proper pid-reuse fix would compare each row's `ts` against
`/proc/<pid>/stat.starttime` and accept only rows written after the
current process started. That refinement is deferred — `ProcessSnapshot`
does not expose start time today, and the 24-hour bound is adequate for
the realistic failure shape.

The full `process_uuid` value is parsed as `pid:<os_pid>:<uuid>`. The trailing
UUID disambiguates pid reuse across Codex restarts; the linker keeps the
parsed `(pid, uuid)` pair on the candidate's source metadata.

The linker emits a `LinkedToMux` candidate joining the resolved Codex
`AgentSession` (from the state reader) to the matched `MuxSession`. The
candidate kind ranks **above** `active_pane_command_session_match` and
`active_pane_fd_session_match` for Codex, matching the priority hook-sidecar
evidence already enjoys for Claude Code in ADR 0028. When a stale
`active_pane_command_session_match` already names a different Codex session
for the same mux, the linker marks that candidate overridden by the
log-derived candidate, preserving the stale evidence for diagnostics but
removing it from the preferred mapping. Resolver tests cover this priority
explicitly.

If the freshest `logs.thread_id` names a session that has not yet been
discovered by the state reader (for example, a brand-new Codex thread whose
`threads` row has been written but where state and logs disagree on read
ordering), Conspectus may synthesize a sparse `AgentSession` from the log
record alone, mirroring the hook-sidecar synthesis path in ADR 0028. The
synthesized node carries the harness key, the log-derived state scope, the
thread id, and a `synthesized_from = "codex_logs"` marker on its source
metadata. The next state-reader pass replaces the sparse node with the
richer state-backed node naturally.

### Lock and privacy hygiene

Both readers open SQLite with `OpenFlags::SQLITE_OPEN_READ_ONLY |
SQLITE_OPEN_NO_MUTEX`, issue `PRAGMA query_only = ON` immediately after
connect, and keep connections short-lived. Conspectus does not use
`immutable=1` because we want to observe fresh writes through the WAL.
No write paths are introduced.

Payload access follows the three-tier invariant recorded in
ADR 0086:

- The **log reader** is a Tier 3 rebuildable-observation reader.
  `feedback_log_body` is never selected.
- The **state reader**'s `threads.first_user_message` read is a
  Tier 2 operator-facing preview surface: capped and normalized
  before it reaches the operator, following the same shape as
  the opencode preview path.

The earlier wording of this ADR implied a blanket "readers never
select privacy-sensitive payload columns" rule. That rule was
already narrower than the codebase, since opencode's preview
query and the native transcript viewer (ADR 0052) intentionally
surface payload for operator display. ADR 0086 grades reader
purposes into three tiers and places each of Conspectus's
current readers explicitly; this ADR's readers are placed above.

### opencode

`CSP-218` for opencode is closed out by this ADR with no code change:
the audit found nothing in `opencode.db` beyond what the existing reader
extracts that would strengthen live mux attribution. Live opencode↔mux
binding remains the responsibility of `CSP-221` / `CSP-229`.

### Explicitly deferred

- Codex `remote_control_enrollments` (control-plane attribution) belongs to
  `CSP-219` / `CSP-220`, not 004.
- Codex `jobs`, `agent_jobs`, `agent_job_items`, `thread_goals`, and
  `stage1_outputs` are background-agent surfaces. They were empty on the
  audit machine; defer until in-the-wild usage justifies a separate slice.
- `feedback_log_body` is privacy-sensitive and is never selected.

## Consequences

- The Codex equivalent of the CSP-227 stale-`--resume` drift is fixed
  without requiring a hook, control plane, or terminal interaction.
- Codex `AgentSession` discovery becomes the indexed `threads` table rather
  than enumerating rollout JSONL files. The existing rollout-file reader can
  fall back when state is missing or unreadable.
- Codex subagent spawn lineage gets first-class `parent_session` candidate
  links with `lineage_kind = "spawn"`, distinct from existing rollout-fork
  lineage. Resolver and TUI handling stays on the ADR 0018 surface.
- A `(process_uuid, ts)` compound index would make the log lookup faster but
  is not required: the 24-hour query-bound and `idx_logs_thread_id` keep
  cost bounded. If real-world log volumes ever make this expensive, the
  linker can pre-filter by candidate Codex pids in process snapshot order
  so the WHERE-clause comparisons stay narrow.
- Conspectus now reads Codex schema across two filename-versioned databases.
  Adding `state_6` or `logs_3` upstream will require a new column-probe pass
  and may require a new resolved-relationship shape, but the
  highest-suffix-wins discovery rule degrades gracefully until that pass
  lands.
- The synthesized-session path follows the same shape as ADR 0028, so TUI
  and resolver code that already tolerates sparse hook-synthesized agent
  session nodes will tolerate log-synthesized nodes too.
- Records are rebuildable observations of harness-owned local state.
  Conspectus does not persist or cache them itself.

## Alternatives Considered

- **Treat `logs.process_uuid` as opaque and match by recency alone.**
  Rejected: the `pid:<os_pid>:<uuid>` format is stable on this Codex release
  and parsing the OS pid lets the linker join directly against process-tree
  evidence from ADR 0046. The trailing UUID still guards against pid reuse.
- **Use the rollout JSONL file's mtime as the live-attribution signal.**
  Rejected as primary: rollout files are also touched by replay tools and
  the file system mtime resolution is coarser than the log-row timestamp.
  Rollout-path match remains useful as a corroborating signal against
  active-pane open-fd evidence, but it is not the freshness source of truth.
- **Read `logs.feedback_log_body` to enrich session previews.** Rejected on
  privacy grounds. Preview content for Codex sessions should come from
  `threads.first_user_message` or the rollout file, not log payloads.
- **Read all `state_<N>.sqlite` files in parallel and union the rows.**
  Rejected: schema breaks make older files unreliable, and the highest
  numeric suffix is the file Codex is actually writing.
- **Skip the log reader and only read state.** Rejected: state alone has no
  way to express "which thread is this process currently active in." Without
  the log reader, Codex stays vulnerable to the same stale-launch drift that
  ADR 0028 fixed for Claude Code.
- **Introduce `RuntimeProcess` nodes now to carry the Codex pid binding.**
  Deferred to ADR 0047. The log-derived `LinkedToMux` candidate already
  carries the parsed `(pid, uuid)` pair in source metadata; promoting
  process state to a first-class node remains a separate, broader design
  question.
- **Open the databases with `immutable=1` to avoid touching `-shm`.**
  Rejected because we explicitly want fresh writes; `-shm` access is
  consistent with WAL read-only behavior and matches the existing opencode
  reader.

## Open Questions Answered

- `CSP-218` for opencode resolves as a no-op: the schema offers no
  live-binding signal beyond what the existing reader already consumes.
- The default query bound on log-derived current-session evidence is 24
  hours, intentionally wider than the ADR 0028 hook-sidecar TTL because
  the pid liveness filter (via `cross_link::active_harness_pids_per_mux`)
  already provides the freshness guarantee hook records get from TTL.
  The bound serves a dual purpose — query performance and pid-reuse
  defense — and is overridable via `CONSPECTUS_CODEX_LOG_WINDOW_SECONDS`.
  The linker can be skipped entirely via `CONSPECTUS_DISABLE_CODEX_LOG`.
  A proper pid-reuse fix using `/proc/<pid>/stat.starttime` is deferred
  because `ProcessSnapshot` does not expose start time today.
- Fresh Codex log evidence may synthesize a sparse `AgentSession` when the
  state reader has not yet observed the thread row, mirroring the
  hook-sidecar synthesis path.
- Codex log-derived current-session evidence overrides stale
  `active_pane_command_session_match` candidates for the same mux, matching
  the demotion rule established in ADR 0028.
- Codex `thread_spawn_edges` rows produce `parent_session` candidate
  links with `lineage_kind = "spawn"`, distinct from rollout
  `forked_from_id` which continues to use `lineage_kind = "fork"`.
- The richer `threads` columns (rollout_path, git_*, model, cli_version,
  agent_*, archived_at) are read by the v1 reader but not stored on
  `AgentSessionNode` because the node intentionally has no metadata
  channel. Promoting any of those fields, or routing `git_*` through a
  `Repo` candidate link, requires a separate ADR.
- `logs.feedback_log_body` is privacy-sensitive and is never read.
- Codex `remote_control_enrollments`, `jobs`, `agent_jobs`, and
  `thread_goals` are out of scope for this ADR. They remain available for
  later work behind the audits that own those surfaces.

## Amendment: Resolver Rank For Log Evidence (2026-10-01)

The resolver had not implemented the ranking decided above.
`codex_log_current_thread_match` ranked 0 in the session ↔ mux
comparator, below even `exact_cwd_match` (20). A Codex session in a
directory shared by several panes could resolve to the wrong one,
and so could a session whose state file another pane also held open.
That kind now ranks 55, above `active_pane_fd_session_match` and the
hook-sidecar kinds (50) and every cwd kind. It also counts as process
evidence, so the resolver doesn't derive a duplicate runtime-process
link for the same session and mux. Typing the match kinds in
`CSP-567` surfaced the gap.
