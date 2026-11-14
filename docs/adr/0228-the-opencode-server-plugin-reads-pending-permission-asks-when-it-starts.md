# ADR 0228: The opencode Server Plugin Reads Pending Permission Asks When It Starts

## Status

Accepted. Extends [ADR 0206](0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
decisions 20 and 23 (the opencode reducer and the 2.x server
entrypoint). Adds no program, subcommand, socket, signal, or write path
to Conspectus (ADR 0117, ADR 0143): the calls are the plugin's, inside
opencode, through the context opencode hands it.

## Context

In opencode 2.x a permission ask and a form are events that opencode
publishes once and never repeats (research note 03, sections 3.2 and
5.4). The server entrypoint's reducer starts empty, so a plugin that
starts or reloads while a session waits on the operator never hears that
`permission.asked`: the session reads working, or nothing, until its
next event, which for a blocked session comes only once the operator
answers. The state the operator most wants is the one lost.

What the server plugin context offers, by `@opencode/plugin` 2.0.32's
published types and the server plugin documentation:

- `ctx.permission.list({sessionID})`: a session's pending permission
  requests, each `{id, sessionID, action, resources, save?, metadata?,
  source?, message?}`. The session id is required; there is no list for
  a whole location in this context.
- `ctx.session.list({directory, limit})`: a location's sessions, page by
  page through a cursor. The documentation shows it; 2.0.32's types do
  not include it in the plugin's session domain.
- No form domain at all: a pending form cannot be listed from a server
  plugin.

## Decision

1. **Read once, after subscribing.** When the server entrypoint starts,
   it first subscribes to its events, then lists its location's most
   recent sessions (at most 100, 50 a page) and asks each for its pending
   permission requests. A session with some has them added to the
   reducer as if their asks had just arrived: the session reads
   `waiting` (`permission`) from that moment, a turn counts as running,
   and a subagent's ask raises its root as any other ask does.

2. **Only adds.** The seeding never removes a wait and never changes a
   session's state other than by adding asks. An ask the instance has
   seen end (answered, rejected with its session's others, or cleared by
   the turn's end) is not added again, so a list read before an answer
   arrived cannot bring the ask back; the instance remembers the last 512
   ended asks. An ask the list names for another session is skipped, and
   an ended session takes none.

3. **What is read.** A session's id and the cursor from the session
   list; an ask's `id` and `sessionID`. The action, resources, patterns,
   metadata, source, and message are never read. The reducer's seeding
   also takes a form's `kind` and a turn's `running` or `idle` status
   word from a caller that has them; the server has neither to give.

4. **Best effort.** A context without either call (such as 2.0.32's,
   whose session domain has no `list`), a call that fails, an answer it
   does not recognize, and an unloaded plugin stop or skip the reading
   without a word. Nothing waits on it: the events go on being handled
   while it runs.

5. **Forms are not re-read.** The server plugin cannot list them; a form
   pending when the plugin starts shows from its session's next event.

## Consequences

- A server plugin that starts or reloads while a session waits on a
  permission reports it waiting at once, on an opencode whose server
  plugins can list sessions.
- Each start costs one session list per 50 sessions and one permission
  list per session, at most 100 of each, inside the service.
- An ask in a session older than the 100 most recent is missed until its
  next event, and so is a pending form.
- The event script `v2-seed-pending` encodes this for the plugin's node
  tests and the Rust replay; a stand-in context test covers the calls,
  and a node test runs the server entrypoint itself against stand-ins.

## Alternatives Considered

- **Ask lazily, when a session's first event arrives.** A blocked session
  sends no event until it is answered, which is the case to fix.
  Rejected.
- **The location-wide request list of opencode's client
  (`permission.request.list`).** Not part of the server plugin's
  permission domain, which picks `list`, `get`, and `reply`. Rejected.
- **Re-read on a timer.** Would repair a lost ask at any time, at the
  cost of a list per session per period while nothing changes. The event
  stream already says when an ask arrives; only a start misses one.
  Rejected.
- **Persist the instance's state across reloads** (in opencode's plugin
  storage). A write to opencode state, which the plugin never makes
  (ADR 0126). Rejected.

## Open Questions Answered

- *Do forms have an equivalent of `ctx.permission.list`?* Not in a
  server plugin's context; forms are not re-read.
