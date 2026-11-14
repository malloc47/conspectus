# @conspectus/opencode-hook

OpenCode plugin that tells [Conspectus](https://github.com/malloc47/conspectus)
what each opencode session is doing (working, waiting for you, ready,
compacting, failed, and how many subagents it runs) and which session each
pane shows, even after in-app session switches. It reports through
`conspectus hook write opencode`. One package serves both opencode majors:

- **opencode 1.x** loads the package root (`dist/index.js`). See
  [ADR 0049](../../docs/adr/0049-opencode-hook-plugin-distribution.md).
- **opencode 2.x** loads the server and TUI entrypoints in `dist/v2/`. See
  [ADR 0126](../../docs/adr/0126-opencode-2-pane-attribution.md) and its
  amendment.

What each event means is decided in
[ADR 0206](../../docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md).

## What it does

The plugin keeps a small state per session (`src/live.ts`) and, whenever
what a session shows changes, sends that whole state as a snapshot; while
a session is not ready it sends it again every 60 seconds. The Rust writer
adds the pane's tmux context and stores it in Conspectus's live-state
store, `$XDG_STATE_HOME/conspectus/live-state.json`. A wait of a child
session (a subagent asking for permission or a question) also shows on
its root session, which is the one you look at.

On **opencode 1.x** the plugin runs inside each TUI. It reads
`session.status`, `session.idle`, `session.error`, `session.created`,
`session.updated`, `session.deleted`, `session.compacted`,
`permission.asked`, `permission.updated`, `permission.replied`,
`question.asked`, `question.replied`, and `question.rejected`, plus the
`experimental.session.compacting` and `dispose` hooks. A root session's
`session.created`, `session.updated`, `session.status`, `session.idle`,
and `session.compacted` are also recorded for attribution (which session
the pane shows); a child session's never are.

On **opencode 2.x** sessions run in one shared background service.

- The **server entrypoint** (`dist/v2/server.js`) runs there, so it sees
  every session, including background subagents and turns that go on
  after their client closed. It reads the `session.execution.*`,
  `session.retry.scheduled`, `session.step.started`,
  `session.compaction.*`, `session.created`, `session.deleted`,
  `session.archived`, `session.status`, `permission.*`, and `form.*`
  events of its own location and sends live state only. Its writes come
  from the service, so they never say anything about panes. opencode
  never repeats a permission ask, so when it starts (or reloads) it also
  asks opencode for the pending permissions of its location's 100 most
  recent sessions and reports those sessions waiting; this needs an
  opencode whose server plugins can list sessions (`ctx.session.list`),
  and forms cannot be listed there.
- The **TUI entrypoint** (`dist/v2/tui.js`) runs inside each client and
  records which session that client shows: whenever the client's route
  moves to a session (including in-app switches, as `tui.route`), and on
  `session.created`, `session.renamed`, `session.status`, and
  `session.idle` of that session. Events for other sessions are ignored
  for this, since every client receives them.
- When no server entrypoint of this plugin is active in the service
  (the package is listed only in `cli.json`), the TUI entrypoint also
  reports live state itself (`src/v2/fallback.ts`), for the sessions its
  client has shown: the routed session, its family, and their
  subagents, from the first time the client shows them until it closes.
  It runs the same reducer over the events the client receives, fills a
  session in from the client's stores (its turn's status and pending
  permissions and forms) when it joins, and asks the service for its
  plugin list (through opencode's client) when it loads and every
  minute, staying quiet while the list shows this plugin's server
  entrypoint active, or has not answered.

Only ids, paths, state names, counts, and times leave the plugin: never a
title, a message, a permission's patterns or resources, a question, a
form's fields, or an error's text.

## Install

The plugin is not published to npm yet. Build it from a local checkout:

```sh
cd plugins/opencode-hook
npm install
npm run build
```

Install the `conspectus` binary from the same checkout: an older binary
treats every live-state snapshot as an attribution record.

### opencode 1.x

```sh
opencode plugin "$(pwd)"
```

This tells opencode to load the plugin from your local filesystem and
updates an opencode config file accordingly. Scope depends on where you
run it: invoking it from inside a directory that already contains
`.opencode/` (or any opencode project root) writes a local-scope entry to
`<project>/.opencode/opencode.json`; otherwise it writes a user-scope
entry to `~/.config/opencode/config.json`. The install command prints the
resolved scope and path.

### opencode 2.x

List the built `dist/v2` directory as a server plugin in
`~/.config/opencode/opencode.jsonc` (or `$XDG_CONFIG_HOME/opencode/opencode.jsonc`).
opencode loads its server entrypoint in the service and its TUI entrypoint
in every client:

```jsonc
{
  "plugins": [{ "package": "/path/to/conspectus/plugins/opencode-hook/dist/v2" }]
}
```

`opencode plugin add` accepts only npm and Git specifiers, so edit the
file directly. Then restart the service as well as your clients: the
service (`opencode serve --service`) outlives the client that started it
and loads server plugins when it starts, so stop that process and the
next client starts it again. `opencode plugin list` shows the entry once
it resolves.

Listing `dist/v2` in `~/.config/opencode/cli.json` instead, as plugin
0.2.0 asked, loads only the TUI entrypoint. Conspectus learns which
session each pane shows, and, from plugin 0.4.0, what the sessions each
client has shown are doing; sessions no client has shown (a background
subagent of a session nobody opened, a turn that goes on after its last
client closed) are missed, and a session reads `exited` once every
client that showed it has closed. `opencode.jsonc` sees every session.
When you move the entry to `opencode.jsonc`, remove it from `cli.json`,
or each client loads the TUI entrypoint twice.

### Upgrading from 0.2.0

Pull, rebuild (`npm install && npm run build`), install the matching
`conspectus`, and restart opencode. On 2.x, also move the `dist/v2` entry
from `cli.json` to `opencode.jsonc` as above.

### Both majors

Make sure `conspectus` is on `PATH`, or set `CONSPECTUS_HOOK_BIN` to its
absolute path before starting opencode. If the binary is missing the
plugin silently no-ops (with a one-shot warning on stderr) and does not
destabilize the opencode session.

To uninstall, remove the plugin entry from the config file it was added
to and run `npm uninstall`.

## Opt out

On opencode 1.x, pass `--pure` to disable all external plugins, including
this one, for a single invocation:

```sh
opencode --pure
```

opencode 2.x has no `--pure`; remove the `opencode.jsonc` (or `cli.json`)
entry instead.

## Verify

After starting an opencode session and sending a prompt, list the live
state Conspectus has for it:

```sh
conspectus table sessions --columns +live
```

and check the attribution records:

```sh
conspectus graph --format json \
  | jq '[.candidate_links[] | select(.source_metadata.adapter == "hook_sidecar" and .source.harness_key == "opencode")] | length'
```

If either is empty after a few seconds of activity, check stderr for the
plugin's one-shot spawn warning and confirm that `conspectus` is on
`PATH` from inside the opencode process environment. On opencode 2.x,
also confirm that `opencode plugin list` shows the `dist/v2` entry; an
entry only in `cli.json` reports only the sessions a client shows.

## Development

```sh
npm run typecheck   # needs npm install
npm test            # Node 22.6 or later; no install needed
```

`npm test` (or `just opencode-plugin-test` from the repository root)
replays the event scripts in
[`tests/fixtures/hooks/opencode-live/`](../../tests/fixtures/hooks/opencode-live/)
through `src/live.ts` and compares every payload it emits.
`test/entrypoints.test.ts` loads the 2.x entrypoints themselves against
stand-ins for opencode's plugin context and for `conspectus`; it needs
Node 22.15 or later and is skipped on an older one. The Rust tests
feed the same payloads through `conspectus hook write opencode`'s parser
and the live-state store. CI runs neither the build nor these tests, so
run both after a change here.

## Implementation notes

- `src/live.ts` is the reducer. It imports nothing, does nothing when
  loaded, and uses only TypeScript that Node can strip, so its tests run
  without a build; so is `src/v2/fallback.ts`, which imports only types.
  The entrypoints (`src/index.ts`, `src/v2/server.ts`, and
  `src/v2/tui.ts` through the fallback) hand it events; `src/write.ts`
  sends what it returns, keeps the heartbeat timer, and spawns the writer
  detached and unawaited, so a write never holds opencode up or keeps it
  running.
- Payload: flat JSON over stdin, `{session_id, cwd?, hook_event_name?,
  attribution?, live?}`, where `live` is `{instance, seq, observed_ms,
  heartbeat_s, pid, pid_start, state, detail, subagents, via_subagent,
  since_ms}`. `instance` is fresh each time the plugin loads and `seq`
  counts up within it, which orders snapshots that land within the same
  second. A payload without `live` is recorded for attribution only, as
  0.2.0 sent it; one with `live` is recorded for attribution as well only
  when `attribution` is true.
- The Rust writer (`HarnessAdapter::live_snapshot_from_payload` and
  `is_attribution_event`, in `src/discovery/harness/opencode_live.rs`) is
  the stable interface boundary; if opencode renames events, this plugin
  updates, but the Rust side does not.
- The 2.x entrypoints have no runtime dependencies. They are typed
  against `@opencode/plugin` but import nothing from it at run time.
- The plugin does *not* read message, text, or tool events, which carry
  payload and would cost a write per token.
