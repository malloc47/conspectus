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
conspectus node show <id> [--scan-root PATH]...
conspectus columns {sessions|mux|union|prs|forks}
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

## Caches

Conspectus does not maintain a machine-generated cache yet. When a
cache is introduced (per CLAUDE.md), it will live outside the project
tree under `$XDG_CACHE_HOME/conspectus/` (or the platform
equivalent) rather than inside `.conspectus.toml` or the project
config directory.
