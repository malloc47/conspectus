# Codex hook payloads (synthetic, checked against recordings)

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

## Checked against real sessions (2026-11-16, Codex 0.167.0)

Real traces are in `../recorded/codex/`. The four claims above, and what
else the recordings settled:

- **An approval fires nothing until the next tool, `Stop`, or `Interrupt`:
  confirmed.** Granting one fires `PostToolUse` when the tool ends, with
  nothing between (the pane title shows the spinner meanwhile, which is
  how ADR 0217 clears the wait); declining one fires `Interrupt` at once.
  An Esc mid-generation fires `Interrupt` too.
- **A subagent's `PostToolUse` carries its `agent_id`: confirmed,** and so
  does its `PermissionRequest`.
- **`SubagentStop` fires once per child:** confirmed for one child at a
  time; two at once were not tried (Claude Code's two were two stops).
- **`/clear` fires `SessionStart` (`clear`) and no `SessionEnd` at that
  moment: confirmed,** but the old session's `SessionEnd` (reason `other`)
  arrives about 56 s later, when the app server unloads its thread.
- A plan-mode `Stop` carries `permission_mode` `default`, not `plan`
  (`stop-plan-mode.json` and `seq-plan-mode-stop.jsonl` keep the `plan`
  form, which the mapping ignores either way). The plan-approval choice
  shows in no hook and in no title.
- `SessionStart` comes with the first prompt, not at launch; a compaction's
  `SessionStart` (`compact`) comes with the next turn, after `PreCompact`
  and `PostCompact`.
- A question (`request_user_input`) fires no installed hook while it is
  open and `PostToolUse` when it is answered; the title reads `[ . ]
  Action Required` meanwhile.
- **Hooks run in the shared app server** (`codex app-server
  --managed-daemon`) unless `--no-daemon` is given, in its environment
  and with its pid (ADR 0243). Its `SessionEnd` for a session whose
  terminal UI is gone comes about 60 s later.
- Hooks stay silent until trusted: with the startup review declined
  (`Continue without trusting`) no hook ran at all. Trust is recorded at
  `hooks.state."<config path>:<event>:<group>:<hook>"`, keyed to the
  config file's path and to each handler's place in its event's list, so
  removing an earlier group asks for the review again.
- `SessionEnd` and `Interrupt` hooks are clamped to 3 s, with a warning on
  every start.
