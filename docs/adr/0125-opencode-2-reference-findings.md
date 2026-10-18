# ADR 0125: OpenCode 2.x Reference Findings

## Status

Accepted.

## Context

OpenCode 2.0 replaced the per-TUI process model with one background
service that every TUI, web, and desktop client talks to. It also changed
the storage schema, the plugin API, and the server API. Conspectus reads
opencode's database, launches and resumes opencode in tmux, classifies
pane processes, and attributes sessions to panes through an in-process
plugin (ADR 0049), so each of those needed checking against a real 2.x
install. Neither the 2.0 announcement nor the V1 migration guide
documents on-disk storage, CLI flags, or the service's process shape.

This ADR records what a sandboxed spike found, so the implementation
decisions that follow (CSP-603 to CSP-608, ADR 0126) cite one place.

### Method

- **Versions.** opencode 2.0.32 (npm `@opencode/cli` 2.0.32, platform
  package `@opencode/cli-linux-x64`, tarball integrity checked) and
  opencode 1.18.32 (the operator's nix build). Plugin and schema types
  come from npm `@opencode/plugin` 2.0.32 and `@opencode/schema` 2.0.32.
- **Isolation.** Every run used its own `HOME` and `XDG_CONFIG_HOME`,
  `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`, and
  `XDG_RUNTIME_DIR` under a scratch directory, with `TMUX` and
  `OPENCODE_CONFIG*` unset unless a test needed a pane. tmux tests used a
  private tmux server. The model was a local OpenAI-compatible mock, so
  no provider was contacted. The operator's install was never upgraded.
  Afterwards the operator's `opencode.db` still had no `session_v2`
  table and no 2.x migrations, and their config dir had no
  `service.json`. The one change seen in the operator's state dirs, a
  `locks/` directory mtime, matches their running 1.x instances.
- **Fixtures.** `tests/fixtures/opencode/v2-fresh.sql` (a fresh 2.x
  database) and `tests/fixtures/opencode/v2-migrated.sql` (a 1.18.32
  database after 2.0.32 migrated it) are sanitized dumps of the sandbox
  databases: paths rewritten to `/home/user`, unrelated tables dropped,
  and literal quotes that the `run` CLIs add around prompts removed.

Each finding below is marked **verified** (observed in the sandbox) or
**unverified** (read from published types or docs, or not reproduced).

## Decision

The findings below are the reference for opencode 2.x support.

### Storage

- **verified:** 2.x keeps `$XDG_DATA_HOME/opencode/opencode.db` (WAL
  mode). A fresh 2.x database has no 1.x `session`, `message`, or `part`
  tables.
- **verified:** Sessions live in `session_v2`. Columns Conspectus needs:
  `id`, `parent_id`, `fork_session_id`, `fork_boundary`, `directory`,
  `title`, `agent`, `version`, `time_created`, `time_updated`,
  `time_archived`, plus `time_idle`, `idle_outcome`, and `time_viewed`.
  Timestamps are epoch milliseconds.
- **verified:** `parent_id` is set only on sessions spawned by the
  `subagent` tool, and that child row carries `agent` (for example
  `general`) and the task description as `title`. A fork is a new
  session with `fork_session_id` pointing at its source, a
  `fork_boundary` naming the source message, a copy of the source's
  messages up to that boundary, and a `(fork #n)` title suffix.
  `parent_id` stays null on forks.
- **verified:** Transcripts live in `session_message`, ordered by `seq`,
  which is unique per session but not contiguous. `type` mirrors
  `data.type`. Observed types: `user` (`data.text`, `files`), `assistant`
  (`agent`, `model`, `content`, `finish`, `tokens`, `time`), `idle`
  (`outcome`: succeeded, failed, or interrupted), and `system` (`text`,
  injected by migration to describe changed tools).
- **verified:** `assistant.content` items are `text`, `reasoning`, and
  `tool`. Tool items carry `id` (the call id), `name`, and
  `state.status`: `completed` holds `input` and a non-empty `content`
  array of `{type: "text", text}`, and `error` holds `input` and
  `error: {type, message}`. A subagent tool result embeds the child
  session id in `state.metadata.sessionID`.
- **unverified:** The remaining types and states come from
  `@opencode/schema` 2.0.32 only: `synthetic` (`text`, `description`),
  `skill` (`skill`, `name`, `text`), `shell` (`command`, `status`,
  `exit`, `output`), `compaction` (`status` running, completed, or
  failed; `reason`; `summary`), `model-switched`, `agent-switched`,
  `location-switched`, and tool states `streaming` and `running`. Driving
  a model switch or `/compact` through the 2.x CLI hung in the sandbox.

### Migration

- **verified:** On its first start against a 1.x database, the 2.x
  service copies every 1.x session into `session_v2` under the same id,
  with the same directory, title, and timestamps, and copies its
  messages into `session_message`. The 1.x tables are left in place,
  and the 2.x service did not modify them during or after migration.
- **verified:** Migration appends a `system` message to each migrated
  session that used tools, describing the tool changes (for example,
  `read` now takes `path` instead of `filePath`).
- **unverified:** Whether a later 1.x write to the stale tables (a 1.x
  binary still running against the same database) is ever re-migrated.

### CLI

- **verified:** `opencode --session <id>` (`-s`) still resumes a
  session. For an id that does not exist, the TUI opens empty and its
  first prompt creates a session under exactly that id, so a resume
  never fails because the session is missing.
- **verified:** `opencode [directory]`, `--continue`, `--prompt`,
  `run`, `session list`, `serve`, `service`, `api`, and `plugin`
  exist. `--standalone` runs a private server instead of the shared
  service.

### Process Model

- **verified:** The first client to need the service starts
  `opencode serve --service` as a child of itself in a new session
  (`setsid`). When that client exits, the service is reparented and keeps
  running. Its endpoint, pid, and a password are written to
  `$XDG_STATE_HOME/opencode/service.json`. The port defaults to 49374;
  `opencode service set port` stores another in
  `$XDG_CONFIG_HOME/opencode/service.json`. Two services on one port
  conflict.
- **verified:** The service inherits the environment of the client that
  started it, including `TMUX` and `TMUX_PANE`. Every later client
  shares that one service, whatever pane it runs in.
- **verified:** The service is the only process holding `opencode.db`
  open. A TUI client holds only its pty, the log file, and TCP sockets
  to the service, so fd evidence no longer ties a pane to opencode
  state.
- **verified:** A TUI client's argv is its launch argv (`opencode`,
  `opencode --session <id>`, or `opencode <dir>`). It never changes when
  the operator switches sessions in-app.
