# Claude Code hook payloads (synthetic)

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
