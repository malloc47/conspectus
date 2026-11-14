# opencode live-state event scripts (synthetic)

Every file here is **synthetic**. None was recorded from a real opencode
session: each was built by hand from the event shapes the live-state
research recorded on 2026-11-11 (`docs/plans/live-state/research/03-opencode.md`,
read against opencode 1.18.32, 2.0.32, and 2.2.7). Every field that would
carry conversation, tool, or error text holds a `SENTINEL-opencode-...`
placeholder, so a test can tell at once whether any of it got through.
Treat them as the documented contract, not as observed behavior, until
they are re-recorded from real sessions with a redacting recorder.

The scripts are shared by two test suites:

- `plugins/opencode-hook/test/live.test.ts` (Node 22.6 or later: `just
  opencode-plugin-test`) replays each script through the plugin's
  reducer, `plugins/opencode-hook/src/live.ts`, and compares what it
  emits after every step.
- The Rust tests of the opencode adapter feed every emitted payload
  through `conspectus hook write opencode`'s parser and the live-state
  store, and check what each session reads as.

## Layout

`v1-<scenario>.jsonl` drives the opencode 1.x plugin, `v2-<scenario>.jsonl`
the 2.x server entrypoint. The first line is a header:

```json
{"fixture": "opencode live event script", "synthetic": true, "major": 1,
 "scenario": "...", "instance": "...", "pid": 52817, "pid_start": 918273645,
 "directory": "/home/user/projects/my-project"}
```

`instance`, `pid`, `pid_start`, and `directory` are the plugin instance's
options. Every later line is a step:

- `at_ms`: when it happens, Unix epoch milliseconds (small numbers, for
  reading).
- `note`: why the step is there, when it is not obvious.
- One input, or none for a step that only checks: `event` (a 1.x
  `{type, properties}` event, or a 2.x `{id, created, type, data,
  location, durable}` envelope as the server's event stream yields it),
  `compacting` (the 1.x `experimental.session.compacting` hook, with its
  session id), `heartbeat: true` (the 60 s timer), `dispose: true` (the
  instance going away), or `seed` (what opencode lists of sessions, by
  session id: `{"permissions": [...], "forms": [...], "status": ...}`,
  as the 2.x server entrypoint reads the pending permissions of its
  location's sessions when it starts).
- `emit`: the payloads the reducer sends after the input, in order, each
  exactly as `conspectus hook write opencode` reads it on stdin.
- `shown`: what a session reads as after the step, by session id:
  `state`, and when they matter `detail`, `since_ms`, `subagents`,
  `via_subagent`, a reader's `confidence` (`fresh`, `quiet`, `stale`), and
  the process's `liveness` (`alive`, `dead`). Confidence and liveness are
  checked by the Rust tests only.

`emit` is the reducer's output and can be regenerated after an intended
change with `OPENCODE_LIVE_UPDATE=1 npm test` in `plugins/opencode-hook`;
review the diff. `shown` is written by hand and is the check on it.

## What the scripts assume

Where the research could not confirm a shape, the scripts encode the
assumption the reducer makes. These are the claims a recording has to
confirm:

- 1.x `permission.asked`, `permission.replied`, and `question.*` reach the
  plugin's `event` hook; `session.error` carries the error as `{name,
  data}` with an API error's `data.statusCode`; a failed compaction
  publishes nothing; `dispose` runs when the TUI quits.
- 2.x `ctx.event.subscribe` yields `{type, data, location}` with
  `location.directory`, and each server plugin instance should keep only
  its own location's events; step, compaction, and execution events carry
  `data.sessionID`; `form.created` carries `data.form`, and `form.replied`
  and `form.cancelled` carry `data.id`.
- A 2.x retry is rate limited when its error has status 429 or a
  `retryAfterMs`; a failed execution's class comes from its error's status,
  retry delay, or type word.
- The server plugin context's `session.list({directory, limit})` returns
  pages `{data: [{id}], cursor: {next}}`, and `permission.list({sessionID})`
  the session's pending permission requests, each `{id, sessionID, ...}`,
  as the plugin package's types and documentation describe them.
- `session.execution.interrupted` with `superseded` or `inactivity` reads
  as an interrupted turn, and with `shutdown` as the end of the session in
  this process.
