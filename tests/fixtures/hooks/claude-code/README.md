# Claude Code hook payloads (synthetic, confirmed by recordings)

Every file here is **synthetic**. None was recorded from a real Claude Code
session: each was built by hand from the payload shapes the Claude Code
hooks reference documents (read for Claude Code 2.1.322), with placeholder
text in every field that would carry conversation or tool content. Treat
them as the documented contract, not as observed behavior, until they are
re-recorded from real sessions with a redacting recorder that replaces
every non-enum string with a placeholder.

- `<event>[-<variant>].json`: one hook payload as Claude Code writes it to
  the hook's stdin. `payload-large-tool-response.json` is small on disk;
  the test that reads it inflates `tool_response.file.content` to many
  megabytes.
- `seq-<scenario>.jsonl`: an ordered sequence. The first line is a header
  (`"synthetic": true`, the scenario, and the writer's default `pid` and
  `pid_start`). Each later line is a step:
  - `at_ms`: when the writer received the payload, Unix epoch
    milliseconds; a reader looks at the state at the same time.
  - `payload`: the hook payload. A step without one only checks the state
    at `at_ms`, for time passing or a killed process.
  - `pid`, `pid_start`: the writer's process when it differs from the
    header's.
  - `liveness`: what the reader found about the process (`alive`, the
    default, `dead`, or `unknown`).
  - `expect`: the effective state of the payload's session (or, for a
    check, the last payload's): `state`, `detail` (absent means none),
    `since_ms` when given, `subagents` (default 0), `via_subagent`
    (default false), and `confidence` (default `fresh`).
  - `also`: the same for other sessions, each naming its `session_id`
    and optionally its own `liveness`.
  - `note`: what the step stands for, when the payload does not say.

Sessions, agent ids, tool-use ids, and paths are placeholders in the
formats the reference shows. The sequences encode the behavior the
reference documents and, where it is silent, the assumption the mapping
makes (for example that a denied prompt fires nothing); those are the
claims a recording has to confirm.

## Checked against real sessions (2026-11-16, Claude Code 2.1.322)

Real traces are in `../recorded/claude-code/` (see its README). Every
claim above that a recording could test held, and no sequence needed to
change:

- a denied prompt fires nothing (`seq-permission-deny-esc.jsonl`), and so
  does a rejected plan (`seq-plan-reject.jsonl`);
- an Esc interrupt fires no `Stop`, no `PostToolUseFailure`, and no
  `idle_prompt` in the 77 s after it
  (`seq-esc-interrupt.jsonl`), and `claude agents --json` lists the
  session `idle` for each of these, which is what the repair reads;
- an approval fires nothing until the tool's `PostToolUse`
  (`seq-permission-approve-long-tool.jsonl`);
- `PermissionRequest` does fire for `AskUserQuestion` and `ExitPlanMode`,
  each after its own `PreToolUse` (`seq-askuserquestion.jsonl`,
  `seq-plan-approve.jsonl`);
- a subagent outlives the turn's `Stop`, its tools carry `agent_id`, and
  its report arrives as a turn of its own
  (`seq-background-subagent-outlives-stop.jsonl`);
- `/clear` ends the session and starts a new one with a new id
  (`seq-clear.jsonl`), a `kill -9` fires no `SessionEnd`
  (`seq-kill9-no-sessionend.jsonl`).

What the recordings added, none of it changing the mapping:

- `SessionStart` (`startup`) comes at launch, before any prompt.
- Compaction runs `PreCompact`, `SessionStart` (`compact`), `PostCompact`,
  in that order, not the order the research guessed.
- A closed pane (SIGHUP) fires `SessionEnd` with reason `other` within
  seconds.
- `Notification permission_prompt` fires once, 6 s after a wait begins,
  for a question and a plan as well as a prompt; `idle_prompt` comes 60 s
  after a `Stop`.
- The `Agent` tool returns at once (`tool_response.status` is
  `async_launched`) and the subagent's run is told by `SubagentStart`,
  `SubagentStop`, and `Stop`'s `background_tasks`; a finished background
  shell wakes its subagent again under the same `agent_id`.
- `StopFailure` `error` is `authentication_failed`, `rate_limit`,
  `server_error` (for a 500 and a 529 with retries off), or `unknown`; its
  payload also carries `prompt_id`, `effort`, and `last_assistant_message`.
  `overloaded`, which `stopfailure-overloaded.json` and the mapping keep,
  was not seen.
- The pane title carries no state: it reads `✳ Claude Code`, then the
  session's name.
