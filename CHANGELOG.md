# Changelog

All notable changes to Conspectus are tracked here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the project uses [Semantic Versioning](https://semver.org/) from 0.1.0
onward (pre-1.0, so minor versions may break).

## [Unreleased]

Targeted for 0.2.0, after 0.1.0 is tagged. History, rebuild, and pins
(ADRs 0140 to 0144) rework how pinning works.

### Added

- **History.** Conspectus remembers every mux, checkout, repo, and PR it
  sees under `$XDG_STATE_HOME/conspectus/history/`, so an entity that
  disappears stays visible as *gone* (decided by a targeted check, never
  by absence from a scan). `conspectus history list|show|reap|forget`
  inspects and prunes it; `reap` honors `[history.reap_after]` and never
  removes a pinned or Unknown record.
- **Pins.** `conspectus pin
  add|edit|rename|retarget|refresh|move|rm|list|show|migrate`. A pin is
  a named pin in `$XDG_DATA_HOME/conspectus/pins.toml` that points at a
  mux, session, worktree, repo, or PR (live or gone), with overrides
  layered over the live graph, history, and a snapshot. Pinning writes
  only that file. Renaming a live mux pin renames its tmux session with
  it, and `mux new|launch --pin [--no-launch]` pins a new mux or plans
  one.
- **Rebuild.** `conspectus rebuild <pin|h_id|node|mux> [--dry-run]`
  brings a gone mux (resuming its last session), session, worktree
  (through worktrunk), or repo (`git clone` into an absent path) back as
  a plan that stops at the first failure. Launch argv comes from the
  pin, the recorded launch, `[launch.<harness>]` profiles, `atelier
  exec`, then the harness default.
- **Pinned and History views** (`4`, `5`), ghost muxes in the Mux view,
  a `.` row menu with Pin / Unpin (`p`), a `◉` pin marker, a `pinned
  only` filter, and `N` for mux launch with a worktree toggle.

### Changed

- **Breaking: pins moved and the pin verbs changed.** Pins live in
  `$XDG_DATA_HOME/conspectus/pins.toml`, not as `[[pins.entries]]` in
  config files. `pin create`, `launch`, `attach`, `bind`, `rebind`, and
  `adopt` are retired and print their replacement (`pin add`,
  `conspectus rebuild`, `pin retarget`). Legacy entries still load, read
  only, with a diagnostic until `conspectus pin migrate` converts them
  and removes the old tables. The Pins modal and the `B`, `b`, `A`, and
  `L` keys are gone.
- **Breaking: the hidden `union` and `forks` TUI views are removed.**
  Neither was in the view switcher, and their groupings did nothing.
  `conspectus tui --view union|forks` now fails and lists the valid
  views, and `[tui] default_view = "union"` or `"forks"`, or a
  `[tui.views.union]` or `[tui.views.forks]` table, is reported as a
  config diagnostic and ignored. `conspectus table union|forks` is
  unchanged.

### Fixed

- **Paging and scrolling in the TUI stay on the content.** `PgDn`/`PgUp`
  move by the rows on screen in the tree, help, the full-value viewer,
  the Messages record, and the viewers' help pages (the tree stepped by
  the terminal height and the dialogs by 8 lines). `G`, or paging past
  the end, of a full value or a Messages record shows its last lines
  instead of a blank body, and the next key up moves at once. A tree
  that shrinks no longer leaves blank rows below it, the `.` row menu
  scrolls to its cursor and keeps its key hints, and the row, mux, and
  worktree menus no longer crash a very short terminal.
- **Harness hooks work while `conspectus serve` runs.** The daemon
  applied each hook observation to its live graph only, and the next
  refresh, a few seconds later, dropped it. It now records the
  observation in `hooks-latest.json` first, so hook attribution lasts
  across refreshes and daemon restarts.

