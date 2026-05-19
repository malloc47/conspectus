# Phase 08: Interactive TUI

## Summary

Add `conspectus tui`: an interactive, keyboard-first terminal UI for searching,
selecting, inspecting, and attaching to Conspectus graph rows.

The TUI is not a separate discovery model. It is a live projection over the
same resolved graph that backs `conspectus graph --format json`,
`conspectus table <ROWS>`, and `conspectus node show <id>`.

## Product Goals

- Find existing agent sessions quickly across harnesses, projects, worktrees,
  forks, mux sessions, and PRs.
- Reattach to the selected mux session or create a mux attachment for an
  un-muxed agent session when Conspectus has enough evidence to do so.
- Inspect the selected node with the same relationship context as
  `conspectus node show`, without leaving the TUI.
- Preview live mux window contents where available and show recent transcript
  history for un-muxed agent sessions.
- Preserve the existing table row-types as first-class TUI views:
  `sessions`, `mux`, `union`, `prs`, and `forks`.
- Leave room for substantial future growth: additional row-types, actions,
  filters, transcript viewers, forge providers, and continuous-refresh
  behavior should not require a rewrite.

## Prior Art Notes

### Agent Deck

Agent Deck is a useful reference for the operator workflow: one terminal view
for many agent sessions, search, status, fast switching, forked sessions, and
group-aware keyboard navigation. Its README highlights fuzzy search across
sessions, global search across conversations, fork workflows that retain
history, status detection, and fast jumps to sessions or groups.

Conspectus should borrow the "command center" scanning and switching posture,
but keep the graph-oriented mental model. Agent Deck owns orchestration state;
Conspectus observes and relates state across tools.

Reference: <https://github.com/asheshgoplani/agent-deck>

### dmux

dmux is a useful reference for tmux + worktree + agent operations. It centers
on creating panes, launching agents into isolated worktrees, and then merging
or creating PRs from those panes. Conspectus should borrow the mux/worktree
inspection posture and the emphasis on panes as the resumable execution
surface.

Conspectus should not duplicate dmux's full worktree lifecycle in the first
TUI milestone. The initial action set should attach/resume and inspect; create,
merge, cleanup, and PR creation belong behind later explicit stories.

References:

- <https://dmux.ai/>
- <https://github.com/standardagents/dmux>

## TUI Library Evaluation

The first implementation task must produce an ADR before adding a TUI runtime
dependency. Candidate direction:

- **Ratatui + crossterm** is the leading default. Ratatui is a Rust library for
  fast, lightweight, rich TUIs with widgets, tables, dynamic layouts, and
  immediate-mode rendering. Its current installation docs list `ratatui =
  "0.30.0"` and note that the default backend feature is `crossterm`, with
  `termion` and `termwiz` available as alternatives.
- **tui-realm** should be evaluated as a higher-level framework over Ratatui.
  It adds reusable components, properties/state, message/event updates, view
  mounting, and focus/event forwarding. This may help if the first milestone
  already needs modal workflows, async refresh, multiple panels, and nested
  focus regions. It is also a larger architectural commitment.
- **Cursive** should be evaluated as a mature callback/declarative TUI
  framework with a broad view library. Its callback-driven view tree may be
  productive for forms and dialogs, but the graph browser likely needs custom
  drawing, high-control list/tree behavior, and shared styling with existing
  table output.
- **Raw crossterm/termion** should be rejected unless the ADR finds a specific
  blocker in widget libraries. Conspectus needs table/list/tree panels,
  selection state, search overlays, scrollable previews, and responsive splits;
  hand-writing all of that is avoidable risk.

The expected ADR outcome is Ratatui + crossterm with an in-tree Elm-style app
architecture, unless the prototype shows that tui-realm's framework semantics
remove enough complexity to justify the extra layer.

References:

- <https://ratatui.rs/>
- <https://ratatui.rs/installation/>
- <https://docs.rs/tuirealm>
- <https://docs.rs/cursive>

