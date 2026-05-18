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

The current schema is small:

```toml
[session]
projection = "agent"  # one of "agent" | "mux" | "union"; default "agent"
```

Unknown sections and unknown keys are ignored. Malformed TOML and
invalid values for known keys surface as `ConfigDiagnostic`s on
stderr but do not abort the run.

## CLI Surface

```sh
conspectus graph --format json [--scan-root PATH]...
conspectus session [--projection {agent|mux|union}] [--layout {columnar|card}]
                   [--wide | --width N] [--scan-root PATH]...
conspectus node show <id> [--scan-root PATH]...
conspectus declared ...
```

`session` without `--projection` uses the value loaded from
`.conspectus.toml` / user config (defaulting to `agent`). Width
detection: when stdout is a TTY the table truncates to the detected
terminal width; pipes default to wide so `conspectus session | grep`
remains useful. `--wide` forces untruncated output even on a TTY, and
`--width N` pins an exact width for reproducible captures. `--layout
card` renders one column per line per row with blank-line separators,
useful when the columnar form would truncate (long `CWD`, long PR
identifier).

`node show <id>` accepts any of:

- The short content-addressed prefix from the session table's `ID`
  column. Any prefix length ≥ 4 hex chars is accepted; an ambiguous
  prefix errors with the matching candidates listed.
- The full `NodeId` display form, e.g.
  `agent_session:codex:/state:session-x` or `mux_session:tmux:editor`.
- The harness/mux label that appears in the session table's `AGENT`
  or `MUX` column, e.g. `codex:session-x` or `tmux:editor` — when the
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
