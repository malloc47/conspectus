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

The aider adapter is per-repo rather than per-state-root, so it
walks scan roots looking for `.aider*` markers and is not gated by an
environment variable.

### Hook sidecar state

Hook sidecar records are optional, local observations written by
harness hooks through `conspectus hook write`. They refine
session-to-mux attribution when a harness can report the current
session id without terminal input. With `conspectus serve` running,
the hook writer sends observations to the daemon, which updates the
in-memory graph and persists the result through `graph.bin`. When no
daemon snapshot is available, Conspectus writes a minimal latest-only
spool at `hooks-latest.json` under:

1. `$CONSPECTUS_HOOK_SIDECAR_STATE`, when set.
2. `$XDG_STATE_HOME/conspectus/hooks`.
3. `$HOME/.local/state/conspectus/hooks`.

The records are not project intent and should not be committed. Stale
records are ignored for active mux attribution. Older per-event JSON
records in the same directory remain readable as a compatibility path.

Install the Claude Code hook automatically:

```sh
conspectus hook init claude-code --scope user
```

The command is idempotent. `conspectus hook status claude-code` reports
whether the hook is present, and `conspectus hook remove claude-code`
removes only the Conspectus hook entry.

Manual Claude Code hook setup:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "resume|startup|clear|compact",
        "hooks": [
          {
            "type": "command",
            "command": "conspectus hook write claude-code"
          }
        ]
      }
    ]
  }
}
```

Place that in `~/.claude/settings.json` or a local Claude Code
settings file. The subcommand reads Claude's hook JSON from stdin,
records `session_id`, `transcript_path`, `cwd`, and tmux context when
available, then exits without writing to the transcript.

## Configuration File

See ADR 0012 for the layout and precedence rules. Briefly:

1. **Project-local**: Conspectus walks upward from the current
   directory looking for `.conspectus.toml`, stopping at `$HOME` or
   the filesystem root.
2. **User-level**: `$XDG_CONFIG_HOME/conspectus/config.toml`, falling
   back to `$HOME/.config/conspectus/config.toml`.

Project values win over user values, which win over built-in
defaults. Both files are TOML and entirely optional.

The same files also hold user-authored intent: `[declared]` links
(ADR 0014), `[[aliases]]` (ADR 0029), and `[[pins.entries]]` (ADR 0057,
see [Session pins](#session-pins)). The tables below are the
configuration keys.

```toml
[table.sessions]            # also mux, union, prs, forks (ADR 0021)
columns = ["default", "+preview"]   # same tokens as `--columns`

[tui]
default_view = "sessions"   # sessions | mux (union | prs | forks also accepted)
scan_roots = ["~/work"]     # used when --scan-root is absent; `~` expands
show_harness_chips = false  # per-harness count chips in the header
narrow_layout_threshold = 100  # columns below which the panes stack
preview_wrap = "smart"      # smart | plain | none — how the Preview fits a mux pane
# sessions_grouping = "graph"  # deprecated alias for [tui.views.sessions] grouping

[tui.views.sessions]        # also mux, union, prs, forks (ADR 0031)
grouping = "graph"
filters = { harness = ["codex"], max_age = "7d", mux_state = ["attached"] }

[tui.detail]
show_edge_meta = false      # provenance · confidence · state on link rows

[server.intervals]          # conspectus serve cadence; also the CLI's warm-start TTL (ADR 0079)
harness = "5s"
mux = "5s"
git = "30s"
forge = "5m"

[worktree]                  # see docs/worktrees.md
backend = "auto"            # auto | git | worktrunk
teardown_confirm = "live"   # always | live | never
teardown_grace = "3s"
```

- **Grouping values per view:** sessions `graph` (default), `workspace`,
  `repo`, `checkout`, `scan-root`, `none`; mux `session`, `host`,
  `repo`; union `kind`, `repo`; prs `repo`, `state`; forks `provider`,
  `parent`.
- **Filters:** `filters` takes one inline table or an array of tables
  (`[[tui.views.<name>.filters]]`); entries OR together per dimension.
  `max_age` accepts `s`, `m`, `h`, and `d` suffixes; `mux_state` values are
  `attached`, `ambiguous`, and `unmuxed`.
- **Durations** in `[server.intervals]` and `teardown_grace` use
  `<integer><ms|s|m|h>`.
- **View precedence at startup:** `--view`, then the view you last used
  (unless `--no-resume-view`), then `[tui] default_view`, then
  `sessions`.
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

### `[tui.theme]` — palette overrides (ADR 0032)

`conspectus tui` ships a centralized `Theme` covering every color
and modifier the renderer reads. Each field is overridable in
config:

```toml
[tui.theme]
# Per-harness colors used by the row badge and header chip.
harness_claude   = "magenta"
harness_codex    = "cyan"
harness_opencode = "green"
harness_aider    = "red"

