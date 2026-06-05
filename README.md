# Conspectus

Conspectus is a Rust CLI and library for surveying local AI-agent work across
sessions, muxes, repos, checkouts, workspaces, forks, branches, and forge PRs.
It reads local state, records relationship evidence in a provider-neutral graph,
and renders deterministic JSON or compact session tables.

Conspectus is read-only except for explicit `conspectus declared`,
`conspectus alias`, `conspectus rename`, and `conspectus pin`
commands, which store user-authored intent in `.conspectus.toml` or
user config.

## CLI

```sh
conspectus
conspectus tui [--view {sessions|mux|union|prs|forks}] [--scan-root PATH]...
conspectus graph --format {json|dot|html} [--scan-root PATH]...
                                          [--candidates {include|exclude}]
                                          [--diagnostic-nodes {include|exclude}]
conspectus session [--projection {agent|mux|union}] [--scan-root PATH]...

conspectus declared list [--store {all|project|user}] [--scan-root PATH]...
conspectus pin {create|list|show|launch|attach|bind|rebind|adopt|rename|rm} ...
```

Running `conspectus` without a subcommand opens the interactive TUI.
`graph` emits the full evidence-preserving graph document: `--format json`
is machine-readable, `--format dot` pipes through Graphviz for static
inspection, and `--format html` produces a self-contained interactive
explorer (see [graph visualization guide](docs/graph-visualization.md)).
`session` renders
agent-, mux-, or union-oriented table projections after resolution. The
`declared` subcommands can pin, ignore, remove, confirm, or override
relationships. `conspectus pin` (ADR 0057) declares a session pin — a
`(harness, cwd, display_name, mux)` tuple persisted in
`.conspectus.toml` — that renders as a first-class dashboard row
whether or not a live session realizes it, binds 1:1 on the mux
native name through the existing attribution pipeline, and can
launch the configured harness into a fresh tmux session on demand.
This replaces the agent-deck "new card" workflow without inheriting
the broader orchestrator scope; see
[`docs/operations.md`](docs/operations.md#session-pins) for the full
command surface and the agent-deck migration path via `pin adopt`.

## Docs

- [Feature summary](docs/feature-summary.md) describes the current CLI,
  discovery providers, declared-link behavior, and known limits.
- [Operations](docs/operations.md) documents runtime environment variables,
  provider toggles, state-root overrides, and config-file precedence.
- [Graph visualization](docs/graph-visualization.md) covers `--format dot`
  and `--format html`, the HTML explorer chrome, and common debugging
  recipes.
- [Library API](docs/library-api.md) describes stable entry points and
  pure/impure boundaries for consumers.
- [Atelier migration guide](docs/atelier-migration.md) maps overlapping Atelier
  observability commands to Conspectus replacements.
- [ADR index](docs/adr/) records architecture decisions, including the Phase 6
  library API and distribution policies.
- [Design notes](docs/design.md) remain as historical and forward-looking design
  context.

## Development

Use the Nix flake for a local development shell:

```sh
nix develop
```

The shell provides Rust tooling, `cargo-nextest`, `just`, `pre-commit`, tmux,
and GitHub CLI.

Common checks:

```sh
just check
cargo doc --no-deps
```
