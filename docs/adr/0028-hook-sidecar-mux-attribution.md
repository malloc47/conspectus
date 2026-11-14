# ADR 0028: Hook Sidecar Mux Attribution

## Status

Accepted. Refined by ADR 0146: the spool is the durable hook record
with or without a daemon, and a daemon ingest writes it too. Amended:
the SQLite store below was retired with the rest of SQLite (ADR 0082)
for the latest-record spool, which keeps one record per pane, and no
time limit applies to a record (ADR 0146's amendment). See the
amendment at the end. Amended again 2026-11-13: a record that names
its tmux server's socket is matched to a mux of that server only (the
last amendment). Its carve-out for already-visible status
surfaces is used once, by ADR 0217: the title a Codex pane sets, read
for live state only. Its exception for a harness-documented,
non-mutating control channel audited as safe is used once, by ADR 0218:
`claude agents --json`, run as a control-plane adapter for live state
only, never for attribution.

## Context

`CSP-227` found a concrete drift case: Claude Code was launched in tmux
with `claude --resume <session A>`, then the operator used in-app `/resume` to
switch to session B. The process argv still named A, so Conspectus treated
start-command evidence as the active session and attached the mux indicator to
the wrong row.

Conspectus needs a non-mutating current-session source. Terminal injection is
tempting because visible status surfaces can show the current session id, but
commands such as `/usage` or `/status` are user input. They can alter
transcripts, recency, terminal state, or future model context, so they are not
acceptable as background discovery.

The May 2026 audit found:

- Claude Code hooks are viable for the immediate fix. The official hooks
  reference documents lifecycle hooks, including `SessionStart` for startup,
  resume, clear, and compact. Hook input includes common session fields such as
  `session_id`, `transcript_path`, and `cwd`, and command hooks receive JSON on
  stdin. See <https://code.claude.com/docs/en/hooks>.
- The local Codex CLI exposes an experimental `app-server` and generated
  JSON-RPC schemas with thread read/list/resume methods and thread metadata,
  but local CLI help did not expose a hook mechanism. Treat Codex app-server
  attribution as an audit-gated control-plane adapter, not as part of the
  Claude fix.
- OpenCode exposes a headless HTTP server and TUI-server architecture, plus a
  plugin system with session events. Its ACP mode is a JSON-RPC stdio server
  for editor integrations. See <https://opencode.ai/docs/server/>,
  <https://opencode.ai/docs/plugins/>, and <https://opencode.ai/docs/acp/>.

## Decision

Conspectus will support opt-in hook sidecar records as current-session
evidence. Harness-specific hook emitters call `conspectus hook write`; the
subcommand writes observations under the user's state directory, outside
project trees:

- `$CONSPECTUS_HOOK_SIDECAR_STATE` when set
- otherwise `$XDG_STATE_HOME/conspectus/hooks`
- otherwise `$HOME/.local/state/conspectus/hooks`

The durable hook interface is `conspectus hook write`, not direct
provider-specific file writes. The first implementation wrote to a SQLite
database named `hooks.sqlite3` in that state directory and kept the older
per-event JSON reader as a compatibility fallback. (Amended: the SQLite
store is gone; records go to the spool `hooks-latest.json`, one per pane,
and the per-event JSON reader remains.) Because harness
configuration targets the subcommand rather than a storage file, Conspectus
can later move hook observations to a daemon ingest path or another
local-state backend without changing installed hooks.

Schema version 1 records contain:

- `schema_version = 1`
- `harness_key`
- `session_key`
- optional `cwd`
- optional `pid` and `ppid`
- optional `tmux` object with `session_name`, `native_id`, `pane_id`, and
  `socket_path`
- optional `transcript_path`
- optional `hook_event_name`
- `observed_epoch`
- optional `harness_version`

Read-only graph discovery may read this store (now the spool) plus legacy JSON
records in the same directory and convert fresh records into `LinkedToMux`
candidates
after ordinary harness and tmux discovery has produced nodes. Matching order
is:

1. agent session by explicit `harness_key` + `session_key`
2. mux session by tmux native id / session name
3. mux session by active pane pid matching `pid` or `ppid`
4. mux session by cwd as a weak fallback