## [0.1.0] - not yet released

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
  requests through `gh`: open PRs and the last 30 days of history for
  every GitHub remote of every discovered repo, including a fork's
  upstream and remotes on any GitHub Enterprise host `gh` is signed in
  to, linked to a local branch when the PR's head is one of the repo's
  remotes. Forge failures surface as diagnostics.
- **Evidence-preserving resolution.** Every relationship is a
  `GraphLink` candidate with provenance, confidence, and freshness; the
  resolver picks preferred links and keeps the rest, so ambiguity and
  conflicts stay visible (`graph --explain` shows score breakdowns).
  Candidates that agree with the winner are kept as corroborating
  evidence rather than reported as conflicts.
- **Session-to-pane attribution** from layered evidence: opt-in harness
  hooks (`conspectus hook init|status|remove` for Claude Code and Codex,
  plus an opencode plugin), open transcript file descriptors, Codex
  state and log databases, process-tree walks, launch commands, and
  working directories.
- **Interactive TUI** (`conspectus` or `conspectus tui`): Sessions,
  Mux, and PRs views (`1`/`2`/`3`), with graph/workspace/repo/checkout/flat
  grouping for sessions and checkout/repo/state grouping for PRs, where
  a marker shows whether each PR has a local branch, a filter and
  ranking (also settable from config) can include, exclude, or float
  those that do, a state filter keeps only open, draft, merged, or closed
  PRs under any grouping, PRs checked out nowhere sit under their repo, and each
  row shows its number, state, age, and title in aligned columns under a
  group that names the `owner/repo`;
  filters and search; a relationship explorer with one row per
  neighbor, drill-down, and breadcrumbs; a native transcript viewer for
  Claude Code, Codex, and opencode; menu-first controls; and
  `[tui.theme]` theming.
- **Node-specific previews** in the TUI: a session's transcript, a tmux
  session's live pane (with smart, plain, or no wrapping), a checkout's
  `git status` and files, a repo's recent commits, a branch against its
  upstream, a PR's checks and reviews, and a process tree. Moving onto a
  field previews its value, and moving onto a relationship shows the
  link's evidence above the neighbor's preview.
- **Refresh controls**: the TUI refreshes in the background, `r`
  rescans now, and `Ctrl-R` is a hard refresh that also asks GitHub
  again. Forge data is otherwise fetched in the background, off the
  daemon's writer lock, when a repo appears and then hourly (`[server.intervals]
  forge`); no other refresh waits on it. `[forge] concurrency` caps
  concurrent `gh` calls (default 4).
- **CLI views**: `conspectus table sessions|mux|union|prs|forks` with
  column selection, card layout, paging, and color; `conspectus columns`;
  `conspectus node show`.
- **Graph exports**: `conspectus graph --format json|dot|html`, where
  HTML is a self-contained interactive explorer.
- **Declared links and aliases**: `conspectus declared
  list|create|remove|confirm|ignore|override`, `conspectus rename
  session|mux` (alias plus lockstep tmux rename), and `conspectus alias
  list`, all stored as reviewable TOML.
- **tmux lifecycle**: `conspectus mux new` (bare shell) and `conspectus
  mux launch` (a harness in a fresh tmux session).
- **Worktrees**: `conspectus worktree list` (read-only) and
  `new|rm|merge|close|prune`, delegated to
  [worktrunk](https://github.com/max-sixty/worktrunk). `worktree close`
  winds a stream down: it stops the stream's tmux sessions (graceful, then
  forced), merges or discards the branch, removes the worktree, and
  forgets the history records rooted in it.
- **Continuous mode**: `conspectus serve` keeps the graph warm with
  per-source refresh intervals (harness and mux 5s, git 30s, forge 1h)
  and filesystem watchers, serves it over
  a Unix socket, and saves a zero-copy `graph.bin` for warm restarts;
  `conspectus status` and `conspectus refresh` inspect and drive it.
  Every command still works without the daemon.
- **`csp`**, the official short name, installed alongside `conspectus`.
  It is the same program; help and suggested commands use the name you
  ran.
- **Library API** at `conspectus::api` (not yet a stable contract).