# Activity buckets coloring the session recency column.
# Accepts `color`, `color,mod`, or `mod` alone.
recency_fresh  = "bright_green,bold"
recency_active = "green"
recency_recent = "yellow"
recency_cold   = "dim"

# Mux-state glyphs (◉ attached, ◐ ambiguous, ◯ un-muxed).
mux_attached  = "green"
mux_ambiguous = "yellow"
mux_unmuxed   = "dim"

# Background of the chip a mux row shows for its pane's program (`npm`,
# `zsh`, …) when no agent session is linked; text is bold white. A pane
# running a harness, directly or through a launch wrapper such as
# `atelier exec claude`, keeps that harness's badge and color instead.
command_badge = "black"

# Detail-pane and structural cues.
cwd_mark           = "cyan"
link_id            = "blue"
secondary_text     = "dark_gray"   # short ids + preview snippets
disclosure         = "cyan"        # the ▶ / ▼ tree affordance
divider            = "dim"
panel_focus_accent = "cyan"        # the ▸ marker on the active pane title
                                   # and the detail-pane section chip

# PR-state coloring (matches the ADR 0022 table palette defaults).
pr_open   = "green"
pr_closed = "red"
pr_merged = "magenta"
pr_draft  = "yellow"

# Badge composition; default `REVERSED | BOLD` is the chip look.
badge              = "reversed,bold"
# Label characters in every agent / program badge (an integer, at
# least 2). Shorter labels pad, longer ones are cut with `…`.
badge_width        = 8
selection_active   = "reversed,bold"
selection_inactive = "bold"
```

Each value is a single string that may be:

- a named ANSI color: `red`, `magenta`, `cyan`, `bright_green`, …
  plus the alias `default` (or `reset`) for the terminal's default
  foreground;
- an indexed 256-color: `ansi256:42` (0–255);
- a truecolor hex: `#ff8800` (case-insensitive);
- a comma-joined modifier list: `bold`, `dim,italic`, `reversed`,
  `crossed_out`, …;
- a color followed by modifiers: `green,bold`, `#ff8800,italic`.

Unknown keys and malformed values both surface as `ConfigDiagnostic`
warnings; the field falls back to its built-in default and the rest
of `[tui.theme]` continues to load. The TUI never aborts on theme
errors. The `[table]` palette (ADR 0022) is **not** affected by
`[tui.theme]` — table CLI output keeps its own colors.

## CLI Surface

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
conspectus tui [--view {sessions|mux}] [--grouping VALUE] [--sort {hierarchy|recency}]
               [--harness H]... [--max-age D] [--mux-state S]...
               [--scan-root PATH]... [--refresh-interval D] [--no-live-preview]
conspectus declared {list|create|remove|confirm|ignore|override} ...
conspectus rename session <id> [NAME] [--clear] [--no-mux]
conspectus rename mux <id> <NAME>
conspectus alias list
conspectus pin {create|list|show|rename|rm|launch|attach|bind|rebind|adopt} ...
conspectus mux new <NAME> [--cwd PATH] [--socket NAME] [--no-attach]
conspectus mux launch <HARNESS> --name <NAME> [--cwd PATH] [--argv ARG...]
                                              [--worktree-branch B --worktree-repo PATH]
conspectus worktree {list|new|rm|merge|close|prune} ...
conspectus hook {init|status|remove} <claude-code|codex> [--scope {user|project}]
conspectus hook write <HARNESS>          # invoked by the harness hook itself
conspectus serve [--scan-root PATH]...
conspectus refresh [--class {git|mux|harness|forge}]
conspectus status [--format {human|json}]
```

Running `conspectus` with no subcommand opens the TUI. Every command's
`--help` lists its full flag set.

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
compaction summary — separately from the `AGENT` cell. As of H-TBL-015
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
that touches the resolved node. Resolved relationships include the
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
roots.

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

`conspectus alias list [--store project|user|all] [--scan-root PATH]`
prints existing alias entries grouped by store. The output columns are
tab-separated: `<store>\t<path>\t<endpoint>\t<display_name>`.

In the TUI, capital `R` opens an inline rename overlay on the selected
agent-session row. `Enter` confirms, `Esc` cancels; lower-case `r`
continues to mean refresh. The status bar surfaces the result, and
when the target is a live session (mux indicator `Attached` or
`Ambiguous`) appends an informational advisory that the alias
overlays the harness title until the session ends.

## Session pins

Per ADR 0057, **session pins** are user-authored declarations of a
logical agent session — a `(harness, cwd, display_name, mux)` tuple
persisted in a sibling `[[pins.entries]]` TOML table alongside
`[declared]` and `[aliases]`. Pins render as first-class dashboard
rows whether or not a live session realizes them, bind 1:1 on the
mux native name to a discovered harness session, and can launch the
configured harness into a fresh tmux session on demand.

### Authoring

```sh
conspectus pin create <id> --harness <key> --cwd <PATH>
    [--display <name>] [--mux-name <name>] [--mux-socket <name>]
    [--launch-arg <arg>...] [--reason <text>]
    [--store project|user]

