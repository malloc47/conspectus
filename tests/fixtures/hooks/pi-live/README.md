# Pi live-state event scripts and payloads (synthetic)

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
