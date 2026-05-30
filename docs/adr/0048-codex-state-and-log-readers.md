# ADR 0048: Codex State and Log Readers for Mux Attribution

## Status

Accepted

## Context

`H-MUXPROC-004` calls for read-only queries against harness state databases to
strengthen `AgentSession` discovery and `AgentSession -> MuxSession`
attribution. The May 2026 audit covered the three on-disk SQLite stores
Conspectus sees today: `opencode.db`, Codex `state_<N>.sqlite`, and Codex
`logs_<N>.sqlite`.

`opencode.db` carries session metadata, parent lineage, and recency
timestamps. Those fields are already consumed by
`src/discovery/harness/opencode.rs` per ADR 0018. The schema does not record
process id, terminal, or server-binding evidence. Live opencode↔mux binding
therefore belongs to a plugin/server adapter (`H-MUXPROC-007` /
`H-MUXPROC-014`), not a state reader. The audit closes opencode's slice of
`H-MUXPROC-004`.

Codex's stores have not been read yet and contain the strongest live
attribution evidence found anywhere in the audit:

- `state_<N>.sqlite#threads` holds one row per Codex thread/session with
  `rollout_path` (absolute JSONL transcript path), `cwd`, `git_sha`,
  `git_branch`, `git_origin_url`, `model`, `cli_version`, millisecond
  timestamps, archival state, and a privacy-sensitive `first_user_message`.
- `state_<N>.sqlite#thread_spawn_edges` records parent→child subagent
  lineage, mirroring opencode's `session.parent_id`.
- `logs_<N>.sqlite#logs.process_uuid` is encoded as the literal string
  `pid:<os_pid>:<random_uuid>`. Paired with `thread_id` per most-recent `ts`,
  this names the thread a live Codex process is currently writing — including
  cases where launch argv `codex --resume <session A>` is stale because the
  operator switched sessions in-process. This is the Codex equivalent of the
  Claude Code drift case that motivated `H-MUXPROC-015` and ADR 0028, and the
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
  state root. Extracted fields: `id`, `rollout_path`, `cwd`, `title` (falling
  back to `first_user_message` capped and normalized like opencode previews),
  `updated_at_ms`/`created_at_ms`, `git_sha`, `git_branch`,
  `git_origin_url`, `model`, `model_provider`, `cli_version`, `agent_role`,
  `agent_nickname`, and `archived`/`archived_at` so the resolver can suppress
  archived rows from "active" views.
- One intra-harness `parent_session` candidate link per `thread_spawn_edges`
  row, matching ADR 0018's opencode lineage shape.

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

`:ts_floor` is `now() - 15 minutes`, matching the active-record TTL chosen in
ADR 0028. The 15-minute bound keeps the query cheap on heavy log volumes
because the table lacks a `(process_uuid, ts)` compound index, and it
matches the freshness budget Conspectus already uses for "live session"
evidence.

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
`immutable=1` because we want to observe fresh writes through the WAL. The
log reader never selects `feedback_log_body`; the state reader caps and
normalizes `first_user_message` exactly like the opencode preview path. No
write paths are introduced.

### opencode

`H-MUXPROC-004` for opencode is closed out by this ADR with no code change:
the audit found nothing in `opencode.db` beyond what the existing reader
extracts that would strengthen live mux attribution. Live opencode↔mux
binding remains the responsibility of `H-MUXPROC-007` / `H-MUXPROC-014`.

### Explicitly deferred

- Codex `remote_control_enrollments` (control-plane attribution) belongs to
  `H-MUXPROC-005` / `H-MUXPROC-006`, not 004.
- Codex `jobs`, `agent_jobs`, `agent_job_items`, `thread_goals`, and
  `stage1_outputs` are background-agent surfaces. They were empty on the
  audit machine; defer until in-the-wild usage justifies a separate slice.
- `feedback_log_body` is privacy-sensitive and is never selected.

## Consequences

- The Codex equivalent of the H-MUXPROC-015 stale-`--resume` drift is fixed
  without requiring a hook, control plane, or terminal interaction.
- Codex `AgentSession` discovery becomes the indexed `threads` table rather
  than enumerating rollout JSONL files. The existing rollout-file reader can
  fall back when state is missing or unreadable.
- Codex subagent lineage gets the same `parent_session` candidate-link
  treatment opencode has had since ADR 0018, which lets the resolver and TUI
  share a single subagent-handling code path across both harnesses.
- A `(process_uuid, ts)` compound index would make the log lookup faster but
  is not required: the 15-minute floor and `idx_logs_thread_id` keep cost
  bounded. If real-world log volumes ever make this expensive, the linker can
  pre-filter by candidate Codex pids in process snapshot order so the
  WHERE-clause comparisons stay narrow.
- Conspectus now reads Codex schema across two filename-versioned databases.
  Adding `state_6` or `logs_3` upstream will require a new column-probe pass
  and may require a new resolved-relationship shape, but the
  highest-suffix-wins discovery rule degrades gracefully until that pass
  lands.
- The synthesized-session path follows the same shape as ADR 0028, so TUI
  and resolver code that already tolerates sparse hook-synthesized agent
  session nodes will tolerate log-synthesized nodes too.
- The state reader exposes `git_sha`, `git_branch`, and `git_origin_url` per
  Codex thread. ADR 0045 already governs how session-level repo signals
  participate in repo binding; this ADR opts into that path without changing
  the binding rules.
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

- `H-MUXPROC-004` for opencode resolves as a no-op: the schema offers no
  live-binding signal beyond what the existing reader already consumes.
- The freshness budget for log-derived current-session evidence is 15
  minutes, consistent with ADR 0028's hook-sidecar TTL.
- Fresh Codex log evidence may synthesize a sparse `AgentSession` when the
  state reader has not yet observed the thread row, mirroring the
  hook-sidecar synthesis path.
- Codex log-derived current-session evidence overrides stale
  `active_pane_command_session_match` candidates for the same mux, matching
  the demotion rule established in ADR 0028.
- Codex `thread_spawn_edges` participates in `parent_session` candidate
  links on the same shape ADR 0018 defines for opencode.
- `logs.feedback_log_body` is privacy-sensitive and is never read.
- Codex `remote_control_enrollments`, `jobs`, `agent_jobs`, and
  `thread_goals` are out of scope for this ADR. They remain available for
  later work behind the audits that own those surfaces.
