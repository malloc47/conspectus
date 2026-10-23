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

Install a harness hook automatically:

```sh
conspectus hook init claude-code   # a SessionStart entry in ~/.claude/settings.json
conspectus hook init codex         # a hook entry in ~/.codex/config.toml
conspectus hook init pi            # <agent-dir>/extensions/conspectus-hook.ts
```

`--scope project` installs into the current project instead
(`.claude/`, `.codex/`, `.pi/extensions/`). The commands are idempotent.
`conspectus hook status <harness>` reports whether the hook is present,
and `conspectus hook remove <harness>` removes only what Conspectus
installed. The opencode hook is a plugin; see
[`plugins/opencode-hook/README.md`](../plugins/opencode-hook/README.md).

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

Pi has no hook settings, so its hook is a TypeScript extension that
`hook init pi` writes as one file (ADR 0121). Restart Pi or run
`/reload` to load it; Pi asks you to trust a project before it loads
project extensions. The extension reports each interactive session
start, including `/new`, `/resume`, `/fork`, and `/clone`, through
`conspectus hook write pi`, and skips `--no-session` runs and Pi's
print, JSON, and RPC modes. Besides `installed` and `not-installed`,
`hook status pi` reports `outdated` (an older copy) and `foreign` (a
file at that path Conspectus didn't write, which `init` and `remove`
leave alone). See [`plugins/pi-hook/README.md`](../plugins/pi-hook/README.md).

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
default_view = "sessions"   # sessions | mux | prs | pinned (union | forks also accepted)
scan_roots = ["~/work"]     # tui and serve, when --scan-root is absent; `~` expands
show_harness_chips = false  # per-harness count chips in the header
narrow_layout_threshold = 100  # columns below which the panes stack
preview_wrap = "smart"      # smart | plain | none — how the Preview fits a mux pane
session_preview = "transcript"  # transcript | pane — the Preview for a session in tmux
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
  `repo`, `workspace` (ADR 0139); union `kind`, `repo`; prs `checkout` (default), `repo`, `state`
  (ADR 0114); forks `provider`,
  `parent`; pinned `project` (default), `kind`, `none` (ADR 0141).
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
- **Forge concurrency:** `[forge] concurrency` (default 4) sets how many
  `gh` calls run at once. Higher fetches faster but can trip a forge's
  rate limits.
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
harness_pi       = "bright_blue"

# Activity buckets coloring the session recency column.
# Accepts `color`, `color,mod`, or `mod` alone.
recency_fresh  = "bright_green,bold"
recency_active = "green"
recency_recent = "yellow"
recency_cold   = "dim"

# Mux-state glyphs (◉ attached, ◯ not attached or no definitive mux).
mux_attached  = "green"
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
conspectus tui [--view {sessions|mux|prs|pinned}] [--grouping VALUE] [--sort {hierarchy|recency}]
               [--harness H]... [--max-age D] [--mux-state S]...
               [--scan-root PATH]... [--refresh-interval D] [--no-live-preview]
conspectus declared {list|create|remove|confirm|ignore|override} ...
conspectus rename session <id> [NAME] [--clear] [--no-mux]
conspectus rename mux <id> <NAME>
conspectus alias list
conspectus pin {add|edit|rename|retarget|refresh|move|rm|list|show|migrate} ...
conspectus rebuild <REF> [--dry-run] [--no-attach] [--session ID]
conspectus history {list|show|reap|forget} ...
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

