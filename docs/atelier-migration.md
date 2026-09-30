# Atelier Migration Guide

Conspectus is the observability surface for local agent work: graph
discovery, session tables, mux association, fork context, and forge PR
evidence. Atelier remains the workspace/fork authoring tool.

This guide maps overlapping Atelier commands to the Conspectus commands
that cover the same inspection workflow.

## Command Mapping

| Atelier workflow | Conspectus replacement | Notes |
| --- | --- | --- |
| `atelier session list` | `conspectus table sessions` | One row per agent session, with preferred mux, fork, branch, and PR context when discovered. |
| `atelier mux status` | `conspectus table mux` | One row per tmux session, with attached agent counts. |
| forge-related status surfaces | `conspectus table prs` or `conspectus graph --format json` | The PR table shows each PR with its branch context; graph JSON preserves all PR nodes, branch links, unresolved endpoints, and diagnostics. |
| graph-heavy parts of `atelier status` | `conspectus graph --format json` | Graph JSON is the stable machine-readable surface for repos, checkouts, branches, workspaces, forks, sessions, mux sessions, and PRs. |

Use `--scan-root PATH` when the command should inspect a workspace or
repo other than the current working directory:

```sh
conspectus table sessions --scan-root /work/example
conspectus table mux --scan-root /work/example
conspectus graph --format json --scan-root /work/example
```

## Behavioral Differences

Conspectus discovery is read-only. It never creates forks, edits Atelier
metadata, or writes provider state. Its writes are limited to the mutation
envelope in ADR 0087: user-authored intent in `.conspectus.toml` or user
config (`declared` links, aliases, pins), rebuildable caches under the XDG
directories, and tmux sessions or worktrees you ask it to create or tear
down.

Conspectus keeps ambiguous evidence visible. A table row shows the
preferred relationship, while `graph --format json` retains weaker,
ignored, overridden, ambiguous, or unresolved candidates for tools that
need the full evidence set.

## Runtime Knobs

The environment variables below are documented in `docs/operations.md`
and are useful during Atelier migration tests:

| Variable | Effect |
| --- | --- |
| `CONSPECTUS_DISABLE_TMUX` | Skip tmux discovery. |
| `CONSPECTUS_DISABLE_FORGE` | Skip GitHub PR discovery through `gh`. |
| `CONSPECTUS_CODEX_STATE` | Override the codex state root. |
| `CONSPECTUS_CLAUDE_CODE_STATE` | Override the Claude Code state root. |
| `CONSPECTUS_OPENCODE_STATE` | Override the opencode state root. |

Library consumers should prefer explicit roots and injected runners via
`conspectus::api::LocalDiscoveryConfig` rather than depending on process
environment in tests.

## Library Integration

Atelier-side delegation should depend on the curated facade introduced
for Phase 6:

```rust
use conspectus::api::{
    LocalDiscoveryConfig, discover_local_with, render_graph_json,
    resolve_snapshot,
};
```

For table output, call `conspectus::api::table::render` with a
`conspectus::api::Projection`. For deterministic tests, construct
`LocalDiscoveryConfig::empty()` and inject any needed tmux or forge
runners.

See `docs/library-api.md` for the pure/impure module inventory and ADR
0015 for the stable API contract.