conspectus pin list [--store all|project|user] [--state all|bound|unbound|stale]
conspectus pin show <id>
conspectus pin rename <id> [<new-id>] [--display <name>]
conspectus pin rm <id>
```

`pin create` writes to the nearest project-local `.conspectus.toml`
by default; `--store user` lands the entry in the user-level config
instead. The pin's `display_name` doubles as the bound session's
alias overlay (ADR 0029 precedence) and as the initial tmux session
name at launch. `--mux-socket <name>` is the tmux `-L <socket>`
equivalent — pins on non-default sockets launch and attach
correctly in v1, though discovery-side enumeration of non-default
sockets is a deferred follow-up (H-PIN-F-001) so they render as
`unbound` even when their tmux session is live.

### Launch / attach

```sh
conspectus pin launch <id> [--no-attach]
conspectus pin attach <id> [--no-attach]
```

`pin launch` resolves the binding state and branches:

- **Bound** → attaches to the live mux via `tmux attach-session`
  (or `tmux switch-client` inside an existing tmux client).
- **Stale-mux** → injects the configured argv into the existing pane
  via `tmux send-keys ; Enter`, then attaches. Preserves the
  operator's window/pane layout. If the pane already runs the pin's
  harness (its session just isn't identified), launch only attaches
  (ADR 0102). If the pane is dead, launch replaces the session as
  for an unbound pin (ADR 0103).
- **Unbound** → spawns `tmux new-session -d -s <name> -c <cwd>
  <argv>` then attaches.

Sessions Conspectus creates keep a pane whose process exits non-zero
(`remain-on-exit failed`, ADR 0103). After spawning, launch watches the
pane for up to 0.5 s (1.5 s when resuming). If the harness dies in that
window, launch reports its output. A failed resume is retried as a
fresh launch and the pin's resume sidecar is cleared; a failed fresh
launch exits non-zero and removes the dead session. A harness that
fails later stays visible as a dead pane until the next launch
replaces it.

The default argv comes from the harness adapter's `launch_argv`
(per `HarnessAdapter::launch_argv`); pass `--launch-arg <arg>...`
on `pin create` to override on a per-pin basis. `--no-attach`
short-circuits the terminal hand-off so scripts can spawn detached
and print the equivalent attach command.

`pin attach` is the same flow with a different intent label — an
unbound pin under `pin attach` falls through to launch with a
one-line "note: unbound; falling through to launch" hint.

### Operator escape hatches

```sh
conspectus pin bind <id> --to <SESSION_KEY> [--reason <text>] [--store project|user]
conspectus pin rebind <id> --mux <NEW_NAME> [--mux-socket <NAME>]
conspectus pin adopt <new-id> <existing-mux-name>
    [--harness <key>] [--display <name>] [--mux-socket <name>] [--cwd <PATH>]
    [--store project|user]
