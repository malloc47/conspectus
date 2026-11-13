# Codex hook payloads (synthetic)

Every file here is **synthetic**. None was recorded from a real Codex
session: each was built by hand from the payload shapes the Codex hooks
guide and its generated input schemas document, as the live-state
research recorded them on 2026-11-12 (the latest release then was
0.167.2). Every field that would carry
conversation or tool content holds a `SENTINEL-codex-...` placeholder, so
a test can tell at once whether any of it reached a file or an output.
Treat them as the documented contract, not as observed behavior, until
they are re-recorded from real sessions with a redacting recorder.

- `<event>[-<variant>].json`: one hook payload as Codex writes it to the
  hook's stdin. `pretooluse-request-user-input.json`,
  `premodelrequest.json`, `postmodelrequest-failed.json`, and
  `unknown-event.json` are events `hook init codex` does not install;
  they map to nothing.
- `seq-<scenario>.jsonl`: an ordered sequence, laid out as the Claude Code
  corpus's README describes (a header with `"synthetic": true` and the
  writer's default `pid` and `pid_start`, then steps with `at_ms`, an
  optional `payload`, `pid`, `pid_start`, `liveness`, `expect`, `also`,
  and `note`).
- `install/`: `config.toml` cases for the installer, with a README of
  their own.

Sessions, turn ids, and agent ids are placeholders in the UUIDv7 form
Codex uses; paths are placeholders too. Where the documentation is silent
the sequences encode the assumption the mapping makes, and those are the
claims a recording has to confirm:

- an approval, granted or declined, fires nothing until the next tool,
  `Stop`, or `Interrupt`;
- a subagent's `PostToolUse` carries its `agent_id`
  (`posttooluse-subagent-bash.json`), as its `PermissionRequest` does;
- `SubagentStop` fires when the child is done, and repeated starts and
  stops of one id count once;
- `/clear` starts the new session with `SessionStart` (`clear`) and fires
  no `SessionEnd` for the old one, whose entry the process's new session
  ends.
