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

| Variable                    | Effect                                                                                     |
| --------------------------- | ------------------------------------------------------------------------------------------ |
| `CONSPECTUS_DISABLE_TMUX`   | When set (any value), the tmux provider is skipped entirely. No `MuxSession` nodes appear. |
| `CONSPECTUS_DISABLE_FORGE`  | When set (any value), the GitHub forge provider is skipped. No `ForgePr` nodes appear.    |

Use these in CI, automated tests, or shells where running `tmux
list-sessions` / `gh pr list` is unwanted, slow, or noisy. Both
providers are otherwise best-effort: missing binaries, unauthenticated
runs, and command failures degrade silently to "no rows for that
provider" rather than aborting the whole graph.

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
session id without terminal input. Conspectus stores them in a local
SQLite database under:

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

The schema is keyed on the `[table]` parent with one subsection per
row-type rendered by `conspectus table <ROWS>` (see ADR 0021):

```toml
[table.sessions]
# Per-row-type knobs land here; H-TBL-007 adds `columns = [...]`.

[table.mux]

[table.union]
```

Unknown sections and unknown keys are ignored. Malformed TOML
surfaces as a `ConfigDiagnostic` on stderr but does not abort the
run. The legacy `[session]` section (pre-ADR 0021) is recognized
solely to emit a one-line diagnostic pointing at the new schema; its
contents are ignored.

## CLI Surface

```sh
conspectus graph --format json [--scan-root PATH]...
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
conspectus declared ...
```

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
manually if needed.

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
that touches the resolved node.

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

## Caches

Conspectus does not maintain a machine-generated cache yet. When a
cache is introduced (per CLAUDE.md), it will live outside the project
tree under `$XDG_CACHE_HOME/conspectus/` (or the platform
equivalent) rather than inside `.conspectus.toml` or the project
config directory.
