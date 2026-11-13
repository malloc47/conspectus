# Conspectus Operations

This document covers the runtime knobs that affect a Conspectus
invocation: the environment variables that gate discovery providers,
the harness state-root overrides, and the config-file locations.
Behavioural details (graph shape, resolver semantics) live in
`docs/design.md` and the ADRs.

## Environment Variables

All environment variables are read by `LocalDiscoveryConfig::from_env`
the first time discovery runs. They are read-only inputs; Conspectus
never sets or modifies them.

### Provider toggles

| Variable                       | Effect                                                                                          |
| ------------------------------ | ----------------------------------------------------------------------------------------------- |
| `CONSPECTUS_DISABLE_TMUX`      | When set (any value), the tmux provider is skipped entirely. No `MuxSession` nodes appear.      |
| `CONSPECTUS_DISABLE_FORGE`     | When set (any value), the GitHub forge provider is skipped. No `ForgePr` nodes appear.         |
| `CONSPECTUS_DISABLE_PROCTREE`  | When set (any value), active-pane process-tree linking is skipped. Other tmux evidence still runs. |

Use these in CI, automated tests, or shells where running `tmux
list-sessions`, walking `/proc`, or running `gh pr list` is unwanted,
slow, or noisy. These providers are otherwise best-effort: missing
binaries, unreadable processes, unauthenticated runs, and command
failures degrade silently to "no rows for that provider" or "no
process-tree evidence" rather than aborting the whole graph.

### Harness state-root overrides

The harness discovery providers look for session state under
`$HOME`-relative paths by default. Each can be overridden with an
explicit absolute path. The override path is used verbatim, so it can
point at a fixture directory in tests or a non-standard install
location.

| Variable                       | Default                       | Provider     |
| ------------------------------ | ----------------------------- | ------------ |
| `CONSPECTUS_CODEX_STATE`       | `$HOME/.codex`                | codex        |
| `CONSPECTUS_CLAUDE_CODE_STATE` | `$HOME/.claude`               | claude-code  |
| `CONSPECTUS_OPENCODE_STATE`    | `$HOME/.local/share/opencode` | opencode     |
| `CONSPECTUS_PI_STATE`          | `$HOME/.pi/agent/sessions`    | pi           |

The aider adapter is per-repo rather than per-state-root, so it
walks scan roots looking for `.aider*` markers and is not gated by an
environment variable.

When `CONSPECTUS_PI_STATE` is unset, the pi provider follows Pi's own
settings (ADR 0121): `PI_CODING_AGENT_SESSION_DIR`, then an absolute
`sessionDir` in `<agent-dir>/settings.json`, then `<agent-dir>/sessions`,
where `<agent-dir>` is `PI_CODING_AGENT_DIR` or `~/.pi/agent`. Both of
Pi's layouts are read: grouped by working directory, and flat. The
working directory always comes from the session file, not the directory
name. Pi sessions Conspectus can't see or match:

- `--no-session` runs, which write no file.
- Sessions under a project-level or relative `sessionDir`, or a per-run
  `--session-dir`. Point `CONSPECTUS_PI_STATE` at that directory.
- A custom `--session-id` in a pane's command line, which pane
  attribution can't recognize (UUID ids can be). The Pi hook still
  reports it.

### Hook sidecar state

Hook sidecar records are optional, local observations written by
harness hooks through `conspectus hook write`. They refine
session-to-mux attribution when a harness can report the current
session id without terminal input. Each observation lands in a
minimal latest-only spool at `hooks-latest.json`, one record per tmux
pane (so agents in split panes of one session keep their own), which
every refresh replays. With `conspectus serve` running, the hook writer sends the
observation to the daemon, which writes it to the spool and updates the
graph at once; with no daemon, the hook writer writes the spool itself.
It waits at most 2 seconds for the daemon to answer, so a stuck daemon
delays a session start by that much, and then writes the spool itself.
The spool lives under:

1. `$CONSPECTUS_HOOK_SIDECAR_STATE`, when set.
2. `$XDG_STATE_HOME/conspectus/hooks`.
3. `$HOME/.local/state/conspectus/hooks`.

The records are not project intent and should not be committed. A
record counts for its pane until a newer one from that pane replaces it,
however old it is; it is ignored once its agent process has exited, when
its tmux session was created after it, or when the session's active
pane now runs a different harness. The spool keeps at most 256 records, dropping the
oldest. Older per-event JSON records in the same directory remain
readable as a compatibility path.

Install a harness hook automatically:

```sh
conspectus hook init claude-code   # 14 hook entries in ~/.claude/settings.json
conspectus hook init codex         # 11 hook entries in ~/.codex/config.toml
conspectus hook init pi            # <agent-dir>/extensions/conspectus-hook.ts
```

