# ADR 0028: Hook Sidecar Mux Attribution

## Status

Accepted

## Context

`H-MUXPROC-015` found a concrete drift case: Claude Code was launched in tmux
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
provider-specific file writes. The first implementation writes to a SQLite
database named `hooks.sqlite3` in that state directory and keeps the older
per-event JSON reader as a compatibility fallback. Because harness
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

Read-only graph discovery may read this SQLite store plus legacy JSON records
in the same directory and convert fresh records into `LinkedToMux` candidates
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
uses a 15-minute active-record TTL; future continuous mode may use event
eviction instead.

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
