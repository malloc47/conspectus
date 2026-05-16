# Conspectus

Conspectus is a Rust CLI and library for surveying local AI-agent work across
sessions, muxes, repos, worktrees, workspaces, forks, branches, and forge PRs.
It reads local state, records relationship evidence in a provider-neutral graph,
and renders deterministic JSON or compact session tables.

Conspectus is read-only except for explicit `conspectus declared` commands,
which store user-authored relationship intent in `.conspectus.toml` or user
config.

## CLI

```sh
conspectus graph --format json [--scan-root PATH]...
conspectus session [--projection {agent|mux|union}] [--scan-root PATH]...

conspectus declared list [--store {all|project|user}] [--scan-root PATH]...
```

`graph` emits the full evidence-preserving graph document. `session` renders
agent-, mux-, or union-oriented table projections after resolution. The
`declared` subcommands can pin, ignore, remove, confirm, or override
relationships.

## Docs

- [Feature summary](docs/feature-summary.md) describes the current CLI,
  discovery providers, declared-link behavior, and known limits.
- [Operations](docs/operations.md) documents runtime environment variables,
  provider toggles, state-root overrides, and config-file precedence.
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
