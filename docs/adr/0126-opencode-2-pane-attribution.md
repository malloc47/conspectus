# ADR 0126: OpenCode 2.x Session-To-Pane Attribution

## Status

Accepted. Amends ADR 0049 for opencode 2.x.

## Context

In opencode 1.x the Conspectus plugin (ADR 0049) ran inside each TUI
process, in that TUI's pane. When it ran `conspectus hook write
opencode`, the writer read the pane from its own `TMUX` environment and
found the TUI as its first `opencode` ancestor. Those hook records were
the strongest session-to-pane evidence Conspectus had for opencode: they
followed in-app session switches, which launch argv cannot.

opencode 2.x breaks that contract (ADR 0125):

- Server plugins run inside one shared `opencode serve --service`
  process. It inherits the environment of whichever client started it,
  so its `TMUX_PANE` names that first client's pane for every session.
  A record written from it would bind every session to that pane at hook
  rank, outranking correct process and activity evidence.
- Stripping the tmux context is not enough. `discovery::hook_sidecar`
  falls back to the record's pid and then its cwd to find a mux, so a
  pane-less service record would still bind at hook rank to any pane
  that shares the cwd.
- The V2 server API has no read-only way to map clients to sessions.
  `GET /api/session/active` lists only sessions with a running turn, the
  `/api/event` stream carries no client identity, and
  `POST /api/session/{id}/view` is a write. `latest/tui/tabs.json` is
  shared by every client. TUI clients no longer hold `opencode.db` open,
  so fd evidence is gone, and a client's argv never follows in-app
  session switches.
- A 2.x TUI plugin does run inside its client's process, so it has that
  pane's `TMUX` and `TMUX_PANE`. Its child processes have the client as
  their parent, and `ctx.ui.router.current()` returns the session the
  client is showing.

CSP-221, an HTTP/ACP server adapter, was closed because the in-process
plugin already gave pane-accurate records. Its closure note asked for it
to be reopened if that stopped holding.

## Decision

1. **Background-harness hook records.** `HookRecord` gains
   `background_harness`, a provider-neutral flag meaning that the harness
   process that ran the hook is a background process (a shared service or
   helper daemon) rather than a pane's agent. `conspectus hook write` sets
   it when the command line of its harness ancestor, the same process
   the pid walk already finds, matches that harness's
   `RuntimeSignature::is_background_process`. For opencode that means a
   first non-flag argument of `serve` or `acp` (ADR 0125: the 2.x service
   is `opencode serve --service`). The writer then records no tmux
   context. `discovery::hook_sidecar` produces no mux link from such a
   record, whether through tmux, pid, or cwd. The record is still stored,
   since it is a truthful observation; it just says nothing about panes.
2. **opencode 2.x attribution comes from the plugin's TUI entrypoint.**
   The Conspectus opencode plugin ships a 2.x `tui` entrypoint that runs
   `conspectus hook write opencode` from the client process. That
   restores the 1.x contract unchanged: the writer reads the pane's tmux
   context, its pid walk finds the TUI client, and the records keep the
   existing `hook_session_match` rank, above activity, cwd, and launch
   argv evidence (ADR 0048 ordering). The entrypoint writes a record when
   the client's routed session changes, including in-app switches, and
   for lifecycle events of that routed session only. Events for other
   sessions reach every client, so forwarding them would repeat the 1.x
   service bug from the TUI side.
3. **No server entrypoint and no server API client.** A 2.x server
   entrypoint could only produce background-harness records. Querying the
   V2 server API would mean reading the service password from
   `service.json` to reach endpoints that cannot answer the question.
   CSP-221 stays closed.

The TUI entrypoint only reads client state. It never calls the server
API, never writes opencode state, and never sends input. Its only effect
is the hook sidecar write that ADR 0087 already sanctions (category 2),
and ADR 0028 is untouched.

## Consequences

- Two opencode 2.x TUIs in different panes on the same repo each produce
  records naming their own pane and their own routed session, so each
  binds to its own session.
- Records from a 2.x service, whether from an operator's own server
  plugin or a mis-installed one, can no longer bind sessions to the
  wrong pane.
- Without the 2.x plugin, opencode 2.x attribution falls back to the
  remaining evidence: launch argv such as `opencode --session <id>`
  (pane-exact until the operator switches sessions in-app), activity
  timing, and cwd. Discovery keeps working; it is just less precise when
  several panes share a repo.
- The detection reuses the runtime signature that already classifies
  background processes in pane process trees (ADR 0047). Any harness
  whose hooks can run from a background process gets the same
  protection by classifying it there.
- The 1.x plugin and its records are unchanged.

## Alternatives Considered

- **Query the V2 server API.** Rejected. No endpoint maps clients or
  tabs to sessions, `session.active` covers only running turns, and
  `view` is a write. It would also mean handling the service's password.
- **Subscribe to the service event stream.** Rejected. The events carry
  session ids but no client identity, and holding a stream open is a
  long-lived connection the daemon does not otherwise need.
- **Read `latest/tui/tabs.json`.** Rejected. One file is shared by every
  client and only lists open tabs per directory.
- **Rely on launch argv and fd evidence.** Not enough. TUIs hold no
  database fds in 2.x, and argv goes stale after an in-app switch. Argv
  stays as weaker evidence.
- **Only strip the tmux context from service records.** Rejected because
  of the pid and cwd fallbacks described above.
- **Drop background records in the writer.** Rejected. Keeping them
  costs nothing, and a future pane-less consumer (for example session
  recency) could use them.
