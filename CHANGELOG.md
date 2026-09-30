# Changelog

All notable changes to Conspectus are tracked here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/) from 0.1.0
onward (pre-1.0, so minor versions may break).

## [Unreleased]

The first public release, 0.1.0. Conspectus builds one evidence-backed
graph of the AI-agent work on your machine and gives you a TUI,
scriptable tables, and exports on top of it. See the
[README](README.md) for the overview.

### Added

- **Discovery** of agent sessions from Claude Code, Codex, opencode,
  and aider (titles, last-message previews, compaction/resume/fork
  lineage); tmux sessions, plus zellij for discovery and attach; git
  repos, checkouts, linked worktrees, and branches; Atelier,
  agent-deck, and generic multi-repo workspaces; and GitHub pull
  requests through `gh`.
- **Evidence-preserving resolution.** Every relationship is a
  `GraphLink` candidate with provenance, confidence, and freshness; the
  resolver picks preferred links and keeps the rest, so ambiguity and
  conflicts stay visible (`graph --explain` shows score breakdowns).
- **Session-to-pane attribution** from layered evidence: opt-in harness
  hooks (`conspectus hook init|status|remove` for Claude Code and Codex,
  plus an opencode plugin), open transcript file descriptors, Codex
  state and log databases, process-tree walks, launch commands, and
  working directories.
- **Interactive TUI** (`conspectus` or `conspectus tui`): sessions and
  mux views, graph/workspace/repo/checkout/flat grouping, filters and
  search, a relationship explorer with drill-down and breadcrumbs, a
  native transcript viewer for Claude Code, Codex, and opencode,
  menu-first controls, and `[tui.theme]` theming.
- **CLI views**: `conspectus table sessions|mux|union|prs|forks` with
  column selection, card layout, paging, and color; `conspectus columns`;
  `conspectus node show`.
- **Graph exports**: `conspectus graph --format json|dot|html`, where
  HTML is a self-contained interactive explorer.
- **Declared links and aliases**: `conspectus declared
  list|create|remove|confirm|ignore|override`, `conspectus rename
  session|mux` (alias plus lockstep tmux rename), and `conspectus alias
  list`, all stored as reviewable TOML.
- **Session pins**: `conspectus pin
  create|list|show|launch|attach|rename|rm|bind|rebind|adopt`. Pins stay
  on the dashboard when nothing is running, bind to the live tmux
  session, and resume the last conversation on relaunch. Pins can be
  worktree-backed (`--worktree`).
- **tmux lifecycle**: `conspectus mux new` (bare shell) and `conspectus
  mux launch` (a harness with no pin).
- **Worktrees**: `conspectus worktree list` (read-only) and
  `new|rm|merge|close|prune`, delegated to
  [worktrunk](https://github.com/max-sixty/worktrunk). `worktree close`
  winds a stream down: it stops the stream's tmux sessions (graceful, then
  forced), merges or discards the branch, removes the worktree, and drops
  its pins.
- **Continuous mode**: `conspectus serve` keeps the graph warm with
  per-source refresh intervals and filesystem watchers, serves it over
  a Unix socket, and saves a zero-copy `graph.bin` for warm restarts;
  `conspectus status` and `conspectus refresh` inspect and drive it.
  Every command still works without the daemon.
- **Library API** at `conspectus::api` (not yet a stable contract).
