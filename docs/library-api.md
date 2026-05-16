# Library API Inventory

This inventory supports Phase 6 delegation from Atelier to Conspectus.
ADR 0015 defines the stable public modules; this page names the practical
entry points and records where process-global or filesystem side effects
exist.

## Consumer Workflow

Most consumers should start with the curated API facade:

```rust
use conspectus::api::{
    LocalDiscoveryConfig, discover_local_with, render_graph_json,
    resolve_snapshot,
};

let graph = discover_local_with(["/workspace"], LocalDiscoveryConfig::empty())?;
let graph = resolve_snapshot(graph);
let json = render_graph_json(&graph)?;
# anyhow::Ok(())
```

For table output, call `output::table::render` with a resolved snapshot
and a `config::Projection`. For declared-link mutations, use the helpers
in `declared`; read-only graph and session commands should not call those
write helpers.

## Pure Modules

Pure modules do not read process environment, shell out, inspect the
current directory, or mutate global state when called with explicit
inputs.

| Module | Classification | Stable entry points |
| --- | --- | --- |
| `model` | pure data model | `GraphSnapshot`, `GraphNode`, `GraphLink`, typed IDs, relation/provenance/confidence enums |
| `resolve` | pure resolver | `resolve_snapshot`, `resolve_links` |
| `output` | pure renderer | `render_graph_json` |
| `output::table` | pure renderer | `render`, `Projection` re-export |
| `discovery::cross_link` | pure graph enrichment | `infer` |
| `discovery::declared` | pure graph enrichment when loader is injected | `apply_declared_links` |
| `discovery::forge::github` | pure parser/fragment builder except tests | `GhPullRequestParser`, `RepoContext`, `fragment_for_repo`, `parse_github_remote` |
| `discovery::harness::{aider,claude_code,codex,opencode}` | pure adapter logic over explicit state roots | adapter `new` constructors and `HarnessAdapter` implementations |
| `discovery::harness::fixtures` | test-support only | fixture writers for Conspectus tests |
| `discovery::atelier` | pure Atelier metadata parser and fragment builder; filesystem reads happen only through explicit paths | `AtelierWorkspaceConfig::load`, `AtelierForkIndex::load`, `fork_records_fragment`, record types |
| `discovery::workspace` | pure provider over explicit scan roots | `GenericWorkspaceDiscovery::new` |

## Impure Boundary Modules

Impure modules touch process environment, current working directory,
external commands, or disk writes. Consumers should inject dependencies
where possible.

| Module | Boundary | Stable entry points and injection points |
| --- | --- | --- |
| `cli` and `main` | current directory, stderr/stdout, argument parsing | application-only; not a stable library surface |
| `config` | `ConfigLoader::from_env` reads environment; `load_from_cwd` reads current directory; normal loading reads config files | prefer `ConfigLoader::new().with_home(...).with_xdg_config_home(...)` plus explicit `load_from(cwd)` in tests |
| `declared` | mutation helpers write TOML files atomically; temp filenames include process id | use read/write helpers only for explicit declared-link commands |
| `discovery` | `DiscoveryContext::from_current_dir` reads current directory; `LocalDiscoveryConfig::from_env` reads env and installs real runners | prefer `DiscoveryContext::from_roots` and `LocalDiscoveryConfig::empty` plus explicit runners in tests |
| `discovery::git` | `GitProbe` shells out to `git`; `GitDiscovery` uses probes over explicit roots | call `fragment_from_probe` for pure tests, or run `GitDiscovery` only where `git` is allowed |
| `discovery::tmux` | `SystemTmux` shells out to `tmux` | inject a `TmuxRunner`; parse rows with `parse_list_sessions` for pure tests |
| `discovery::forge` | `SystemGh` shells out to `gh`; `ForgeDiscovery` asks adapters to inspect repos | inject a `GhRunner`; use `GhPullRequestParser` and `github::fragment_for_repo` for pure tests |

## Stable Entry Points

- Discovery orchestration:
  `discover_local_with`, `discover_local_at_roots`,
  `DiscoveryContext`, `LocalDiscoveryConfig`, `LocalDiscovery`,
  `DiscoveryProvider`, `GraphFragment`, `merge_fragments`.
- Injectable process seams:
  `tmux::TmuxRunner`, `tmux::TmuxOutcome`, `tmux::SystemTmux`,
  `forge::GhRunner`, `forge::GhOutcome`, `forge::SystemGh`.
- Pure parsers and builders:
  `git::fragment_from_probe`, `tmux::parse_list_sessions`,
  `forge::github::GhPullRequestParser`, `forge::github::fragment_for_repo`,
  `atelier::AtelierWorkspaceConfig`, `atelier::AtelierForkIndex`,
  `atelier::fork_records_fragment`, and harness adapters under
  `discovery::harness`.
- Resolution and rendering:
  `resolve_snapshot`, `resolve_links`, `render_graph_json`,
  `output::table::render`.
- Configuration and declarations:
  `ConfigLoader`, `load_from_cwd`, `parse_declared_document`, `to_toml`,
  `select_store_for_declaration`, `load_declared_link_by_id`,
  `upsert_declared_link`, and `remove_declared_link`.

## Environment Toggles

`LocalDiscoveryConfig::from_env` recognizes:

- `CONSPECTUS_CODEX_STATE`
- `CONSPECTUS_CLAUDE_CODE_STATE`
- `CONSPECTUS_OPENCODE_STATE`
- `CONSPECTUS_DISABLE_TMUX`
- `CONSPECTUS_DISABLE_FORGE`
- `HOME`
- `XDG_CONFIG_HOME` through `ConfigLoader::from_env`

Consumers that need deterministic tests should avoid `from_env`, provide
explicit roots, and inject fake or custom runners.

## Audit Notes

The Phase 6 audit checked for `std::env`, `std::process`,
`current_dir`, and command construction under `src/`. The only
production process command boundaries are git, tmux, and gh. The only
production current-directory readers are CLI convenience paths,
`DiscoveryContext::from_current_dir`, and `config::load_from_cwd`.
Additional process command uses found under `#[cfg(test)]` are local test
fixture setup and are not library entry points.
