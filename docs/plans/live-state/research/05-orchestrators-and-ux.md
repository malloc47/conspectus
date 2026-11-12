# Research 05: How other orchestrators detect and present live agent state

Research notes for the live-state feature (show whether each active
agent session or mux is waiting on the operator, ready for a new
prompt, being worked on by an agent or its subagents, or in error,
rate-limited, compacting, starting, or exited). This file covers prior
art only. Per-harness signals, the graph representation, and the TUI
design are other researchers' work. Everything here is evidence for
those decisions, not a decision.

Method: I read pages with WebFetch on 2026-11-11/12 (GitHub READMEs,
raw source files, issue lists, vendor docs). A small model summarized
each page, so short quotes are close to verbatim but not guaranteed
byte-exact. I treated fetched content as untrusted data. Every external
claim cites a URL. **UNVERIFIED** marks a claim I could not confirm from
a fetched page. Issue titles come from filtered issue-list pages, so
only titles were read unless the issue is linked on its own.

## Contents

1. Tools surveyed: summary table
2. Per-tool notes (detection, taxonomy, presentation, lessons)
3. Union state taxonomy
4. Detection approaches compared
5. Recurring presentation patterns as candidates for Conspectus
6. Constraints from Conspectus's own design that shape the choice
7. Open design questions for the operator

---

## 1. Tools surveyed

| # | Tool | Kind | Primary detection | State names (as shipped) | Notable presentation |
|---|---|---|---|---|---|
| 1 | agent-deck | Go TUI over tmux | Hooks first (Claude, Codex, Gemini, pi, Hermes), OpenCode SSE, then tmux pane-title spinner, then pane capture with per-tool patterns, plus transcript evidence | running, waiting, idle, error, stopped (+ substates) | `●`/`◐`/`○`/`✕`, waiting list in tmux status bar, status filters, parent nudges |
| 2 | claude-squad | Go TUI over tmux | Pane capture every 500 ms, SHA-256 change detection, prompt-string match | Running, Ready, Loading, Paused | spinner / green `●` / `⏸` at the row end |
| 3 | ccmanager | Node TUI owning a PTY | Headless terminal emulator plus per-harness regex "strategies", polled every 100 ms | idle, busy, waiting_input, pending_auto_approval | `●`/`◐`/`○`, `[BG:N]`, `[Team:N]`, per-project counts, state filter, status-change hooks |
| 4 | Crystal (now Nimbalyst) | Electron app that spawns the agent | Owns the process: stream-json result messages, an MCP permission-prompt bridge, view timestamps | initializing, ready, running, waiting, stopped, completed_unviewed, error | spinning/pulsing badges, `Bell` "New activity" |
| 5 | Claude Code agent view (`claude agents`) | The harness's own multi-session UI | The harness reports its own state (supervisor process) | Working, Needs input, Idle, Completed, Failed, Stopped; JSON `working`/`blocked`/`done`/`failed`/`stopped` | groups by state with Needs input on top, header counts, tab-title and footer counts, `waiting 3m`, notifications |
| 6 | Claude Code (single session) | Harness | Emits OSC 7501 status and a title glyph; Notification hooks; status line | working / waiting / done (OSC 7501 wording) | terminal tab glyph, desktop notification or bell |
| 7 | Codex CLI | Harness | Emits title spinner, OSC 9 or BEL notifications, `notify` program, lifecycle hooks | n/a (events) | `tui.terminal_title` defaults to spinner and project |
| 8 | workmux | tmux tool | Claude Code hooks (plugin), per-agent hooks | working, waiting, done, stale | emoji in tmux window name, dashboard with Priority sort, `last-done` jump |
| 9 | tmux-agent-status | tmux plugin | Hooks writing status files; optional process detection | working, done, wait, ask, parked | per-agent pulsing glyphs in status-right, INBOX sidebar, fzf switcher sorted by state, unread completion inbox |
| 10 | cmux | Ghostty-based macOS terminal | OSC 9/99/777 sequences and a `cmux notify` CLI for hooks | pending / unread / read | blue ring on pane, lit tab, unread badge, notification panel, jump-to-latest-unread |
| 11 | Herdr | Native agent terminal app + TUI | Process detection, "screen manifests" (remotely updatable), integrations, OSC 7501 | blocked, working, done, idle, unknown | sidebar rollup, Done badge per client, mobile inbox sorted by state |
| 12 | amux | tmux-based agent runner | OSC 7501 "status tap", `amux report`, BEL | working, done, blocked (+kind), error | done/blocked badges, `n` jump to next needing attention, toasts, bell |
| 13 | zj-radar | Zellij sidebar plugin | Push-only: hooks send JSON over `zellij pipe`; never polls | needs you, working, done, error, idle | `◆ ⠋ ● ✗ ○` legend, `attention-next`, background task lines, desktop notifications |
| 14 | zellaude | Zellij tab-bar plugin | Claude Code hooks via `zellij pipe` | starting, thinking, per-tool activity, waiting for prompt, waiting for permission, done, idle | per-activity glyphs, elapsed timer after 30 s, flash on permission, click jumps to waiting pane |
| 15 | zj-agents | Zellij plugin | Screen scraping of the last 40 lines, about once a second, fixture-backed rules | Idle, Working, Blocked, Done | floating sidebar, desktop notifications |
| 16 | VS Code agent sessions | Editor | Owns or discovers sessions (Copilot, Claude, Codex) | in progress, unread; Inbox mode categories | title-bar badges that filter, Inbox mode triage (dismiss, hold, snooze) |
| 17 | Paradev (formerly Superset) | Desktop agent terminal | Status hooks installed per agent | running, finished/waiting, needs attention, unread | chime, dock badge, "Clear Status" for stale running |
| 18 | uzi | Go CLI over tmux | Pane capture, two substrings | running, ready, unknown | `uzi ls -w` table refreshed every second |
| 19 | vibe-kanban | Kanban app that spawns agents | Owns execution processes | running, completed, failed, killed; `has_pending_approval` | kanban board (README now says it is sunsetting) |
| 20 | Others (thin evidence) | opcode, Conductor, hcom, Omnara, LangChain agent-inbox, ccswarm, sesh, Copilot, Cursor | see 2.20 | | |

---

## 2. Per-tool notes

### 2.1 agent-deck (asheshgoplani/agent-deck)

Conspectus already reads agent-deck's workspaces and `state.db`
instance titles (ADR 0060, ADR 0066, `src/discovery/agent_deck.rs`).
agent-deck has the most mature, and the most painfully debugged,
state engine in the survey. Its docs and issues are the best evidence
on accuracy.

**Taxonomy.** Instance statuses are `running` (green), `waiting`
(yellow), `idle` (grey), `error` (red), and `stopped`. Pane-frame
statuses are `active`, `waiting`, `idle`, `starting`, `error`, and
`inactive`. Additive substates are `running`, `idle-at-empty-prompt`,
`interactive-menu`, `background-work`, `watching`, `auth-401`,
`usage-limit`, `model-unavailable`, `unknown-exit`, `hook-lag`, and
`process-exited`. Rate limits come from the transcript, not the pane:
`usage-limit` for a 429 window, `model-limit`, and `account-blocked`.
The docs say nothing about a compacting substate.
<https://github.com/asheshgoplani/agent-deck/blob/main/docs/status-detection.md>
README glyphs: `●` running ("Agent is actively working"), `◐` waiting
("Needs your input"), `○` idle ("Ready for commands"), `✕` error
("Something went wrong").
<https://github.com/asheshgoplani/agent-deck>
The web board adds an operator-set semantic annotation with columns
Needs input, Ready for review, In progress, Parked, Done, and
Untriaged. A set annotation "always wins over runtime state."
<https://github.com/asheshgoplani/agent-deck> (README, second half)

**Detection, in precedence order** (status-detection.md, as above):
1. Dead or missing pane gives `inactive`.
2. A Braille spinner in the pane title gives `active`, with no capture.
3. Capture the visible pane (no scrollback). Claude frames are trimmed.
4. A model-unavailable or error banner gives `error`.
5. An open Claude menu with no live busy cue is not busy.
6. A busy indicator gives `active`: tool patterns over the last 25
   lines, a spinner scan, and a 6 s grace.
7. Claude background work gives `active`, never over a menu or error.
8. A prompt indicator gives `waiting`, or `idle` once the operator
   attached.
9. Otherwise use history: `starting` inside a 2 min startup window,
   else the previous stable status.