```

- `pin bind` resolves `PinAmbiguous` by writing a
  `LocalDeclared linked_to_mux` link tagged with `label =
  "pin:<id>"`. The resolver's precedence pipeline picks the
  override automatically.
- `pin rebind` updates the pin's `mux.name` (and optionally
  `mux.socket_name`) after an external tmux rename. Pure TOML
  mutation — does not touch tmux.
- `pin adopt` is the agent-deck migration path: takes the name of
  an existing live tmux session and writes a pin whose `mux.name`
  matches, inferring `harness` from current attribution and `cwd`
  from the mux's observed working directory. Pass `--harness` and
  `--cwd` to override either inference.

### Diagnostics

The resolver emits four pin-specific diagnostics:

- **PinUnbound** — `pin.mux.name` does not match any live mux.
  Action: `pin launch <id>` to spawn.
- **PinStaleMux** — mux is live but no `pin.harness` session is
  attributed. Action: `pin launch <id>` to re-inject the harness
  into the existing pane.
- **PinAmbiguous** — multiple sessions of the right harness are
  attributed to the bound mux. Action: `pin bind <id> --to
  <session-key>` to pick one explicitly.
- **PinDrift** — bound session's observed cwd diverges from the
  pin's declared cwd. Advisory; binding still holds.

`conspectus pin show <id>` surfaces each diagnostic on its own
`diagnostic   <text>` line so the operator can pipe / grep the
output. In the TUI, selecting a pin row shows a per-binding-state
hint in the status bar; `Enter` on a pin row exec-spawns
`conspectus pin launch <id>` as a subprocess so the launch logic
stays in one place.

### TUI controls

Pin CRUD lives in its own modal, separate from the view/grouping/
filter controls overlay (`f`). Every action also has a direct
shortcut so the modal is the discoverable surface, not a required
step.

Direct shortcuts:

- `Enter` on a pin launches/attaches through `conspectus pin launch`.
- `L` launches the selected pin via the same code path as `Enter`;
  useful when muscle-memory wants a distinct key independent of the
  row's default action. Refuses with a status hint when no pin row
  is selected.
- `N` opens the create form seeded from the current selection
  (harness/cwd/display from a selected session, cwd from a selected
  group, or harness/cwd/mux from a selected mux row).
- `R` opens the display-name edit on a pin row (same key as session
  rename).
- `B` opens the mux-only rebind form for the selected pin —
  `mux.name` and optional `mux.socket_name`, matching the CLI's
  `pin rebind` scope.
- `b` opens the bind picker when the selected pin has a
  `PinAmbiguous` diagnostic; surfaces a status hint otherwise.
- `A` adopts the selected live mux row as a new pin. Refuses with a
  status hint on any other row kind because the form needs the
  mux's name, observed cwd, and harness as seeds.
- `Delete` opens a two-press confirmation before removing the pin.

Pins modal (`p`):

- `p` opens the discoverable menu listing
  `create / launch / rename / remove / bind / rebind / adopt`. `↑/↓`
  navigate, `Enter` opens the form (or in the case of `launch` /
  `bind`, executes directly) for the chosen action, `Esc` closes the
  modal. Each form is the same one the direct shortcut opens, so the
  two surfaces stay 1:1.
- Entries that need a pin selection (`launch`, `rename`, `remove`,
  `rebind`) surface a status hint instead when no pin row is
  selected, and `bind` only opens its picker when the resolver
  flagged `PinAmbiguous` candidates.

Static scenario TUIs and read-only navigation paths keep these
mutations disabled; they surface a status message instead of writing.

### Session continuity

Per ADR 0058, every fresh discovery cycle records the resolver's
`Bound` pin → session attribution to a per-pin JSON sidecar under
`$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`. The sidecar
is a *rebuildable cache*, not authoritative state: the resolver
never reads it, and the binding shown in `pin show` / the TUI
always reflects the current cycle's observation, not the cache.

When a pin's mux dies (operator closes the tmux session, machine
reboots, etc.) and discovery flips the pin to `PinUnbound`,
`pin launch <id>` consults the sidecar before falling back to a
fresh start:

1. Read the sidecar. Absent → default argv.
2. Look up the recorded session in the current snapshot. Missing
   on disk → **delete the sidecar** and fall back to default argv.
3. Walk forward through ADR 0018 `parent_session` lineage to the
   current head. If the chain forks (multiple successors share an
   ancestor), refuse to disambiguate — fall back to default argv
   with a `multiple successors` hint.
4. Look up `HarnessAdapter::resume_argv(<head>, <cwd>)`. `None`
   (the harness has no resume CLI, e.g. aider today) → fall back
   to default argv with a hint. `Some(argv)` → splice into the
   tmux `new-session` call.

The `PinUnbound` diagnostic carries an optional `last_session`
field populated from the sidecar so downstream consumers can
advertise the resume affordance without re-reading the cache:

- `pin show <id>` adds a `last_session <session-id> (observed
  <iso8601>)` line for unbound pins with a recorded prior session.
- The TUI status hint reads `Enter resume <session-id>` instead
  of `Enter launch` when `last_session` is populated.
- The right detail pane mirrors the same with an `Enter resume`
  annotation.

**Per-harness support.** Codex (`codex exec --resume <id>`), Claude
Code (`claude --resume <id>`), and opencode (`opencode --session
<id>`) expose resume commands and participate fully. Aider tracks
chat history per-cwd rather than per-session, returns `None` from
`resume_argv`, and its unbound pins always launch fresh with a
status hint naming the missing capability.

**Sidecar lifecycle.** Files are written atomically (tempfile +
rename) and use a `skip-on-unchanged` comparison so quiet cycles
produce no mtime churn. Stale entries (session deleted) are
cleared at launch time by the consumer rather than on a TTL —
the cache self-prunes as the operator drives it.

### Read-only invariant

`graph`, `node show`, `table <rows>`, `pin list`, and `pin show`
never create, mtime-touch, or content-modify
`.conspectus.toml` / user-config files bearing a `[pins]` section.
Mutation is reserved to the explicit `pin create / rename / rm /
bind / rebind / adopt` commands and the TUI write paths they back.
H-PIN-019's `tests/cli_pin_invariants.rs` enforces this with
content + mtime fingerprinting around each read-only command.

The same commands also leave the pin-binding sidecar cache alone
when no `Bound` resolution is in play: a command run against a
config with only unbound pins (no live mux yet) will not create
`$XDG_CACHE_HOME/conspectus/pin-bindings/`, and an existing
sidecar for an unbound pin is preserved byte-for-byte (no mtime
bump) across read-only commands. The positive case — a `Bound`
resolution producing a sidecar write — is by design, since the
sidecar is what powers the next `pin launch`'s continuity.
H-PIN-RESUME-006's `tests/cli_pin_resume_invariants.rs` enforces
the cache-side rules.

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
- **The pin-binding sidecar** described under
  [Session continuity](#session-continuity) — per-pin JSON
  files under `$XDG_CACHE_HOME/conspectus/pin-bindings/`.
  The sidecar is fully rebuildable from a fresh discovery
  cycle, so clearing it (`rm -r`) only loses continuity
  until the next `pin launch` from a bound state.
- **The pin-store registry** at
  `$XDG_STATE_HOME/conspectus/pin-stores.json` (ADR 0090).
  `pin create`, `pin adopt`, and the TUI's pin-create action
  record the project store they wrote to, so pins in repos
  outside the current scan roots stay visible. Entries whose
  store no longer exists are pruned; clearing the file only
  hides such out-of-root pins until they are next written.

Future caches (PR fetches, transcript indices, etc.) land
under the same `$XDG_*_HOME/conspectus/` roots rather than
inside `.conspectus.toml` or the project config directory.

## TUI messages

Every operation the TUI runs (pin launch, mux new/launch, attach,
renames, pin-store and worktree changes, refresh and daemon failures)
is logged in memory for the life of the TUI process (ADR 0105). Press
`!` to open the Messages overlay: newest entries first, with the
selected entry's full record below, including argv, exit status,
stderr and stdout for subprocesses, and the pane output when a
harness died in its pane. `j`/`k` select, `J`/`K` or PgUp/PgDn scroll
the record, `y` copies it, `Esc` closes.

A warning or error also:

- sets the status message to its summary plus `· ! details`;
- keeps a `⚠ N · ! messages` chip at the start of the status bar until
  you open the overlay;
- leads the Preview of the row it concerns (its pin or mux) until a
  later operation on that row succeeds.

The log is never written to disk, because pane output and harness
stderr can contain conversation content.

## TUI state

The TUI persists the operator's last-active view at
`$XDG_STATE_HOME/conspectus/tui-state.json` (sibling to the hook
state-root under `$XDG_STATE_HOME/conspectus/hooks/`). The file
is rebuildable cache, not authoritative state — losing it costs
nothing more than starting the next `conspectus tui` in the
configured default view instead of the one the operator last
switched to.

Schema v1:

```json
{
  "schema_version": 1,
  "last_view": "sessions"
}
```

`last_view` is one of `sessions`, `mux`, `union`, `prs`, or
`forks`. Unknown values, malformed JSON, or a missing file all
collapse to "no persisted view" without surfacing an error;
unknown fields round-trip through writes so future schema bumps
do not strand old payloads.

**Startup precedence:** explicit `--view <name>` flag wins →
persisted `last_view` wins → config `[tui].default_view` →
built-in `View::Sessions`.

**Opt-outs:** pass `--no-resume-view` to ignore the persisted
value for a single run (useful for scripts and tests that need a
deterministic starting view). The `--snapshot` dev path implies
`--no-resume-view` so snapshot regeneration never depends on
whatever view the operator last touched outside the fixture.

**Read-only invariant:** only `conspectus tui` reads or writes
the file. The `graph`, `table`, `node show`, and `pin *`
surfaces all leave it byte-identical, enforced by
`tests/cli_tui_state_invariants.rs`. Writes are atomic
(tempfile + rename) and skip-on-unchanged so quiet TUI sessions
produce no mtime churn.

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
(declared links per ADR 0014, pins per ADR 0057), user-level
config files (aliases per ADR 0029), and the pin-binding
sidecar under `$XDG_CACHE_HOME/conspectus/pin-bindings/` all
keep their existing shapes and locations. Phase 11 changed
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
