# Hook traces recorded from real sessions

Every file here was recorded on 2026-11-16 from a real session of Claude
Code 2.1.322, Codex 0.167.0, or Pi 1.5.2, driven in a private tmux server
with a scratch configuration directory per harness, a scratch state
directory (so the operator's own live-state store was never touched), and
`CONSPECTUS_HOOK_TRACE` set (ADR 0242). OpenCode was not recorded. The
prompts were a few words each; the model was the operator's.

A file is a hook trace as `conspectus hook write` wrote it, one JSON line
per call: `{v, t_ms, harness, payload, env, events, applied, after}`.
`payload` is redacted at the writer (every free-text string is a
`<str:N>` placeholder, every id is `id:` and eight hex digits salted by
the trace's name, every path is `<path>`; only enumerated words and tool
names survive), so these files hold no prompt, output, path, or id of the
operator's. `events` and `after` are what the adapter and the reducer made
of the call when it was recorded. They are the reducers' own output at
that commit, so a replay compares the adapters with themselves; what
shows they were right is the reading of each recording below, made
against the screen and `claude agents --json` at the same moments.

The sequences are not scripts: a few steps of the Claude Code runs
drifted (a prompt typed into an open dialog, an Esc that arrived before
the dialog) and the trace shows what Claude Code did anyway. The files
that matter for a claim are named for it.

## Claude Code

| File | What it holds |
| :- | :- |
| `turn-idle-prompt-deny-approve-esc-exit.jsonl` | A turn; `Notification idle_prompt` 60 s after its `Stop`; a permission prompt denied (no hook follows, the state stays `waiting/permission` for 41 s); a long approved tool (`PermissionRequest`, then nothing until `PostToolUse`); an Esc interrupt (no `Stop`, no `idle_prompt` in the 77 s that follow); `SessionEnd` `prompt_input_exit` |
| `question-subagent-compact-clear.jsonl` | `AskUserQuestion` (`PreToolUse` then `PermissionRequest`, both `waiting/question`); a subagent; `/compact` (`PreCompact`, `SessionStart` `compact`, `PostCompact`); `/clear` (`SessionEnd` `clear`, then `SessionStart` `clear` with a new session id) |
| `plan-approve-question.jsonl` | `ExitPlanMode` (`PreToolUse` then `PermissionRequest`, `waiting/plan`), approved (`PostToolUse` with the mode now `auto`); a clean question |
| `approve-long-tool-and-subagents.jsonl` | The `Agent` tool returns at once with `async_launched`; `Stop` fires while the subagent runs, with `background_tasks`; the child's `PostToolUse` carries `agent_id`; the report arrives as a turn of Claude Code's own; two parallel subagents; a background shell that wakes its subagent again under the same `agent_id` |
| `plan-reject-and-permission-prompt-notification.jsonl` | A plan rejected with Esc (no hook); `Notification permission_prompt` 6 s after a wait begins, once, for a question and for a plan (a shell permission dialog was not left open long enough to see it) |
| `subagent-permission-and-agent-list.jsonl` | A subagent's `PermissionRequest` while `claude agents --json` lists `waiting` / `permission prompt` |
| `closed-pane-session-end-other.jsonl` | A closed pane (SIGHUP): `SessionEnd` with reason `other` within seconds |
| `kill9-no-session-end.jsonl` | `kill -9`: no `SessionEnd` |
| `stopfailure-*.jsonl` | `StopFailure` against a local stand-in for the model gateway answering 401 (`authentication_failed`), 429 (`rate_limit`), 500 and 529 (`server_error`), and 400 (`unknown`), with retries off |

## Codex

| File | What it holds |
| :- | :- |
| `daemon-approval-interrupt-clear-compact-plan-question-subagents-kill.jsonl` | One trace for several sessions, because the hooks ran in a shared app server whose environment named the first session's trace and pane: an approval granted (nothing between `PermissionRequest` and `PostToolUse`) and declined (`Interrupt`); an Esc interrupt; `/clear` (`SessionStart` `clear`; the old session's `SessionEnd` about 56 s later); `/compact` (`PreCompact`, `PostCompact`, and `SessionStart` `compact` with the next turn); plan mode (`permission_mode` stays `default`); `request_user_input` (no hook while open, `PostToolUse` when answered); a subagent, and one whose command needed approval (`PermissionRequest` with its `agent_id`); a terminal UI killed with SIGHUP and with `kill -9` (`SessionEnd` about 60 s later, from the app server) |
| `no-daemon-kill9.jsonl` | `codex --no-daemon`: the hooks run in the terminal UI, which `kill -9` leaves with no `SessionEnd` |

## Pi

| File | What it holds |
| :- | :- |
| `tool-abort-compact-failed-new-reload.jsonl` | A turn; a tool run (`working/generating`, `working/tool`, `working/generating`, `ready/finished`); an Esc abort (`ready/interrupted`); a manual compaction of a session too small to compact (`session_compact_failed`, which read `error/failed` then and reads as before the compaction now); `/new` (`session_shutdown`, then `session_start:new` for a new id); `/reload` (`session_start:reload`, no shutdown) |
| `quit-lost.jsonl`, `closed-pane-lost.jsonl` | A quit and a closed pane before the fix of ADR 0243: a second writer call whose stdin was empty (`<unparsed:0 bytes>`), so no `session_shutdown` landed |
| `kill9.jsonl` | `kill -9`: no writer call at all |
| `quit-after-fix.jsonl`, `closed-pane-after-fix.jsonl` | The same two after the fix: `live:session_shutdown`, `exited/ended` |

`claude agents --json` was recorded beside these, and its three background
listings are in `tests/fixtures/claude-agents/` (`recorded-*.json`).
