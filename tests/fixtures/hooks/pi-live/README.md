# Pi live-state event scripts and payloads (synthetic, checked against recordings)

Every file here is **synthetic**. None was recorded from a real Pi
session: each was built by hand from the extension event shapes the
live-state research recorded on 2026-11-11
(`docs/plans/live-state/research/04-pi.md`, read against
`@earendil-works/pi-coding-agent` 1.5.2). Every field that would carry
conversation, tool, dialog, or error text holds a `SENTINEL-pi-...`
placeholder, and one tool name does too, so a test can tell at once
whether any of it got through. Treat them as the documented contract, not
as observed behavior, until they are re-recorded from real sessions with
a redacting recorder.

The scripts are shared by two test suites:

- `plugins/pi-hook/test/conspectus-hook.test.ts` (Node 22.6 or later:
  `just pi-plugin-test`) replays each script through the extension's state
  machine in `plugins/pi-hook/conspectus-hook.ts` and compares what it
  emits after every step.
- The Rust tests of the Pi adapter feed every emitted payload through
  `conspectus hook write pi`'s parser and the live-state store, and check
  what each session reads as.

## Event scripts

`<scenario>.jsonl`. The first line is a header:

```json
{"fixture": "pi live event script", "synthetic": true, "scenario": "...",
 "instance": "...", "pid": 48213, "pid_start": 771234567}
```

`instance`, `pid`, and `pid_start` are the extension load's options, and
`subagent_tools`, when present, is the value of
`CONSPECTUS_PI_SUBAGENT_TOOLS` the load reads (the extra tool names whose
calls count as subagents).
Every later line is a step:

- `at_ms`: when it happens, Unix epoch milliseconds (small numbers, for
  reading).
- `note`: why the step is there, when it is not obvious.
- `load`: the extension was loaded again (a `/reload`) as `{"instance":
  ...}`, before the step's input.
- One input, or none for a step that only checks:
  - `on` and `event`: a Pi extension event by its handler name, with the
    event object Pi passes; for `session_start` also `ctx`, what the
    extension reads from Pi's context (`mode`, `idle` from
    `ctx.isIdle()`, `session_id`, `session_file`, and `cwd` from
    `ctx.sessionManager`).
  - `timer`: `flush` (the 500 ms window for a change between working
    sub-states ended) or `heartbeat`, at the time the previous step said
    it was due.
- `emit`: the payloads the extension sends after the input, in order,
  each exactly as `conspectus hook write pi` reads it on stdin.
- `flush_at`, `heartbeat_at`: when the extension asks to be called back
  after the input; absent when nothing is due.
- `shown`: what a session reads as after the step, by session id:
  `state`, and when they matter `detail`, `since_ms`, `subagents`, a
  reader's `confidence` (`fresh`, `quiet`, `stale`), and the process's
  `liveness` (`alive`, `dead`, `unknown`). Confidence and liveness are
  checked by the Rust tests only.

`emit`, `flush_at`, and `heartbeat_at` are the state machine's output and
can be regenerated after an intended change with `PI_LIVE_UPDATE=1 just
pi-plugin-test`; review the diff. `shown` is written by hand and is the
check on them.

## Payloads

`payload-<case>.json` are single payloads for the Rust parser: what the
first extension version sent (`legacy-session-start`), a newer
extension's state and detail this version does not know
(`unknown-state`, `unknown-detail`), and the payload the research
sketched, which keeps the instance and sequence number outside `live`
(`research-shape`) and so is no snapshot.

## What the scripts assume

Where the research could not confirm a shape, the scripts encode the
assumption the extension makes. These are the claims a recording has to
confirm:

- Every event object carries the fields the research lists for it:
  `toolCallId`, `toolName`, and `parentToolCallId` on tool events;
  `message.role`, `message.stopReason`, and `message.errorMessage` on
  `message_end`; `kind` on the dialog events; `reason`, `willRetry`, and
  `aborted` on the compaction events; `aborted` on `agent_settled` from
  Pi 1.1.0; `status` on `after_provider_response`.
- A gate extension's dialog opens between the tool's
  `tool_execution_start` and its end; the scripts put it there, the case
  where waiting must outrank working.
- `/reload` keeps the session and loads the extension again, so the new
  load's `session_start` names the same session; `/new`, `/resume`, and
  `/fork` fire `session_shutdown` before the next `session_start`.
- Retries fire `agent_end` after the failed attempt and `agent_start`
  again for the next one, inside one run that `agent_settled` ends; a
  threshold compaction runs before the run settles.

## Checked against real sessions (2026-11-16, Pi 1.5.2)

Real traces are in `../recorded/pi/`. Pi loaded the extension and ignored
its one extra export, and the events it handled had the shapes the scripts
assume: a turn, a tool run, an abort (`ready/interrupted`), `/new` (a
`session_shutdown`, then a `session_start:new` with a new id), and
`/reload` (`session_start:reload`, the same session, no shutdown). The
attribution payload carries Pi's own pid and start time, and `ctx.isIdle()`
is true at `session_start`, so a launch reads `ready`. Two things were
not as written:

- **The session's last report never arrived.** Pi is a Bun executable,
  which writes a child's piped stdin on its event loop, so a quit or a
  closed pane (where Pi exits at once) gave the writer an empty stdin
  (`quit-lost.jsonl`, `closed-pane-lost.jsonl`). The extension now runs
  the writer to completion for that one report (ADR 0243), and the same
  two recordings after the fix show `exited/ended`. A `kill -9` sends
  nothing, as `kill-no-shutdown.jsonl` says.
- **A failed compaction of a small session reads as an error.** A manual
  `/compact` of a session too small to compact fires
  `session_compact_failed` ("Nothing to compact"), and the state stays
  `error/failed` until the next event. `compaction-failed.jsonl` is
  right about the mapping and the recording shows the mapping is wider
  than a real failure.

Not exercised: a permission gate's dialog and the `subagent` tool (no
extension that gates or spawns was installed), `after_provider_response`
and a 429, a threshold or overflow compaction, and whether Pi exposes its
version to an extension (backlog CSP-851).