`--scope project` installs into the current project instead
(`.claude/`, `.codex/`, `.pi/extensions/`). The commands are idempotent.
A settings file that is a symlink, as a dotfile manager leaves it, is
written through to its target and keeps its mode; a read-only one is
refused.
`conspectus hook status <harness>` reports whether the hook is present
and ends with a [live-state health line](#live-state-health), and
`conspectus hook remove <harness>` removes only what Conspectus
installed. The opencode hook is a plugin; see
[`plugins/opencode-hook/README.md`](../plugins/opencode-hook/README.md).

Manual Claude Code hook setup puts the same handler under each event
listed in [Claude Code live state](#claude-code-live-state), with the
matcher `hook init` uses where it uses one; for session start:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "startup|resume|clear|compact|fork",
        "hooks": [
          {
            "type": "command",
            "command": "conspectus hook write claude-code",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

Place that in `~/.claude/settings.json` or a local Claude Code
settings file. The subcommand reads Claude's hook JSON from stdin,
records `session_id`, `transcript_path`, `cwd`, and the tmux session
and pane the hook runs in (`TMUX_PANE`'s) when available, then exits
without writing to the transcript. Only a session
start also writes the attribution record.

`hook write` never disturbs the harness that runs it. It prints nothing
on stdout or stderr and exits 0 even when the payload is empty or
unreadable, the harness name is unknown, tmux is missing, or a store
can't be written, so a broken hook shows up as missing attribution, not
as an error in the harness. Only a malformed command line (no harness
name, an unknown flag) prints a usage message and exits 2, which Claude
Code treats as a block; `hook init` always installs a well-formed one.
To check a hook by hand, pipe a payload in and look for the records:

```sh
echo '{"session_id":"test","cwd":"/tmp","hook_event_name":"SessionStart"}' |
  conspectus hook write claude-code --state-root /tmp/hook-check
ls /tmp/hook-check   # hooks-latest.json, live-state.json, and its lock
```

`--state-root PATH` writes both files under `PATH` and never asks the
daemon.

Pi has no hook settings, so its hook is a TypeScript extension that
`hook init pi` writes as one file (ADR 0121). Restart Pi or run
`/reload` to load it; Pi asks you to trust a project before it loads
project extensions. The extension reports each interactive session
start, including `/new`, `/resume`, `/fork`, and `/clone`, and what the
session is doing (see [Pi live state](#pi-live-state)) through
`conspectus hook write pi`, and skips `--no-session` runs and Pi's
print, JSON, and RPC modes. Besides `installed` and `not-installed`,
`hook status pi` reports `outdated` (an older copy) and `foreign` (a
file at that path Conspectus didn't write, which `init` and `remove`
leave alone). See [`plugins/pi-hook/README.md`](../plugins/pi-hook/README.md).

## Live agent state

Live state says what each running agent is doing now, in three buckets:

- **Waiting on you**: a permission prompt, a question, a plan to approve,
  or other input the agent asked for.
- **Ready for a prompt**: the turn is over.
- **Working**: the model is generating, a tool is running, a request is
  being retried, or the context is being compacted.

The same hooks and plugins that say which session runs in which pane
([Hook sidecar state](#hook-sidecar-state)) report it, so it is opt-in: a
harness reports nothing until you turn it on, and a session without live
state is one Conspectus has no evidence about, not an idle one.
Conspectus only watches. It never answers a prompt or types into an
agent's pane; to act on a waiting agent, attach to its mux as usual.

Live state shows in the TUI (a mark after the age on session and mux
rows, a count of waiting agents on group headers and in the header line,
a word on pinned sessions, and fields in the detail pane; see
[Live state in the TUI](#live-state-in-the-tui)), in `conspectus table`,
`conspectus node show`, and the graph JSON.

### Turning live state on

| Harness | Turn it on | Then |
| --- | --- | --- |
| Claude Code | `conspectus hook init claude-code` | Restart agents that were already running. Hooks run in a folder only once you accept Claude Code's workspace trust dialog there. |
| Codex | `conspectus hook init codex` | Open `/hooks` in Codex and trust the Conspectus entries, and again after any `hook init` that changed them. Restart agents that were already running. |
| opencode | Build the plugin, version 0.3.0 or later, from the checkout your `conspectus` came from: `npm install && npm run build` in `plugins/opencode-hook` | Restart opencode. On 2.x, list `dist/v2` in `opencode.jsonc` and restart `opencode serve --service` too (see [OpenCode live state](#opencode-live-state)). |
| Pi | `conspectus hook init pi` | Restart Pi or run `/reload`. Waiting needs Pi 0.84.4 or later; 1.1.0 or later is recommended. |

`--scope project` installs for the current project only (`.claude/`,
`.codex/`, `.pi/extensions/`), and `hook status` checks the scope you
name. After upgrading Conspectus, run the same command again: `hook init`
brings an older install up to date in place and otherwise changes
nothing. For opencode, rebuild the plugin with every new `conspectus`,
since an older binary treats each report as an attribution record, and
an older plugin reports less. Nothing in this repository's CI builds the
plugins, so check a build with `npm run typecheck` (after `npm install`)
in `plugins/opencode-hook`. For Pi, `hook status pi` says `outdated` for
an older extension; one older than version 2 reports which session a
pane runs but no live state.

Then run `conspectus hook status <harness>` and read its last line
([Live-state health](#live-state-health)): `receiving` once an agent has
done something.

### What each state means

| Bucket | State | Detail | Meaning |
| --- | --- | --- | --- |
| Waiting on you | `waiting` | `permission`, `question`, `plan`, `input` | Blocked on you: a permission prompt, a question, a plan to approve, or other input, such as a form a tool asks you to fill in. |
| Ready for a prompt | `ready` | `finished`, `interrupted`, none | The turn ended, by itself or by an interrupt. No detail means a session with no turn yet, or one the harness called idle without saying how its turn ended. |
| Working | `working` | `generating`, `tool`, `retrying`, `rate_limited` | A turn runs: the model generates, a tool runs, a request is retried, or a retry waits out a rate limit. |
| Working | `compacting` | none | The harness is compacting the context. |
| Other | `error` | `rate_limited`, `auth`, `overloaded`, `failed` | The turn ended on an error: a rate limit, a sign-in or credential problem, an overloaded provider, or anything else. The agent takes a prompt again. |
| Other | `exited` | `ended`, `killed` | The session is over. `ended`: it quit, or its process moved to a new session (`/clear`, `/new`, a resume). `killed`: its process is gone without a word, or its pid now belongs to another process. |
| Other | `unknown` | `stale`, none | No usable evidence. `stale`: the process can't be checked and nothing has arrived for fifteen minutes. No detail: the source knows the session but not its state. |
| Other | `starting` | none | The process is up and has said nothing more. No current hook reports it. |

Around the state:

- **`+N`**: how many subagents run: Claude Code's and Codex's
  subagents, opencode's child sessions, or Pi's subagent tool calls.
- **Via a subagent** (`via_subagent` in JSON): the wait is a child
  agent's, shown on its root session.
- **Since**: when the state began. A wait shows when the wait began; when
  it ends, the session shows its own state again with the start it had,
  so a turn does not look new because a dialog came and went.
- **Quiet**: see below.

### How a state ages

A recorded state is read against the clock and the agent's process each
time you look:

- **A gone process wins.** When the agent's process has exited, or its
  pid now belongs to another process, the session reads `exited:killed`,
  whatever it said last.
- **Waiting, ready, and error do not age** while the process runs. A
  prompt you leave for hours still reads `waiting`.
- **Working never turns into ready; it turns quiet.** A working or
  compacting session that has said nothing for a while is marked `quiet`:
  as far as anyone knows it is still at work (a forty-minute build is real
  work), but there is no news. The window is ten minutes for Claude Code
  and Codex, whose hooks speak only when something happens, and three
  missed reports (three minutes) for opencode and Pi, whose plugins repeat
  theirs every minute while a session is busy.
- **The 15-minute rule.** When the process can't be checked (no pid was
  recorded, or the system has no `/proc`), every state but `exited` reads
  `unknown:stale` once nothing has arrived for fifteen minutes.
- **No entry is no evidence.** A session without live state shows `—`
  in `conspectus table` and `·` in the TUI, never `ready`.

Every window counts from the latest event, so a repeated report keeps a
state fresh. A recorded `exited` stays as it is.

### Live state in the TUI

Every session row and mux row has one cell right after its age that says
what the agent is doing, in every view that lists them (Sessions, the
agents under a mux or a PR, and Search). It is always filled, and each
state has its own shape, so it reads without color:

| Mark | Means | Color |
| --- | --- | --- |
| `!` | Waiting on you: a permission prompt, a question, a plan, other input | yellow |
| `»` | Working, compacting, or starting | green |
| `›` | Ready for a prompt: the turn is over | green |
| `×` | The turn ended on an error | red |
| `?` | Can't tell: a process that can't be checked has said nothing for fifteen minutes | dim |
| `·` | No live state: the harness's hooks are off, or the session has ended | dim |

- **Quiet** work (no news past the quiet window) draws the same `»`
  dimmed.
- **A mux** shows the state of its agents that needs you first: waiting,
  then error, then ready, then working, then the rest. Under a mux with
  several agents, each agent's row shows its own mark.
- **Group headers** count the agents beneath them that wait on you, as
  `! 2` after the `(N)` session count, each agent once however many rows
  list it. In the Mux view a header counts every agent of its muxes,
  listed as a row or not, so it can show `! 1` with no `(N)`. Pinned and
  History buckets show no counts.
- **The header line** shows the same `! 2` when agents anywhere wait on
  you: every session and every pane-only agent, whatever the view lists
  or the filter hides, so one glance says whether anything needs you. It
  sits before `▲ N`, and on a narrow screen it outlasts `▲ N` and the
  stopped count.
- **Subagents** show as a dim `+N` right after a session's label while
  its agent runs any (`+9+` for ten or more), on every session row,
  nested ones included, but not on mux rows. A session that has exited
  or can't be told shows none, as in `conspectus table`. A wait raised by
  a subagent reads `!` on its root session.
- **Pinned** spells it as a word: a pinned session (or one listed under a
  pinned mux) whose agent is waiting, working, ready, or failed reads
  `waiting`, `working`, `ready`, or `error` instead of `in mux` or
  `resumable`. Can't-tell, ended, and no-state sessions keep `in mux` or
  `resumable`, and the detail pane still says which mux a session runs
  in.
- **The detail pane** spells it out. A session with live state lists,
  after `status`: `state`, its row's mark and then the state and detail
  in words (`! waiting · permission`, `› ready · finished`, `? unknown ·
  stale`); `since`, when that state began (`5m ago`); `source`, what
  reported it (`hook`); `evidence`, `fresh`, `quiet`, or `stale`, the
  last two dimmed along with the state's words; and `subagents` (`2
  active`) while the row shows `+N`. A mux lists `agents`, its mark and
  its agents' counts, most urgent first (`! 1 waiting · 2 working`), and
  `pane_agents`, the agents in its panes that no session matches yet
  (`%3 working · generating`). Moving the cursor onto a field previews
  what its words mean. A node without live state lists none of them.
  The fields change with the row's mark.
- **Nothing animates.** The spinner still means a refresh is pending, and
  takes the link cell, not the live mark, which keeps its value until the
  agent reports again.
- The colors are the theme's `warning`, `info`, `success`, and `error`
  colors and its `muted` look, which
  [`[tui.theme]`](#tuitheme--palette-overrides-adr-0032-adr-0153) sets.
  `info` and `success` are both green by default; set `info` to tell
  working from ready by color too (it also colors Info messages).
- `?` in the TUI lists the marks under "Agent state".

A `·` on every row means no harness reports live state yet: turn it on as
in [Turning live state on](#turning-live-state-on). The TUI picks up a new
state within about a second, with or without `conspectus serve`.

### Where live state shows

An entry describes one session. It shows on the session Conspectus
discovered for it or, when there is none yet (a new session with no
transcript), on the tmux pane it runs in, counted on that pane's mux. An
entry that matches neither shows nowhere, and live state never adds a
session.

- **`conspectus table sessions --columns +live`** and **`conspectus table
  mux --columns +live`** add a `live` column: see
  [Live state columns](#live-state-columns).
- **`conspectus node show <id>`** prints a `live state` section for a
  session: the state and detail, `via: a subagent` when a child agent
  raised it, the recorded state when the rules above replaced it
  (`recorded: working:tool` under `state: exited`), when it began
  (`since`, UTC) and how long ago (`age`), the last event, the confidence
  (`fresh`, `quiet`, or `stale`), running subagents, the source, and the
  process with whether it ran when last checked. A mux gets its counts
  per state and, for each of its panes whose agent no session matches
  yet, that agent's fields. A node without live state has no section.
- **`conspectus graph --format json`** lists the bound entries under
  `node_live`, one per session, and `pane_live`, one per pane whose agent
  no session matches yet (with the mux's `node_id` and a `pane` such as
  `%2`). Each holds the state as recorded and `liveness` (`alive`,
  `dead`, or `unknown`) from the process check made when it was bound, so
  a script applies the rules above itself, as `table` and `node show` do:

  ```json
  "node_live": [
    {
      "node_id": { "type": "agent_session", "harness_key": "codex",
                   "state_scope": "/home/me/.codex", "session_key": "session-a" },
      "state": "waiting", "detail": "permission", "subagents": 1,
      "since_ms": 1790768940000, "observed_ms": 1790769000000,
      "source": "hook", "pid": 4242, "pid_start": 123456, "liveness": "alive"
    }
  ]
  ```

  A field at its default is left out: no `detail`, no `subagents`, no
  `pid`, `via_subagent` false, `heartbeat_s` (seconds between a plugin's
  repeated reports) 0. Each list is left out when it is empty.
- **`conspectus hook status <harness>`** ends with the
  [health line](#live-state-health).

With `conspectus serve` running, these commands read its snapshot, which
binds the store again every few seconds; `--refresh` binds the store as
it is now.

### Known gaps

Hooks report what the harness tells them, and every harness leaves gaps:

| Gap | Where | What you see | What repairs it |
| --- | --- | --- | --- |
| An Esc interrupt fires no stop event | Claude Code | `working`, then quiet after ten minutes | The next turn event: your next prompt, or Claude Code's idle notification |
| Approving a prompt fires nothing until the approved tool ends | Claude Code, Codex | `waiting` through the whole of a long approved tool | The next tool event, when that tool ends |
| A killed process says nothing | Every harness | The last state, until the next process check | The process check: `exited:killed` at the next discovery (every few seconds with `conspectus serve`) |
| Hooks do nothing until trusted | Claude Code (the workspace trust dialog; also `disableAllHooks`, `--bare`, managed hooks only), Codex (`/hooks`) | No live state; `hook status` says `silent` while the agent runs | Trust the hooks (or turn them back on), then restart the agent |
| Waiting covers only extension dialogs | Pi | Pi's own pickers, a provider login, and the project trust prompt never read `waiting` | Nothing: Pi tells extensions nothing about them |
| Older harness versions | opencode 1.x, Pi before 0.84.4 | An interrupted opencode 1.x turn reads `ready:finished`; a Pi session never reads `waiting` | Upgrading the harness |

Each harness's own gaps are in its section below.

### Claude Code live state

`hook init claude-code` installs one entry for each Claude Code hook
event that says whether a session is working, waiting for you, or ready:
`SessionStart`, `UserPromptSubmit`, `PermissionRequest`, `PreToolUse`
(only `AskUserQuestion` and `ExitPlanMode`), `PostToolUse`,
`PostToolUseFailure`, `Notification` (only the dialog, idle, and
usage-limit types), `SubagentStart`, `SubagentStop`, `Stop`,
`StopFailure`, `PreCompact`, `PostCompact`, and `SessionEnd`, each with a
5 s timeout. An older install that covered only session start is
upgraded in place when you run `conspectus hook init claude-code` again;
nothing changes until you do. `--command` sets the command every
Conspectus entry runs.

The install edits `settings.json` in place: it adds or changes only
Conspectus's entries, in the file's own indentation and line ending, and
leaves every other byte as you wrote it, your key order and your own
hook groups included, so a dotfiles diff shows only the new entries.
`hook remove claude-code` cuts out exactly what the install added and
gives back the file you had. A file that isn't plain JSON (a comment, a
trailing comma) is refused rather than rewritten, as is one that names
the same key twice in an object the install has to change.

`conspectus hook status claude-code` prints `installed`, `partial`, or
`not-installed` for the file, then one line per event (`installed`,
`outdated`, or `missing`, followed by the event name), and says on
stderr how to complete a partial install. `hook remove claude-code`
takes out every Conspectus entry and nothing else.

Each event updates the session's entry in the
[store](#the-live-state-store), and a subagent's events count on its
parent's session. Beyond the [known gaps](#known-gaps):

- Denying a permission prompt reads `waiting` until the next prompt.
- A `Stop` that another hook blocks reads `ready` while the continuation
  that hook forces runs.

### Codex live state

`hook init codex` installs one entry for each Codex hook event that says
whether a session is working, waiting for you, or ready:
`SessionStart`, `UserPromptSubmit`, `PermissionRequest`, `PostToolUse`,
`Stop`, `Interrupt`, `SessionEnd`, `SubagentStart`, `SubagentStop`,
`PreCompact`, and `PostCompact`, each its own group with no matcher and a
5 s timeout:

```toml
[[hooks.Stop]]
hooks = [{ type = "command", command = "conspectus hook write codex", timeout = 5 }]
```

Recent Codex versions skip a hook they don't manage until you trust it, so
after `conspectus hook init codex`, open `/hooks` in Codex and review the
Conspectus entries. Do it again after any `hook init` that changed them:
the trust Codex records covers each entry's command, matcher, and timeout,
and its place in the list. Conspectus never writes that trust itself, and never touches
`hooks.state` (where Codex keeps it) or `notify`.

The install edits `$CODEX_HOME/config.toml` (default `~/.codex/config.toml`)
in place: comments, layout, and your own hooks stay as they were, in
whichever form you wrote them (`[[hooks.<Event>]]` tables, lists under
`[hooks]` or in dotted keys, or an inline `hooks = { ... }`). Conspectus's
groups go after yours, so an install never moves one of your entries. An
older session-start-only install is upgraded in place when you run
`conspectus hook init codex` again; nothing changes until you do. A
`hooks` value or event list of the wrong type stops the install with an
error instead of being replaced. If a `hooks.json` sits beside
`config.toml`, Codex reads both; `hook init` says so, installs in
`config.toml` anyway, and never edits `hooks.json`.

`conspectus hook status codex` prints `installed`, `partial`, or
`not-installed` for the file and one line per event, as for Claude Code.
`hook remove codex` takes out every Conspectus entry and nothing else,
wherever it sits; when that moves one of your entries up a place, it
warns that Codex may ask you to review that entry again.

Events update the session's entry as for Claude Code, and a subagent's
events count on its parent's session. Beyond the
[known gaps](#known-gaps):

- Declining a prompt reads `waiting` until a later tool, the end of the
  turn, or an interrupt.
- MCP elicitations and sign-in prompts fire no hook, so a session waiting
  on one reads `working` (quiet after ten minutes).
- A turn that ends in plan mode reads `ready`, whether or not it proposed
  a plan you still have to accept.
- With hooks turned off or managed hooks only, the hooks report nothing,
  as before you trust them.

### OpenCode live state

The opencode plugin (`plugins/opencode-hook`, 0.3.0 or later) reports
what each session is doing: working (generating, retrying, rate
limited), waiting for a permission, a question, or other input, ready,
compacting, failed, and how many subagents it runs. It works on opencode
1.x and 2.x. There is no `hook init opencode`: rebuild the plugin from
the same checkout as your `conspectus` (`npm install && npm run build` in
`plugins/opencode-hook`), since an older binary treats every report as an
attribution record, then restart opencode.

- **1.x**: the existing install (`opencode plugin <path>`) is enough;
  the plugin runs in each TUI.
- **2.x**: list `dist/v2` in `opencode.jsonc` as `{"package":
  "<checkout>/plugins/opencode-hook/dist/v2"}` instead of in `cli.json`,
  and restart the `opencode serve --service` process too. That loads the
  plugin's server entrypoint in the shared service, which sees every
  session, as well as its TUI entrypoint in each client. Listed only in
  `cli.json`, the plugin still says which session each pane shows but
  reports no live state.

See [`plugins/opencode-hook/README.md`](../plugins/opencode-hook/README.md)
for the details. The plugin sends a session's whole state whenever it
changes and every minute while the session is not ready; each report
replaces the session's entry in the store. A subagent's permission or
question also shows on its root session, and only a root session's
lifecycle events still go to attribution, so a pane no longer names a
running subagent. Beyond the [known gaps](#known-gaps):

- opencode 1.x publishes nothing for an Esc interrupt, so the turn reads
  `ready` (finished), not interrupted; 2.x says which. Not every 1.x
  failure publishes an error either, so a failed 1.x turn may read
  `ready`.
- On 2.x the process Conspectus checks is the shared service's, so
  closing a client leaves its sessions as the service runs them.
- When opencode reloads the plugin without restarting (1.x instance
  disposal, a 2.x plugin unload), sessions still working or waiting read
  `exited` until their next event.
- `hook status` does not cover opencode yet; check with `conspectus table
  sessions --columns +live`.

### Pi live state

The Pi extension (version 2, which `conspectus hook init pi` installs)
reports what its session is doing: working (generating, running a tool,
retrying, rate limited), waiting for you, ready (finished or
interrupted), compacting, failed (rate limited, auth, overloaded, or
otherwise), ended, and how many subagent tool calls run. An older copy
reports only which session a pane runs; re-run `conspectus hook init pi`
to upgrade it, then restart Pi or run `/reload`. `hook status pi` says
when the installed copy is older, and that waiting needs Pi 0.84.4 or
later (1.1.0 or later is recommended).

The extension keeps the session's state inside Pi and sends all of it
when what the session shows changes, and every minute while it is not
ready; each report replaces the session's entry in the store. A change
between working sub-states waits half a second, so a burst of tool calls
costs one report. The report carries Pi's own pid, which Conspectus uses
instead of looking for a process named `pi`, for live state and for pane
attribution. The extension sorts an error into rate limited, auth,
overloaded, or failed itself, and sends only the class. Beyond the
[known gaps](#known-gaps):

- Pi asks for nothing on its own: it has no approval prompt, so
  `waiting` means a dialog an extension opened (a permission gate, a
  question tool, plan mode's approval). A `confirm` reads as a permission
  and a `select`, `input`, or `editor` as a question, so a gate that asks
  with a select, like Pi's example, reads as a question. Custom dialogs
  never read as waiting, since Pi reports loaders and widgets the same
  way.
- A provider login and Pi's own pickers (`/model`, `/resume`, settings)
  are invisible to extensions, and the project trust prompt comes before
  the session starts, so none of them reads as waiting.
- Before Pi 0.84.4 the dialog events don't exist and a session never
  reads as waiting; before 1.1.0 an interrupted run is told only by its
  last message's stop reason.
- Quitting, `/new`, `/resume`, and `/fork` end the session they leave at
  once.
- After `/reload` the extension reports the session afresh, so a
  finished turn reads `ready` without its detail.

### The live-state store

Every report lands in one file,
`$XDG_STATE_HOME/conspectus/live-state.json` (or
`$HOME/.local/state/conspectus/live-state.json`), with `live-state.lock`
beside it. `hook write --state-root PATH` writes under `PATH` instead
(see [Hook sidecar state](#hook-sidecar-state)).

It holds one entry per harness and session: the state and its detail,
when the state began and when the latest event came, the running
subagents and whether the state is a subagent's, the source, the agent's
pid and that process's start time, how often a plugin repeats its report,
the working directory and transcript path, the tmux socket, session, and
pane, a plugin report's instance and sequence number, and, for hooks that
report events, the ids of pending waits and running subagents and the
state to return to after compaction.

It never holds a prompt, a message, a tool's input, output, or name, a
title, a question or a form's fields, a permission's patterns or
resources, an error's text, or an agent's type. Each writer reads the harness's payload in
memory against a fixed list of fields and drops the rest, and the tests
plant a marker in every text field of every payload and check that none
reaches the file.

The file is a rebuildable observation, like the hook spool: local to the
machine, never part of a project, and safe to delete (sessions read `—`
until their next event). Entries that exited more than an hour ago, and
entries whose process died and that have not changed for a day, are
dropped, and the file keeps at most 512. A missing, unreadable, or
newer-format file means no live state, never an error.

Writing it never slows or stops the agent. `hook write` prints nothing,
always exits 0, and waits at most 200 ms for the lock before it drops the
event. It talks to a running daemon only for an attribution record, such
as a session start's, and gives up on it after 2 s.

### Live-state health

The last line of `conspectus hook status <harness>` says whether the
harness's hooks feed live state:
`<health>\t<harness>\t<live-state file>\t<message>`. It goes by what
Conspectus already has: the entries in `live-state.json`, whether the
hooks are installed in the scope you asked about, and the harness's agent
processes in the graph the last discovery saved (`graph.bin`, which a
running `conspectus serve` rewrites every few seconds), each checked
against the running processes. It starts no process and asks no daemon.

| Health | Meaning |
|---|---|
| `receiving` | The file has entries for the harness: `live state: 2 sessions, newest event 12s ago`. |
| `silent` | The hooks are installed and one of the harness's agents runs, but no event has arrived: the hooks may be untrusted, disabled, or the agent was started before they were installed. Restart the agent after `hook init`, and accept the workspace trust prompt (for Codex, trust the entries in `/hooks`). |
| `no-events` | The hooks are installed, nothing has arrived, and no running agent of the harness is known. Start one, or run a command that discovers, such as `conspectus table sessions`, so the saved graph lists it. |
| `off` | The hooks are not installed in this scope, or for Pi the installed extension is too old to report live state; the message names the `conspectus hook init` command that installs them. |
| `unsupported` | The harness's hook reports sessions but no live state. Claude Code, Codex, and Pi all report it now; the line stays for harnesses added later. |

### Troubleshooting live state

Start from the health label:

- **`off`**: run the `conspectus hook init` command the message names, with
  the same `--scope` you checked; an install in one scope does not count
  in the other. For Pi, `outdated` on the line above means an extension
  too old to report live state: run `conspectus hook init pi` and restart
  Pi or run `/reload`.
- **`no-events`**: nothing has arrived and no agent of the harness is
  known to run. Start one and give it a prompt. If one already runs, the
  saved graph has not seen it: run `conspectus table sessions` (or keep
  `conspectus serve` running) and check again, which turns the label
  into `silent` or `receiving`.
- **`silent`**: an agent runs but nothing arrives.
  - It was started before the hooks were installed: restart it.
  - Claude Code: accept the workspace trust dialog in that folder, and
    check that `disableAllHooks`, `--bare`, or a managed-hooks-only policy
    is not turning hooks off.
  - Codex: trust the Conspectus entries in `/hooks`, again after any
    `hook init` that changed them.
  - Claude Code with `CLAUDE_CONFIG_DIR` set reads its settings from
    that directory, but `hook init claude-code` writes
    `~/.claude/settings.json`. Install with `--scope project` in each
    project, or copy the Conspectus entries into that directory's
    `settings.json`.
  - The lines above the health line say `partial` or `outdated`: run
    `conspectus hook init <harness>` again.
  - Claude Code and Codex: the command the entries run no longer
    exists, as when the install recorded a path a package upgrade
    removed. Run `conspectus hook init <harness> --command "conspectus
    hook write <harness>"` so they run `conspectus` from your `PATH`. The
    opencode plugin and the Pi extension run `$CONSPECTUS_HOOK_BIN`, else
    `conspectus` from the `PATH` the harness started with.
  - To test the writer itself, pipe it a payload with `--state-root`, as
    [Hook sidecar state](#hook-sidecar-state) shows.
- **`receiving`, but a session shows `—`**: the file has entries, but
  none is bound to that session. The entry may be on the session's tmux
  mux instead, when Conspectus has no session for it yet (look at
  `conspectus table mux --columns +live`), or Conspectus may not discover
  the session at all, as for Codex sessions under a `CODEX_HOME` other
  than `~/.codex` (see
  [Harness state-root overrides](#harness-state-root-overrides)). An agent
  outside tmux with no discovered session shows nowhere. `receiving` also
  counts entries kept after their session ended; the newest event's age
  says how recent the latest is.
- **`receiving`, but the state looks wrong**: check the
  [known gaps](#known-gaps) and the harness's own section first.
  `exited:killed` for an agent that runs means its recorded process is
  gone (a session resumed in a new process, say), and its next event
  corrects it. `unknown:stale` means Conspectus
  can't check the process and has heard nothing for fifteen minutes. An
  answer a few seconds old comes from `conspectus serve`'s snapshot; pass
  `--refresh` to read the store as it is now.
- **`unsupported`**: the harness's hook reports no live state. No harness
  says this today. opencode has no health line yet; check it with
  `conspectus table sessions --columns +live`.

## Configuration File

See ADR 0012 for the layout and precedence rules. Briefly:

1. **Project-local**: Conspectus walks upward from the current
   directory looking for `.conspectus.toml`, stopping at `$HOME` or
   the filesystem root. The walk is skipped when the current directory
   no longer exists (ADR 0111).
2. **User-level**: `$XDG_CONFIG_HOME/conspectus/config.toml`, falling
   back to `$HOME/.config/conspectus/config.toml`.

Project values win over user values, which win over built-in
defaults. Both files are TOML and entirely optional.

The same files also hold user-authored intent: `[declared]` links
(ADR 0014) and `[[aliases]]` (ADR 0029). Pins live in their own
file (see [Pins, history, and rebuild](#pins-history-and-rebuild)).
The tables below are the configuration keys.

```toml
[table.sessions]            # also mux, union, prs, forks (ADR 0021)
columns = ["default", "+preview"]   # same tokens as `--columns`

[tui]
default_view = "sessions"   # sessions | mux | prs | pinned | history | search
scan_roots = ["~/work"]     # tui and serve, when --scan-root is absent; `~` expands
show_harness_chips = false  # per-harness count chips in the header
narrow_layout_threshold = 100  # columns below which the panes stack
preview_wrap = "smart"      # smart | plain | none — how the Preview fits a mux pane
session_preview = "transcript"  # transcript | pane — the Preview for a session in tmux
# sessions_grouping = "graph"  # deprecated alias for [tui.views.sessions] grouping

[tui.views.sessions]        # also mux and prs (ADR 0031)
grouping = "graph"
filters = { harness = ["codex"], max_age = "7d", mux_state = ["attached"] }

[tui.detail]
show_edge_meta = false      # provenance · confidence · state on link rows

[server.intervals]          # conspectus serve cadence; also the CLI's warm-start TTL (ADR 0079)
harness = "5s"
mux = "5s"
git = "30s"
forge = "1h"                # gh pr list per remote; Ctrl-R in the TUI asks now (ADR 0116)

[forge]
concurrency = 4             # gh calls at once (ADR 0136)

[worktree]                  # see docs/worktrees.md
backend = "auto"            # auto | git | worktrunk
teardown_confirm = "live"   # always | live | never
teardown_grace = "3s"
```

- **Grouping values per view:** sessions `graph` (default), `workspace`,
  `repo`, `checkout`, `scan-root`, `none`; mux `session`, `host`,
  `repo`, `workspace` (ADR 0139); prs `checkout` (default), `repo`, `state`
  (ADR 0114); pinned `project` (default), `kind`, `none` (ADR 0141).
- **Removed views:** the `union` and `forks` views no longer exist.
  `--view union` and `--view forks` are rejected with the list of valid
  views, and a `default_view` of either, or a `[tui.views.union]` or
  `[tui.views.forks]` table, is reported at startup and ignored. The
  `table union` and `table forks` row-types are unchanged.
- **Filters:** `filters` takes one inline table or an array of tables
  (`[[tui.views.<name>.filters]]`); entries OR together per dimension.
  `max_age` accepts `s`, `m`, `h`, and `d` suffixes; `mux_state` values are
  `attached`, `ambiguous`, and `unmuxed`. For the PRs view, `pr_branch` takes
  `tracked` and/or `untracked` to keep only PRs with or without a local branch
  (both, or neither, keeps every PR), `pr_state` takes `open` (drafts
  included), `draft`, `merged`, and/or `closed` to keep only PRs in those
  states, and `float_tracked_prs_top = true` lists PRs with a local branch
  first (ADR 0129, 0130, 0134).
- **Durations** in `[server.intervals]` and `teardown_grace` use
  `<integer><ms|s|m|h>`.
- **Forge cadence (ADR 0116, 0136):** each GitHub remote's PR listing is
  fetched in the background, by the daemon's forge class or, without a
  daemon, by the TUI, and reused until it is older than `forge`. A failed
  fetch retries after at most five minutes. Nothing else waits on a forge:
  routine refreshes, `r`, and the refresh after a tmux detach build PRs
  from the cached listings. `Ctrl-R` is the explicit opt-in: it drops the
  listings, fetches them for you, and reloads PR previews.
  `conspectus refresh --class forge` (or a plain `conspectus refresh`) does
  the same from a shell. One-shot commands without a daemon (`graph`,
  `table prs`) fetch, since they have nothing cached.
- **Refreshes see what you just did.** The mux listing is otherwise reused
  for a few seconds. `conspectus refresh --class mux` (and a plain
  `refresh`), `r`, the refresh after any action that starts, stops, or
  renames a mux, and the refresh after you detach from or kill a mux all
  ask the mux server again first, and a running daemon does not reply until
  the re-probe is published, so the next read of its snapshot has the new
  state. After a detach the TUI re-probes up to twice more, 150ms apart,
  while the listing still shows a mux you returned from that it could not
  confirm is still running (see below); after you switch back to the TUI's
  pane inside tmux, it re-probes while the set of running muxes is exactly
  as it was. Timer refreshes still reuse the cached listing. Forges are
  never re-fetched by these refreshes, and their data is kept. Inside
  tmux, attaching switches your tmux client to the mux and returns at
  once, so the TUI also watches whether its own pane is on screen and
  refreshes the moment you switch back; a mux you exited while away is
  already a stopped mux when you return.
- **Rows after a detach.** When you come back from a mux, its row and the
  rows of the agent sessions in it dim, show a spinner in the attach
  column, and refuse attach, resume, rename, and the other actions on
  them (`refreshing after tmux · wait for the row to update`), since the
  mux may have ended while you were in it. Moving, filtering, previews,
  and every other row stay live. The spinner clears, and the rows act
  again, as soon as tmux answers that the mux is still the same session
  (its session id and creation time), which is almost at once after a
  plain detach; the refresh behind it then updates their values. If the
  mux ended, was replaced by a new session with its name, or tmux can't
  say, the rows wait for the refresh, which shows what became of it.
  A mux a rebuild ran in, or one recorded without a session id, also waits
  for the refresh. Each mux clears on its own: attaching another mux right
  away never keeps the first one spinning.
- **Forge concurrency:** `[forge] concurrency` (default 4) sets how many
  `gh` calls run at once. Higher fetches faster but can trip a forge's
  rate limits.
- **View precedence at startup:** `--view`, then the view you last used
  (unless `--no-resume-view`), then `[tui] default_view`, then
  `sessions`.
- **Each view reopens where you left it:** the groups you had expanded,
  and the row you had selected, scrolled into view with the groups above
  it open. A row that is gone gives way to the nearest group above it
  that is still listed, then the first row. The Search view starts
  fresh; `--no-resume-view` opens every view on its default folds. See
  [TUI state](#tui-state).
- **Preview wrap (ADR 0106):** `smart` wraps content but truncates
  rules, box borders, and padding that would spill onto extra rows;
  `plain` wraps every line; `none` keeps tmux's layout at the pane's
  width and clips what doesn't fit. The controls overlay (`f`) has a
  "Preview wrap" section; the last pick is remembered across runs and
  takes precedence over the config value.

Unknown sections and unknown keys are ignored. Malformed TOML
surfaces as a `ConfigDiagnostic` on stderr but does not abort the
run. The legacy `[session]` section (pre-ADR 0021) is recognized
solely to emit a one-line diagnostic pointing at the new schema; its
contents are ignored.

### `[tui.theme]` — palette overrides (ADR 0032, ADR 0153)

`conspectus tui` draws every color and modifier from one theme, and
each entry can be overridden in config. The block below lists every key
with its default; the tests check it against the code, so it is the
complete list. Set only the keys you want to change.

```toml
[tui.theme]
# Shared looks, used by every view, menu, form, and viewer.
accent             = "cyan"            # popup borders and titles, help keys, the active tab, chosen chips, hints
panel_focus_accent = "cyan"            # the focused pane's border and ► marker; follows `accent` unless set
disclosure         = "cyan"            # the ▶ / ▼ arrows in the tree and on the right pane's Other header
muted              = "dim"             # hints, placeholders, notes, rules, footers, the unlinked ∘ (a modifier)
inert              = "dark_gray"       # short ids, previews, gone or stopped rows (a color)
selection_active   = "reversed,bold"   # the cursor row of the list that has focus: a pane, menu, form, or picker
selection_inactive = "bold"            # the cursor row of a list without focus, such as the other pane
success            = "green"           # confirmations: toasts, the status line, Messages; the ready agent mark
warning            = "yellow"          # refusals and fallbacks, in the same three places; the waiting agent mark
error              = "red"             # failures, in the same three places; the failed agent mark
info               = "green"           # hints and notes, in the same three places; the working agent mark
link               = "blue"            # mux names and link rows
match_highlight    = "yellow"          # text a search matched (bold, underlined)
badge              = "reversed,bold"   # the chip look
badge_width        = 8                 # label cells in every chip (an integer, at least 2)
recency_fresh      = "bright_green,bold"
recency_active     = "green"
recency_recent     = "yellow"
recency_cold       = "dim"
surface            = "default"         # the background of modals and toasts
toast_text         = "white"           # a toast's text (its border takes the severity's color)
field              = "light_gray"      # a form field's value
field_active       = "white on dark_gray"  # the focused form field
field_cursor       = "black on white"  # the text cursor in a form field

# Conspectus's own roles.
harness_unknown    = "white"           # the badge of a harness with no entry below
mux_attached       = "green"           # the linked or attached ⊸ and its counts
cwd_mark           = "cyan"            # the (cwd) launch-context mark
command_badge      = "black"           # background of the program chip a mux row shows
command_badge_text = "white"           # its text, drawn bold
pr_open            = "green"           # also a PR glyph drawn without a state
pr_closed          = "red"
pr_merged          = "magenta"
pr_draft           = "yellow"
node_workspace     = "bright_blue"     # the kind glyph of each node in the tree and detail pane
node_repo          = "blue"
node_checkout      = "cyan"
node_agent_session = "bright_green"
node_mux_session   = "magenta"         # pins share it
node_runtime_process = "dark_gray"
node_branch        = "green"
node_fork          = "bright_magenta"
edge_alt_of        = "dark_gray"       # a non-winning alternative link row
edge_conflict      = "yellow"          # a conflicting link row
transcript_compaction = "magenta"      # a compaction summary's chip in the transcript viewer
pin_placeholder    = "bright_yellow"   # no longer drawn; kept so older files load

[tui.theme.harness]
# Badge and chip color per harness, by harness key.
claude-code = "magenta"
codex       = "cyan"
opencode    = "green"
aider       = "red"
pi          = "bright_blue"

[tui.theme.icons]
# One-cell kind glyphs; see the glyph table in docs/tui-columns.md.
node_workspace       = "▦"
node_repo            = "◆"
node_checkout        = "◇"
node_agent_session   = "●"
node_mux_session     = "▣"
node_pin             = "◉"
node_runtime_process = "◈"
node_branch          = "⌥"
node_fork            = "⋎"
node_forge_pr        = "⇄"
```

Each value is a single string (except `badge_width`):

- a named ANSI color: `red`, `magenta`, `cyan`, `bright_green`,
  `dark_gray`, `light_gray`, … plus `default` (or `reset`) for the
  terminal's own color;
- an indexed 256-color: `ansi256:42` (0–255);
- a truecolor hex: `#ff8800` (case-insensitive);
- a comma-joined modifier list: `bold`, `dim,italic`, `reversed`,
  `crossed_out`, …;
- for the keys whose default has both, colors then modifiers:
  `green,bold`, `#ff8800,italic`, and a background with `on`:
  `white on dark_gray`, `on blue`, `black on white,bold`.

A key whose default is a color takes a color, one whose default is a
modifier list takes modifiers, and `recency_*` and `field*` take either
or both.

**Older keys.** These keys from earlier versions still load. Each sets
the key it was merged into:

| Older key | Sets | Note |
| --- | --- | --- |
| `placeholder` | `muted` | |
| `divider` | `muted` | rules drew `dim` like `placeholder` |
| `mux_unmuxed` | `muted` | the unlinked `∘` |
| `secondary_text` | `inert` | |
| `ghost` | `inert` | gone rows stay italic |
| `link_id` | `link` | |
| `panel_focus_accent` | `accent` | still sets the focused pane's border and marker too |
| `harness_claude` | `[tui.theme.harness] claude-code` | |
| `harness_codex` | `[tui.theme.harness] codex` | |
| `harness_opencode` | `[tui.theme.harness] opencode` | |
| `harness_aider` | `[tui.theme.harness] aider` | |
| `harness_pi` | `[tui.theme.harness] pi` | |

Within one file, a key's own name wins over its older names, and among
older names the one higher in the table wins: `muted` over `placeholder`
over `divider` over `mux_unmuxed`, and `inert` over `secondary_text` over
`ghost`. An older key that loses to a different value gets a warning
naming the key that won. Because five keys became two, setting only
`placeholder` now also restyles the rules and the unlinked `∘`, and
setting only `secondary_text` also recolors gone rows. `accent` sets
`panel_focus_accent` as well unless the same file sets it, and
`panel_focus_accent` alone still sets both, as it always did. A project
`.conspectus.toml` overrides the user config whichever name either uses.

Unknown keys and malformed values both surface as `ConfigDiagnostic`
warnings; the entry keeps its built-in default and the rest of
`[tui.theme]` continues to load. The TUI never aborts on theme
errors. The `[table]` palette (ADR 0022) is **not** affected by
`[tui.theme]` — table CLI output keeps its own colors.

## CLI Surface

`csp` is the official short name for `conspectus` and is installed with it
(ADR 0118). Every command below works under either name. Help and suggested
follow-up commands use the name you ran, while log prefixes and the version
string always say `conspectus`.

```sh
conspectus graph --format {json|dot|html} [--scan-root PATH]...
                                          [--candidates {include|exclude}]
                                          [--diagnostic-nodes {include|exclude}]
                                          [--explain]
conspectus table {sessions|mux|union|prs|forks} [--layout {columnar|card}]
                                                 [--wide | --width N]
                                                 [--columns LIST]
                                                 [--scan-root PATH]...
                                                 [--pager | --no-pager]
                                                 [--color {auto|always|never}]
conspectus node show <id> [--scan-root PATH]...
                          [--pager | --no-pager]
                          [--color {auto|always|never}]
conspectus columns {sessions|mux|union|prs|forks}
                   [--pager | --no-pager]
                   [--color {auto|always|never}]
conspectus tui [--view {sessions|mux|prs|pinned|history|search}] [--grouping VALUE] [--sort {hierarchy|recency}]
               [--harness H]... [--max-age D] [--mux-state S]...
               [--scan-root PATH]... [--refresh-interval D] [--no-live-preview]
               [--contains TEXT]
conspectus declared {list|create|remove|confirm|ignore|override} ...
conspectus rename session <id> [NAME] [--clear] [--no-mux]
conspectus rename mux <id> <NAME>
conspectus alias list
conspectus pin {add|edit|rename|retarget|refresh|move|rm|list|show|migrate} ...
conspectus rebuild <REF> [--dry-run] [--no-attach] [--session ID]
conspectus history {list|show|reap|forget} ...
conspectus search <QUERY> [--source {sessions|panes|all}] [--group {session|message|flat}]
                         [--tools] [--thinking] [--harness H]...
                         [--limit N] [--format {table|json}]
conspectus mux new <NAME> [--cwd PATH] [--socket NAME] [--no-attach] [--pin [PIN] [--no-launch]]
conspectus mux launch <HARNESS> --name <NAME> [--cwd PATH] [--argv ARG...]
                                              [--worktree-branch B --worktree-repo PATH]
                                              [--pin [PIN] [--no-launch]]
conspectus worktree {list|new|rm|merge|close|prune} ...
conspectus hook {init|status|remove} <claude-code|codex|pi> [--scope {user|project}]
conspectus hook write <HARNESS>          # invoked by the harness hook itself
conspectus serve [--scan-root PATH]...
conspectus refresh [--class {git|mux|harness|forge}]
conspectus status [--format {human|json}]
```

Running `conspectus` with no subcommand opens the TUI. Every command's
`--help` lists its full flag set.

`tui` and `serve` scan the `--scan-root` flags, else `[tui] scan_roots`,
else the launch directory, so the daemon's snapshot covers the same roots
the TUI would scan on its own. The one-shot commands scan their flags or
the current directory.

`graph`, `table`, `node show`, and `tui` also take `--refresh` (ignore a
running `conspectus serve` and rebuild in-process) and `--no-cache`
(don't write the rebuilt graph to `graph.bin`).

Worktree operations (`list` is read-only; the rest delegate to a
mutation backend) are covered end to end in `docs/worktrees.md`.

Debug/test builds also include hidden developer scenario commands under
`conspectus dev scenario ...`. They materialize isolated replay worlds for
manual graph/table/TUI inspection and are documented in
`docs/dev-scenarios.md`. Release builds do not expose this surface.

The row-type (`sessions`, `mux`, `union`, `prs`, `forks`) is a required positional;
there is no implicit default. Width detection: when stdout is a TTY
the table truncates to the detected terminal width; pipes default to
wide so `conspectus table sessions | grep` remains useful. `--wide`
forces untruncated output even on a TTY, and `--width N` pins an
exact width for reproducible captures. `--layout card` renders one
column per line per row with blank-line separators, useful when the
columnar form would truncate (long `CWD`, long PR identifier).

`--columns LIST` selects which columns to render. `LIST` is
comma-separated; each token is:

- `default` — the row-type's registered default set.
- `all` — every registered column for the row-type.
- `+name` — add to the running set.
- `-name` — remove from the running set. Use the equals form
  (`--columns=-name,...`) so the shell does not interpret a leading
  dash as a flag.
- `name` — explicit-list mode: clears the running set on the first
  bare token, then appends.

Unknown column names error with the registered names for the
row-type listed. `[table.<rows>].columns` in `.conspectus.toml` /
user config provides a fixed default column list; CLI `--columns`
overrides config when both are present.

`conspectus columns <ROWS>` prints every registered column for the
row-type along with its one-line description, marking each column
in the default set with `(default)`. Useful when you don't remember
exact column names or want to see which columns are opt-in.

The `title` column (registered on `sessions` and `union`, opt-in)
surfaces `AgentSessionNode.title` — opencode's chat topic, claude-code's
compaction summary — separately from the `AGENT` cell. As of CSP-174
the `AGENT` cell always renders `harness:<session_key>` (UUIDs
collapse to `…<last-8>`; shorter human-readable session keys pass
through verbatim) regardless of whether the adapter set `title`.

The `preview` column (registered on `sessions`, `mux`, and `union`)
shows a one-line snippet of the agent session's most recent
user/assistant text message, mimicking Claude Code's `/resume` view.
The snippet is sourced from `AgentSessionNode.last_message_preview`
in the resolved graph (see ADR 0023 — capped at 200 chars,
whitespace-normalized) rather than re-read at render time. The
column is opt-in (default off) for two reasons: previews can
surface user-typed text that you may not want in a default-on
status table, and the 200-char cap is wider than the other defaults
fit alongside in a typical terminal. Opt in with `--columns
+preview` or `[table.<rows>].columns = [..., "preview"]`. On the
`mux` row-type the cell shows the first attached agent's preview,
which keeps the cell scannable when several agents share a mux. On
the `union` row-type the preview shows for agent rows only; mux
rows render `—`.

### Live state columns

The `live` column (registered on `sessions` and `mux`, opt-in like
`activity`) shows what each agent is doing now, from the live state its
hooks report (see [Live agent state](#live-agent-state)).
Opt in with `--columns +live` or `[table.<rows>].columns = [..., "live"]`.
The state is read when the command runs, so a long silence and a gone
process show as they would in the TUI.

On `sessions` the cell is the state and its detail, `+N` for running
subagents, `quiet` when a working agent has said nothing for longer than
its quiet window (ten minutes for hooks), and how long ago the state
began:

```text
AGENT                LIVE
-------------------  ----------------------------
claude-code:builder  working:generating quiet 12m
claude-code:gone     exited:killed 4m
claude-code:lost     unknown:stale 20m
claude-code:unheard  —
claude-code:waiter   waiting:permission +1 3m
```

`exited:killed` is a session whose process is gone, and `unknown:stale`
one whose process can't be checked and that has been silent for more
than fifteen minutes; for those two the age counts from the last event.
`—` is a session with no live state, which is no evidence, not idle.

On `mux` the cell counts the agents the mux hosts in each state, most
urgent first (`1 waiting, 1 working, 1 unknown, 1 exited`), including
each agent in its panes that no session matches yet; `—` when none has
live state.

## Searching session contents

`conspectus search <QUERY>` finds sessions by what was said in them. It
reads every session's transcript on demand, in the command's own process
(the daemon is not asked), and keeps nothing: results exist only while
the command runs. The session list comes from the same discovery as
`conspectus table` and honors `--scan-root`, `--no-cache`, and
`--refresh`.

Matching is one case-insensitive substring of the trimmed query. ASCII
letters fold; other text must match exactly (`CAFÉ` does not match
`Café`). By default only user and assistant messages (and compaction
summaries) are searched; `--tools` adds tool calls and output, and
`--thinking` adds the model's thinking blocks. Aborted and hidden turns
are left out. The `[search]` section of the config sets the default
grouping and kinds.

```sh
conspectus search "retry budget"                 # one row per session, most hits first
conspectus search "retry budget" --group message # one row per matching message
conspectus search "retry budget" --group flat    # matching messages, newest first
conspectus search deadlock --tools --harness claude-code --limit 5
conspectus search deadlock --format json | jq '.results[].session'
```

Flags:

- `--source sessions|panes|all`: search session transcripts (the
  default), the scrollback of live tmux panes, or both. See
  [Searching pane scrollback](#searching-pane-scrollback).
- `--group session|message|flat`: one result per session (ranked by hit
  count, then latest hit), per matching message in turn order within each
  session, or per matching message across all sessions, newest first,
  with no session grouping.
- `--tools`, `--thinking`: also search tool calls and output, or thinking.
- `--harness H` (repeatable), `--max-age DURATION`, `--mux-state STATE`:
  narrow the sessions searched, with the same meaning as on `table`.
- `--limit N`: stop after N results (at least 1).
- `--format table|json`; `--width N` and `--color auto|always|never`
  shape the table (the match is bold on a terminal).
- `--verbose`: report how many sessions were searched on stderr even
  when stderr is not a terminal, how many were skipped because their
  transcript could not be read, and how many belong to a harness with no
  transcript parser (and so were not searched).

**Table.** `--group session` prints `SESSION`, `HARNESS`, `HITS`,
`LATEST` (how long ago the newest hit was) and `SNIPPET`; `--group
message` and `--group flat` print `SESSION`, `HARNESS`, `TURN`, `ROLE`, `KIND`, `TIME` (how
long ago that turn was written) and `SNIPPET`. An unnamed session shows
the first eight characters of its id. Turns count from 1, as in the
viewer; roles read `you` and `ai`, and kinds read `message`, `tool
call`, `tool output`, and so on, as in the TUI. Per-message rows run in
turn order within a session (`flat` rows run newest first across all
sessions, so the `SESSION` column says where each one lives). The snippet is cut to fit `--width`
the way the TUI cuts it: the match stays, two thirds of the room it
leaves goes to the text after it, cuts fall on word boundaries (a blank,
or beside a CJK or other two-cell character) where they can, and a cut
side ends in `…`. A side whose only word is wider than its room shows
nothing rather than part of a word, unless that word is a token: a path,
a file name, a URL, a code name with `_` or `::`, a run over 24 cells
(a hash), or CJK text shows the part of it that fits. Every character of the snippet is kept,
so the table reads as the JSON's `snippet.text`. When nothing matches, the
table is empty and stderr says `No sessions match "<query>"`.

A session with more matching messages than a search keeps (the newest
are kept) is reported on stderr, and under `truncated` in the JSON. With
`--group flat`, `--limit` keeps the newest messages overall; when it cut
some, stderr says `showing the newest N of M matching messages` and the
JSON gains `limited` (`shown`, `total`). `limited` is absent otherwise.

**Exit status** follows `grep`: `0` when at least one session matched,
`1` when none did, `2` on an error (an empty query, an unreadable
config value, discovery failing). Errors are printed to stderr and never
use status 1, so scripts can tell "nothing found" from "it broke".

**JSON.** `--format json` prints one document:

```json
{
  "query": "retry budget",
  "group": "session",
  "kinds": ["messages"],
  "searched": { "sessions": 77, "total": 77, "skipped": 0, "without_parser": 3 },
  "truncated": [],
  "results": [
    {
      "source": "session",
      "harness": "claude-code",
      "session": "aaaaaaaa-0000-0000-0000-000000000001",
      "label": "Retry budget design",
      "hits": 4,
      "latest": "2026-10-01T10:00:05Z",
      "snippet": { "text": "We settled on a retry budget of 3", "highlight": [22, 34] }
    }
  ]
}
```

With `--group message` or `--group flat` each result instead carries `turn` (the turn's
position, counting from 1), `role` (`user`, `assistant`, `system`), `kind` (`message`,
`compaction_summary`, `tool_use`, `tool_result`, `thinking`), `time`, and
its own `snippet`, and has no `hits` or `latest`. `label` is the session's
alias, title, or hosting mux name, or `null`. `snippet.highlight` is the
byte range (not characters) of the match inside `snippet.text`, which is
capped at 160 terminal cells (a `…` on each cut side included), favors the
text after the match, and has its whitespace collapsed. `truncated` lists the sessions
(`harness`, `session`, `matches`, `shown`) whose per-message results were
capped. `searched.skipped` counts sessions whose
transcript could not be read; `without_parser` counts sessions of
harnesses that cannot be searched yet.

### Revealing a row and going back

In the Search, History, and Pinned views, `Shift+Enter` (also `Alt+Enter`,
and `O` where a terminal cannot tell Shift+Enter from Enter) does what
**Reveal in its view** does for a pin: it switches to the view where the
row's session, mux, or PR lives (Sessions, Mux, or PRs; History for one
that is gone) and selects it. `Backspace` with the left pane focused then
goes back to the view and row you left. The back list holds the last 20
jumps: reveals, opening a search result in another view, and `.` menu
reveals; the status bar shows `Backspace back` while it is not empty. A
row that has since disappeared says `that row is gone`. With nothing to
return from, the left pane's `Backspace` says so; it leaves the right
pane's drill alone. With the right pane focused `Backspace` (or `Esc`)
backs the explorer out of its drill; see
[Keys on the right pane](#keys-on-the-right-pane).

Most terminals send `Shift+Enter` as plain `Enter`; Conspectus does not
turn on the kitty keyboard protocol, so it sees `Shift+Enter` only from a
terminal configured to report it. `Alt+Enter` works in every common
terminal. `Enter` itself means `Enter` with no modifier on either pane,
so `Ctrl+Enter` does nothing.

### Searching pane scrollback

`--source panes` (or `all`) also searches what is on screen and in
scrollback in every pane of every tmux server Conspectus knows: the
default server and each named socket a known mux lives on. Each pane's
last 5,000 lines are read with `tmux capture-pane`, joined where tmux
soft-wrapped them, with terminal escapes removed and trailing blank lines
dropped. Nothing is kept; the command reads panes read-only. With
`CONSPECTUS_DISABLE_TMUX` set no panes are searched.

A pane that hosts an agent shows that agent's session, so the same
sentence would otherwise be reported twice. A pane line is left out when
its text (case folded, whitespace collapsed, box-drawing characters
removed) is contained in the text of any turn of the session attributed to
that pane's mux that matches the query, of any kind, so text the session
search withholds by default (tool output) is not reported through the pane
either. A line shorter than 12 characters is compared joined with its
neighbors. `--verbose` reports how many lines were left out.

Pane results are listed after session results. `--group session` gives one
row per pane (`PANE`, `COMMAND`, `HITS`, `CAPTURED`, `SNIPPET`), showing
its newest match; `--group message` and `flat` give one row per matching
line, with `LINE` in place of `HITS`. A pane has no message times, so
`CAPTURED` is when the pane was read. In JSON, every result carries
`"source": "session"` or `"pane"`; pane results have `backend`, `socket`,
`mux`, `window`, `pane`, `pane_id`, `command`, `hits`, `line`, `captured`,
and `snippet`, and `searched.panes` reports `scanned`, `total`, `skipped`,
and `deduplicated`. A pane that cannot be read counts as skipped.

### Quick select with `/`

`/` opens quick select, a wide modal (about 85% of the terminal, up to 200
columns) for jumping to something by name. Its title carries a tab
identifier, `[ Current ] All` or `Current [ All ]`; `Tab` and `Shift-Tab`
switch between the two, and the modal always opens on Current.

- **Current** ranks every row of the active view after filters, rows under
  collapsed groups included. `Enter` selects the row in the left pane and
  expands the groups above it.
- **All** ranks everything Conspectus knows by a name you would
  recognize, whichever view it lives in: sessions (alias, title, id,
  directory, and the last message when live preview is on), muxes (name,
  directory, running command), repos, checkouts and worktrees (path,
  branch, remote), pull requests (number, title, branch), pins, workspaces,
  forks, and the gone entities history remembers. Matches in the name rank
  above matches in other fields; ties go to sessions, muxes, pins,
  checkouts, repos, pull requests, then history, and the most recent first.

Each result in All shows its kind glyph, name, a dim context (path, branch,
or what a pin points at), and a word at the right edge saying what `Enter`
will do:

- `here`: the current view lists it. It is selected in the left pane and
  focus stays where it is.
- `via <kind>` or `3 hops`: it is related to what the right pane is on
  (the selected row, or wherever you have drilled to) within three hops of
  relationships Conspectus has settled on. Focus moves to the right pane,
  which drills along the shortest path and leaves the cursor on the
  result, so `Backspace` retraces the way. `via mux` names the first
  neighbor on the path.
- `in <View> view`: otherwise the view that lists it is shown and the row
  selected.
- `not listed`: nothing the right pane is on leads to it, and no view
  lists it as you have the views set up, so `Enter` leaves you where you
  are and a message says why. No view lists a fork; a repo or checkout
  appears in Sessions only as the group of its sessions, so one with no
  sessions is not there; and a row a view's filter hides is not listed
  in that view.

The keys are on the modal's last row: `Up`/`Down` (or `Ctrl-P`/`Ctrl-N`)
move, `Enter` picks, `Tab` switches Current and All, `Esc` closes.
`PgUp`/`PgDn` move by the results shown, stopping at the first and the
last. Typing
keeps the highlight on the same position while it is still a result. A
list longer than the modal scrolls with a scrollbar beside it, moving only
as far as the highlight needs.

### Searching contents with `Ctrl-/`

`Ctrl-/` opens the contents modal: it searches what was said in the
sessions and what is in mux pane scrollback. Terminals send that key as
one of three keys; Conspectus accepts all of them (`Ctrl-/`, `Ctrl-7`,
`Ctrl-_`), and `s` works in every terminal. The modal uses most of the
terminal's width (85%, between 60 and 200 columns). The top border shows
`Current [ All ]`; `Tab` switches. *Current* is the
sessions and muxes the view you are in lists, folded rows included (the
Search view's current context is the one its shown search was made in);
*All* is every session and live mux. It opens on All, so a fresh search
reads everything; `Tab` narrows it to the view. It always opens with an
empty query; `Ctrl-R` brings back earlier search strings. The modal shows
the query, a status line (grouping, the kinds searched, how many places are
scanned, the skipped count), the best results so far, and a footer naming
the keys it understands. Each row says where the match is (`session` or
`pane`), and each snippet is cropped around the match as the `conspectus
search` table crops it (on word boundaries, keeping text on both sides of
the match when there is room), with the match picked out in the theme's
`match_highlight` color. The keys:

- Type to search. The scan starts when the query has rested for about
  200 ms and restarts whenever the query, the reach, or an option changes.
  Results arrive while it runs.
- The title shows two choices. `Tab` switches the reach between
  **Current** and **All**; `Shift-Tab` switches the granularity between
  **Sessions** (one row per session or pane) and **Messages** (every match,
  newest first). Changing the granularity lists the same matches another
  way and never searches again; the starting value comes from `[search]`.
- `Ctrl-R` fills the field from the previous search string (newest
  first, each with the reach and options it ran with) and searches it;
  press it again for the one before, and `Ctrl-T` goes forward again, back
  to the empty field. Paging stops at the oldest string and never records
  a search by itself: the one you land on joins the history on exit like
  any other, replacing its twin. A blank query is never remembered.
- `Ctrl-O` opens the search options in the modal: whether tool calls and
  output, thinking, and pane scrollback are searched (`Space` or `Enter`
  flips one, `Esc` goes back to the results). Changing one searches again.
  The tool and thinking options apply to sessions; the starting values
  come from `[search]`.
- `Up`/`Down` (or `Ctrl-P`/`Ctrl-N`) move through the results, and
  `PgUp`/`PgDn` by the results shown, stopping at the first and the last;
  a list longer than the modal scrolls with a scrollbar beside it, and the
  columns stay put as results arrive. `Enter` opens a session hit in the
  viewer at the match, with the query already searched, or a pane hit in
  the full-screen pane viewer on a fresh capture, at the line, with the
  match picked out (the pane's mux is selected in the Mux view first). The pane viewer has the session viewer's keys (`j`/`k`,
  `PgUp`/`PgDn`, `g`/`G`, `Ctrl-D`/`Ctrl-U`, `/` with `n`/`N`, `?` for help,
  `Esc` stepping back from help and find, `q` leaving at once) and `a`,
  which attaches to the pane's mux. A stopped or gone mux refuses with a
  note in the footer, and attaching closes the viewer. Long lines wrap
  under their line number; `w` cuts them at the edge instead (ending in
  `…`), and the footer's `wrap·on (w)` chip says which. A find, and the
  match the viewer opens on, puts its line one row below the top, its
  text in the current-match look and its number in the accent color.
  The `[ n/m ]` chip counts screen rows, so a wrapped line counts once
  per row it takes.
- `Ctrl-S` keeps the search and opens the Search view on it (a scan still
  running goes on there). `Ctrl-F` keeps the query as the filter on the
  Sessions, Mux, and PRs lists and closes the modal; `F` clears it.
- `Esc` closes the modal and stops a running scan. The lists are left as
  they were; if a content filter was active when the modal opened, it is
  restored. The Search view does not change while the modal is open: the
  search typed in the modal joins the remembered searches when the modal
  exits (by `Esc`, `Ctrl-S`, `Ctrl-F`, or `Enter` on a hit), as the newest
  one, marked `stopped` if its scan had not ended. A search that never
  started, or whose query was emptied, is not remembered, and neither is a
  repeat of one already remembered (the same text, kinds, scope, and pane
  setting), which takes that one's place instead.

A pane that runs an agent shows what the agent's own session already
contains only once: the pane lines found in the transcripts of the
sessions attached to its mux are left out. Panes with no such session, such
as a shell or a build, keep all their matches. Reading scrollback reads
the last 5,000 lines (2 MiB) of each pane, in memory only.

`s` and `Ctrl-/` from the Search view open on the search it shows, with its
results listed; the first change searches again as a new search.
Content search is off under `--no-live-preview`; the keys then say why and
do not open the modal.

## Paging

`conspectus table <ROWS>`, `conspectus columns <ROWS>`, and
`conspectus node show <id>` pipe their output through a pager when
stdout is a TTY (git-log style). Resolution order:

1. `$PAGER` (when set and non-empty; whitespace-split into program +
   args).
2. `less` — Conspectus sets `LESS=FRX` by default when `$LESS` is
   unset: `F` quits if the content fits on one screen so short
   tables print inline, `R` passes raw control characters through,
   `X` skips the screen init/deinit sequences.
3. `more`.
4. Direct print, if none of the above can spawn.

Non-TTY output (pipes, redirects) prints directly so
`conspectus table sessions | grep …` keeps working. `--no-pager`
disables paging even on a TTY; `--pager` forces paging even when
stdout is not a TTY (useful for `PAGER=cat` captures). `--pager` and
`--no-pager` conflict.

`conspectus graph --format json` and `conspectus declared list` do
not page; JSON output is machine-consumable and the declared listing
is short-lived tab-separated text. Pipe either through a pager
manually if needed. `conspectus graph --format dot` is also
machine-consumable (pipe through `dot -Tsvg` etc.) and `--format
html` writes a single self-contained `.html` file you redirect to
disk; see [`graph-visualization.md`](graph-visualization.md) for
the full guide.

## Color

`conspectus table <ROWS>`, `conspectus columns <ROWS>`, and
`conspectus node show <id>` emit ANSI color/styling when stdout
supports it. See ADR 0022 for the full palette; in short, headers
are bold, the `—` placeholder is a faded gray (256-color 244),
the short ID column is blue, the `agent` cell's harness prefix
gets a stable color per harness (`claude-code` bright yellow,
`codex` bright blue, `opencode` bright green, `aider` bright
red), provenance tiers are colored by indicator (`LD`/`GD`=green,
`SD`=cyan, `C`/`$`=dim), the ambiguity `*` and unresolved-lineage
`?` prefixes are yellow, PR states use `open`=green / `closed`=red
/ `merged`=magenta / draft yellow, and DECLARED cells use
`declared`=green / `ignored`=dim / `overridden`=yellow.

`--color {auto|always|never}` controls when ANSI is emitted.
Resolution rules (highest priority first):

1. `--color=never` → off.
2. `--color=always` → on (overrides every env signal below).
3. `NO_COLOR` set to a non-empty value → off
   (<https://no-color.org>; respected for `auto` only).
4. `CLICOLOR_FORCE` set to a non-zero value → on (BSD-style force).
5. `TERM=dumb` → off.
6. `CLICOLOR=0` → off.
7. Otherwise `auto`: color iff stdout is a TTY.

Pipes resolve to "no color" under `auto`, so
`conspectus table sessions | grep` keeps text plain unless you pass
`--color=always`. The pager respects ANSI (`less -R` is one of the
default flags), so paged output retains color.

`node show <id>` accepts any of:

- The short content-addressed prefix from any `conspectus table
  <ROWS>` projection's `ID` column. Any prefix length ≥ 4 hex chars
  is accepted; an ambiguous prefix errors with the matching
  candidates listed.
- The full `NodeId` display form, e.g.
  `agent_session:codex:/state:session-x` or `mux_session:tmux:editor`.
- The harness/mux label that appears in the `AGENT` column of
  `conspectus table sessions` (e.g. `codex:session-x`) or the `MUX`
  column of `conspectus table mux` (e.g. `tmux:editor`), when the
  label uniquely identifies one node.

The command prints the node itself plus every candidate link (outgoing
and incoming), resolved relationship, source metadata, and diagnostic
that touches the resolved node, and a `live state` section for a session
or mux with live state (see
[Where live state shows](#where-live-state-shows)). Resolved relationships include the
same resolver score breakdown exposed by `conspectus graph --explain`:
the selected link's score axes, competing links' score axes, and the
first axis that decided the winner when there is a competitor.

`conspectus graph --explain` keeps the normal graph JSON shape but
adds an `explanation` object to each resolved relationship. The field
is opt-in so existing JSON consumers do not pay for verbose resolver
internals unless they ask for them. The current score axes cover the
generic resolver ordering plus the specialized `linked_to_mux` and
`branch_has_forge_pr` comparators.

All commands run from the current working directory by default;
passing one or more `--scan-root` flags overrides that with explicit
roots. The working directory is optional (ADR 0111): if it's been
deleted, discovery runs without it and config comes from the user file
alone. The TUI and `serve` re-check it on every refresh, so deleting
the directory they were started in drops it rather than breaking them.
Commands that default a target to "here" (`mux new`, `mux launch`, the
`worktree` mutations, `hook --scope project`) say which flag to pass
instead.

## Renaming sessions

`conspectus rename session <ID> [<NAME>] [--no-mux] [--clear]` stores
an operator-chosen display name as an ADR 0029 alias overlay in the
nearest `.conspectus.toml` (or the user-level config for orphan
sessions). `<ID>` accepts the same forms as `node show`. By default a
single linked tmux session is renamed in lockstep; pass `--no-mux` to
skip that side, or `--clear` to drop the alias entirely.

`conspectus rename mux <ID> <NAME>` renames a tmux session without
writing any alias — mux ids are the native tmux name, so the rename
mutates the identity directly (per ADR 0029).

Both renames go through the mux's own backend. A backend that cannot
rename sessions (zellij today) is reported by name; `rename session`
still writes the alias.

`conspectus alias list [--store project|user|all] [--scan-root PATH]`
prints existing alias entries grouped by store. The output columns are
tab-separated: `<store>\t<path>\t<endpoint>\t<display_name>`.

In the TUI, capital `R` opens an inline rename prompt on the selected
agent-session row. `Enter` confirms, `Esc` cancels; lower-case `r`
continues to mean refresh. Confirming an empty name clears a session's
alias; a tmux session needs a name, so its prompt refuses an empty one
and says so. The status bar surfaces the result, and
when the target is a live session (mux indicator `Attached` or
`Ambiguous`) appends an informational advisory that the alias
overlays the harness title until the session ends.

## Pins, history, and rebuild

Three layers, kept apart (ADRs 0140–0142):

- the **graph** is what exists now, rebuilt by discovery every cycle;
- **history** is every rebuildable entity Conspectus has observed,
  gone ones included, with what it takes to rebuild them;
- **pins** are the entities you keep at hand, named and ordered.

### History

History lives in `$XDG_STATE_HOME/conspectus/history/` as one JSON file
per kind (`muxes.json`, `checkouts.json`, `repos.json`, `prs.json`)
plus a `history.lock`. It is an observation: the daemon, a daemonless
TUI, and one-shot commands all append what they saw fresh, under the
lock, and nothing is written for `--no-cache`, fixtures, or snapshot
mode. Records are small and carry a stable id (`h_` and 12 hex
digits). A mux record keeps its launch argv verbatim, which is the one
place Conspectus stores a command line.

A record's presence is **live** (the default), **gone**, or
**unknown**, decided by a targeted check and never by absence from a
scan. A mux is gone when a successful probe of its server didn't list
it, a worktree or repo when its path is missing under a parent that
exists. If the check couldn't run (the binary is missing, the probe
failed, an unmounted drive), the record is unknown, and unknown records
are never counted as gone or reaped.

```sh
conspectus history list [--kind {mux|checkout|repo|pr}] [--gone]
conspectus history show <h_...>
conspectus history reap [--kind K] [--older-than 30d] [--dry-run]
conspectus history forget <h_...> [--force]
```

`reap` removes gone records past a per-kind age. The defaults are 14
days for muxes, 30 for checkouts, and 30 for PRs (counted from when they
left the listing); repos have no default and go only through `forget`.
A record a pin targets is never reaped, and `forget` refuses one
unless you pass `--force`. Reaping is always operator-initiated and only
removes records; it never touches tmux, git, or the filesystem.

```toml
[history.ignore]            # globs that are never recorded
mux = ["scratch*"]          # matched on the mux name
checkout = ["*/tmp/*"]      # checkout and repo match the path
pr = ["acme/legacy#*"]      # pr matches owner/repo#number

[history.reap_after]        # "never" keeps a kind until `history forget`
mux = "14d"
checkout = "30d"
pr = "30d"
repo = "never"
```

Content search reads transcripts on demand and keeps nothing on disk.
`[search]` sets what it reads and how it groups results. A project's
`.conspectus.toml` overrides the user config key by key:

```toml
[search]
group = "session"           # the TUI's granularity: "session" lists sessions; "message" or "flat" lists the matching messages, newest first (`conspectus search --group` still tells the three apart)
kinds = ["messages"]        # add "tools" and/or "thinking" to search those too
history = 10                # content searches the Search view remembers (0 keeps only the one it shows)
```

`kinds` lists what is searched, so `kinds = ["tools"]` searches tool
calls and output only. Aborted turns and turns the harness never showed
are not searched.

Launch profiles say how to start a harness when nothing more specific
does. A project's `.conspectus.toml` replaces the user config's profile
for the same harness, and a wrapper prefix is fine:

```toml
[launch.claude-code]
argv = ["claude", "--dangerously-skip-permissions"]

[launch.codex]
argv = ["nix", "develop", "-c", "codex"]
```

### Pins

A pin is a table in `$XDG_DATA_HOME/conspectus/pins.toml`:

```toml
[pins]
schema_version = 2

[[pins.slots]]
id = "conspectus-main"
kind = "mux"
target = "history:h_7f3a9c0e21d4"
pinned_at = 2026-10-19T12:00:00Z

[pins.slots.overrides]
argv = ["claude", "--dangerously-skip-permissions"]
```

The kind is `mux`, `session`, `checkout`, `repo`, `branch`, `pr`,
`workspace`, or `fork`. The target is `history:<id>` (muxes,
checkouts, repos), a node id (sessions, PRs, branches, workspaces,
forks), or absent for a **planned** pin that its first rebuild builds.
Each recipe field resolves `overrides > live graph > history > snapshot
> defaults`; `pin show` prints every field with the layer that supplied
it. The overridable fields are `name`, `socket`, `cwd`, `harness`,
`argv`, and `worktree_branch` for a mux, `resume_into` for a session,
`branch`, `base`, and `location` for a checkout, and `remote` and `path`
for a repo; `label` works on any pin. Pin order is the order of the
tables, and writes preserve comments and unknown keys.

A pin's status is `live`, `gone`, `unknown`, `missing` (the target is in
neither graph nor history, so the pin renders from its snapshot), or
`planned`.

```sh
conspectus pin add <mux name | node id | h_...> [--name NAME] [--set FIELD=VALUE]...
conspectus pin add --planned --kind KIND --name NAME [--set FIELD=VALUE]...
conspectus pin edit <name> [--set FIELD=VALUE]... [--unset FIELD]...
conspectus pin rename <name> <new-name> [--no-mux]
conspectus pin retarget <name> <target>     # same kind only
conspectus pin refresh <name>               # copy the current recipe into the snapshot
conspectus pin move <name> {--top|--bottom|--before N|--after N|--up|--down}
conspectus pin rm <name>                    # the entity and its history stay
conspectus pin list
conspectus pin show <name>
```

`KIND` is `mux`, `session`, `checkout` (also spelled `worktree`), `repo`,
`pr`, `branch`, `workspace`, or `fork`. `pin rename` on a live mux pin also
renames the tmux session and pins the `name` override; `--no-mux` renames
the pin alone. It checks the new name first: one another pin has, or one
that is not a usable pin name, fails with the reason and renames nothing,
so the session, the overrides, and `pins.toml` stay as they were.
`FIELD=VALUE` lists (such as `argv`) are split like a shell command line, as the launch form's `argv` is (see the launch form below), and one that
ends inside a quote is refused. Pinning a live mux, worktree, or repo first records it in
history so the pin has a stable record to point at. `rm` removes the
pin and nothing else. `pin add`, `edit`, `rename`, `retarget`,
`refresh`, `move`, `rm`, and `migrate` are the only commands that write
`pins.toml`; the daemon never does, and `graph`, `table`, `node show`,
`pin list`, and `pin show` leave it alone. The store, like the history
files, is created with mode 0600 (directories 0700), since pins and
records can hold launch command lines. A relative `$XDG_DATA_HOME` is
ignored in favor of `$HOME/.local/share`, so pins never land inside a
project tree.

The `pin create`, `launch`, `attach`, `bind`, `rebind`, and `adopt`
verbs from the earlier pin model are retired. They still parse, then
exit with a message naming the replacement.

### Rebuild

```sh
conspectus rebuild <pin | h_... | node id | mux name> [--dry-run [--assume-stopped] [--script]] [--no-attach] [--session ID]
```

A rebuild is a plan of steps. `--dry-run` prints it and stops, with the exact
command under each step, quoted for a shell; a real
run executes the steps in order and stops at the first failure, with no
rollback. Steps chain prerequisites: a mux whose worktree is gone
rebuilds the worktree first, which in turn clones a missing repo.

- A **gone mux** gets `tmux new-session` with its effective argv, and
  the last session it hosted is resumed after the harness binary
  (falling back to a fresh launch if the harness rejects the resume).
  `--session` resumes a different session.
- A **live mux whose agent exited** gets the built argv typed into its
  pane, but only when no harness is running there. Conspectus never
  types into a pane that has a live agent.
- A **session** resumes in the mux that last hosted it, or a new one
  when that mux is busy.
- A **linked worktree** comes back through `wt switch` (worktrunk must
  be on your `PATH`), creating a deleted branch from its last head.
- A **repo or primary checkout** is cloned from its recorded remote with
  `git clone`, only into a path that doesn't exist.

**Planning for something that is running.** A real rebuild leaves a running
mux alone (it only attaches). To see what it would take to recreate one from
what Conspectus knows now, add `--assume-stopped` (only with `--dry-run`): the
plan is built as if the mux, worktree, repo, or session's mux had stopped, from
its live command, directory, harness, and last session, and is marked
hypothetical. `--script` prints just the commands, with `#` comment lines for
the step titles, so you can paste them into a shell or pipe them to `sh` and
rebuild the state outside Conspectus. The commands come from the same builders
the rebuild runs, so what the plan shows is what would execute:

```sh
conspectus rebuild work --dry-run --assume-stopped --script --no-attach
# rebuild mux `work` (running; planned as if stopped)
# note: hypothetical: `work` is running; this is what a rebuild would do if it had stopped
# new session, resume e44d01cb
tmux new-session -d -s work -c /home/me/src/app claude --resume e44d01cb \; set-option -t work remain-on-exit failed
```

The launch argv is the first of: the pin's `argv` override, the recorded
argv, the project's `[launch.<harness>]` profile, the user's profile, an
`atelier exec <harness>` wrapper (when the mux's directory is inside an
Atelier workspace and `atelier` is on your `PATH`), and the harness
default. `rebuild --dry-run` notes when the argv comes from a profile or
from `atelier exec`; `mux launch` uses the same last four layers. After attaching, Conspectus watches the
resumed harness for its first moments and reports if it didn't survive
(ADR 0103). Muxes owned by an orchestrator such as agent-deck are shown
but not rebuilt.

### Renames and worktrees

`conspectus rename mux` keeps pins that name the renamed mux in
step; it refuses a gone mux. `conspectus worktree close` forgets the
gone history records rooted in the removed worktree, except records a
pin targets.

### The Pinned view

Key `4` (or `--view pinned`) lists every pin, whatever its kind, in one set
of columns: `<kind glyph> <pin> <status> <age> <content>`. The pin name is
cut at 28 cells, the status is one word (`live`, `stopped`, `removed`,
`unknown`, `planned`, `missing`, `in mux`, `resumable`, or a PR's state), the age is the
last activity of a live entity or the time a gone one was last seen, and the
content is the harness and place, the title, or the remote. Gone, missing,
planned, and unknown rows wear the ghost style (the theme's `inert` color, in italics). The agent or program that runs is a
filled chip, the same as in the other views, ahead of the text that follows
it. A pin with nothing running (`planned`, `stopped`, `removed`, `missing`, or
`unknown`) wears a hollow pin glyph, `◎`, in the `inert` color after its kind
glyph; the filled `◉` is for a pin on an entity that is running, and every
row that has a pin wears one of the two, in the same place in every view.
A mux that stopped reads `stopped` and a worktree or repo that vanished reads
`removed`; the word `gone` isn't used in the interface. Views of one kind of
thing (Sessions, Mux, PRs) show state as a glyph only; Pinned and History
mix kinds, so they spell it as a word. Columns pad to the widest
value in the whole tree, so folding a group doesn't move them. Changing the
grouping scrolls back to the top so every group is visible.

Pins group by `project` (the repo above the pin's cwd or path; the rest go
under Other), by `kind`, or not at all (`Ctrl-G` cycles), and sort in pin order or,
with the Recency sort, freshest first. A mux pin expands to the sessions it
hosts, and a checkout pin to its muxes and their sessions. The detail pane
shows the pin, its target, and each effective field with the layer that
supplied it (override, live, history, snapshot, or default).

`Enter` does what it does on the entity itself. A pin whose mux is live
attaches; a pin on a session opens the session viewer when the session isn't
in a mux, exactly as in the Sessions view, and attaches when it is. On a gone,
missing, or planned mux or session pin, `Enter` starts it and attaches, as a
launch always did: it runs `conspectus rebuild <pin>` behind a spinner (the
row dims and spins while it comes back), attaches, and when you return the
row is selected in the Mux view where the mux now lives. A gone worktree or
repo pin is rebuilt and you stay where you are. A PR pin shows its URL, and
the other live kinds say there is nothing to rebuild. While legacy
`[[pins.entries]]` remain, a line under the rows names `conspectus pin
migrate`.

`.` opens the pin menu, the same one every view offers for a pinned entity:
**Rebuild** (start it without attaching; only where there is something to
bring back), **Show rebuild plan** (the plan with its exact commands in a modal,
where `y` copies them as a script; for a running mux, worktree, repo, or
session it is the plan for the case that it had stopped, under a line saying
what `Enter` does; not offered for PRs),
**Edit…** (the edit form, below), **Rename…** (a live mux pin renames its tmux
session with it and pins the name, as `pin rename` does), **Retarget…** (a
searchable list of the other entities of the same kind, running or gone; type
to filter, `Enter` picks), **Refresh snapshot**, **Reveal in its view**
(switches to Sessions, Mux, or PRs and selects the entity; a gone entity
opens in History; offered only when that view lists the entity, so not
for a fork, a branch, or a pin with nothing to point at), and **Unpin**.
Each is the same write
as the `pin` command of that name. Legacy pins refuse edits until they are
migrated.

### Pinning from the other views

`p` pins the selected row in any view with one keypress, and unpins it at
once when a pin already points at it (`u` undoes either; see below). `.` opens the menu for the
row: **Pin** for an unpinned entity (`p` there pins too, as on the list), and
for a pinned one every pin verb above, whichever view you are in.

Accelerators act on what you are looking at. With the right pane focused,
`p`, `.`, `v`, `a`, `R`, `w`, and `X` act on the entity shown there (the
related session or mux the cursor is on, or the pane's own entity); `Tab` back
to the left pane and they act on the selected row again. `Enter` in the right
pane still drills into the link under the cursor. A runtime process can't be pinned.
Pinning a live mux, worktree, or repo records it in history first so the pin
has something to point at. The native views mark a pinned row with `◉` (or
`◎` when the pinned entity isn't running) after its kind glyph and leave it where it sorts; once any pin exists the marker
cell is reserved on every row so columns hold still. Controls has a **Pinned
only** checkbox in Sessions, Mux, and PRs that keeps pinned rows and the
groups above them.

**Content** in Controls (or `--contains TEXT` on `tui` and the list commands)
narrows the Sessions and Mux views to sessions whose transcripts
mention the text. The scan runs in this process on background threads,
streams matches as it goes, and restarts when the text changes; a refresh
scans only sessions that appeared since. The text is never saved, written to
a snapshot, or sent to the daemon. `--no-live-preview` turns it off.
Clearing filters in Controls clears it too.

**Edit…** opens a form with one row per field the pin can change: its name
first (renaming a live mux pin renames its tmux session), then the label and
the fields its kind can change. The rows use the short names the other forms
use: the field `pin edit --set` calls `worktree_branch` reads `branch`, and
`resume_into` reads `resume in`. A short note at the top explains the idea: a
blank field follows the running target, then its last seen value, then what
was saved with the pin; typing a value keeps one of your own with the pin.
Each row has the same three columns: the field, its value, and a word on the
right for where the value comes from (`running`, `last seen`, `saved`,
`default`, `unset`, or `kept` for a value you set), which changes to `will
keep` or `will follow` while an edit is pending. The two lines under the rows
describe the focused field: what it is following now, or what `Ctrl-R` would
go back to. Nothing moves when the selection does. `Ctrl-R` acts on these
field rows only: on a value you set it goes back to following (`will follow`
until you save), and on a field you only edited it puts the value it had
back. Paths complete with `Tab`, the harness row lists the known harnesses
(`←`/`→` choose one), and a harness's launch options, such as skipping
permissions, are checkboxes under it that add or remove their flags on the
`argv` row. `↑`/`↓` move between rows, `Enter` saves from any row, and `Esc`
cancels. A save the form refuses (a name another pin has, an `argv` that
ends inside a quote) says why on the line just above the keys and moves the
cursor to the field it is about.

The launch and new mux forms work the same way: `↑`/`↓` move between fields,
`Tab` and `S-Tab` visit the same fields, `Tab` completes a directory in a
path field first and moves on when there is nothing to complete,
`←`/`→`/`Space` choose among the harnesses, options, and toggles, and a line
shows the command the form will run. `Enter` launches or creates from
whichever field has the cursor. A choice of one among several, such as the
launch form's `pin` row, is drawn as `(•) chosen ( ) other`; a checkbox is
`[x]` or `[ ]`. On a terminal too short for the whole form, its fields
scroll to keep the focused one in view and its keys stay at the bottom.

Opened from a mux, both forms start with the next free name in that mux's
series: with `worker-1` selected the name reads `worker-2`, or the first
number after it that is free; zero padding keeps its width (`work-007`
gives `work-008`, `work-099` gives `work-100`), and a name without a
number gets `-2`, `-3`, and so on. A name is taken when a mux has it, live
or gone, when history recorded a mux under it, or when a pin has it. The
mux is the selected row's: a mux row (live or gone), a session's mux (the
one its row shows), or a mux pin; while the right pane has focus, the
entity it shows decides instead. From a repo, checkout, workspace, or PR
row, a session outside any mux, or nothing, the name starts empty. The
cursor is at the end of the name, so typing extends it and `Ctrl-U`
clears it.

Every path field (the forms' cwd, the launch form's worktree repo, and a
pin's path rows) asks for a directory. `Tab` adds the rest of the best
completion: directories from the graph come first (the selected row's, then
pins', live sessions', muxes', and so on), then the directories on disk that
start with what you typed. While the field is focused and has a completion,
`Tab: …` after it shows exactly what `Tab` would add; otherwise the path gets
the whole row. The mark after the path is `√` for an existing directory, `▲`
for a path that is missing or is not a directory (a file), and `?` for an
empty field; it is advice, and the form still takes the path. A leading `~`
is your home directory: the mark and the completions read it that way, and
the form saves or runs the expanded path.

`N`, or `m` then **Launch harness in new mux…**, opens the mux launch form
with the worktree toggle off (switch it on to create or reuse a worktree). The
mux's **name** is the first field. Under it, the **server** checkbox (`[ ] own
socket`) reveals a **socket** field beneath it, for a tmux server other than
the default; it is a field like the others, and `Space` (or `←`/`→`) toggles
it. With it off the launch uses the default server, and the socket text you
typed is kept if you switch it back on. The harness's launch options, such as
skipping permissions, are checkboxes that add or remove their flags in the
`argv`. The `command` row shows what will run; it is derived output, drawn dim
and italic and indented under a `└`, and never takes focus. An empty `argv`
resolves to the harness's default (a project or user `[launch.*]`
profile, `atelier exec`, or the harness default) and the form shows the
command and where it came from; the form decides that default when it opens
and again when you change the harness or the cwd. The last field, **pin**, can
also pin the new mux or, with *pin, don't launch*, write a planned pin and
start nothing.

An argv you type (the launch form's and the pin edit form's `argv`, and
`conspectus pin edit --set argv=…`) is split like a shell command line, with
nothing expanded:

- Spaces separate arguments.
- Single quotes keep everything between them as written: `'opus 4'` is one
  argument, and `'a\b'` keeps its backslash.
- Double quotes keep everything between them too, except that a backslash
  before `"`, `\`, `$`, or a backtick stands for that character: `"say
  \"hi\""` is `say "hi"`, while `"a\b"` is `a\b`.
- Outside quotes a backslash makes the next character plain: `x\ y` is one
  argument, `x y`.
- Pieces with nothing between them are one argument (`--name="two words"`),
  and `''` is an empty argument.
- `$HOME`, `~`, `*`, `;`, `|`, and backticks are passed as they are.

An argv that ends inside a quote or on a lone backslash is refused with the
reason (``mux launch: launch argv has an unclosed `'` quote``, `` `argv` has a
trailing escape ``) until you finish it. Toggling a launch option or changing
the harness works on what you have typed so far and writes it back finished.

### The Search view

Key `6` (or `--view search`) shows one of the content searches the process
remembers (the last ten; `[search] history` sets how many, 0 keeping only
the one shown). Every search from the contents modal is kept there as soon
as it starts and streams in while it runs, so a new search does not lose
the one before it. Nothing is saved to disk. The first line reads
`search 2/5 "query" · Current · 3 sessions · 1 pane · scanned 12m ago`;
`<` shows the previous (older) search and `>` the next, without scanning
again, and Controls has a `searches` row that lists them. `Ctrl-F` keeps
the search shown as the filter on the Sessions, Mux, and PRs lists (the
line then ends in `filtering`; paging does not change the filter, and `F`
clears it without forgetting any search). `s` and `Ctrl-/` open the modal
on the search shown; `/` is quick select over the rows.

The sessions and panes are ranked together by how many matches they hold
and then by the latest (a pane's latest is when its scrollback was read;
the Recency sort puts the latest first). Groupings (`Ctrl-G`): none,
project, harness (panes sit under **Mux panes**). `Ctrl-L`, or the Controls
`Results` section's `listed as` row, switches between **Sessions** (one row
per session or pane) and **Messages** (a flat list of matches, newest
first); `Ctrl-O`, or the Controls `options` row, opens the search options
(the modal's `Ctrl-O`). `[search] group` sets the default granularity
(`session` for Sessions; `message` and `flat` for Messages). The Messages list reads the
matching sessions' transcripts to find the turns, newest matches first and
at most 100 sessions; the find chip says `reading messages` while it loads
and `showing N of M messages` if that cap left some out. Per-message
turns are read again when a search is shown, not kept with it. Selecting a
hit row shows its turn above the session's detail. Hit, pane, and pane
line rows read as the contents modal's results do, without the `session`
or `pane` word: the harness chip (a pane's command) and the session's or
pane's name, then `turn N · kind`, `line N`, or `N hits`, the age, and
the snippet. A narrow pane lets the chip go first, then the age, then
narrows the name, so the snippet keeps 20 cells.

The right pane's Preview marks the search too. A session row's transcript
preview highlights the query and opens at the session's best match; a hit
row opens at its own turn, and scrolling moves from there. A pane's preview
highlights the query in the capture and scrolls back to the newest match
when none is in the bottom rows. The same marks show under a `Ctrl-F`
filter on the Sessions and Mux lists. They come from the search on display
(so `<` and `>` move them), are never saved, and go with the search.

`Enter` and `v` open the viewer at the best match of a session, or at the
matching turn of a hit row, with the query already searched, so `n` and
`N` step to the others; `Enter` on a pane row or pane line opens the
scrollback. Every other key, `p` and `.` included, acts on the session, or
on the pane's mux. Like all content search, queries and results stay in
memory and are never saved; the view is off under `--no-live-preview`, and
a restart never opens on it.

### The History view

Key `5` (or `--view history`) lists the history records whose entity is
gone, in the same columns as the Pinned view, newest sighting first. A
record is gone when a targeted check found it so: a mux a successful tmux
probe didn't list, a worktree or repo whose path is missing under an
existing parent, or a PR that left the listing once the forge has been read.
Unknown records aren't listed, because an unreadable probe proves nothing.
Rows group by `project` (the repo above the record's path; a worktree
belongs to the repo it was made from), by `kind`, or not at all. A record a
pin points at is already one keypress away in the Pinned view, so Controls
has an **Unpinned only** checkbox, checked by default; clear it to list
pinned records too (they carry `◎`). Revealing a gone pin's entity from the
Pinned view clears it for you.

`Enter` on a mux record runs `conspectus rebuild` behind a spinner, attaches to
the mux, and on return selects it in the Mux view (a gone worktree or repo is
rebuilt and focus moves to where it now lives). `.` opens the row menu:
**Pin** (the same as `pin add <record id>`; the full pin menu once a pin points
at it), **Rebuild** (without attaching), **Show rebuild plan** (the output of
`rebuild --dry-run --assume-stopped` in a scrollable modal; `y` copies the
commands as a script), **Forget** (press `Enter` twice: the first press arms it
and the entry reads `Forget · press Enter again to confirm`, and moving off it
disarms it; the record leaves history and any pin pointing at it falls back to
its snapshot), and **Clean up…**.

Clean up lists what `conspectus history reap` would remove: gone records past
their `[history.reap_after]` age, minus records a pin targets. All are
selected; `Space` deselects the record under the cursor, `n` deselects them
all and `a` selects them all again, `Enter` forgets exactly the selected
ones (with none selected it just closes), and `Esc` cancels. The bottom
border counts the selected records (`3 of 4 selected`). With nothing past
its age the modal says so. Forgetting and
cleaning up only remove history records; they never touch tmux, git, or the
filesystem, and they are the same writes as `history forget` and `history
reap`.

### Ghost muxes in the Mux view

The Mux view lists a stopped mux below the running muxes of its group, in the
ghost style (dimmed gray italics), with a `gone` status and the time it was
last seen, but only when a pin points at it: stopped muxes you haven't pinned
live in History. The header counts them all (`11 mux · 4 gone`). A mux whose
probe couldn't run shows as `unknown` and isn't counted as gone. Controls has
a **Show all stopped muxes** checkbox (off by default), and any narrowing
filter (harness, max age, mux state) leaves ghosts out because they can't
match.
`Enter` on a ghost rebuilds it, `.` opens the same menu as in History, and
attach and rename refuse with a message that names the fix.

### Undo

`u` undoes the last pin or unpin. Unpinning removes a pin together with its
overrides and saved values, and can make its row leave the Pinned view, so the
toast ends with `u to undo`. Undo puts a removed pin back exactly as it was,
with its name, overrides, saved values, and its place in the pin order, and
moves the cursor to its row (the Pinned row in that view, the entity's row
elsewhere) without changing the view. Undoing a pin removes it again.

Undo steps back through the pins and unpins of this session, newest first, up to
20 of them, and says `nothing to undo` when the list is empty. It is not kept
between sessions. If a new pin has taken the removed pin's name, the restore is
refused and says so; a pin whose history record was reaped in the meantime
still comes back, showing as missing. Other actions (rename, retarget, edit,
forget, worktree changes) are not undoable yet; each store write declares
whether it has an inverse, so a new one has to decide.

### Migrating legacy pins

Pins from earlier builds (`[[pins.entries]]` in `.conspectus.toml` or the
user config) still load as in-memory pins after the real ones, and a
`LegacyPins` diagnostic points here. Legacy pins are read-only: every `pin`
write gesture (`edit`, `rename`, `retarget`, `refresh`, `move`, `rm`) and
`rebuild` ask you to migrate first.

```sh
conspectus pin migrate --dry-run
conspectus pin migrate
```

Each entry becomes a mux pin with the same name. Its launch settings seed
a history record (dated by the old binding sidecar, with the session the
pin last bound), and the pin carries overrides only for what the record
doesn't already say. A display name that would otherwise disappear is kept
as an alias on the session. The converted entries are then removed from
their config files with other tables and comments intact; running it
again finds nothing. A pin that can't be converted stays where it was.

## Caches

Conspectus has three persistent cache surfaces:

- **The resolved-graph artifact** at
  `$XDG_DATA_HOME/conspectus/graph.bin` (ADRs 0082 / 0083).
  Written by `conspectus serve` after every successful
  refresh cycle and by daemonless one-shot CLI invocations
  (`conspectus table`, `node show`, `graph`) at the end of
  their cold-rebuild path. The daemon reads it back via mmap
  to restart warm, and library consumers can mmap it with
  `snapshot::open_mmap`; one-shot commands don't read it (they
  ask a running daemon or rebuild). The daemon's `snapshot`
  socket command serves the same bytes.
  Clearing it (`rm`) only loses warm-start; the next
  daemon cycle or CLI invocation rebuilds. There are no
  sidecars, no schema migrations, no backup rotation.
- **The history store** at `$XDG_STATE_HOME/conspectus/history/`,
  described under [History](#history). It is a rebuildable
  observation, but the only copy of a gone entity's recipe, so
  clearing it loses that recipe; `history reap` and `history forget`
  are the supported ways to prune it.
- **Legacy files.** `$XDG_CACHE_HOME/conspectus/pin-bindings/` and
  `$XDG_STATE_HOME/conspectus/pin-stores.json` belonged to the earlier
  pin model. Nothing writes them now; `pin migrate` reads them once and
  they can be deleted afterward.

Future caches (PR fetches, transcript indices, etc.) land
under the same `$XDG_*_HOME/conspectus/` roots rather than
inside `.conspectus.toml` or the project config directory.

## Keys on the right pane

With the right pane focused (and no dialog open), these keys drive the
graph explorer. Every other key means what it means on the left pane:
`E` toggles edge meta, `i` copies the id, `O`, `Shift+Enter`, and
`Alt+Enter` reveal, `J`/`K` scroll the Preview, `Tab` moves focus, `q`
quits. The keys that move the left tree do nothing here, and a letter
means the letter on either pane, so a chord of it (`Alt-j`, `Ctrl-j`)
moves neither the explorer nor the tree. The Preview keeps the place
`J`/`K` scrolled it to through a refresh, `r`, and a filter or sort
change; it goes back to where it starts (the top, a transcript's newest
turns, or the search match it opens on) when it shows something else:
another left selection, another node in the right pane, or another row
under the right pane's cursor.

| Key | Does | On the left pane |
|---|---|---|
| `↑`/`k`, `↓`/`j` | Move the cursor through the fields and the related rows. | Move the selection. |
| `PgUp`, `PgDn` | Page the cursor by the rows the explorer zone shows. | Page the tree. |
| `g`/`Home`, `G`/`End` | First and last row. | First and last row. |
| `Enter` | Copy a field's whole value, drill into a related row, or open and close `Other` on its header. On an unresolved row under `Other` (evidence that names no node) it says so; the Preview shows what the evidence records. | The row's default action. |
| `e` | Open or close `Other` on its header. | Nothing. |
| `l`/`→`, `h`/`←` | Open `Other` on its header; close it from its header or a row inside it. | Expand or collapse the tree row. |
| `o` | Open a cut value whole. | The same (the right pane's cursor row). |
| `F` | Expanded Node Detail: every field of the node, or its summary again. | Clear every filter. |
| `Backspace`, `Esc` | Back out of the last drill; with none, press twice to return to the left pane. | Return from a reveal. |

The status bar names the explorer's keys (`↑/↓ move · Enter copy · e
fold · Backspace back`), with `Enter`'s word for what it does on the
cursor's row: `copy` on a field, `follow` on a related row, `fold` on
the `Other` header. The help page (`?`) lists every key above.

A drilled pane's title shows the chain that led there (`◄ ● claude ›
◇ project › ◆ tools · depth 3`), each hop named after its node: a repo
by its working directory (`project` for `~/repos/project/.git`), a bare
repo by its own directory (`project.git`). Two hops with one name carry
four characters of the `id` field each pane shows (a session's key or a
mux's name ends with them; a repo's, checkout's, or branch's short id
starts with them), so `◆ project·c955` and `◆ project·41f0` read apart.

## The status bar's hints

The status bar names the keys of whatever has them, worded for where you
are, so a hint never names a key that does something else there:

- **The left pane:** what the selected row's keys do (`Enter/a attach
  tmux:main`, `Enter/v view codex:4184 · S resume`, `Enter/v open at the
  match · . menu`, `Enter rebuild` on a gone pin or a history record,
  `Enter show URL` on a live PR pin, `Enter/←/→ fold` on a group), then
  `↑/↓ move · ←/→ fold`, `Tab focus` (`Tab inspect candidates` on a
  row with several candidate muxes), `r refresh` (and `Ctrl-R hard
  refresh` in the PRs view), `q quit`, and `Backspace back` after a
  reveal. `j`/`k` and `h`/`l` work as the arrows do.
- **The right pane:** `a attach …` when its entity attaches, its own
  keys, then `Tab focus`, `r refresh`, and `q quit`.
- **A dialog:** its own keys, the same as its footer; the `[left]` or
  `[right]` tag goes while it is open.

A key that cannot act on the selection is named dimmed, with why
(`a attach: selected row has no mux to attach to`). On a narrow terminal
the hints give way one by one, least important first, and the bar ends
with `…`; a status message too long for the bar ends with `…` too.

On a workspace, repo, or checkout row, `a` attaches to the one mux its
sessions are ambiguous about. With several it says how many and to
select a session to see its candidates: an ambiguous session's right
pane lists the muxes it could be in under `Other` (its hint is `Tab
inspect candidates`), and `a` there attaches to the one the cursor is on.

## Keys in dialogs

Every dialog uses the same three keys the same way:

| Key | Does |
|---|---|
| `Esc` | Leaves the dialog without applying what is still pending. |
| `Enter` | Applies or submits the dialog's main action; in a list it activates the focused item. |
| `Space` | Changes the focused item: toggles a checkbox, steps an option, selects a choice. In a text field it types a space. |

A letter key means the letter typed plain or with Shift, never a `Ctrl`
or `Alt` chord of it, on the main screen and in every dialog: `a`
attaches and `Alt-a` does nothing, `j` moves the tree and `Ctrl-j` does
nothing, and `g` and `G` (or `Home` and `End`) go to the top and bottom
of the tree, the help page, the value viewer, Messages, and the viewers,
where `Ctrl-G` does nothing (on the main screen it cycles the grouping).
A chord a key uses is named as one (`Ctrl-R`, `Ctrl-G`, `Ctrl-O`). Every list moves with `↑`/`↓` and with
`Ctrl-P`/`Ctrl-N`, and with `k`/`j` where nothing is typed: the menus,
Controls and its checklists, Clean up, Messages, the picker, quick select,
content search and its options, and a form's fields.

Leaving a dialog works one way everywhere. `Esc` steps back one level (out
of an editor in Controls, a worktree confirmation, the search options, a
viewer's find or help page) and closes the dialog at its first level.
`Ctrl-C` closes the whole dialog from any level, and so does `q` wherever
nothing is typed: a menu, a checklist, a confirmation, Controls, the help
page, Messages, the search options, and the viewers. In a prompt, a form,
a query, or a find prompt, `q` is a letter. Cancelling says nothing: no
status message reports it, and closing a dialog clears the status line,
so the screen's own hints show again.

Each dialog's footer is generated from the keys it takes, so it names
only keys that work there. A hint is the keys and a short verb (`↑/↓
move`, `Enter pick`); `Esc close` means leaving loses nothing, `Esc
cancel` that something unfinished is dropped, and `Esc back` that it
returns to the pane below in the same dialog.

Every list marks the row its cursor is on the same way: `►` in the two
cells before the row, and the row drawn with `selection_active` (reversed
and bold) while the list has focus, or `selection_inactive` (bold) while
it does not, so the pane without focus still shows its cursor. Menus, the
picker, the checklists, the controls dialog, the search lists, and a
form's fields wrap from their last row to their first and back; the tree,
the right pane, and the messages list stop at their ends.

The controls dialog (`f`) applies each change as you make it, so `Esc`
closes it and keeps what you already changed. In it `Space` selects the
focused choice, toggles a checkbox, steps a cycle, or opens a filter's
editor; `Enter` applies the focused choice and closes (on a filter row it
opens the editor, and on a checkbox it just closes). Its keys stay at the
bottom while its sections scroll, with a scrollbar beside them; `PgUp` and
`PgDn` move by the rows shown, and `Home`/`End` (or `g`/`G`) go to the
first and last row. A filter's editor opens over it: `Enter` there applies
the filter and comes back to the panel, `Esc` comes back without it, and
`Ctrl-C` (or `q`, except while typing an age or a search) closes both.
Scrolling dialogs
(help, the full-value viewer, messages, the transcript and pane viewers)
use `Space` to page down, as pagers do, and `Ctrl-D`/`Ctrl-U` to move half
a page (help, the full-value viewer, and the viewers). A page is the rows the dialog shows,
as it is for `PgDn`/`PgUp` in the tree, and scrolling stops at the last
line. The help, the full-value viewer, and the Messages record show a
scrollbar when their text runs past the dialog and keep their keys on its
last rows; on a terminal
too short for every line of keys, the line with the closing keys stays and
the others scroll with the text. Help and the other long dialogs wrap
their text to the dialog's width, and menus cut a label that cannot fit
with an ellipsis, so no word is clipped at the edge.

Every menu (`.`, `m`, the worktree menu's list, and the menu `p` opens when
several pins point at a row) works the same way. An entry that has a key of
its own shows it in a column before its label, and pressing it picks the
entry from anywhere in the menu: `p` on **Pin** in the row menu, and `n` and
`N` in the mux menu, the keys that do the same on the list. An entry that
asks for a second `Enter` says so on its row once armed. A menu taller than
the screen keeps its keys at the bottom and scrolls its entries, with a
scrollbar beside them, moving only as far as the cursor needs.

Every checklist (Controls' harness, mux state, and PR state editors, and
Clean up) works the same way. `Space` checks or unchecks the item under
the cursor, `a` checks every item, `n` unchecks every item, and `Enter`
takes what is checked; `Esc` goes back without it (in Controls, to the
panel). The bottom border says how many items are checked (`2 of 5
selected`). `PgUp` and `PgDn` move by the rows shown, and `Home`/`End`
(or `g`/`G`) go to the first and last item. A checklist taller than the
screen keeps its keys at the bottom and scrolls its items with the
cursor.

The picker (**Retarget…** in the pin menu, and the remembered searches
in Controls) types into its filter, so `j`, `k`, and `q` are letters
there. Every word typed must appear in a candidate's name or details,
and each change to the filter puts the cursor back on the first match.
The arrows and `Ctrl-P`/`Ctrl-N` move, wrapping, `PgUp`/`PgDn` move by
the rows shown, `Enter` picks, and `Esc` or `Ctrl-C` closes it. It is
the size of quick select and content search, keeps its keys at the
bottom, and scrolls a long list with a scrollbar beside it.

Every dialog takes the same keys. A confirmation (the worktree menu's
merge, remove, and prune) goes ahead on `Enter` or `y` and goes back on
`Esc` or `n`; a choice (close-down's `m` merge and `d` discard) takes its
own keys and goes back on `Esc` or `n`; a notice (Clean up with nothing to
clean up) closes on `Enter`, `y`, `Esc`, or `n`. `Ctrl-C` and `q` close a
dialog too. A dialog or prompt a menu entry opens sits over the menu:
`Esc` there returns to the menu, and `Ctrl-C` (or `q` where nothing is
typed) closes both. On a short terminal a dialog keeps its keys at the
bottom and its text scrolls with the arrow keys.

The transcript viewer and the pane viewer are one reader: they scroll,
page (`PgDn`/`Space`, `PgUp`, `Ctrl-D`/`Ctrl-U`), jump to the ends
(`g`/`Home`, `G`/`End`; `G` keeps the view at the end as it changes), find,
step back with `Esc`, leave with `q`, and show their position as `[ n/m ]`
alike. Their footer is generated from their keys, after the position, the
find, a one-key note, and the viewer's toggle chips (`tools·off (t) ·
think·off (T) · aborted·hide (I)`, and `hidden·hide (H)` for a transcript
with hidden records): `j/k/PgDn/PgUp scroll · / find · ? help · Esc/q
close`. A narrow footer drops the scroll keys first, then the find and
help keys, then the chips from the last, and always keeps `Esc/q close`.
A toggle shows or hides turns in place: at the end the view stays at the
end, elsewhere it keeps its row.

Inside either viewer, `?` opens a help page
drawn like the main help: the same frame and size, section headings, an
accent-colored key column, descriptions that wrap, a scrollbar, and a
footer naming the keys that work there (`j/k/PgDn/PgUp scroll · Esc/?
back · q close`: `Esc` and `?` go back to the viewer, `q` leaves it). Its
rows are generated from the viewer's key table, so a key added to the
viewer is listed automatically. While the
page is open the movement keys (`j`/`k`, `PgDn`/`PgUp`, `g`/`G`, `Ctrl-D`/
`Ctrl-U`) scroll it rather than the transcript, `?` or `Esc` close it
(`q` and `Ctrl-C` leave the viewer), and the toggles wait until it is
closed.

### Editing text in fields

Every text field (the rename and branch-name prompts, the max-age and
contents editors in the controls, the search and picker queries, the
fields of the new tmux session, launch, and pin edit forms, and the
viewers' find prompts) edits the same way. `Alt-B` and `Alt-F`,
`Ctrl-←` and `Ctrl-→`, or `Alt-←` and `Alt-→` move a word back and
forward; `Ctrl-A`/`Ctrl-E` (or `Home`/`End`) go to the start and end and
`Ctrl-B`/`Ctrl-F` move one character; `Ctrl-W` or `Alt-Backspace`
deletes the word before the cursor and `Alt-D` or `Ctrl-Delete` the word
after it; `Ctrl-K` deletes to the end, `Ctrl-U` clears the whole line,
and `Ctrl-Y` puts back what one of those deleted. A dialog's own keys
come first: on a form's option rows, `←`, `→`, and `Space` change the
option, and the search's own chords (`Ctrl-F`, `Ctrl-R`, and the rest)
keep their meanings there.

A field shows its cursor as a highlighted cell (the `field_cursor` style
of `[tui.theme]`) and scrolls sideways to keep it in view, counting the
two cells a wide character such as a CJK character or an emoji takes, so
the cursor stays on the character it is on and the field never runs past
its edge.

The TUI turns on bracketed paste while it runs, so the terminal delivers
a paste as one piece, typed into the field that has the cursor. A field
holds one line, so a paste is flattened instead of cut at its first
line: line breaks at the start and end are dropped, each run of line
breaks inside becomes one space, a tab becomes a space, and other
control characters are dropped. A paste therefore never presses `Enter`
or `Tab`, and cannot submit a prompt or form. Anywhere else (the tree, a
menu, a list, a form's option row) a paste is ignored instead of being
read as keys. A terminal without bracketed paste still sends a paste as
keystrokes, line breaks included.

A path typed as `~` or `~/…` in the new tmux session form, the launch
form (its cwd and worktree repo), or a pin's `cwd`, `location`, or
`path` field is expanded to your home directory when the form commits,
so the command it runs gets the directory, and the pin stores it.

Input a field refuses is explained, and the field stays open for a fix.
A prompt (the rename, branch-name, max-age, and contents prompts) says
why on an error line inside it, which grows the prompt by a row until
your next edit: an invalid max age (`max age: missing unit (expected
ms/s/m/h/d)`), an empty branch name for a new worktree (`worktree:
branch name is required`), and renaming a pin or a tmux session to
nothing (`pin: nothing entered`, `mux rename: name cannot be empty`).
The pin rename prompt also refuses a name another pin has (``a pin named
`x` already exists``) or that is not a usable pin name, with the reasons
the edit form's name row gives, so a live mux pin's tmux session keeps
its name when the pin cannot take the new one.
The new tmux session and launch forms say why on an error line just above
their keys, until your next edit, and move the cursor to the field to fix:
an empty name or cwd in the new tmux session form (`mux: name is
required`, `mux: cwd is required`), and in the launch form an empty or
live name, an empty harness or cwd, an argv that ends inside a quote, or
a worktree without its branch or repo (`mux launch: mux name is
required`, ``mux launch: launch argv has an unclosed `'` quote``).

A prompt shows its keys on its bottom border (`Enter confirm · Esc
cancel`). `Esc` leaves it without saying anything; from the worktree
menu's branch prompt and the controls' max-age and contents editors it
steps back to the menu or the controls, and `Ctrl-C` closes those too.
`q` is typed like any letter.

### Finding text in a transcript

Inside the transcript viewer, `/` opens a find prompt in the footer.
Type a query and press `Enter` to jump to the first match at or below the
top of the screen; `Esc` leaves the prompt and keeps an earlier find, and
the prompt edits like every text field (see Editing text in fields;
`Ctrl-U` clears the whole query). `n` and `N` step to the next and previous
match and wrap at either end, saying so in the footer. The footer shows
`find "query" i/N` (or `no match`; the header's `find:"query"` chip uses the
same wording), and each match is highlighted: the current one in the
selection look (`selection_active`, reversed and bold by default), the
others in the shared match style (bold, underlined). `Esc` clears the find before
it closes the viewer, one step back at a time; `q` (and `Ctrl-C`) leave the
viewer at once, from the help page or with a find active, and `Ctrl-C`
leaves from the find prompt too (where `q` is typed).

Find is a case-insensitive substring match on what the viewer is
showing: messages always, and tool output, thinking, aborted, and hidden
turns only while their toggles (`t`, `T`, `I`, `H`) show them. When a
query matches only in turns the toggles hide, the footer says how many
(`no match · 2 in hidden turns`); turning the toggle on brings them into
the match count. Hiding the turn the find is on leaves it on no match
(`find "query" 3 matches`) until `n` or `N` goes to one. A match the line
wrapping splits across two lines still counts but is not highlighted.

A search result opens the viewer on its matching turn with the query
active. If that turn is hidden, the viewer shows it (full tool output,
thinking, aborted, or hidden turns, as needed) and says what it turned on
in the footer for one key press. If the transcript changed since the
search, the viewer looks near the remembered turn, then anywhere, for the
match, and says when it is no longer there.

## TUI messages

Every operation the TUI runs (rebuild, mux new/launch, attach,
renames, pin and worktree changes, refresh and daemon failures)
is logged in memory for the life of the TUI process (ADR 0105). Press
`!` to open the Messages overlay: newest entries first, with the
selected entry's full record below, including argv, exit status,
stderr and stdout for subprocesses, and the pane output when a
harness died in its pane. `j`/`k` select (stopping at the newest and
the oldest entry), `g`/`G` jump to the newest and the oldest, `J`/`K`
scroll the record a line and PgUp/PgDn (or `Space`) a page, `y` copies
it, and `Esc`, `!`, `q`, or `Ctrl-C` close. A long log or a long record
shows a scrollbar beside it, and a summary too long for its row ends in
`…`; on a narrow terminal the keys line wraps onto a second line.

A warning or error also:

- sets the status message to its summary plus `· ! details`;
- keeps a `▲ N · ! messages` chip at the start of the status bar until
  you open the overlay (`▲ N` while a dialog is open, where `!` does not
  reach the log);
- leads the Preview of the row it concerns (its pin or mux) until a
  later operation on that row succeeds.

The log is never written to disk, because pane output and harness
stderr can contain conversation content.

Every message the TUI shows, on the status line, in a toast, or in the
log, has one of four severities, and each draws in its `[tui.theme]`
color:

| Severity | Means | Color key | Mark | Toast stays |
| --- | --- | --- | --- | --- |
| info | a hint for the next key (`viewing …`, `worktree: ↑/↓ move · Enter pick · Esc close`) | `info` | `•` | 1.5 s |
| success | what you asked for happened (`copied: cwd`, `filters cleared`) | `success` | `√` | 1.5 s |
| warning | the key did not do its job, and nothing failed (`nothing to undo`, a disabled action's reason) | `warning` | `▲` | 3 s |
| error | something failed (`copy failed: …`) | `error` | `×` | 5 s |

A toast's border takes its severity's color; its text is `toast_text`.
A new toast replaces the one showing.

## TUI state

The TUI remembers where the operator left it at
`$XDG_STATE_HOME/conspectus/tui-state.json` (sibling to the hook
state-root under `$XDG_STATE_HOME/conspectus/hooks/`): the last view,
the sort and preview settings, and for each view its filter, grouping,
expanded rows, and selected row. The file is rebuildable cache, not
authoritative state: losing it costs nothing more than the next
`conspectus tui` starting in the configured default view, with each
view on its default filter, grouping, and folds.

What the file holds:

```json
{
  "schema_version": 1,
  "last_view": "sessions",
  "sort": "recency",
  "mux_recency": "last_attached",
  "preview_wrap": "smart",
  "session_preview": "transcript",
  "row_keys": 1,
  "view_states": {
    "sessions": {
      "filter": { "harness": { "any": ["claude-code"] } },
      "grouping": { "Sessions": "workspace" },
      "expanded": [
        "group:workspace:/home/me/src",
        "group:repo:/home/me/src/app/.git"
      ],
      "selected": {
        "row": "session:agent_session:claude-code:/home/me/.claude:4f1c…",
        "path": ["group:workspace:/home/me/src", "group:repo:/home/me/src/app/.git"]
      }
    },
    "mux": { "grouping": { "Mux": "host" } }
  }
}
```

- `last_view` is one of `sessions`, `mux`, `prs`, `pinned`, or `history`
  (never `search`, whose rows answer a query that is not remembered).
- `sort`, `mux_recency`, `preview_wrap`, and `session_preview` are the
  last sort, mux recency basis, preview wrap, and session preview chosen
  in the TUI; each takes precedence over its config value on the next
  run (`--sort` overrides the sort).
- `view_states` has an entry for each view the operator has used:
  `filter` and `grouping` as last set (a `--grouping`, `--harness`,
  `--max-age`, `--mux-state`, or PR filter flag overrides them for the
  starting view), `expanded` (the rows whose children were showing),
  and `selected` (the row under the cursor and the rows above it, the
  top-level one first). The Search view keeps neither `expanded` nor
  `selected`; nothing keeps the right pane's drill or the preview's
  scroll.
- Rows are named by keys: the row's kind (`group`, `session`, `mux`,
  `pr`, `history`, `pin`, `nested`, `bucket`, `remote`, `synthetic`), a
  colon, and the node id, record id, pin id, label, or slug that names
  it. `row_keys` is the version of that encoding.

**How a view reopens.** Each view is put back the first time it lists
rows: the first refresh for the starting view, the first switch to each
of the others. The groups in `expanded` that are still listed open and
every other group stays collapsed; a group that is gone is ignored. The
cursor goes to the `selected` row; if that row is gone, to the deepest
row of its `path` that is still listed (a session whose group is still
there lands on the group); if none is, to the first row. Any collapsed
group above the cursor is opened, and the pane scrolls to it. None of
this is an error or a warning. It happens once per view: a later
refresh, the operator's own moves, a jump into the view, or a regroup
before the rows arrive all win over it.

**Reading the file.** A missing file, malformed JSON, or a `last_view`
this build doesn't know (including the removed `union` and `forks`)
collapse to "nothing remembered" without an error. A value of the wrong
shape costs only that setting: a non-list `expanded` or a `selected`
without a text `row` is dropped for that view, and an entry in a list
that is not text is skipped. A file from an earlier build, with no
`expanded` or `selected`, opens every view on its default folds and its
first row (or the launch directory's group). A file whose `row_keys`
is missing or another version keeps everything but the expanded and
selected rows. Fields this build doesn't know, at the top level or in a
view, round-trip through writes.

**When it is written.** On every view switch (a jump to another view
and `Backspace` back included) and settings change (sort, grouping,
filter, mux recency, preview wrap, session preview), before the TUI hands the
terminal to another program (attaching a mux, a viewer, a launch), and
when the TUI exits, however it exits short of a crash. Moving the
cursor or folding a group writes nothing by itself; the next of those
writes keeps it. Each view's `expanded` keeps only the rows the view
lists at the time, so the groups that went away leave the file and it
does not grow. A view the run never showed is written back as it was.
Writes are atomic (tempfile + rename) and skip-on-unchanged, so quiet
TUI sessions produce no mtime churn.

**Startup precedence:** explicit `--view <name>` flag wins →
persisted `last_view` wins → config `[tui].default_view` →
built-in `View::Sessions`.

**Opt-outs:** pass `--no-resume-view` to ignore the persisted view for a
single run and open every view on its default folds and first row
(useful for scripts and tests that need a deterministic start); the
run's own changes are still written when it ends, and the views it never
showed keep what they had. The `--snapshot` dev path implies
`--no-resume-view` and never writes the file, so snapshot regeneration
never depends on what the operator last touched outside the fixture.

**Read-only invariant:** only `conspectus tui` reads or writes
the file. The `graph`, `table`, `node show`, and `pin *`
surfaces, and `tui --snapshot`, all leave it byte-identical, enforced
by `tests/cli_tui_state_invariants.rs`.

## What changed in the TUI

This release moves every control of the TUI onto one shared toolkit, so
menus, dialogs, forms, lists, readers, and keys behave alike wherever they
appear. Most of what follows is a key or a look that now matches the rest
of the TUI. The sections above describe the current behavior in full;
`CHANGELOG.md` lists the same changes with more detail.

### Views and the command line

- **The hidden `union` and `forks` views are gone.** `--view union` and
  `--view forks` fail and list the valid views; a `default_view` of
  either, a `[tui.views.union]` or `[tui.views.forks]` table, and a
  remembered `last_view` of either are reported or ignored, and the TUI
  starts in the default view. `conspectus table union|forks` is unchanged.
- **Quick select and Reveal promise only a view that lists the entity.**
  A result no view lists (a fork, a repo with no sessions, a row a
  filter hides) reads `not listed`, and a pin's menu offers **Reveal in
  its view** only where that view lists its entity.
- **`conspectus search` cuts snippets like the TUI.** The table's snippet
  column favors the text after the match and cuts at word edges, or inside
  a path, URL, or CJK text beside the match; the stored snippet
  (`snippet.text` in `--format json`) is at most 160 terminal cells with
  its marks. The JSON's shape is unchanged.
- **`conspectus pin rename` checks the new name first**, so a taken or
  unusable name renames no tmux session. **`pin edit --set argv=…`**
  refuses an argv that ends inside a quote or on a lone backslash, and
  previews read a backslash inside double quotes as a shell does (see
  [Pinning from the other views](#pinning-from-the-other-views)).
- **`tui --fixture` and `dev scenario tui`** take the live TUI's keys in
  every menu, prompt, and form, which close as usual; an entry that would
  write or launch says it isn't available in this mode.

### Keys on the main screen

- **A letter means the letter**, typed alone or with Shift, never a
  `Ctrl`, `Alt`, Super, or Meta chord of it. The chords the help names
  (`Ctrl-R`, `Ctrl-G`, `Ctrl-/`, `Ctrl-L`, `Ctrl-O`, `Ctrl-F`, `Ctrl-C`,
  `Shift+Enter`, `Alt+Enter`) still work. Stopped working: `Alt` with `f`,
  `!`, `?`, `a`, `i`, `o`, `w`, `v`, `r`, `n`, `m`, `p`, `u`, `]`, `[`,
  `/`, `s`, `<`, or `>` (so `Alt-a` no longer attaches and `Alt-p` no
  longer pins); `Alt` with the tree's `h`, `l`, or `g`; and every chord
  but Shift of the tree's `j`, `k`, and `G` and the Preview's `J` and `K`
  (`Ctrl-j`, which many terminals send for `Ctrl+Enter`, no longer moves
  the tree). `q`, `.`, and `1` to `6` now work when a terminal reports
  them with Shift.
- **`Enter` on the left pane means `Enter` alone**: `Ctrl+Enter` no longer
  runs the row's default action.
- **`Backspace` on the left pane only returns from a reveal.** It no
  longer pops the right pane's drill, and says so when there is nothing to
  return from.
- **`PgUp` and `PgDn` move the tree by the rows it shows** (they stepped
  by the terminal's height).
- **`e` does nothing on the left pane** (it flipped a flag nothing
  showed); on the right pane it still opens and closes `Other`.
- **`o` opens the right pane's cursor value from either pane**, and the
  `?` page, generated from the keys themselves, lists every key on a row
  of its own.

### Keys on the right pane

[Keys on the right pane](#keys-on-the-right-pane) has the full table.

- **`PgUp` and `PgDn` page** by the rows the explorer zone shows (they
  moved one row).
- **`h`/`l` and `←`/`→` open and close `Other`**, from its header or a
  row inside it (they did nothing); on a row in no section they say there
  is nothing to expand.
- **`Esc` backs out of a drill**, as `Backspace` does (it did nothing).
- **Chords of `j`, `k`, `g`, `G`, `h`, and `l` do nothing** (`Alt-j` moved
  the tree behind the pane), and the pane binds no `Ctrl-N`/`Ctrl-P`.
- **`Backspace` to a frame whose node is gone** skips it in the same
  press, and warns only when every frame is gone.
- **A refresh keeps your place.** The periodic refresh, `r`, and a filter
  or sort change keep a drill (its breadcrumb, `Other` fold, Expanded Node
  Detail, and cursor) and keep the Preview where `J`/`K` scrolled it.
- **`Enter` on a field with no value** (`—`) copies nothing, and on
  evidence that names no node it says the Preview shows the evidence.

### Keys in dialogs and text fields

[Keys in dialogs](#keys-in-dialogs) and
[Editing text in fields](#editing-text-in-fields) describe each in full.

- **One way out of every dialog.** `Esc` steps back one level, `Ctrl-C`
  closes the whole dialog, and so does `q` wherever nothing is typed: the
  menus, Clean up, Controls and its checklists, the worktree questions,
  the search options, the help, Messages, and the viewers (`q` used to do
  nothing in most of them and backed out of the close-down choice).
  Cancelling posts no `…: cancelled`, leaving the transcript viewer no
  `viewer closed`, and closing a dialog clears the status line.
- **A letter in a dialog is the letter.** `Ctrl-G` and `Alt-G` (top and
  bottom of the help, the value viewer, Messages, and the viewers),
  `Ctrl-D` (discard in the close-down choice), `Ctrl-Y` (confirm a
  worktree action; copy in the value viewer), and `Alt-Y` (copy in
  Messages) stopped working; the plain letters still do.
- **Every list moves with `Ctrl-N`/`Ctrl-P`** as well as the arrows, and
  `PgUp`/`PgDn` move the picker, quick select, content search, the
  checklists, and Controls by the rows shown; `Home`/`End` and `g`/`G`
  reach the ends of the checklists and Controls.
- **Menus wrap** from their last entry to their first and back. An entry
  with a key of its own shows it in a column before its label and takes
  it from anywhere in the menu (`p` on **Pin**, `n` and `N` in the mux
  menu).
- **Checklists** take `a` to check and `n` to uncheck every item, and say
  on their border how many are checked.
- **Dialogs** go ahead on `Enter` or `y` and go back on `Esc` or `n` (the
  close-down choice gains `n`). The worktree menu's prompt and questions
  open over the menu, and `Esc` returns to it.
- **Forms submit from any field**: `Enter` in the new tmux session form
  creates the session from its name field (it moved to the cwd first).
- **Readers**: the pane viewer wraps long lines under their number and
  `w` cuts them at the edge instead; `G` keeps either viewer at the end;
  `Ctrl-D`/`Ctrl-U` page by half in the help and the value viewer too; in
  a viewer's help, `?` and `Esc` go back and `q` leaves the viewer.
- **Every text field edits one way.** `Alt-←`/`Alt-→` move by word and
  `Alt-D` deletes the next word in every field; the find prompts take
  every field's editing keys, and `Ctrl-U` there clears the whole query;
  `Shift` with an arrow, `Home`, `End`, `Backspace`, or `Delete` no longer
  drops the key. A paste is typed as one line into the field with the
  cursor and is ignored anywhere else, so it cannot submit a prompt or
  press a menu's keys.
- **Path fields** complete only by extending what you typed (typing `src`
  no longer jumps to `~/work/src/app`), offer only directories, mark a
  file or a missing path `▲` (it was `!`), show the `Tab:` hint only while
  `Tab` has something to add, and expand a leading `~` when the form
  commits.

### How the screen looks

- **One cursor look.** Every list marks its cursor row with `►` in the
  two cells before it, drawn with `selection_active` while it has focus
  and `selection_inactive` while it does not. That includes the left
  pane's tree, whose rows start two cells further right, and the right
  pane, which keeps its cursor visible while the tree has focus.
- **The focused pane's border** takes `panel_focus_accent`, as its `►`
  already did.
- **One frame for every dialog**: one title form (plain, a space each
  side), one of five sizes, and a margin of two columns and a row around
  it. Menus are 64 columns and forms 80; the picker takes the searches'
  size. A dialog opened over another shows the one under it around its
  frame, and a toast shows over the full-screen viewers.
- **One footer.** A dialog's keys sit at its bottom, receding in `muted`
  under a blank row, generated from the keys it takes and ending with the
  key that closes it; a screen too short for all of them keeps the last
  rows. A prompt shows its keys on its bottom border, and a checklist its
  count.
- **Rows give way by one rule on a narrow pane.** A cut ends in `…`,
  each kind of row lets its columns go in a fixed order
  ([`docs/tui-columns.md`](tui-columns.md)), and a list that grows past
  the pane keeps its columns. Nested group headers line up with their
  parents, PR rows keep their `#N`, and Pinned and History rows keep
  their name and state word. The header drops its least important parts
  first, and the tab strip and the right pane's breadcrumb keep what fits
  and end with `…`.
- **Search hits read alike** in the contents modal and the Search view
  (`turn N · kind`, no longer who spoke), and every snippet favors the
  text after the match and cuts at word edges.
- **Text wraps one way.** A word that ends exactly at the edge stays on
  its row, key hints wrap between hints, and the transcript viewer keeps
  the blank lines between Markdown blocks.
- **Scrollbars.** Menus, lists, and readers that overflow show the shared
  scrollbar and scroll only as far as the cursor needs; one too short for
  a bar (under five rows) keeps its last column.
- **Empty places recede.** An empty list, an empty Messages log, and a
  Preview with nothing to show are drawn in the dim look; a Preview that
  failed keeps the plain look.
- **Ages read one way**: the Search view's rows tint their age by how
  recent it is, the search status says `scanned 0s ago`, and Clean up
  counts days (`21d ago`).

### Messages and hints

[The status bar's hints](#the-status-bars-hints) and
[TUI messages](#tui-messages) describe each in full.

- **Each message has a severity**: hints and confirmations are green,
  refusals yellow, failures red, on the status line, a toast's border, and
  in Messages. A warning toast stays 3 seconds and an error toast 5. A
  failed copy says `copy failed: …`.
- **The status bar names only keys that do what it says**: the selected
  row's (`↑/↓ move · ←/→ fold`, `Enter rebuild` on a gone pin, `Ctrl-R
  hard refresh` in the PRs view), the right pane's, or the open dialog's
  own. A key that cannot act is dimmed with the reason, and a message too
  long for the bar ends with `…`. Menus and searches no longer copy their
  footer into the status line when they open.
- **Prompts and forms explain a refusal** on an error line inside them,
  where a toast or nothing used to say it, and stay open for a fix.

### Forms and pins

- **The launch form**: the own-socket checkbox is a `server` row of its
  own under the name; its fields read `name`, `socket`, and `argv`; its
  pin choice is drawn `(•) don't pin ( ) launch, then pin ( ) pin, don't
  launch`; and its worktree repo completes and is marked like the cwd.
- **The new tmux session and launch forms name the next mux in the
  series**: opened from `worker-1` (its row, a session in it, or its pin)
  they start with `worker-2`, where they started empty (see
  [Pinning from the other views](#pinning-from-the-other-views)). From
  anything that is not a mux and runs in none they still start empty.
- **The pin edit form** labels its rows `branch` and `resume in`, says
  why a save is refused just above its keys, and takes `Ctrl-R` on its
  field rows only.
- **Renaming a pin**: the **Rename…** prompt and the edit form's name row
  refuse the same names with the same reasons, before anything is
  renamed, and a blank name reads `pin: nothing entered` in both.
- **One argv grammar** for the forms, `pin edit --set argv=…`, and the
  previews ([Pinning from the other views](#pinning-from-the-other-views));
  the forms write an argv back quoted as the previews quote it.

### Moving a `[tui.theme]` across

A theme from an earlier version loads unchanged, and the default look is
unchanged. The `[tui.theme]` section above lists every key with its default
and the older names. What a custom theme may notice:

- **Five muted keys became two.** `muted` (a modifier, `dim`) takes
  `placeholder`, `divider`, and `mux_unmuxed`; `inert` (a color, dark
  gray) takes `secondary_text` and `ghost`. The older keys still load, so
  setting only `placeholder` now also restyles rules and the unlinked
  `∘`, and setting only `secondary_text` also recolors gone rows. Older
  keys set differently in one file merge by the precedence of the
  older-key table, with a warning for each that lost. To keep two looks
  apart, set `muted` and `inert` themselves.
- **Renamed and joined keys.** `link_id` sets `link`. `accent` also sets
  `panel_focus_accent` unless the file sets that too, and
  `panel_focus_accent` alone still sets both; it now colors the focused
  pane's border as well as its `►`.
- **Harness colors.** Within one file a `[tui.theme.harness]` entry beats
  the flat `harness_<name>` key, which used to win.
- **New keys**, each defaulting to the old look: `accent`, `link`, `info`,
  `surface` (the background of dialogs and toasts), `toast_text`,
  `field`, `field_active`, `field_cursor`, `command_badge_text`, and
  `transcript_compaction`. Values may name a background (`white on
  dark_gray`) and the color `light_gray`.
- **Keys that reach further.** `info` colors hints on the status line, in
  toasts, and in Messages (Messages' info entries took `success`; both are
  green by default). `selection_active` and `selection_inactive` color the
  cursor row of every list. `field_cursor` is the cursor cell of every
  text field, the prompts and find prompts included, which drew reverse
  video. `disclosure` colors the right pane's `Other` arrow. `badge_width`
  sizes every chip, the content search's chip column and ` no agent `
  included.
- **The transcript viewer** reads its own keys: user turns take `accent`
  (they took `harness.codex`), the cwd `accent` with `muted` (it took
  `cwd_mark`), and compaction summaries `transcript_compaction` (they took
  `pr_merged`). A theme that set those three keys to restyle the viewer
  sets the new ones instead.

## Migration from earlier 0.x

Phase 11 (ADRs 0082 + 0083) retired Conspectus's SQLite
persistence layer and the `conspectus query` user-facing SQL
surface in favor of an in-memory daemon plus a single
zero-copy `graph.bin` artifact on disk. Operators upgrading
from a pre-Phase-11 build should expect the following
visible changes:

**Cache file change.** The canonical persisted artifact moves
from `$XDG_DATA_HOME/conspectus/graph.sqlite` (plus `-wal` /
`-shm` sidecars and the `backups/` directory) to a single
`$XDG_DATA_HOME/conspectus/graph.bin` file. The daemon
unlinks leftover `graph.sqlite*` files and the `backups/`
directory on its next startup; one-shot CLIs ignore them
without error. No operator action required, but you can clean
them up yourself if the auto-cleanup didn't fire (e.g.
because you haven't started a daemon yet):

```sh
rm -rf "$XDG_DATA_HOME/conspectus/graph.sqlite"* \
       "$XDG_DATA_HOME/conspectus/backups"
```

**`conspectus query` is gone.** The subcommand and its
flags — `--list-views`, `--similar-to`, `--load-extension`,
`--format {table,json,csv,tsv}` (the table renderer's
identically-named variants are unaffected) — have been
removed. The replacement shape for ad-hoc graph inspection
is the JSON dump:

```sh
conspectus graph --format json | jq '...'
```

If you had scripts piping `conspectus query 'SELECT … FROM
v_*'` into a downstream tool, the simplest port is
`conspectus graph --format json` plus jq expressions over
`.nodes[]`, `.candidate_links[]`, `.resolved_relationships[]`.
The pre-Phase-11 saved-view library (`v_sessions_with_repo`,
`v_mux_attachments`, `v_pr_by_branch`, `v_fork_ancestry`,
`v_workspace_member_repos`) no longer has a runtime surface;
the joins they represented can be expressed as jq pipelines
against the JSON dump.

**Vector search is gone with it.** ADR 0042's
`--similar-to` / `embeddings` overlay / `--load-extension`
machinery retired alongside the SQL surface. If you imported
embeddings for nearest-neighbor lookups, you'll need to
maintain that pipeline outside Conspectus until a future ADR
re-opens the surface against a non-SQL substrate.

**Daemon socket gains a `snapshot` command.** The wire shape
adds a `snapshot` arm alongside `ping` / `refresh` /
`status`. Returns the serialized resolved snapshot
base64-encoded under `data.bytes`. The TUI and one-shot CLI
prefer the daemon path when reachable and fall through to
cold rebuild otherwise. No flag changes; the routing is
transparent.

**User-authored TOML is unaffected.** `.conspectus.toml`
(declared links per ADR 0014) and user-level config files
(aliases per ADR 0029) keep their existing shapes and locations;
pins moved to `pins.toml` in a later release (ADR 0141). Phase 11 changed
*how the resolved graph is cached* — not *what the operator
authors*.

**Daemonless cold rebuild is the new floor.** Without
`conspectus serve` running, every CLI invocation does a full
discovery pass. This was single-digit seconds at target
scale before Phase 11 (when SQLite warm-start was active)
and remains so afterward — the architectural pivot trades
warm-start latency for the maintenance cost of the SQLite
machinery. Operators who want sub-second CLI response should
run `conspectus serve` (systemd user unit, launchd agent,
or just `conspectus serve &`).