A fresh hook verdict short-circuits the pane read. Claude hooks map
`UserPromptSubmit`/`PreToolUse` to running, `Stop`/`PermissionRequest`/
Notification(permission) to waiting, and `SessionEnd` to dead.
`PostToolUse` is deliberately not subscribed.
<https://github.com/asheshgoplani/agent-deck/blob/main/docs/claude-status-hooks.md>
Hook freshness windows: 2 min for Claude, Gemini, pi, and Hermes. For
Codex, 20 s after turn start and 5 s after turn end. OpenCode SSE gets
30 s. A pane-derived flip away from running is held for one sample.
The TUI polls every 2 s, backing off to 10 s, and writes the `status`
column on change. One batched capture runs per tmux socket.
<https://github.com/asheshgoplani/agent-deck/blob/main/docs/status-detection.md>
The hook-to-status mapping lives in one function (`Derive`) because
"The duplicates drifted" across three copies.
<https://github.com/asheshgoplani/agent-deck/blob/main/internal/sessionstatus/sessionstatus.go>
Pane classification is scored against a golden corpus of real captured
frames (`testdata/status_corpus`).
<https://github.com/asheshgoplani/agent-deck/blob/main/internal/tmux/frame_classify.go>
The per-tool patterns are in
<https://github.com/asheshgoplani/agent-deck/blob/main/internal/tmux/detector.go>.
For Claude: "ctrl+c to interrupt" / "esc to interrupt", spinner glyphs
`✳ ✽ ✶ ✢` plus braille, prompt `❯`, and permission text such as "No,
and tell Claude what to do differently". For Codex: a "Working (3s •
esc to interrupt)" status line and the `›` composer. Error banners:
"API Error: 401", "Please run /login".

**Acknowledgement.** "Acknowledged" means the user attached since the
last waiting event. A fresh waiting hook then becomes `idle` for
non-Codex tools.
<https://github.com/asheshgoplani/agent-deck/blob/main/internal/sessionstatus/sessionstatus.go>

**Presentation.** Status filters are search prefixes: `!` running, `@`
waiting, `#` idle, `&` error, `0` clears. Waiting sessions appear in
the tmux status bar as `⚡ [1] frontend [2] api [3] backend`, sorted by
most recently began waiting by default, and `Ctrl+b` then `1`–`6`
jumps to one. A `notify-daemon` nudges a parent session when a child
goes `running -> waiting|error|idle`. Remote rows dim after 30 s with
"· status 47s old", and a failed poll shows a grey `?` "· last known".
<https://github.com/asheshgoplani/agent-deck>,
<https://github.com/asheshgoplani/agent-deck/blob/main/docs/status-detection.md>
I found no per-status count header and no "waiting for 5m" timer in
the README.

**Accuracy problems from the tracker.** The status issue search returns
123 issues. Titles from page 1:
<https://github.com/asheshgoplani/agent-deck/issues?q=is%3Aissue+status+waiting>
- False running from logos that look like spinners: #3037 (Codex logo
  braille row), #2519 (OpenCode logo matches the `█` pulse rule).
- Stale on-screen text read as busy forever: #2948 (a stale pi
  "Working" banner).
- Stuck at running because the hook aged out and the pane wasn't
  re-read: #2896. The 2-minute hook window expired, and `GetStatus`
  skipped the capture because the pane showed no activity change. The
  fix: a terminal event (Stop/waiting/idle/error) holds until the next
  turn starts. <https://github.com/asheshgoplani/agent-deck/issues/2896>
- Flicker of the seen mark: #2950. About 70 false `◐`/`○` flips in 75
  minutes across about 20 sessions, because reloads dropped the
  acknowledged flag and an old Stop event reset it. The fix persisted
  the seen mark: "opening a session moves the visit time, and a new
  running turn clears the mark."
  <https://github.com/asheshgoplani/agent-deck/issues/2950>
- Missed transitions: #2900 (Codex turn ends with no running→waiting),
  #2806 (an idle child that dies is never reported).
- Wrong state after a restart or an intentional stop: #2938 (a stale
  startup-timeout banner in scrollback bounces to error), #2853 (an
  intentional stop reported as running→error), #2669 (a CLI stop
  recorded as error).
