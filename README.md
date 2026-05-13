# Conspectus

Conspectus is a planned standalone CLI for surveying local AI-agent work across
sessions, muxes, repos, worktrees, workspaces, forks, branches, and forge PRs.

The project is currently in design. See:

- [docs/design.md](docs/design.md) for the working product and data-model plan
- [docs/naming.md](docs/naming.md) for the naming history

## Development

Use the Nix flake for a local development shell:

```sh
nix develop
```

The shell provides Rust tooling, `cargo-nextest`, `just`, `pre-commit`, tmux,
GitHub CLI, and Beads from `numtide/llm-agents.nix`. Beads is available as
`bd`.
