# ADR 0229: An opencode 2.x Client Reports The Sessions It Shows When No Server Plugin Does

## Status

Accepted. Amends [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
decision 23 (its last sentence: the fallback is now built) and
[ADR 0126](0126-opencode-2-pane-attribution.md) (the TUI entrypoint's
one call to the service, below); extends the
[ADR 0049](0049-opencode-hook-plugin-distribution.md) amendments. Builds
on [ADR 0228](0228-the-opencode-server-plugin-reads-pending-permission-asks-when-it-starts.md)'s
seeding. Adds no program, subcommand, socket, signal, or write path to
Conspectus (ADR 0117, ADR 0143).

## Context

On opencode 2.x the plugin's live state comes from its server
entrypoint, which runs in the shared service, and opencode loads it
only for a package listed in `opencode.jsonc` (ADR 0206 decision 23).
An operator who lists `dist/v2` only in `cli.json`, as plugin 0.2.0
asked, loads the TUI entrypoint alone: pane attribution works, but no
session reports live state.

Research note 03 (section 5.2, option A) described a fallback: the TUI
entrypoint reports its routed session's family from its own client's
stores, accepting that sessions no client shows are missed. Two writers
for one session would replace each other's entries in the store, since
each is a different plugin instance (ADR 0204 decision 3), and their
pids differ, so the fallback has to stay quiet while a server entrypoint
reports.

What a 2.x TUI plugin has, by `@opencode/plugin` 2.0.32's published
types and ADR 0125:

- `ctx.data.listen`: every event the service publishes, the same stream
  a server plugin subscribes to.
- `ctx.data.session`: `status(id)` (`idle` or `running`),
  `permission.list(id)` and `form.list(id)` (pending asks and forms,
  `undefined` before the client synced them), and `family(id)` (the ids
  of the session's tree).
- `ctx.client`: opencode's own client to the service, whose
  `plugin.list({location})` answers with the service's plugins for that
  location, each `{id, source, features: {server?, tui?}, state:
  {status, error?}}`.

Nothing in the TUI context says how the plugin was listed.

## Decision

1. **The same reducer, on the client's stream.** The TUI entrypoint runs
   the 2.x reducer (`live.ts`) over every event `ctx.data.listen`
   delivers, keeping its location's, as the server entrypoint does. The
   reducer follows every session; what is sent is decided below.

2. **The sessions the client has shown.** It reports the session its
   client routes to, that session's family as the stores list it, and
   every descendant the events reveal, from the moment the client first
   routes to it until the client closes; moving to another session adds
   that one and keeps the first. A session no client has shown is not
   reported.

3. **Filled in when a session joins.** When a session joins, or the
   fallback starts reporting, the client's stores fill in what the events
   before then did not say, through the reducer's seeding (ADR 0228): a
   `running` turn for a session with no evidence, and pending permissions
   and forms (a form of kind `question` is a question, any other input).
   It reads a status word, an ask's id and session, and a form's kind.
   Then every joined session, and each descendant of one, sends its
   current state.

4. **One writer per session: only while no server entrypoint is
   active.** The entrypoint asks the service for its plugin list through
   `ctx.client.plugin.list({location})` when it loads and every 60 s.
   While the list has an entry with this plugin's id (`conspectus-hook`)
   whose `features.server` is set and whose `state.status` is `active`,
   or until the list has answered at all, the fallback sends nothing. A
   call that fails changes nothing. Of each entry it reads the id, the
   server flag, and the status, never the source or an error's text.
   This amends ADR 0126's "the TUI entrypoint never calls the server
   API": it makes this one read-only call, through the client opencode
   hands it, with no credential of Conspectus's. ADR 0126 decision 3's
   objection (reading the service password to reach endpoints that
   cannot answer) does not apply: opencode authenticates the call, and
   the endpoint answers the question asked.

5. **The client's process, and nothing at close.** Snapshots carry the
   client's own pid and start time, the process the fallback runs in.
   When the client closes, the fallback sends nothing: its sessions go
   on in the service, and nothing it could say would be true. Once the
   process is gone, those sessions read `exited` (`killed`) until
   another source reports them.

6. **Attribution is unchanged.** The entrypoint's attribution records
   (the route and the routed session's lifecycle events) are sent as
   before and carry no live state; the fallback's snapshots carry no
   attribution mark, so they reach only the live-state store.

## Consequences

- With only `cli.json` listing the plugin, a 2.x client's routed session
  and its child sessions report live state, including a wait that was
  already pending when the client routed to them.
- With the package in `opencode.jsonc`, the server entrypoint reports
  every session and the client sends no live state, as before.
- What a `cli.json`-only install misses: sessions no client has shown
  (a background subagent of a session nobody opened, a turn that goes on
  after its last client closed), and every session's state once its
  clients have closed. Two clients that show one session both report
  it; their entries agree on the state but replace each other.
- Each client asks the service for its plugin list once a minute.
- The event scripts `v2-tui-fallback` and `v2-tui-server-reports`
  encode this for the plugin's node tests and the Rust replay, and a
  node test runs the TUI entrypoint itself against a stand-in context.

## Alternatives Considered

- **Read opencode's configuration files** to tell whether the package is
  listed in `opencode.jsonc`. Would mean following opencode's config
  precedence (global, project, `.opencode/`, environment overrides) and
  parsing JSONC inside the plugin, and would still say nothing about
  whether the service loaded it. Rejected.
- **Let the store arbitrate** (a client's snapshot never replaces a
  fresh one from the service). Every client would still spawn a writer
  per change, and the store would need a source rank it does not have.
  Rejected.
- **A separate directory for `cli.json` installs.** Existing installs
  list `dist/v2` in both places, so either kind would need editing.
  Rejected.
- **Report every session the client hears of.** Every client would write
  every session, so their entries would replace each other all the time.
  Rejected; a client reports what it has shown.
- **Send `unknown` or `exited` when the client closes.** Either would
  claim something about a session the service may still run. Rejected.

## Open Questions Answered

- *How does the client know a server entrypoint reports?* It asks the
  service for its plugin list and looks for this plugin's active server
  entrypoint.