## End-State Behavior

### Command Surface

```sh
conspectus tui [--scan-root PATH]... [--view sessions|mux|union|prs|forks]
               [--refresh-interval DURATION] [--no-live-preview]
               [--color auto|always|never]
```

- `--view` selects the initial left-panel organization. Default:
  `sessions`.
- `--scan-root` matches existing discovery commands.
- `--refresh-interval` controls background graph refresh until Phase 7 server
  mode can provide push or snapshot updates. Default should be conservative
  enough to avoid hammering `gh` and transcript state.
- `--no-live-preview` disables mux capture/transcript tailing for privacy and
  performance.
- `--color` reuses the ADR 0022 resolution rules where possible.

### Layout

The initial TUI has two primary panels:

- **Left navigation panel**: list/tree-like row browser.
- **Right detail panel**: expanded selected-node view with relationships,
  lineage, preview/history, and action affordances.

The left panel supports these modes:

- `sessions`: projects/repos/worktrees as parents, agent sessions as children,
  and fork/session history nested under each session where lineage is known.
- `mux`: mux sessions as parents, attached/nested agent sessions underneath,
  with pane/window labels and live preview metadata where known.
- `union`: graph-row view equivalent to `conspectus table union`.
- `prs`: PR rows equivalent to `conspectus table prs`, grouped by repo or
  state when the user toggles grouping.
- `forks`: fork rows equivalent to `conspectus table forks`, grouped by
  workspace or provider.

The right panel starts as a TUI rendering of `conspectus node show <id>`:

- selected node attributes
- resolved relationships
- candidate links and ambiguity
- adjacent nodes grouped by relationship kind
- diagnostics touching the selected node
- source metadata where useful

It then layers richer previews:

- selected mux: semi-live capture of the selected tmux session/window/pane
- selected agent session with mux: live mux preview plus linked session
  metadata
- selected agent session without mux: recent transcript/history preview using
  the same raw-state semantics that ADR 0019 requires
- selected PR: latest status, draft/merge state, checks summary, review/comment
  recency, linked branch/worktree/session rows
- selected fork: parent/child lineage, context effects, related worktrees, and
  child sessions

### Search And Navigation

- `/` opens in-view fuzzy search across visible rows.
- `g` / `G`, arrow keys, PageUp/PageDown, Home/End navigate the left panel.
- Tab cycles focus between navigation, detail, preview, and command/status
  regions.
- `1`-`5` switch row modes: sessions, mux, union, PRs, forks.
- `Enter` opens the default action for the selected row:
  - mux row: attach to mux session
  - agent row with attached mux: attach to that mux
  - agent row without mux: open an action picker for resume/attach choices
  - PR row: open details; forge browser action is a later story
- `r` refreshes graph discovery immediately.
- `?` opens a keybinding/help overlay.
- `q` exits after restoring the terminal.

### Actions

The v1 action surface is intentionally narrow:

- Attach to existing mux session.
- Attach to selected mux window/pane if tmux metadata is available.
- Resume an un-muxed agent session inside a new or selected mux window when the
  harness adapter supports a deterministic resume command.
- Copy or print the selected node id / short id for use with `node show`.
- Toggle grouping and visibility filters.

Destructive or workflow-changing operations are deferred:

- creating worktrees
- starting new agents
- merging branches
- creating PRs
- deleting sessions, panes, or worktrees
- mutating declared links

### Semi-Live Data

The TUI needs periodic refresh, but its initial implementation should stay
compatible with today's one-shot discovery:

- Poll graph discovery on a configurable interval.
- Poll tmux capture for the selected mux row more frequently than full graph
  discovery.
- Never block input handling on discovery, `gh`, tmux capture, or transcript
  reads.
- Show stale/error state in the status bar when a provider fails.
- When Phase 7 server mode lands, add a server-backed data source that replaces
  in-process polling without changing UI components.

## Implementation Changes