In the TUI, capital `R` opens an inline rename overlay on the selected
agent-session row. `Enter` confirms, `Esc` cancels; lower-case `r`
continues to mean refresh. The status bar surfaces the result, and
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
the pin alone. `FIELD=VALUE` lists (such as `argv`) are split like a shell command
line. Pinning a live mux, worktree, or repo first records it in
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
conspectus rebuild <pin | h_... | node id | mux name> [--dry-run] [--no-attach] [--session ID]
```

A rebuild is a plan of steps. `--dry-run` prints it and stops; a real
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
cut at 28 cells, the status is one word (`live`, `gone`, `unknown`,
`planned`, `missing`, `in mux`, `resumable`, or a PR's state), the age is the
last activity of a live entity or the time a gone one was last seen, and the
content is the harness and place, the title, or the remote. Gone, missing,
planned, and unknown rows are dimmed. The agent or program that runs is a
filled chip, the same as in the other views, ahead of the text that follows
it. A pin with nothing running yet (`planned`) wears a hollow pin glyph, `◎`,
in the dimmed ghost color after its kind glyph; the filled `◉` is for an
entity a pin points at. `gone` means an entity that existed and is no longer
running, and never describes an unstarted pin. Columns pad to the widest
value in the whole tree, so folding a group doesn't move them. Changing the
grouping scrolls back to the top so every group is visible.

Pins group by `project` (the repo above the pin's cwd or path; the rest go
under Other), by `kind`, or not at all (`Ctrl-G` cycles), and sort in pin order or,
with the Recency sort, freshest first. A mux pin expands to the sessions it
hosts, and a checkout pin to its muxes and their sessions. The detail pane
shows the pin, its target, and each effective field with the layer that
supplied it (override, live, history, snapshot, or default).

`Enter` attaches a pin whose mux is live (or a session pin whose session is
in a mux). On a gone, missing, or planned mux, session, worktree, or repo pin,
it runs `conspectus rebuild <pin> --no-attach` behind a spinner and refreshes;
press `Enter` again to attach. A live session that isn't in a mux is rebuilt
the same way, which resumes it. A PR pin shows its URL, and the other live
kinds say there is nothing to launch. While legacy `[[pins.entries]]`
remain, a line under the rows names `conspectus pin migrate`.

`.` opens the pin menu: **Rebuild**, **Show rebuild plan**, **Edit…** (the
edit form, below), **Rename…** (a live mux
pin renames its tmux session with it and pins the name, as `pin rename`
does), **Retarget…** (type a node id, a history id, or a mux name of the same
kind), **Refresh snapshot**, **Reveal in its view** (switches to Sessions, Mux,
or PRs and selects the entity; a gone entity opens in History), and **Unpin** (press `Enter` twice). Each is
the same write as the `pin` command of that name. Legacy pins refuse edits
until they are migrated.

### Pinning from the other views

`p` pins the selected row in any view, and unpins it (after a confirming menu)
when a pin already points at it; `.` offers the same as **Pin** or **Unpin
\<pin\>** at the top of every row's menu. A runtime process can't be pinned.
Pinning a live mux, worktree, or repo records it in history first so the pin
has something to point at. The native views mark a pinned row with `◉` after
its kind glyph and leave it where it sorts; once any pin exists the marker
cell is reserved on every row so columns hold still. Controls has a **Pinned
only** checkbox in Sessions, Mux, and PRs that keeps pinned rows and the
groups above them.

**Edit…** opens a form with one row per field the pin can change: its name
first (renaming a live mux pin renames its tmux session), then the label and
the fields its kind can override, the same names `pin edit --set` takes. Each
row shows the value in effect and where it comes from (`override`, `live`,
`history`, `snapshot`, or `default`). Typing in a row makes an override;
`Ctrl-R` drops the row's override so it follows the entity again. Paths
complete with `Tab`, the harness row lists the known harnesses (`←`/`→` choose
one), and a harness's launch options, such as skipping permissions, are
checkboxes under it that add or remove their flags on the `argv` row. `↑`/`↓`
move between rows, `Enter` saves, and `Esc` cancels.

The launch and new mux forms work the same way: `↑`/`↓` move between fields,
`Tab` completes a directory in a path field (offering directories from
the graph as well as the filesystem) and moves on elsewhere, `←`/`→`/`Space`
choose among the harnesses, options, and toggles, and a line shows the command
the form will run.

`N` opens the mux launch form with the worktree toggle on, and `m` then
**Launch harness in new mux…** opens it with the toggle off. An empty `launch
argv` resolves to the harness's default (a project or user `[launch.*]`
profile, `atelier exec`, or the harness default) and the form shows the
command and where it came from. The last field, **pin**, can also pin the new
mux or, with *pin, don't launch*, write a planned pin and start nothing.

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
pinned records too (they carry `◉`). Revealing a gone pin's entity from the
Pinned view clears it for you.

`Enter` runs `conspectus rebuild <record id> --no-attach` behind a spinner and
refreshes. `.` opens the row menu: **Rebuild**, **Pin** (the same as `pin add
<record id>`; **Unpin \<pin\>** once a pin points at it), **Show rebuild plan** (the output of `rebuild --dry-run` in a
scrollable modal), **Forget** (press `Enter` twice; the record leaves history
and any pin pointing at it falls back to its snapshot), and **Clean up…**.

Clean up lists what `conspectus history reap` would remove: gone records past
their `[history.reap_after]` age, minus records a pin targets. All are
selected; `Space` deselects, `Enter` forgets exactly the selected ones, and
`Esc` cancels. With nothing past its age the modal says so. Forgetting and
cleaning up only remove history records; they never touch tmux, git, or the
filesystem, and they are the same writes as `history forget` and `history
reap`.

### Ghost muxes in the Mux view

The Mux view lists a gone mux below the live muxes of its group, dimmed, with
a `gone` status and the time it was last seen; the header reads `11 mux ·
4 gone`. A mux whose probe couldn't run shows as `unknown` and isn't counted
as gone. Controls has a **Hide gone muxes** checkbox, and any narrowing filter
(harness, max age, mux state) leaves ghosts out because they can't match.
`Enter` on a ghost rebuilds it, `.` opens the same menu as in History, and
attach and rename refuse with a message that names the fix.

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

## TUI messages

Every operation the TUI runs (rebuild, mux new/launch, attach,
renames, pin and worktree changes, refresh and daemon failures)
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

`last_view` is one of `sessions`, `mux`, `union`, `prs`,
`forks`, or `pinned`. Unknown values, malformed JSON, or a missing file all
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