- Version drift: "Remote version skew: remotes on 1.16.16 or older …
  still report idle-prompt shells as running". Capture storms: a
  fallback after a batch timeout multiplied tmux calls from 5 to 164
  (#3089). <https://github.com/asheshgoplani/agent-deck/blob/main/docs/status-detection.md>
- Hook gaps the docs state outright: "there is no signal between the
  last PreToolUse and Stop", and a turn that thinks before its first
  tool keeps the previous verdict.
  <https://github.com/asheshgoplani/agent-deck/blob/main/docs/claude-status-hooks.md>

**Lessons.** (1) Hooks plus pane fallback is the design that
converged, after a long tail of fixes: the status search alone returns
123 issues, and the numbers above run past #3000. (2) Terminal
events must hold until contradicted, not expire. (3) The "seen"
concept is valuable, and fragile if it isn't persisted with the state.
(4) Keep one derivation function and test it against a real corpus.
(5) The hook path is cheap. Captures are where the cost and the
fragility live.

### 2.2 claude-squad (smtg-ai/claude-squad)

**Detection.** Every 500 ms, after the previous tick finishes,
`HasUpdated()` runs `tmux capture-pane -p -e -J` and compares a SHA-256
of the content to the last one. If the content changed, the status is
`Running`. If not, and a prompt string is present, it calls `TapEnter`
(auto-yes). Otherwise the status is `Ready`. The prompt strings are
"No, and tell Claude what to do differently" (Claude), "(Y)es/(N)o/(D)on't
ask again" (aider), and "Yes, allow once" (Gemini).
<https://github.com/smtg-ai/claude-squad/blob/main/session/tmux/tmux.go>,
<https://github.com/smtg-ai/claude-squad/blob/main/app/app.go>

**Taxonomy.** `Running` ("claude is working"), `Ready` ("waiting for
user input"), `Loading` (never assigned in instance.go), and `Paused`
("worktree removed but branch preserved"). A restore that finds the
tmux session gone parks the instance as `Paused`.
<https://github.com/smtg-ai/claude-squad/blob/main/session/instance.go>

**Presentation.** A spinner for Running/Loading, a green `● ` for
Ready, and a grey `⏸ ` for Paused, at the right end of the title row.
No sorting or grouping by status: rows stay in insertion order.
<https://github.com/smtg-ai/claude-squad/blob/main/ui/list.go>

**Lessons.** "Changed since last tick" is the cheapest possible signal
and conflates an animated spinner with real work. It can't tell
"needs you" (a permission dialog) from "ready for a prompt", which is
exactly the distinction the operator wants. Issues show capture
failures (#218, #216, #51) and a prompt sent before the CLI was ready
(#266). <https://github.com/smtg-ai/claude-squad/issues?q=is%3Aissue+status>

### 2.3 ccmanager (kbwo/ccmanager)

**Detection.** ccmanager owns each session's PTY, feeds output into a
headless terminal, and polls a per-harness detector every 100 ms
(`STATE_CHECK_INTERVAL_MS = 100`).
<https://github.com/kbwo/ccmanager/blob/main/src/services/sessionManager.ts>
There are detector files for claude, codex, cursor, gemini, copilot,
cline, opencode, kimi, grok, and mcode.
<https://github.com/kbwo/ccmanager/tree/main/src/services/stateDetector>
The Claude detector: waiting_input matches `/(?:do you want|would you
like).+\n+[\s\S]*?(?:yes|❯)/`, "esc to cancel", or a deny-option line.
Busy matches "esc to interrupt", "ctrl+c to interrupt", a spinner
activity pattern `^[spinner chars] \S+ing.*…`, or a token-stats line.
Idle is the fallback, debounced: the content must be stable for 1500
ms (`IDLE_DEBOUNCE_MS`). Background tasks come from
`/(\d+)\s+(?:background\s+task|local\s+agent)/` in the last 3 lines.
Team members are `@name` tokens on the "shift+↑ to expand" line.
<https://github.com/kbwo/ccmanager/blob/main/src/services/stateDetector/claude.ts>
The Codex detector uses "press enter to confirm or esc to cancel",
"allow command?", "[y/n]", and similar for waiting, and `/esc.*interrupt/i`
for busy. <https://github.com/kbwo/ccmanager/blob/main/src/services/stateDetector/codex.ts>

**Taxonomy.** `idle`, `busy`, `waiting_input`, and `pending_auto_approval`.
Menu wording: Idle "Ready for new input", Busy "Processing a request",
Waiting "Awaiting user confirmation".
<https://github.com/kbwo/ccmanager>,
<https://github.com/kbwo/ccmanager/blob/main/docs/status-hooks.md>

**Presentation.** Busy `●`, Waiting `◐` (and Pending Auto Approval
`◐`), Idle `○`, with no per-state colors in the constants file. Dim
suffixes: `[BG]`, `[BG:N]`, `[Team:N]`.
<https://github.com/kbwo/ccmanager/blob/main/src/constants/statusIcons.ts>
The footer has a dim legend (`Status: ● Busy ◐ Waiting ○ Idle`), there
is a state filter cycled with Tab, and recent-project rows show counts
as `[active/busy/waiting]`. A state tag gets its own aligned column
when the width allows, else it's appended to the branch.
<https://github.com/kbwo/ccmanager/blob/main/src/components/Menu.tsx>,
<https://github.com/kbwo/ccmanager>
Status-change hooks get `CCMANAGER_OLD_STATE`/`CCMANAGER_NEW_STATE`,
for things like `noti` or a Slack webhook.
<https://github.com/kbwo/ccmanager/blob/main/docs/status-hooks.md>

**Accuracy problems.** #227 "State detection shows idle when Claude
Code is busy". Root cause: "with a statusbar the `esc to interrupt` is
removed", so a user's status line setting silently broke busy
detection. The reporter also asked that the fallback keep the previous
state instead of returning idle. <https://github.com/kbwo/ccmanager/issues/227>
Claude Code's docs confirm that a custom status line hides `esc to
interrupt`. <https://code.claude.com/docs/en/statusline>
#113 and #117 are both titled "Update state detection strategy for
Claude Code", and #84 is "state detection for latest codex version".
Each harness UI change needs a ccmanager release.
<https://github.com/kbwo/ccmanager/issues?q=is%3Aissue+state+detection>

**Lessons.** Scraping is version-coupled and also coupled to the user's
own harness configuration. An unknown frame should keep the last state
rather than fall to "idle".

### 2.4 Crystal (stravu/crystal), now Nimbalyst

Crystal was deprecated in February 2026 in favour of Nimbalyst.
<https://github.com/stravu/crystal>

**Detection.** Crystal owns the agent process. A stream-json `system`/
`result` message moves a running session to `completed`. A
`completed_unviewed` status is derived when `lastViewedAt` is missing
or older than `updatedAt`. On startup all active sessions are marked
stopped.
<https://github.com/stravu/crystal/blob/main/main/src/services/sessionManager.ts>
Permission requests come through Crystal's own MCP tool
(`approve_permission`), which relays to the main process. This looks
like a `--permission-prompt-tool`, though the flag wiring wasn't in the
fetched file.
<https://github.com/stravu/crystal/blob/main/main/src/services/mcpPermissionBridge.ts>

**Taxonomy.** `'initializing' | 'ready' | 'running' | 'waiting' | 'stopped'
| 'completed_unviewed' | 'error'`.
<https://github.com/stravu/crystal/blob/main/frontend/src/types/session.ts>

**Presentation.** initializing/running: spinning loader, green.
waiting: `PauseCircle`, "Waiting for input", pulsing amber.
stopped: `CheckCircle` "Completed". completed_unviewed: `Bell` "New
activity" ("Session has new unviewed results"), pulsing.
error: `AlertCircle`. Animation runs only while the window is visible.
<https://github.com/stravu/crystal/blob/main/frontend/src/components/StatusIndicator.tsx>

**Nimbalyst.** The README describes a session kanban and a mobile
dashboard to "see which agents need you and which are still working",
with push "when an agent is waiting". Detection isn't documented.
<https://github.com/Nimbalyst/nimbalyst>

**Lessons.** "Unviewed" is derived from two timestamps (last update,
last view) instead of a mutable flag. That's simpler and immune to the
reload flicker agent-deck hit.

### 2.5 Claude Code agent view (`claude agents`)

This is the harness vendor's own answer to the operator's question, so
it is the strongest UX precedent.
<https://code.claude.com/docs/en/agent-view>

**Taxonomy.** Working ("actively running tools or generating a
response"), Needs input ("waiting on something only you can provide:
an answer to a question, a permission decision, or another prompt only
you can answer"), Idle ("nothing to do and is ready for your next
prompt", dimmed), Completed (green), Failed (red), and Stopped (grey).
In `claude agents --json`, `state` is `working`, `blocked`, `done`,
`failed`, or `stopped`. `status` is `busy`, `waiting`, or `idle`.
`waitingFor` is `permission prompt`, `input needed`, `sandbox request`,
`worker request`, or `dialog open`. "A session that finished its turn
and is waiting for your next instruction reads `done`, not `blocked`."

**Shape versus color.** Color carries state. Shape carries process
liveness: `✻`/animated `✽` means the process is running or needs
input, `∙` means the process exited and can be restarted from its
conversation, and `✢` marks a `/loop` session sleeping between
iterations, with a countdown (`in 4m`).

**Grouping and sorting.** Groups run Pinned, Ready for review, Needs
input, Working, Completed. Groups don't map one-to-one to states:
Ready for review means an open PR needs review or has failing checks,
and Completed collects finished, failed, and stopped. `Ctrl+S`
switches to grouping by directory. Rows then open with a colored state
word (`Needs input · double jump or wall climb?`). Overflow folds into
`… N more`. On short terminals the header compacts so working and
needs-input rows stay visible.

**Counts.**
- A header line counts sessions awaiting input, working, and completed.
- The terminal tab title reads `2 awaiting input · claude agents`.
- A normal session's footer shows `← 2 agents` while sessions wait on
  input, refreshed about every 10 s, and `← 2 done` briefly when work
  finishes. The count changes color when it moves, which
  `prefersReducedMotion` disables.

**Timers.** Row age counts from session creation and freezes at the run
length when finished. The peek panel shows `waiting 3m`, "how long a
session has waited on you". Working row text updates "at most once
every 15 seconds".

**Row text.** A working row shows what the session says it is doing. A
blocked row shows the question it is asking. PR badges (`#1234`, `3
PRs`) are colored by PR state.

**Keys and filters.** `Alt+↑/↓` jump between group headers. There is no
dedicated next-needs-input key. Filters: `s:<state>`, `s:blocked` ("lists
everything waiting on you"), `a:<agent>`, `n:<text>`, `o:<text>`.

**Notifications.** "While agent view is open … sends a notification
through your configured terminal notification channel when a local
background session starts needing your input, finishes, or fails",
and fires the `Notification` hook with `agent_needs_input` or
`agent_completed`.

**Subagents.** Subagents and teammates "aren't listed as separate
rows". A running subagent, workflow, or monitor counts as working.
Release notes add a `done/total` count for parallel work items
(v2.1.161). Another note fixes flips: "A turn with no readable text
keeps the session's previous state instead of flipping it back to
`Working`" (v2.1.205).

**Coverage limit.** Only background sessions appear. Interactive
sessions in other terminals "don't appear until you background them".
The docs tell scripts to read `claude agents --json` instead of the
state files (`~/.claude/daemon/roster.json`,
`~/.claude/jobs/<id>/state.json`).

### 2.6 Claude Code single-session signals: status line, notifications, OSC 7501

- **OSC 7501.** "If your terminal implements the OSC 7501 Program
  Status Protocol, it can show whether each interactive Claude Code
  session is working, waiting on you, or done." It needs v2.1.295 or
  later. "Inside tmux, Claude Code checks tmux for support instead of
  your terminal", and `allow-passthrough` has no effect on that. Where
  the protocol is supported, the tab title drops "the glyph it puts in
  front of the title in other terminals." So Claude also prefixes a
  status glyph to the terminal title, which tmux exposes as
  `#{pane_title}`. Which glyph means which state is **UNVERIFIED**.
  Background sessions don't report to the terminal.
  <https://code.claude.com/docs/en/terminal-config>
  `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` turns off both the title and
  the status reports. <https://code.claude.com/docs/en/env-vars>
- **Notification timing gates.** `permission_prompt` fires after "about
  six seconds" without typing (each keystroke defers it). `idle_prompt`
  fires "about 60 seconds after Claude finishes responding", only if
  no background agent is running, and not while waiting on a usage
  limit. The docs point to `PermissionRequest` for an immediate signal.
  Other types: `agent_needs_input`, `agent_completed`,
  `elicitation_dialog`, `quota_auto_resume_*`.
  <https://code.claude.com/docs/en/hooks>
- **Stop and interrupts.** `Stop` "Does not run if the stoppage
  occurred due to a user interrupt. API errors fire StopFailure
  instead." `StopFailure` matchers include `rate_limit`, `overloaded`,
  `authentication_failed`, and `billing_error`. `Stop` carries
  `background_tasks` (`type` among `shell`, `subagent`, `monitor`,
  `workflow`, `teammate` …) so hooks can tell "session is done" from
  "paused waiting for background work". `PreCompact`/`PostCompact`
  carry a `manual|auto` trigger. <https://code.claude.com/docs/en/hooks>
- **Status line.** It gets JSON (session_id, context_window percentages,
  `rate_limits.five_hour/seven_day`, worktree, agent). It reruns on new
  assistant messages, on `/compact` finishing, on mode changes, and so
  on, debounced at 300 ms. It "can go quiet when the main session is
  idle". <https://code.claude.com/docs/en/statusline>
- **Transcripts.** "The entry format is internal to Claude Code and
  changes between versions, so scripts that parse these files directly
  can break on any release." <https://code.claude.com/docs/en/sessions>

### 2.7 Codex CLI

- `tui.notifications` (bool or list of event types such as
  `agent-turn-complete` and `approval-requested`).
  `tui.notification_method` is `auto|osc9|bel`, and
  `tui.notification_condition` is `unfocused|always` (default
  unfocused). `notify` runs an external program on `agent-turn-complete`
  with `thread-id`, `turn-id`, `cwd`, and `last-assistant-message`.
  <https://learn.chatgpt.com/docs/config-file/config-advanced>
  (redirected from developers.openai.com/codex/config-advanced)
- `tui.terminal_title` defaults to `["spinner", "project"]`, so the
  tmux pane title carries a live spinner while Codex works.
  `tui.status_line` is a list of footer items; I didn't see the item
  list (**UNVERIFIED**).
  <https://learn.chatgpt.com/docs/config-file/config-reference>
- Hooks include `SessionStart`, `UserPromptSubmit`, `PermissionRequest`,
  `PreToolUse`, `PostToolUse`, `PreCompact`/`PostCompact`,
  `SubagentStart`/`SubagentStop`, `Stop`, `Interrupt`, and `SessionEnd`.
  Unlike Claude, Codex has an `Interrupt` event. Hooks are on by
  default, and non-managed hooks need trust review via `/hooks`.
  <https://learn.chatgpt.com/docs/hooks>

### 2.8 workmux (raine/workmux)

- **Detection.** A Claude Code plugin maps `UserPromptSubmit` and
  `PostToolUse` to `working`,
  `Notification(permission_prompt|elicitation_dialog)` to `waiting`,
  and `Stop` to `done`. `SessionStart` registers the agent.
  <https://github.com/raine/workmux/blob/main/.claude-plugin/plugin.json>
  The README says a working agent with no pane output for 10 seconds is
  treated as interrupted. That covers Claude's missing interrupt hook.
  <https://github.com/raine/workmux>
- **Presentation.** Icons go in the tmux window name: 🤖 working, 💬
  waiting, ✅ done (configurable). Waiting and done "clear on their own
  when you focus the window". The dashboard Status column shows the
  icons or "stale". `s` cycles sort modes, and the default **Priority**
  is "Waiting > Done > Working > Stale". `f` hides stale agents.
  `workmux list` shows counts like "2🤖 1✅". `workmux last-done` jumps
  to the most recently finished or waiting agent and cycles back.
  <https://github.com/raine/workmux>
- **Issues.** Pane attribution, not state semantics: #300 (background
  sessions with no `TMUX_PANE` attributed by process ancestry), #304
  (an inherited `TMUX_PANE` from Codex's shared app-server daemon),
  #291 (OpenCode v2 shared service breaks pane identity), #301 (done
  shown while a background task runs, because Stop's `background_tasks`
  is ignored), #335 (Copilot hooks never set waiting).
  <https://github.com/raine/workmux/issues?q=is%3Aissue+status>
- **Lessons.** Clear-on-focus is a cheap acknowledgement model. Shared
  harness daemons break the "hook env tells you the pane" assumption.
  This is the same problem Conspectus's mux attribution solves
  (ADR 0028).

### 2.9 tmux-agent-status (samleeney/tmux-agent-status)

- **Detection.** Hooks for Claude (`UserPromptSubmit`, `PreToolUse`,
  `Stop`, `Notification`), Codex, and Devin write `working`/`done`/`wait`
  into `~/.cache/tmux-agent-status/<session>.status`. Per-pane and
  `parked` variants exist. A Stop with a non-empty `background_tasks`
  keeps the session `working`. Process detection is optional, and
  there is no pane scraping. A collector wakes every second and
  re-collects every 5 ticks.
- **Taxonomy.** `working`, `done`, `wait`, `ask`, and `parked` (a user
  choice to set an agent aside).
- **Presentation.** One glyph per agent in `status-right`: Claude `✳`
  pulsing `✳`/`✻`, Codex `⬢`/`⬡`. Working is yellow and pulsing, waiting
  cyan, ask magenta, done green. Pulses are "staggered by position so a
  row of busy agents doesn't blink in unison". There are no numeric
  counts: the glyph count is the agent count. The sidebar has
  SESSIONS, INBOX, and RECENTLY READY sections. The fzf switcher's
  agents view sorts `ask`, `done`, `working`, `wait`, `parked`, and
  `prefix + N` jumps to the next inbox item.
- **Completion inbox.** A Stop that moves an active pane to done
  records an entry. Visiting the pane marks it read. "Highlighting or
  previewing a row does not mark it read." Entries persist on disk.
  Finish sounds are configurable.
  <https://github.com/samleeney/tmux-agent-status>
- **Issues.** #14, "Waiting jobs should auto-transition when timer
  expires". <https://github.com/samleeney/tmux-agent-status/issues?q=is%3Aissue>

### 2.10 cmux (manaflow-ai/cmux)

- **Detection.** The terminal "picks up terminal sequences (OSC
  9/99/777)" and offers `cmux notify` for agent hooks. The motivation:
  Claude's default notification "Claude is waiting for your input" has
  no context.
- **Presentation.** "Panes get a blue ring and tabs light up when coding
  agents need your attention", there are unread badges in the sidebar,
  and a notification panel. Keys: ⌘I panel, ⌘⇧U "Jump to latest
  unread", ⌥⌘U toggle unread, ⌃⌘U "mark current item as oldest unread
  and jump to next latest unread". The sidebar also shows branch,
  linked PR status/number, cwd, ports, and the latest notification
  text.
  <https://github.com/manaflow-ai/cmux>
- **Lesson.** The attention model is an unread queue of notifications,
  not a state machine. That's robust (it relies only on the harness's
  own notifications) but coarse.

### 2.11 Herdr (herdr.dev) and OSC 7501

- **Taxonomy (the cleanest in the survey).** `blocked` ("needs input,
  approval, or a decision"), `working`, `done` ("finished and you have
  not looked at it yet"), `idle` ("finished or waiting and has been
  seen"), and `unknown` ("cannot confidently classify").
  <https://herdr.dev/docs/concepts/>
- **Seen state is per client.** "Each client tracks which completions it
  has displayed". Viewing in one client doesn't clear another's Done
  badge. CLI/API statuses use the server's seen state.
  <https://herdr.dev/docs/concepts/>
- **Detection.** "Foreground processes, screen manifests, and optional
  integrations". Manifests are per-agent rules mapping the live bottom
  of the screen to idle, working, or blocked. They are overridable in
  `~/.config/herdr/agent-detection/<agent>.toml` and updated remotely
  from herdr.dev without a binary release. `herdr agent explain`
  debugs a classification. "Blocked is strict": it is marked only when
  the screen matches known approval, question, or permission UI, and
  otherwise a known agent falls back to `idle`
  (`default_known_agent_idle_fallback`). Caveat: "Unusual new agent
  prompts may initially show as `idle` instead of `blocked`".
  Integrations that report state replace screen reading. For Claude
  Code, the hook only reports session identity, and "state comes from
  Herdr's screen manifest detection."
  <https://herdr.dev/tui/docs/agents/>, <https://herdr.dev/tui/docs/integrations/>
- **Report API.** `pane report-agent --source --agent --state
  working|idle|blocked --seq --message`. `--seq` must increase; reports
  that don't are ignored. `release-agent` clears on quit. If an agent
  exits without releasing, Herdr notices the idle shell prompt "within
  a second or two". <https://herdr.dev/docs/add-herdr-support/>
- **OSC 7501.** herdr.dev calls it "an open status protocol", for
  example `printf '\e]7501;state=blocked:app=my-agent\e\\'`. Agents run
  inside tmux can't report through it. <https://herdr.dev>
  herdr-core: "An agent can say it is working, done, blocked, or in
  error through OSC 7501, the Program Status Protocol", and "A program
  that does not report status has none."
  <https://github.com/herdrdev/herdr-core>
  I found no formal spec page. The full field grammar is **UNVERIFIED**.
- **Presentation.** Workspace state "rolls up from the agents inside
  it": a blocked agent makes its pane, tab, and workspace look blocked.
  A done agent stays visible until viewed. The mobile inbox sorts
  "needs you, done, working and idle". <https://herdr.dev>,
  <https://herdr.dev/tui/docs/agents/>

### 2.12 amux (andyrewlee/amux)

- **Detection.** Every hosted pane has an OSC 7501 "status tap"
  (`state=blocked:kind=question:msg=<base64>`), which "replaces guessing
  from output heuristics". There is also `amux report --state`. BEL
  counts as needing attention, and the README says Claude rings it on
  notification and permission events. Reported state is stored in
  tmux pane options `@amux_reported_*`.
- **Presentation.** Done and blocked badges, `n` jumps to the next
  workspace needing attention, a bell plus an "Agent finished" toast
  (`notify_on_done`), and an event log. `amux agent ls --json` reports
  each session's "strongest state signal".
  <https://github.com/andyrewlee/amux>
- **Lesson.** amux runs agents inside tmux yet still reads OSC 7501
  through its own tap. Claude's docs say that inside tmux Claude asks
  tmux about support, so how amux gets Claude to emit there is
  **UNVERIFIED**.

### 2.13 Zellij plugins: zj-radar, zellaude, zj-agents, and others

Listed in the "Coding Agent Management" section of
<https://github.com/zellij-org/awesome-zellij>: zj-agents, zj-radar,
zj-agent-mob, zellaude, zellij-attention, zellij-tree-tabs, and
zellij-ai-stats.

- **zj-radar.** Push-only: hooks send `zj_radar.status.v1` JSON over
  `zellij pipe`, and it "never polls panes". Legend: `◆ needs you · ⠋
  working · ● done · ✗ error · ○ idle`. Background work (subagents,
  backgrounded tests, dev servers) goes on lines under the agent
  (`task_lines lines|count|off`, Claude only). Running commands get an
  elapsed tag. `attention-next` jumps. Desktop notifications have
  per-state toggles (`notify_done`, `notify_error`, `notify_pending`)
  and `notify_when_focused` false by default. `density`:
  cards/comfortable/compact. An `─ earlier ─` ledger lists recent
  completions. <https://github.com/marktoda/zj-radar>
  Its postmortem of the predecessor: polling per pane output (about 6
  queries/s per busy pane) blocked the Zellij server, logging 35,225
  timeouts at about 19/s. It is "not a pane-count ceiling, it's
  per-pane-per-output polling". Design rules adopted: "Push, not
  poll", an event-gated 1 Hz timer only while animating, else 60 s.
  <https://github.com/marktoda/zj-radar/blob/main/docs/smart-tabs-postmortem.md>
- **zellaude.** Claude hooks go through `zellij pipe`. Per-activity
  glyphs: `◆` starting, `●` thinking, `⚡` Bash, `◉` reading, `✎`
  editing, `⊜` spawning subagent, `◈` web, `⚙` other tool, `▶` waiting
  for prompt, `⚠` waiting for permission, `✓` done, `○` idle. Elapsed
  timer after 30 s in a state. A permission request pulses the tab
  (Persist/Brief/Off). Notifications are limited to one per 10 s per
  tab, with an Unfocused mode. Clicking `⚠` focuses the exact waiting
  pane. <https://github.com/ishefi/zellaude>
- **zj-agents.** Screen scraping of the last 40 visible lines about
  once a second, fixture-backed for claude/codex/grok/pi/omp, with
  overridable manifests "inspired by Herdr". Pane text is "not stored
  in engine state". <https://github.com/kaankoken/zj-agents>

### 2.14 VS Code agent sessions

- **Badges.** Sessions are "in progress" or "unread". A title-bar
  indicator (`chat.agentsControl.enabled`) shows unread and in-progress
  badges, and selecting one filters the list.
  <https://code.visualstudio.com/docs/copilot/chat/chat-sessions>,
  <https://code.visualstudio.com/docs/agents/run/chat-view>
- **Inbox mode** (experimental, `sessions.list.inboxEnabled`).
  Categories: **Pinned**, **Inbox** (a question, approval request,
  error, failing PR check, or finished response awaiting review),
  **Working**, **Ready to merge** (non-draft PRs with passing CI),
  **On hold** (held or snoozed), and **Done**. Triage: "Dismiss from
  Inbox" moves to Done without stopping or archiving, and "New
  activity or notifications can bring it back". There are also Put on
  Hold, Snooze (until a time or new activity), and Mark Session as Done
  (archives). <https://code.visualstudio.com/docs/agents/run/sessions/organize-sessions>
- **Lesson.** Attention is treated as an operator-curated queue layered
  over the machine state. Dismiss and snooze are user intent, not
  agent state.

### 2.15 Paradev (formerly Superset)

Per-agent status hooks can be toggled in Settings. "Claude Code reports
both finished and waiting states", while other agents send only
completion. The UI has a chime on finish, a macOS dock badge for unread
activity or needed attention, and a manual **Clear Status** action
that "resets a stale running indicator, for example after
force-quitting an agent". Status works only for agents launched
through Paradev.
<https://docs.paradev.com/agent-status>,
<https://github.com/superset-sh/superset>
Lesson: a manual "clear stale" escape hatch is an admission that hook
state can stick after a crash.

### 2.16 uzi (devflowinc/uzi)

`getAgentStatus` runs `tmux capture-pane` on the `agent` window and
returns `running` if the content contains "esc to interrupt" or
"Thinking", `ready` otherwise, and `unknown` on capture error.
<https://github.com/devflowinc/uzi/blob/main/cmd/ls/ls.go>
`uzi ls -w` refreshes every second, and `uzi auto` presses Enter on
prompts. <https://github.com/devflowinc/uzi>
This is the minimal form of scraping, and the statusline issue in 2.3
breaks it.

### 2.17 vibe-kanban (BloopAI/vibe-kanban)

`ExecutionProcessStatus { running, completed, failed, killed }` and
`ExecutionProcessRunReason` (`setupscript`, `codingagent`, `devserver`,
…). Approvals are first-class (`ApprovalStatus pending|approved|denied|timed_out`,
`ToolStatus pending_approval`), and `WorkspaceSummary` carries
`has_pending_approval`, `latest_process_status`, and `pr_status`.
<https://github.com/BloopAI/vibe-kanban/blob/main/shared/types.ts>
The README says "Vibe Kanban is sunsetting."
<https://github.com/BloopAI/vibe-kanban>
Lesson: when the tool owns the agent loop, "waiting on you" is a
structured pending approval with a timeout, not an inference.

### 2.18 Conductor (conductor.build)

The docs describe isolated workspaces and a review lifecycle driven by
checks and PR state. I found no published status names or indicators.
<https://www.conductor.build/docs/>,
<https://www.conductor.build/docs/concepts/workflow>
Changelog titles include "Workspace Status" (0.35.0, Feb 2026) and
"Mark Workspaces as Unread" (0.25.11, Dec 2025), but the entry bodies
didn't load. Conductor's state UI is **UNVERIFIED**.
<https://www.conductor.build/changelog>

### 2.19 hcom (aannoo/hcom)

Hooks record activity to SQLite. `idle` and `blocked` appear as event
subscriptions (`hcom events --idle luna --wait 600`, `auto_subscribe`
presets `collision`, `created`, `stopped`, `blocked`). `title_mode`
writes the terminal title. <https://github.com/aannoo/hcom>
Lesson: for agent-to-agent coordination, "idle" is an event to wait on.

### 2.20 Thin or negative evidence

- **opcode** (formerly Claudia): "Background Execution" and
  "Execution History". No status names are documented.
  <https://github.com/winfunc/opcode>
- **Omnara**: now runs its own agent loop with a Postgres event log
  ("That record is the agent's state"). Waiting is a built-in
  ask-a-person tool and per-tool approval gates.
  <https://github.com/omnara-ai/omnara>
- **LangChain agent-inbox**: an inbox of LangGraph interrupts with
  actions `accept`, `edit`, `response`, `ignore`. The repo is archived.
  <https://github.com/langchain-ai/agent-inbox>
- **ccswarm**: now runs stages in Herdr tabs, reports state to Herdr,
  and reads the transcript to detect a question.
  <https://github.com/nwiizo/ccswarm>
- **sesh**: a session picker with source icons and a source sort order.
  No agent state. <https://github.com/joshmedeski/sesh>
- **GitHub Copilot sessions**: stopped/archived only. No status
  vocabulary documented.
  <https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/track-copilot-sessions>
- **Cursor agents window**: no status docs found.
  <https://cursor.com/docs/agent/agents-window>
- **WezTerm plugins**: none found (**UNVERIFIED**, not searched beyond
  the awesome lists).
- **tmux itself** offers `monitor-activity`, `monitor-silence
  [interval]`, `monitor-bell`, the `alert-activity`/`alert-silence`/`alert-bell`
  hooks, `#{window_bell_flag}`, `#{window_silence_flag}`, and
  `#{pane_title}` ("can be set by application" via `\e]2;…\e\\`, gated
  by `allow-set-title`). <https://man7.org/linux/man-pages/man1/tmux.1.html>

---

## 3. Union state taxonomy

The tools agree on three operator-facing buckets (needs me, working,
finished) and split "finished" by whether the operator has seen it.

| Union state | Meaning for the operator | Names in the wild | Notes |
|---|---|---|---|
| **needs you** (blocked) | The agent can't continue until the operator answers a permission, question, plan approval, or elicitation | waiting (agent-deck, ccmanager `waiting_input`, Crystal), Needs input / `blocked` (Claude agent view), blocked (Herdr, amux, OSC 7501), needs you (Herdr, zj-radar), ask (tmux-agent-status), waiting (workmux), Waiting for permission (zellaude), pending approval (vibe-kanban, ccmanager) | Claude separates `waitingFor`: permission prompt, input needed, sandbox request, worker request, dialog open |
| **working** | A turn is running: the model is generating or a tool is executing | Running (agent-deck, claude-squad), Busy (ccmanager), working (Claude, Herdr, workmux, tmux-agent-status, zj-radar, amux), active (agent-deck frame), in progress (VS Code), Thinking (zellaude) | zellaude splits by tool (Bash, read, edit, web, subagent) |
| **done** (unseen) | The turn finished, the agent is ready for a new prompt, and the operator hasn't looked yet | done (Claude JSON, Herdr, workmux, zj-radar, amux), completed_unviewed / "New activity" (Crystal), unread (VS Code, cmux), RECENTLY READY / inbox (tmux-agent-status), Completed (Claude group) | The operator's "(b) ready for a new prompt" |
| **idle** (seen) | Ready for a new prompt and already acknowledged | idle (agent-deck after attach, Herdr seen, ccmanager), Ready (claude-squad), Idle (Claude, dimmed), Waiting for user prompt (zellaude `▶`) | agent-deck and Herdr flip done→idle on view, workmux clears on focus |
| **error** | The turn ended on an API, auth, or crash error | error (agent-deck, Crystal, amux, zj-radar, OSC 7501), failed (Claude, vibe-kanban), StopFailure (Claude hook) | agent-deck substates: `auth-401`, `model-unavailable` |
| **rate-limited** | Blocked by a usage or quota limit, auto-resumes later | `usage-limit`, `model-limit`, `account-blocked` (agent-deck substates), `StopFailure: rate_limit`, `quota_auto_resume_*` (Claude) | Nobody makes it a top-level state. It's a substate of error or idle with a reset time |
| **compacting** | Context compaction in progress | `PreCompact`/`PostCompact` events (Claude, Codex) | **No surveyed tool displays it.** A gap, or evidence it isn't needed |
| **starting** | The process launched and isn't ready yet | Loading (claude-squad), initializing (Crystal), starting (agent-deck, 2 min window), Session starting (zellaude) | Always time-boxed |
| **stopped / exited** | No process; resumable from the record | stopped (Claude, agent-deck), Paused (claude-squad), killed (vibe-kanban), `process-exited`, `inactive` (agent-deck), `∙` exited (Claude shape) | Conspectus already has `stopped`/`missing` words and the `⊘` glyph for muxes |
| **unknown / stale** | Can't classify, or the signal is too old | unknown (Herdr, uzi), stale (workmux), `hook-lag` (agent-deck), `?` "last known" (agent-deck remotes) | Herdr and agent-deck keep the last known value and mark its age |
| **background work** (modifier) | The foreground turn is done but subagents, shells, monitors, or workflows run | `background-work`, `watching` (agent-deck), `[BG:N]`, `[Team:N]` (ccmanager), task lines (zj-radar), `done/total` (Claude) | Claude counts it as working. agent-deck won't let a waiting hook override it |
| **sleeping / scheduled** (modifier) | A `/loop` or cron waiting for its next run | `✢` with `in 4m` (Claude) | Rare |
| **parked / on hold / dismissed** (user intent) | The operator set it aside | parked (tmux-agent-status), On hold, Snooze, Dismiss (VS Code), annotation Parked (agent-deck) | User-authored, not observed |
| **ready for review** (PR-derived) | Agent work produced a PR needing review or failing checks | Ready for review (Claude group), Ready to merge (VS Code), Ready for review annotation (agent-deck) | Conspectus already has PR state |

Two distinctions matter most for the operator's (a)/(b)/(c) split:
- **blocked versus done.** Claude Code's docs are explicit: "A session
  that finished its turn and is waiting for your next instruction
  reads `done`, not `blocked`."
  <https://code.claude.com/docs/en/agent-view> Tools that lump these
  together (claude-squad Ready, uzi ready, early agent-deck) can't
  answer "which agents are stuck on me".
- **done versus idle.** Six tools track whether the operator has seen
  a finished turn: agent-deck, Herdr, Crystal, tmux-agent-status, cmux,
  and VS Code.

---

## 4. Detection approaches compared

| Approach | Used by | Accuracy | Latency | Setup cost | Fragility across harness versions | Privacy and cost | Known failure modes |
|---|---|---|---|---|---|---|---|
| **Harness-native state API** (`claude agents --json`, OpenCode SSE `/event`, herdr-core's reported state) | Claude agent view, agent-deck (OpenCode), Herdr | Highest: the harness knows | Low (poll or stream) | None to low | Low; sanctioned interface | Good; state only | Coverage: `claude agents` lists only background sessions |
| **Lifecycle hooks** (Claude, Codex, Gemini, pi, OpenCode plugins) | agent-deck, workmux, tmux-agent-status, zj-radar, zellaude, Paradev, hcom, Conspectus (attribution only, ADR 0028/0146) | High at transitions, gaps between them | Immediate (push) | Operator installs hooks per harness and config dir | Medium; documented but evolving schemas | Payloads carry prompts and messages, so read only state fields and don't persist payload | Claude `Stop` doesn't fire on user interrupt, so state sticks at working. No signal while the model thinks before its first tool. Notification gates (6 s, 60 s). A crash leaves a stale state (Paradev "Clear Status", agent-deck #2896). Shared harness daemons break pane attribution (workmux #300, #304, #291) |
| **Terminal status protocols** (OSC 7501; OSC 9/99/777 and BEL notifications; OSC 0/2 title glyph or spinner) | Herdr, amux, cmux, agent-deck (title spinner), Claude Code and Codex as emitters | Medium-high for OSC 7501 when emitted; title spinner says busy only | Immediate | None, but the terminal or mux must support it | Medium; Claude's OSC 7501 needs v2.1.295+ | Good; no content | Inside tmux, Claude checks tmux for OSC 7501 support (so it likely won't emit there, **UNVERIFIED**). Title glyph meanings undocumented. BEL is overloaded |
| **Pane scraping** (capture-pane or PTY emulation plus regex or manifests) | claude-squad, ccmanager, uzi, agent-deck (fallback), Herdr, zj-agents | Medium-low; best on dialogs, weakest on "busy vs idle" | Poll interval (100 ms to 2 s) | None | **High**: every UI change breaks it (ccmanager #84/#113/#117/#227, agent-deck #2519/#2521/#3037/#2948). The user's statusline config changes the text | Reads screen content that may hold secrets; keep it ephemeral (zj-agents). Capture cost scales with panes (agent-deck #3089; zj-radar postmortem) | Logos read as spinners, stale banners in scrollback, prompts read as idle. Mitigations: golden corpus, remote manifests, `explain`, strict blocked with idle fallback, one-sample hold, idle debounce, keep last state on an unknown frame |
| **Output-change heuristics** (content hash diff, tmux `monitor-activity`/`monitor-silence`) | claude-squad, workmux (10 s no-output means interrupted) | Low semantics, good liveness | Poll or tmux alert | None | Low | Good | Animated spinners and clocks count as change. Can't tell blocked from done |
| **Transcript tailing** | agent-deck (rate limits, background-work veto), ccswarm (question detection), Conspectus (activity age today) | Medium | Seconds; the transcript is "written asynchronously and may lag" (Claude hooks doc) | None | High: Claude says the format "changes between versions" | Reads content | Lags the screen. Can't see live dialogs |
| **Process polling** (foreground process, pane_current_command, PID liveness) | Herdr, tmux-agent-status (optional), Conspectus (runtime processes) | High for alive/exited/starting only | Poll | None | Low | Good | Wrappers and sandboxes hide the agent. Suspended processes (Herdr) |
| **Own the agent loop** (spawn with stream-json, own the permission tool) | Crystal, vibe-kanban, Omnara, Claude supervisor | Exact | Immediate | Requires launching every agent through the tool | Medium | Tool sees everything | Only covers sessions it launched. Conflicts with Conspectus's survey-only stance |
| **Ingest another orchestrator's state** | (proposed for Conspectus) agent-deck `state.db` `status` column, `claude agents --json`, `herdr agent list`, `amux agent ls --json`, `@workmux_status`/`@amux_reported_*` tmux options, tmux-agent-status cache files | As good as the source | Source's cadence | None beyond the source | Tied to the source's schema | Good | Only covers that tool's sessions; schemas are third-party |

Synthesis:
- Every mature tool converges on **push first, poll as fallback**:
  agent-deck (hooks, then title, then pane), Herdr (integrations, then
  manifests), and zj-radar (push only, after polling "melted" its
  predecessor).
- Scraping is where nearly all the bugs are, and it is the only path
  for harnesses without hooks.
- The cheap, version-stable, content-free signals that are underused:
  the pane title (Codex's default spinner, Claude's glyph prefix), the
  bell flag, process liveness, and tmux silence. They answer "busy?"
  and "exited?" but not "blocked on what?".
- No approach covers interrupts on its own. Claude has no interrupt
  hook, so the fallback is a timeout or pane check (workmux's 10 s
  rule, agent-deck's frame read).
- State needs hold rules: terminal events persist until contradicted
  (agent-deck #2896), an unknown frame keeps the last state (ccmanager
  #227, Claude v2.1.205), and freshness is shown when the signal ages
  (agent-deck remotes, Herdr `unknown`).

---

## 5. Recurring presentation patterns as candidates for Conspectus

Conspectus context: the left panel already has a state cell per row
(`⊸`/`∘` mux link on Sessions and Mux rows, `⊘`/`?` for muxes), state
words in Pinned and History (`live`, `in mux`, `resumable`, `stopped` …),
a four-cell recency-colored age, ghost styling for non-running
entities, a braille spinner meaning "refresh pending", group headers
with `(N)` counts and a `▲` warning, and a ranked header line (`updated
Ns ago · N/M sessions · M mux · K stopped · chips · ▲ N`).
(`docs/tui-columns.md`, ADR 0145, `src/tui/ui/header.rs`)
`docs/comparison.md` already proposes an "Attention Mode" chip, lane,
or view in the style of k9s.

| # | Pattern | Seen in | Fit for Conspectus: pros | Cons and conflicts |
|---|---|---|---|---|
| P1 | **Single-cell live-state glyph per row** | agent-deck `●◐○✕`, ccmanager `●◐○`, claude-squad, zj-radar `◆⠋●✗○`, Claude `✻∙✢` | Matches ADR 0145's rule that single-kind views show state as glyphs. Scannable | **Glyph collisions**: `●` is the session kind glyph, `○` open PR, `◐` draft PR, `◆` repo, `◉` pin, `◎` hollow pin, and the braille spinner already means "refresh pending". ADR 0145 forbids reuse and color-only differences, and ADR 0073 requires one-cell width vetting. Needs a new vetted set |
| P2 | **State word in mixed views** | Claude's directory grouping (`Needs input · …`), ccmanager labels | Pinned and History already use words. `in mux` could become or carry `working`, `needs you`, `ready` | Word width costs columns. Must compose with existing words (`in mux` is a link fact, live state is an activity fact) |
| P3 | **Header attention counts** | Claude header, tab title `2 awaiting input`, footer `← 2 agents`; workmux `2🤖 1✅`; ccmanager `[active/busy/waiting]`; VS Code badges that filter | Fits the ranked header and the existing `▲ N` precedent; one glance answers "does anything need me?" | Header space is contested. Rank against freshness, counts, and `▲`. Decide whole-graph or filtered scope |
| P4 | **Attention-first ordering** | workmux Priority "Waiting > Done > Working > Stale"; tmux-agent-status `ask, done, working, wait, parked`; Claude groups; Herdr inbox; agent-deck waiting list by most-recent wait | Conspectus already has "float sessions with a mux to the top", so "float sessions that need you" is the same kind of control | Reordering under the cursor while states change is disorienting. Conspectus keeps rows still on fold (ADR 0182) and would need a stable-sort or no-live-resort rule |
| P5 | **State grouping mode** | Claude `Ctrl+S` state vs directory; VS Code Inbox categories | Sessions already cycles groupings (`Ctrl-G`); "state" would be one more | State groups churn as agents move. VS Code mixes in user triage (hold, snooze) |
| P6 | **Dedicated attention view or queue** | tmux-agent-status INBOX, cmux notification panel, VS Code Inbox, agent-inbox, Herdr mobile inbox, comparison.md "Attention Mode" | A view of only "needs you" plus "done unseen", across sessions and muxes; can fold in other attention items (ambiguity `▲`, gone pins) | A seventh view to learn and key. Overlaps Pinned. Mixed kinds need words (ADR 0145) |
| P7 | **Jump to next needing attention** | tmux-agent-status `prefix+N`, zj-radar `attention-next`, amux `n`, cmux ⌘⇧U, workmux `last-done`, agent-deck `Ctrl+b 1–6`, zellaude click on `⚠` | Cheapest high-value control; works in every view; pairs with attach (the existing gesture) | Needs a key in the scoped keymap. Must define order (oldest wait first or newest) |
| P8 | **Seen/unread tracking (done versus idle)** | agent-deck acknowledged-on-attach, Herdr per-client seen, Crystal `lastViewedAt < updatedAt`, tmux-agent-status read-on-visit (not on preview), workmux clear-on-focus, VS Code unread/dismiss | Splits "ready for a new prompt" into new and old; drives the attention count down as the operator works | Where does "seen" live? Persisting is a write (ADR 0143). Deriving it from tmux client focus is read-only. agent-deck's flicker bug (#2950) shows the cost of getting this wrong |
| P9 | **Time in state ("waiting 5m")** | Claude peek `waiting 3m`, zellaude timer after 30 s, zj-radar elapsed tags, workmux sidebar elapsed | Conspectus's age cell already means "time since last activity". For a blocked or done session that *is* time since the agent stopped, so the existing cell already reads "waiting for 5m" with no new column | Holds only if "last activity" means the agent's last output, and if a working session's age stays near 0s. A heartbeat could make it look always-fresh |
| P10 | **Notifications on transition** | Claude (desktop or bell via `preferredNotifChannel`), Codex (OSC 9 or BEL, unfocused only), zj-radar per-state toggles, zellaude rate limit, amux bell and toast, tmux-agent-status sounds, ccmanager hooks | The TUI could ring BEL or send OSC 9 on its own terminal on blocked or done; some operators run Conspectus in a side pane | The harnesses already notify, so doubling is noise. A desktop notification means launching a program (capabilities entry plus ADR 0117). The daemon must not "notify" as a background effect beyond category 2 |
| P11 | **Activity detail text** (what it's doing, or the question it's asking) | Claude row text and blocked row question, zellaude per-tool glyphs, agent-deck substate labels | Conspectus already shows a dim preview of the last message; for a blocked session the question is more useful | Payload privacy: rendering in memory is fine, persisting is not (ADR 0086). Hooks carry the question text (Notification `message`) |
| P12 | **Rollup to container** | Herdr (blocked pane makes the tab and workspace look blocked), ccmanager per-project counts, zj-radar tab rollup, agent-deck notification bar | Mux rows and group headers already roll up counts and `▲`; a mux with three agents shows its strongest state | Needs a precedence order (for example needs you > error > done > working > idle > unknown). Conflicts with ADR 0145's "a mux's age is its own activity" if state blends agents |
| P13 | **Freshness and confidence marks** | agent-deck "· status 47s old", grey `?` "last known"; Herdr `unknown`; workmux `stale` | Conspectus already has `?`, ghost styling, and `updated Ns ago`; the evidence-chip idea in comparison.md fits (hook vs scraped vs title) | More glyph and legend load. Must not hide behind color alone |
| P14 | **Animation for working** | claude-squad spinner, zj-radar `⠋`, tmux-agent-status staggered pulse, Claude `✽`, Crystal spin and pulse (paused when hidden) | Motion reads as "working" instantly | Conspectus's braille spinner means "refresh pending". Animation forces redraws, and agent-deck and zj-radar both note redraw and poll cost. Respect reduced motion (Claude `prefersReducedMotion`) |
| P15 | **Status filters** | agent-deck `!@#&`, Claude `s:blocked`, ccmanager Tab cycle, workmux `f` hide stale, VS Code badge click | Conspectus has a Controls panel and filters; "state: needs you" is one more filter | More controls. Filtered state lists churn |
| P16 | **Subagent and background indicators** | ccmanager `[BG:N]`, `[Team:N]`; zj-radar task lines; Claude `done/total`; agent-deck `background-work`; VS Code nested subagent chats | A dim `+N bg` chip, or nesting under the session (Conspectus already nests forks) | Claude deliberately doesn't list subagents as rows. Nesting multiplies rows |
| P17 | **User triage on top of state** (park, snooze, dismiss) | tmux-agent-status parked, VS Code hold/snooze/dismiss, agent-deck annotations | Fits "user-intent TOML stores" if persisted | Scope creep for v1. A write path (ADR 0143) |

---

## 6. Constraints from Conspectus's own design that shape the choice

These come from the repo's own rules and decide which prior art
transfers.

- **No terminal input injection** (ADR 0028, "absolute" per CLAUDE.md).
  The auto-yes and auto-approve features in claude-squad, uzi, and
  ccmanager are out of scope. So is acting on a blocked prompt from
  Conspectus: the action is attach, the existing gesture.
- **Payload privacy** (ADR 0086). Hook payloads, pane text, and
  Notification messages may be read and shown in memory, but not
  persisted. Only state facts go into the graph or cache. zj-agents'
  rule ("pane text is not stored in engine state") is the model.
- **Mutation envelope** (ADR 0143). Installing state hooks is
  operator-initiated `hook init` (already sanctioned). A persisted
  "seen" mark, a dismiss, or a park would need a category: user-intent
  TOML, or a rebuildable observation under `$XDG_STATE_HOME`. The
  daemon may write only category 2 observations.
- **Capabilities** (ADR 0117). Running `claude agents --json`, `herdr
  agent list`, `amux agent ls`, or `notify-send` is a new subprocess
  that needs a `docs/capabilities.toml` entry. Reading files (agent-deck
  `state.db`, tmux-agent-status cache) and tmux format variables
  (`#{pane_title}`, `#{window_bell_flag}`, pane options) doesn't need a
  new program, since tmux is already invoked. Conspectus already calls
  `tmux capture-pane` (`src/discovery/tmux/mod.rs`).
- **Glyph rules** (ADR 0145, ADR 0073 amendment). One meaning per
  glyph, no reuse of kind glyphs, shape not color, one-cell width
  vetted in DejaVu Sans Mono. This rules out the de facto
  `●`/`◐`/`○` set and zj-radar's `◆`. It also rules out the braille
  spinner for "working" unless the pending-refresh spinner moves.
- **Graph-first** (CLAUDE.md design guardrails). Live state should be
  provider-neutral attributes with source and freshness. Harness and
  orchestrator specifics (agent-deck substates, Claude `waitingFor`)
  belong in adapters and evidence. The ingest-another-orchestrator row
  in section 4 fits "provider-specific reality belongs in adapters".

---

## 7. Open design questions for the operator

Taxonomy and words
1. **Which top-level states, and their words?** A candidate union is
   needs you / working / done / idle / error / starting / stopped /
   unknown, with rate-limited, compacting, and background work as
   substates. Prior art splits on `needs you` (Herdr, zj-radar),
   `needs input` (Claude), `waiting` (agent-deck, ccmanager), and
   `blocked` (Claude JSON, OSC 7501). Note that "waiting" is ambiguous
   across tools: it can mean waiting on you, or waiting for a prompt.
2. **Separate done (unseen) from idle (seen)?** Six tools do. If yes,
   where does "seen" come from? Options: (a) in-memory per TUI
   instance, like Herdr's per-client seen; (b) derived read-only from
   tmux: a client attached with the pane active after the state change;
   (c) persisted as a rebuildable observation, which agent-deck had to
   do to stop flicker. Does attaching through Conspectus count as
   seeing?
3. **Rate-limited and compacting: states, substates, or detail-pane
   facts only?** No surveyed tool shows compacting. Rate limits are
   substates everywhere.

Glyphs and cells
4. **Which glyphs?** The common `●◐○` set collides with Conspectus's
   kind and PR glyphs. Should the live-state glyph replace the session
   kind glyph `●` on session rows (it's redundant in single-kind
   views), take a new cell beside the `⊸`/`∘` link cell, or merge with
   it? Every candidate needs width vetting.
5. **Animate "working"?** If yes, the pending-refresh spinner needs a
   different mark. If no, which static shape reads as busy?
6. **What do Pinned and History say for a pinned live session?**
   Replace `in mux` with the live word, or compose them (`in mux ·
   needs you`)?

Rollups and the header
7. **Mux rollup precedence** for a mux running several agents (for
   example needs you > error > done > working > idle > unknown)? Does
   the mux row show it, or only nested session rows? Do group headers
   carry an attention count next to `(N)` and `▲`?
8. **Header chip.** What does it count (needs you only, or needs you +
   done unseen + working), for the whole graph or the filtered view,
   and at what rank against freshness, counts, and `▲ N`? Should a key
   filter to it, like VS Code's badges?

Ordering, views, keys
9. **Attention-first ordering**: a control ("float sessions that need
   you"), a "state" grouping in the `Ctrl-G` cycle, a dedicated
   Attention or Inbox view, or a combination? If rows re-sort live,
   what keeps the cursor and the operator's place stable?
10. **Jump-to-next key.** Which order (longest-waiting first, as
    Claude's `waiting 3m` suggests, or most recent first, as agent-deck
    and cmux do)? Should it include done-unseen or only needs you?
11. **Age semantics.** Is the existing age (time since last activity)
    enough to read as "waiting for N", or does a blocked row need an
    explicit time in state? Claude separates the two only because its
    row age counts from creation.

Detection policy (TUI-facing consequences)
12. **Which sources are on by default?** Hooks via `hook init` (the
    operator opts in per harness), cheap tmux signals (pane title,
    bell, silence, process), ingesting other orchestrators (agent-deck
    `state.db` status, `claude agents --json`), and pane scraping as an
    opt-in fallback? Scraping is the highest-maintenance path in every
    tool surveyed.
13. **Staleness display.** After how long without a signal does a state
    become `unknown` or dim "last known"? What does a crash show (agent
    gone, hook said working)? Is there a manual "clear" like Paradev's?
14. **Provenance.** Should the detail pane say where a state came from
    (hook, pane title, scraped screen, another orchestrator) and how
    old it is, in line with the evidence-chip idea?

Notifications and triage
15. **Does Conspectus notify at all?** Options: nothing (the harnesses
    already notify), BEL or OSC 9 on its own terminal while the TUI is
    open and unfocused, or desktop notifications (a new capability).
    Per-state toggles? Rate limits?
16. **Subagents and background work**: a count chip on the session row,
    nested rows, or detail pane only?
17. **PR-derived "ready for review"**: merge agent state with the PRs
    view (as Claude and VS Code do), or keep them separate?
18. **User triage** (park, snooze, dismiss): out of scope for the first
    version, or a pin-like user-intent store?