If a fresh hook record identifies a mux but the corresponding agent session
node has not been discovered yet, Conspectus may synthesize a sparse
`AgentSession` node from the hook record. This covers fresh Claude Code
sessions that have fired `SessionStart` but have not yet persisted a JSONL
transcript because no user message has been recorded. When a transcript path is
present, the synthesized node infers its state scope from the path's state
root; otherwise it uses a hook-sidecar fallback scope.

Fresh explicit hook session evidence ranks above active-pane fd evidence,
active-pane command/start-command evidence, file activity evidence, and
cwd-only evidence. Stale records are ignored for active mux attribution. V1
used a 15-minute active-record TTL. (Amended: no time limit applies. A
record stays evidence until a fresher record for its pane supersedes it,
and is ignored when its process is gone, its mux was created after it, or
the mux's active pane runs another harness.)

When a fresh hook record identifies session B for a mux that already has an
`active_pane_command_session_match` candidate for session A, Conspectus marks
the A candidate overridden by the hook candidate. The stale launch evidence
stays visible in diagnostics but no longer drives the preferred mapping.

Conspectus must not use `tmux send-keys`, slash commands, prompt injection, or
generic terminal scraping to ask an agent for its current session id. The only
exception is a harness-documented non-mutating control channel audited as safe;
such channels must be implemented as explicit control-plane adapters.

## Consequences

- Claude Code `/resume` drift can be fixed without sending text into the
  conversation.
- The sidecar path remains opt-in: users install a hook emitter when they want
  definitive current-session attribution, and ordinary discovery still works
  without it.
- Records live outside project trees and are rebuildable observations, not
  durable user-authored intent.
- Sidecar records are local state and may contain paths or session ids. They
  should be written with user-only permissions where possible.
- Hook records can be generalized to Codex and OpenCode if their audits prove
  non-mutating hook/plugin payloads with current-session identity.
- `conspectus hook write` is the compatibility boundary between harness hooks
  and Conspectus storage. Storage changes after v1 should happen behind that
  command rather than requiring users to edit every harness hook.
- Rebuildable hook observations remain semantically separate from durable
  user-authored intent such as session aliases in ADR 0029, even if both are
  eventually backed by SQLite-shaped implementation code.

## Alternatives Considered

- **Inject `/usage` or `/status` into tmux panes.** Rejected because this is
  user input and can mutate transcripts or model context.
- **Scrape visible terminal text generically.** Rejected as brittle and
  privacy-hostile. It can be considered only for already-visible status
  surfaces after non-mutating control, hook, and state sources fail, and only
  with a separate ADR.
- **Treat command-line `--resume` as definitive.** Rejected by the observed
  Claude Code drift case.
- **Persist sidecar records in project config.** Rejected because hook events
  are rebuildable local observations, not user-authored project intent.
- **Append hook events to a per-harness JSONL file.** Deferred. JSONL would
  keep a compact chronological log and reduce directory entries, but
  concurrent hooks need append/locking discipline, cleanup is less direct, and
  readers must tolerate partial or corrupt lines.
- **Store one latest-record file per harness/session or pane.** Deferred.
  This bounds file growth and makes "current state" cheap to read, but it
  discards event history and requires careful key design for pane reuse,
  resumed sessions, and multiple hook events from the same process.
- **Store hook observations in a Conspectus SQLite database.** Accepted once
  hooks call `conspectus hook write`. SQLite supports indexing, retention,
  de-duplication, "latest fresh record" queries, and richer continuous-mode
  state. The Rust writer subcommand owns migrations, busy timeouts, atomicity,
  and fallback behavior so provider scripts can remain thin shims.
- **Send hook events to a local Conspectus daemon or socket.** Deferred until
  continuous mode exists. A daemon can validate records centrally and update
  views immediately, but hooks still need a no-daemon fallback and discovery
  cannot depend on a long-running process.
- **Keep provider-specific emitter scripts as the durable interface.**
  Accepted only as a bootstrap path. `conspectus hook write` is the preferred
  writer so the schema, permissions, storage behavior, and migrations live
  beside the Rust reader.
- **Use one SQLite database for hook observations and session aliases.**
  Deferred. Sharing implementation machinery may make sense, but hook records
  are rebuildable local observations while ADR 0029 aliases are durable
  user-authored intent with project/global placement and review expectations.
  A shared physical backend must preserve that semantic split: evicting or
  rebuilding hook state must not delete aliases.
- **Install hooks manually forever.** Rejected for the durable path. Manual
  setup remains documented, but `conspectus hook init` provides idempotent
  install/status/remove flows while preserving unrelated harness
  configuration.

## Open Questions Answered

- Start-command session ids are launch evidence, not current-session truth.
- Terminal injection is not an acceptable attribution strategy for read-only
  discovery.
- Claude Code hook sidecars are the preferred first fix path for
  post-`/resume` current-session attribution.
- SQLite under user state is the v1 writer storage format. Legacy per-event
  JSON files remain readable so existing hook configs degrade cleanly during
  migration.
- `conspectus hook write` is the forward-compatible boundary for hook storage
  changes. It owns migrations and locking behavior.
- Hook observations are local operational state. User-authored aliases remain
  durable intent per ADR 0029 unless a later ADR explicitly changes that
  storage contract.
- Fresh hook observations may synthesize sparse session nodes when the harness
  has not yet persisted the transcript/state file that ordinary discovery
  would otherwise read.

## Amendment: storage, panes, and freshness (2026-11-12)

What this ADR says about storage and freshness no longer describes the
code. The decision to attribute through `conspectus hook write` stands.

- **Storage.** The SQLite store was retired with the rest of SQLite
  (ADR 0082). Hook records go to the spool `hooks-latest.json` in the
  same state directory, which every refresh replays and a daemon ingest
  writes first (ADR 0146). The per-event JSON reader remains.
- **One record per pane.** The spool keeps the latest record per pane,
  so agents in split panes of one tmux session keep separate records,
  and attribution keeps one Active hook link per pane, the freshest
  (ADR 0146's amendment).
- **No time limit.** The 15-minute TTL is gone. A record is ignored for
  active attribution when its process is no longer running, when its mux
  was created after it, or when the mux's active pane runs another
  harness; otherwise it stays evidence until a fresher record for its
  pane supersedes it. (Amended by
  [ADR 0220](0220-a-hook-record-is-judged-by-the-harness-its-own-pane-runs.md):
  the harness gate reads the record's own pane when the mux probe
  listed it, and the active pane only when the record names no pane or
  no pane listing is at hand.)
- **The hook's own pane.** `hook write` asks tmux about the pane named
  by `TMUX_PANE` (`display-message -t`), so the record's tmux session and
  pane are the hook's. Untargeted, tmux answers a caller with no
  terminal, such as a hook a harness runs detached, with the session's
  active pane (checked on tmux 3.6 and 3.7). A `TMUX_PANE` that is not a
  pane id leaves the question untargeted; a pane tmux doesn't know gets
  no answer rather than another pane's.

## Amendment: a record's mux is on its own tmux server (2026-11-13)

Matching order step 2 compared a record's tmux session name with each
mux's native id, whatever server the mux was on, and step 4 took any
mux in the record's directory. A record from a default server under
another `TMUX_TMPDIR` or another uid's, whose session shares a name
with one on the probed default server, was tied to that mux by a strong
hook link. A record from a `-L work` server never matched by name (its
mux's native id is `work:<name>`) and fell back to any mux in its
directory, on any server. The live-state binding already matched an
entry's server by its full socket path
([ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md)'s
amendment of 2026-11-13); attribution now uses the same rule.

1. **A record that names its server's socket path** (`tmux.socket_path`,
   which `hook write` takes from `$TMUX`) matches only a tmux mux of that
   server: the server's name is the path's last component (`tmux -L`
   names the socket file), and where the mux's probe read the server's
   `#{socket_path}`, the record's path must be that path exactly. Step 2
   then matches the record's session name against the mux's bare session
   name, so a `-L` server's record matches its own session; step 4 takes
   a mux in the record's directory on that server only.
2. **A record that names no socket path**, from a tmux that left `$TMUX`
   without one or a record written before the field, matches by name and
   directory as before. A probe that read no socket path leaves the
   server's name to decide.
3. **Step 3 is unchanged.** A mux whose active pane runs the record's
   process is that process's, whichever server the record names.

Alternatives considered: comparing names only for `-L` servers, which
leaves the default server under another `TMUX_TMPDIR` attributed to
yours; and keeping the directory fallback on every server, which ties a
record from a server Conspectus does not list to a session of yours
that shares its directory, with the same strong link a name match makes.
