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
    explain_resolved_relationships, resolve_snapshot,
};

let graph = discover_local_with(["/workspace"], LocalDiscoveryConfig::empty())?;
let mut graph = resolve_snapshot(graph);
explain_resolved_relationships(&mut graph); // optional, for resolver score details
let json = render_graph_json(&graph)?;
# anyhow::Ok(())
```

For table output, pass a `GraphSnapshot` to
`output::table::render` / `render_with` with a
`config::Projection`. For declared-link
mutations, use the helpers in `declared`; read-only graph and session
commands should not call those write helpers.

## Pure Modules

Pure modules do not read process environment, shell out, inspect the
current directory, or mutate global state when called with explicit
inputs.

| Module | Classification | Stable entry points |
| --- | --- | --- |
| `model` | pure data model | `GraphSnapshot`, `GraphNode`, `GraphLink`, typed IDs, relation/provenance/confidence enums |
| `model::live` | pure data model | `LiveState`, `LiveDetail`, `LiveSource` (`hook`, or `pane_title` for a state read from an agent's pane title), `TitleMark` and `PaneTitle` (what a mux probe read from a session's active pane title, kept on `MuxProbe::titles` as a mark, never the title), `LiveObservation` (its `effective` read rule), `Liveness`, `LiveBinding` (an observation with the liveness found when it was bound), `LiveConfidence`, `EffectiveLive`, `LiveCounts`; a snapshot's `node_live` table of `LiveBinding`s (by agent session) and its `pane_live` table (by `LivePane`, a tmux mux and pane, for agents no session matches yet) are read through `GraphSnapshot::live_state_of`, `GraphSnapshot::live_panes_of`, and `GraphSnapshot::live_counts_of_mux`, which apply the bound liveness; `GraphSnapshot::live_tables` borrows both as `LiveTables`; the read rule's windows are the constants `QUIET_WINDOW_MS`, `QUIET_HEARTBEATS`, `UNKNOWN_LIVENESS_WINDOW_MS`, and `STARTING_WINDOW_MS`, and `LiveCounts::dominant` picks a mux's most urgent state by `LiveState::default_precedence` (`dominant_by` takes a viewer's own ranking) |
| `resolve` | pure resolver | `resolve_snapshot`, `resolve_links`, `explain_resolved_relationships` |
| `output` | pure renderer | `render_graph_json` |
| `output::table` | pure renderer | `render`, `render_with`, `Projection` re-export; the opt-in `live` columns read live state at `RenderOptions::now_epoch`, or at the current time when it is `None` (as the `activity` columns always do) |
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
| `cli` and `main` | current directory, stderr/stdout, argument parsing | application-only; `main.rs` calls `conspectus::cli::run` (doc-hidden), not a stable library surface |
| `config` | `ConfigLoader::from_env` reads environment; `load_from_cwd` reads current directory; normal loading reads config files | prefer `ConfigLoader::new().with_home(...).with_xdg_config_home(...)` plus explicit `load_from(cwd)` in tests |
| `declared` | mutation helpers write TOML files atomically; temp filenames include process id | use read/write helpers only for explicit declared-link commands |
| `discovery` | `DiscoveryContext::from_current_dir` reads current directory; `LocalDiscoveryConfig::from_env` reads env and installs real runners | prefer `DiscoveryContext::from_roots` and `LocalDiscoveryConfig::empty` plus explicit runners in tests |
| `discovery::git` | `GitProbe` shells out to `git`; `GitDiscovery` uses probes over explicit roots | call `fragment_from_probe` for pure tests, or run `GitDiscovery` only where `git` is allowed |
| `discovery::tmux` | `SystemTmux` shells out to `tmux` | inject a `TmuxRunner`; parse rows with `parse_list_sessions` for pure tests |
| `history` | `HistoryStore::at(dir)` reads and writes the history files under an advisory lock; `HistoryStore::from_env` reads `$XDG_STATE_HOME` | construct a store over a temp directory in tests; `merge::merge` and `project::project_into` are pure |
| `discovery::live_state` | `apply_live_state` reads the live-state store in a directory and checks recorded pids through a `PidProbe` (`/proc` on Linux); `LocalDiscoveryConfig::from_env` binds `$XDG_STATE_HOME/conspectus` | `bind_live` is pure over a snapshot, the store's file, and a probe, and returns both tables as a `BoundLive`; the file (`LiveFile`, read with `LiveStore::at(dir).read()`) and the probe (the `PidProbe` trait, with `ProcProbe` over `/proc`) come from the doc-hidden `live_store`; `LocalDiscoveryConfig::empty` binds nothing, `with_live_state_root` points at a temp directory |
| `pin_slots` | `SlotStore::at(path)` reads and writes `pins.toml` atomically; `SlotStore::from_env` reads `$XDG_DATA_HOME` | construct a store over a temp path in tests; `parse_slots` is pure |
| `pin_ops`, `pin_migrate` | slot gestures and legacy conversion over a `SlotStore` and a `HistoryWriter` | inject temp stores; `pin_migrate::plan` is pure |
| `rebuild` | `plan` is pure over a snapshot and an `Env` of injected backends; `execute` runs the plan through those backends | inject a mux backend, worktree backend, and `Cloner`; print `Plan::render` for dry runs |
| `discovery::forge` | `SystemGh` shells out to `gh`; `ForgeDiscovery` asks adapters to inspect repos | inject a `GhRunner`; use `GhPullRequestParser` and `github::fragment_for_repo` for pure tests |

## Internal Modules

These modules are not part of the library contract (ADR 0015, amended
2026-10-01):

- `pub(crate)`, unreachable from outside the crate: `server`, `viewer`,
  `tui_state`, `legacy_pins`, `legacy_bindings`, `legacy_registry` (read-only
  readers for the earlier pin model, kept until `pin migrate` is retired).
- `pub` but `#[doc(hidden)]`, reachable only because the binary,
  integration tests, or the widget preview example use them: `cli`,
  `tui`, `snapshot`, `hook`, `live_store` (the live-state store that
  hook writers fold events into; library consumers read live state
  through `model::live` on a snapshot), `filter`, `dev_scenarios`.

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
  `resolve_snapshot`, `resolve_links`, `explain_resolved_relationships`,
  `render_graph_json`, `output::table::render`, `output::table::render_with`.
- Configuration and declarations:
  `ConfigLoader`, `load_from_cwd`, `parse_declared_document`, `to_toml`,
  `select_store_for_declaration`, `load_declared_link_by_id`,
  `upsert_declared_link`, and `remove_declared_link`.

## Errors

Facade functions return typed errors (ADR 0101). Discovery entry points
return `DiscoveryError`, which tells scan-root problems apart from a
failing provider (`DiscoveryError::Provider` names the provider's keys).
`render_graph_json` returns `serde_json::Error`. Implementors of
`DiscoveryProvider` return `anyhow::Result`.

## Environment Toggles

`LocalDiscoveryConfig::from_env` recognizes:

- `CONSPECTUS_CODEX_STATE`
- `CONSPECTUS_CLAUDE_CODE_STATE`
- `CONSPECTUS_OPENCODE_STATE`
- `CONSPECTUS_PI_STATE`, then Pi's own `PI_CODING_AGENT_SESSION_DIR`,
  `PI_CODING_AGENT_DIR`, and the `sessionDir` in its global settings
  (see `docs/operations.md`)
- `CONSPECTUS_DISABLE_TMUX`
- `CONSPECTUS_DISABLE_FORGE`
- `XDG_STATE_HOME` (the history and live-state stores)
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