- **verified:** The service runs tools and subagents. Shell tools
  surface as `shell.created` and `shell.exited` service events, and a
  subagent is a child session the service drives, not a client process.
- **unverified:** The exact process shape of a running shell tool (its
  parent is presumably the service). The test commands exited too fast
  to capture.

### Plugins

- **verified:** 2.x plugins are `{id, setup(ctx)}` objects from npm
  `@opencode/plugin`. A package can ship separate entrypoints: `server`
  (falling back to the package root) runs inside the service, and `tui`
  runs inside each TUI client. The host resolver looks for
  `<dir>/server`, then `<dir>/index`, and `<dir>/tui` in a local
  directory; a package installed by name resolves the same subpaths
  through its `exports`.
- **verified:** Server plugins are listed in `opencode.jsonc` as
  `"plugins": [{"package": "<dir>"}]`, and the TUI also loads the `tui`
  entrypoint of a package listed there. TUI-only plugins are listed in
  `$XDG_CONFIG_HOME/opencode/cli.json` (`"plugins": ["<dir>"]`), which
  2.x migrates from the 1.x `tui.json`. A directory with no server
  entrypoint listed in `opencode.jsonc` is ignored. `opencode plugin
  add` accepts only npm and Git specifiers and writes a server package
  to `opencode.jsonc` and a TUI-only one to `cli.json`.
- **verified:** A server plugin is instantiated once per location
  (directory) the service serves. Its `process.env` is the service's,
  so `TMUX_PANE` names the first client's pane for every session.
  `ctx.event.subscribe` yields every event: `session.created`,
  `session.renamed`, `session.execution.started`,
  `session.execution.succeeded`, `session.viewed`, `session.step.*`,
  `session.text.*`, `session.tool.*`, and others. No event identifies
  the client.
- **verified:** A TUI plugin runs in its client's process, so its
  `process.env` carries that pane's `TMUX` and `TMUX_PANE`, and a child
  it spawns has the client as its parent. `ctx.ui.router.current()`
  returns `{type: "session", sessionID}` for the session the client is
  showing, or `{type: "home"}`, and it changed as soon as a new
  session was created. `ctx.data.listen` delivers the same event stream.
- **verified:** 1.x plugins do not load under 2.x (the config key is
  now `plugins`, not `plugin`).

### Server API

- **verified:** The service exposes an OpenAPI 3.1 document at
  `/openapi.json`, behind HTTP basic auth (`opencode:<password>`).
- **verified:** No endpoint maps clients or tabs to sessions.
  `GET /api/session/active` lists only sessions with a running turn, and
  `GET /api/event` (SSE) carries session ids and a location directory but
  no client identity. `POST /api/session/{id}/view` is a write (it sets
  `time_viewed`).
- **verified:** `$XDG_STATE_HOME/opencode/latest/tui/tabs.json` holds
  open tabs per directory and is shared by every client, so it cannot
  say which pane shows which tab.

## Consequences

- **Discovery (CSP-603).** Read `session_v2` when present and prefer it
  over a same-id `session` row. Take the preview from `session_message`.
  Map `parent_id` to `spawn` lineage and classify those children as
  subagents. Map `fork_session_id` to `fork` lineage (ADR 0018 and
  ADR 0048 vocabulary).
- **Viewer (CSP-604).** Parse `session_message` by `seq`, with tool
  results from `state.content` and errors from `state.error.message`.
  Treat `idle` with outcome `interrupted` as the abort signal.
- **Hook safety (CSP-605).** A hook writer whose harness ancestor is the
  2.x service must not trust its tmux context. The cwd and pid fallbacks
  in `discovery::hook_sidecar` would also bind such a record to an
  unrelated pane, so service-context records must not produce mux links
  at all.
- **Attribution (CSP-606, ADR 0126).** The server API offers no
  read-only client-to-session mapping. A TUI plugin does have the pane's
  environment and the client's current session, which makes it the
  replacement attribution source.
- **Plugin (CSP-607).** The Conspectus plugin needs a 2.x `tui`
  entrypoint. A 2.x `server` entrypoint could only produce pane-less
  records.
- **Launch and processes (CSP-608).** Launch and resume argv are
  unchanged. `opencode serve` (and `opencode acp`, which is driven over
  stdio) is a background process, not a human pane. Missing-session
  resumes no longer fail, so ADR 0103's failed-start fallback stops
  firing for opencode 2.x. The resumed pane gets a new session under the
  pinned id, and the pin stays bound to it.

## Alternatives Considered

- **Upgrading the operator's install to observe 2.x.** Rejected by the
  task: it would migrate the operator's real database.
- **Building 2.x with nix.** The npm release runs once its interpreter is
  patched for NixOS, which is faster and matches what operators install.
- **Committing binary fixture databases.** Rejected in favour of SQL
  dumps, which follow the existing `state.sql` precedent, can be
  reviewed, and show that the sanitizing worked.