- Add a `src/tui/` module with clear boundaries:
  - `app`: state machine, commands, focus, mode, selection, filters
  - `data`: graph loading/refresh adapter over existing discovery and future
    server snapshots
  - `rows`: tree/list row builders backed by the same row-types as
    `conspectus table <ROWS>`
  - `detail`: selected-node view model backed by `output::node_show`
  - `preview`: mux capture and transcript tail adapters
  - `actions`: attach/resume command construction and confirmation flow
  - `ui`: Ratatui widgets/rendering
- Add `conspectus tui` to `src/cli.rs`, but keep CLI parsing separate from the
  TUI runtime.
- Extract reusable, non-string view models from `output::table` and
  `output::node_show` where needed. The TUI should not scrape rendered table
  text.
- Add a graph row tree builder that can group by:
  - repo/project/worktree/session lineage
  - mux session/window/pane/attached session
  - PR repo/state/branch/session
  - fork provider/workspace/parent/child
- Add a fuzzy matcher dependency only after an ADR or dependency note. Prefer
  an in-tree simple matcher for v1 unless a library clearly improves quality.
- Add tmux capture support for previews using the existing tmux runner seam,
  not direct command calls from rendering code.
- Add harness resume command modeling per harness before wiring "resume inside
  mux" actions. Unsupported harnesses should show a disabled action with a
  reason.

## Tests

- Pure unit tests for tree row builders in every view mode.
- Pure unit tests for selection persistence across graph refreshes.
- Pure unit tests for filtering and fuzzy search ranking.
- Snapshot tests for small Ratatui render buffers at fixed dimensions.
- Tests for detail view models proving parity with `node show` content.
- Fake tmux runner tests for preview capture, capture failure, no pane, and
  disabled live preview.
- Fake harness-action tests for generated resume/attach commands.
- CLI smoke test for `conspectus tui --help`.
- Panic/terminal cleanup test path where feasible: terminal mode must restore
  on normal exit and error exit.

## Manual Checks

```sh
cargo run -- tui
cargo run -- tui --view sessions
cargo run -- tui --view mux
cargo run -- tui --view prs
cargo run -- tui --no-live-preview
```

Run inside and outside tmux. Verify:

- navigation does not lag while graph refresh is running
- selection survives refresh when the selected node still exists
- attach action reaches the expected tmux target
- unsupported resume actions are visibly disabled
- terminal state is restored after `q`, Ctrl-C, and provider errors

## Open Product Questions

These should be answered before implementing behavior that would be expensive
to change:

- Should `Enter` on an un-muxed agent session create a new mux session/window by
  default, or should all resume/attach paths go through an action picker?
- Which mux targets are in v1 scope: tmux sessions only, tmux windows, tmux
  panes, or backend-neutral mux targets?
- What should "project" mean in the sessions tree: repo common-dir, workspace,
  Atelier project, configured scan root, or a user-configurable grouping?
- Should global search include transcript contents in v1, or only node/table
  metadata and previews?
- How much PR data is required in the right panel for v1: status only, checks,
  review comments, inline comments, or timeline?
- Should live previews default on even though they may surface sensitive mux or
  transcript content?
- Should mouse support be included in v1, deferred, or explicitly out of scope?
- Should the TUI read directly from one-shot discovery in v1, or should Phase 7
  snapshot/server work land first?
- Should unsupported action attempts be hidden, disabled, or visible with
  explanatory failure messages?
- What is the expected behavior when several candidate mux links exist for one
  agent session?

## Assumptions

- The graph remains the source of truth; the TUI does not invent relationships
  that are not represented as nodes, candidate links, or resolved
  relationships.
- The first implementation is read-mostly. Mutations are limited to attaching
  or resuming into mux, and only after explicit user action.
- tmux is the first live-preview backend because Conspectus already has tmux
  discovery. Other mux backends require their own adapters.
- The first preview implementation may be "semi-live" polling, not streaming.
- A future server-backed mode should be an implementation detail behind the TUI
  data adapter.
