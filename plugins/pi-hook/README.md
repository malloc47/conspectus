# Pi hook extension

A [Pi](https://pi.dev) extension that tells
[Conspectus](https://github.com/malloc47/conspectus) which session a Pi
process runs and what it is doing: working, waiting for you, ready,
compacting, or failed, and how many subagents it runs. It reports through
`conspectus hook write pi`. Pi doesn't keep its session file open and
reports its own state only to the terminal, which tmux does not pass on,
so without the extension Conspectus can only guess from the command line
and cwd. [ADR 0121](../../docs/adr/0121-pi-harness-support.md) records
the design, and
[ADR 0206](../../docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md)
what each event means.

## What it does

Pi waits for every extension handler before it goes on, so the extension
keeps the session's state itself and starts a writer only when what the
session shows changes, plus once a minute while it is not ready. It
handles `session_start`, `session_shutdown`, `agent_start`, `agent_end`,
`agent_settled`, `message_end` (the assistant's stop reason only),
`tool_execution_start` and `_end` (calls a tool makes itself are left
out), `ui_prompt_start` and `_end`, `session_before_compact`,
`session_compact`, `session_compact_failed`, and
`after_provider_response` (only its status). It never subscribes to an
event that streams (`message_update`, `tool_execution_update`) or one
that can change what Pi does.

What the session shows, first match wins, as Pi's own status reporter
orders it:

| State | When |
|---|---|
| `waiting` (`permission`) | an extension's `confirm` dialog is open |
| `waiting` (`question`) | an extension's `select`, `input`, or `editor` dialog is open (a plan approval or a gate that asks with a select reads as a question) |
| `compacting` | between `session_before_compact` and its end |
| `working` (`retrying`, `rate_limited`) | a failed attempt's backoff; rate limited when the provider answered 429 or the error says so |
| `working` (`tool`) | a top-level tool call runs; a call of the `subagent` tool counts as a subagent |
| `working` (`generating`) | otherwise, while a run is active |
| `error` (`rate_limited`, `auth`, `overloaded`, `failed`) | the run settled after an error, or a compaction failed |
| `ready` (`finished`, `interrupted`) | the run settled; interrupted when you pressed Esc |
| `ready` | a session that has not run yet, or one a `/reload` reported afresh |
| `exited` (`ended`) | you quit, or `/new`, `/resume`, or `/fork` left the session |

A change to `waiting`, `ready`, `error`, `exited`, or `compacting` is
sent at once. A change between working sub-states (generating, a tool,
retrying) is held for 500 ms and the latest is sent, so a burst of tool
calls costs one writer. Each `session_start` (reasons `startup`,
`reload`, `new`, `resume`, and `fork`) in the interactive TUI also names
the session for pane attribution, as the first version of the extension
did. Every payload is flat:

```json
{
  "session_id": "<id>",
  "cwd": "<session cwd>",
  "transcript_path": "<session file>",
  "hook_event_name": "live:ui_prompt_start",
  "live": {
    "instance": "<random per load>", "seq": 12, "observed_ms": 1794400904859,
    "heartbeat_s": 60, "pid": 48213, "pid_start": 771234567,
    "state": "waiting", "detail": "permission", "subagents": 0,
    "since_ms": 1794400900000
  }
}
```

A session start is named `session_start:<reason>`. `pid` is the Pi
process, which Conspectus uses instead of searching for it. Only ids,
paths, state names, counts, and times leave the extension: an error
message is classified inside it and dropped, and a tool's name is read
only to tell a subagent call. Prompts, tool arguments and results, dialog
titles, and message text are never read.

It writes nothing for `--no-session` runs, which have no session file,
or in Pi's print, JSON, and RPC modes, which don't run in a pane.

## Install

`conspectus` carries this file and installs it:

```sh
conspectus hook init pi                   # ~/.pi/agent/extensions/conspectus-hook.ts
conspectus hook init pi --scope project   # ./.pi/extensions/conspectus-hook.ts
```

The user scope follows `PI_CODING_AGENT_DIR` when it is set. Pi asks you
to trust a project before it loads project extensions. Restart Pi, or
run `/reload`, to load the extension. To upgrade an older copy, run the
same command again, then restart Pi or run `/reload`. Install the
extension with the `conspectus` that runs it: an older `conspectus`
treats every live-state report as an attribution record.

`conspectus hook status pi` reports `installed`, `outdated` (an older
copy; it says which version and what it lacks), `not-installed`, or
`foreign` (a file at that path that Conspectus didn't write, which
`hook init` and `hook remove` leave alone).

No build step or package is needed. Pi loads TypeScript extensions
directly.

## Pi versions

Waiting needs Pi 0.84.4 or later, which fires the dialog events; an older
Pi never reads as waiting and reports the other states. Pi 1.1.0 or
later is recommended: it says when a run was interrupted (older versions
fall back to the last stop reason), and from 0.84.3 a failed compaction
ends compacting. A run ends only on `agent_settled` (Pi 0.80.4).

## Known gaps

- Pi has no approval prompt of its own, so `waiting` means an
  extension's dialog is open. A provider login and Pi's own pickers
  (`/model`, `/resume`, settings) are not visible to extensions, and the
  project trust prompt comes before the session starts; none of them
  reads as waiting.
- `ctx.ui.custom()` dialogs never read as waiting: Pi reports every one,
  loaders and widgets included, without saying which wait for you.
- A killed Pi (SIGKILL, a crash) sends nothing. Conspectus finds its pid
  gone and shows the session `exited`; until then the heartbeat stops and
  a working session turns quiet after three minutes.
- After `/reload` the extension reports the session afresh, so a
  finished turn reads `ready` without saying it finished.

## Binary lookup

The extension runs `conspectus` from `PATH`. Set `CONSPECTUS_HOOK_BIN` to
an absolute path before starting Pi to use a different binary. If the
binary is missing or the write fails, the extension does nothing and Pi
carries on. Each writer runs in its own process group, so a report sent
as Pi quits still lands.

## Tests

`just pi-plugin-test` (Node 22.6 or later) replays the event scripts in
`tests/fixtures/hooks/pi-live/` through the extension's state machine and
runs its handlers against a stand-in Pi and a stand-in writer. The Rust
tests feed the same scripts' payloads through Conspectus's parser and
store.

## Opt out

```sh
conspectus hook remove pi                   # or --scope project
```

This deletes only the file Conspectus installed. Other extensions in the
directory are untouched. Pi's own extension controls can also disable it
for a run.
