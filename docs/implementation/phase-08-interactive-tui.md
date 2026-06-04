# Phase 08: Interactive TUI

## Summary

Add `conspectus tui`: an interactive, keyboard-first terminal UI for
searching, selecting, inspecting, and attaching to Conspectus graph rows.

The TUI is a periodically-refreshed projection over the same resolved
graph that backs `conspectus graph --format json`,
`conspectus table <ROWS>`, and `conspectus node show <id>`. Refresh is
polling-based in v1; later phases may layer a Phase 7 server-backed
push transport behind the same data adapter.

## Personas And Primary User Journey

### Primary persona (v1): Returning Operator

You closed your laptop yesterday with three or four agent sessions still
running across two repos. You open `conspectus tui` this morning and want
to know, within a few seconds:

1. Which agent sessions are still alive, and which mux session each is
   attached to.
2. What each session was last doing, well enough to remember whether to
   resume, abandon, or merge.
3. How to reattach to the right tmux pane without typing a long ad-hoc
   command.

The v1 TUI optimizes for this journey. Defaults flow from it: sessions
view first, hierarchy-first tree with `last_message_preview` visible at
the row level, recency-aware grouping, single-key attach.

### Secondary personas (informed but not optimized for v1)

- **Fork Inspector**: works in atelier-style forked workspaces; needs the
  forks view and fork-lineage detail in the right panel. The TUI shell
  serves this persona via `--view forks`, but defaults are tuned for the
  operator.
- **PR Reviewer**: scans `prs` view for which agent owns which branch.
  Same shell, default-overridable via `--view prs`.

The v1 plan does not invent new row-types for these personas; they reuse
the existing `forks` / `prs` projections.

## Product Goals

Goals derive from the returning-operator journey above:

- Find an existing agent session in under five keystrokes from launch.
- Show enough context per session (cwd, mux attachment, PR, last
  message) that the operator can decide attach / resume / leave in one
  glance.
- Reattach to the selected mux session in one keystroke when there is
  no ambiguity.
- Inspect the selected node with the same relationship context as
  `conspectus node show`, without leaving the TUI.
- Preview the selected mux session/window/pane on a slow refresh and
  surface the agent's last-message snippet on the same panel for
  un-muxed sessions.
- Preserve the existing table row-types as first-class TUI views:
  `sessions`, `mux`, `union`, `prs`, and `forks`.
- Leave room for substantial future growth: additional row-types,
  actions, filters, transcript viewers, forge providers, and
  continuous-refresh behavior should not require a rewrite.

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

dmux is a useful reference for tmux + git-worktree + agent operations. It centers
on creating panes, launching agents into isolated git worktrees, and then merging
or creating PRs from those panes. Conspectus should borrow the mux/checkout
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
               [--refresh-interval DURATION] [--mux-preview-interval DURATION]
               [--sort hierarchy|recency] [--no-live-preview]
               [--color auto|always|never]
```

- `--view` selects the initial left-panel organization. Default:
  `sessions`. Configurable via `[tui].default_view` in `.conspectus.toml`
  or user config; the flag overrides.
- `--scan-root` matches existing discovery commands.
- `--refresh-interval` controls background graph refresh. Default
  **30 seconds**. The interval applies to all providers in v1; per-
  provider tuning is a later refinement.
- `--mux-preview-interval` controls how often the selected mux row's
  pane capture refreshes. Default **2 seconds**. Faster than the graph
  refresh because tmux capture is cheap and the operator-journey use
  case wants the preview to feel responsive.
- `--sort` controls the row-tree ordering inside each group. Default
  **`hierarchy`** (workspace → repo → checkout → session, alphabetical
  within each level). `recency` re-orders within each group by the
  freshest contained agent session's activity. Configurable via
  `[tui].default_sort`; the flag overrides.
- `--sessions-grouping` controls the top-level grouping in the
  sessions tree. Default **`repo`** — group by repo common-dir,
  adding checkout rows only when a repo has multiple checkouts in
  the visible set. `graph` derives a richer hierarchy from existing
  graph relationships (workspace → `WorkspaceContainsRepo` → repo →
  `BelongsToRepo` → checkout → cwd-matched agent session) and nests
  resolved `parent_session` lineage under the parent session. Other
  values: `checkout` (group by checkout root, no workspace/repo
  nesting), `scan-root` (group by the configured discovery scan
  root). Configurable via
  `[tui].sessions_grouping`; the flag overrides. Orphan sessions
  (no resolved repo/checkout) fall into a single "Ungrouped" bucket
  regardless of mode.
- `--no-live-preview` disables the live extras only: mux pane
  capture and the transcript-tail read that the right-panel preview
  uses to fill space beyond the graph-resident snippet. Inline
  previews in the row tree and the graph-resident right-panel
  preview keep rendering, because both come from the resolved
  graph rather than from live re-reads.
- `--color` reuses the ADR 0022 resolution rules.

### Layout

Two primary panels split roughly 50/50 by default, with a single-line
status bar at the bottom. On terminals narrower than ~100 columns the
panels stack vertically (left panel on top, right panel below). The
80×24 wireframe below shows the v1 target shape:

```
┌─ Conspectus · sessions ─ updated 12s ago · 24 agents · 3 mux ─────────────┐
│ ▼ ~/src/conspectus                  │ codex:…b4fdee8                       │
│   ▼ ~/src/conspectus                │ harness  codex                       │
│     4e90b4 codex:…b4fdee8  2m  ◐    │ cwd      ~/src/conspectus            │
│       could you give me a bit mo…   │ title    fork lineage q&a            │
│     f9f3cc claude:…6b6346f  17m ◉   │ mux      — (2 candidates) ⚠          │
│   ▶ ~/.agent-deck/multi-repo-…      │ pr       octo/repo#7 (open) ⟳        │
│ ▼ ~/src/atelier                     │ lineage  — (no parent)               │
│     73566c codex:…b4fdee8  5m  ◯    │                                      │
│ ▶ ~/src/oss/worktrunk               │ ─ preview ─────────────────────────  │
│                                     │ Could you give me a bit more context │
│                                     │ ? "The fork didn't seem to work"     │
│                                     │ could mean a few different things    │
│                                     │ given recent work … ▲ scroll: J/K    │
├─────────────────────────────────────┴──────────────────────────────────────┤
│ / search  1-5 view  a attach  i copy id  r refresh  ? help  q quit │ gh ⟳  │
└────────────────────────────────────────────────────────────────────────────┘
```

See [`docs/tui-sessions-mockup.md`](../tui-sessions-mockup.md) for an
annotated walk-through of this layout, the ambiguous-mux expansion,
and the responsive same-line preview behavior.

#### Left panel: hierarchical row browser

The left panel renders one of the registered row-tree views:

- `sessions` (**v1 default**): repo common-dir → checkout → agent
  session. In `graph` grouping, known session lineage nests under the
  parent session.
- `mux`: mux session → attached agent sessions, with pane/window labels
  when the mux adapter records them.
- `union`: agent + mux rows side by side, equivalent to
  `conspectus table union`.
- `prs`: PR rows equivalent to `conspectus table prs`, grouped by
  repo. Future toggles may regroup by state.
- `forks`: fork rows equivalent to `conspectus table forks`, grouped by
  workspace/provider.

Default ordering inside each group is **hierarchy-first** (alphabetical
within each level). `--sort recency` re-orders so the group containing
the freshest agent session bubbles to the top within its parent.

#### Right panel: header + preview

The right panel is split top-to-bottom into two zones with **no
toggling and no tabs in v1**:

1. **Header** (top): the equivalent of `conspectus node show <id>` for
   the selected row — node kind, primary identifier, key attributes,
   resolved relationships, candidate links, ambiguity markers, source
   metadata. Compact form: one row attribute per line, dim placeholders
   for missing values, no excessive whitespace.
2. **Preview** (bottom): the last known state of the selected row. The
   exact "last known state" depends on the row kind:
   - **mux** row: tmux `capture-pane` snapshot of the selected session
     (or window / pane when the v1 mux-target decision lands). Refreshes
     on the `--mux-preview-interval` cadence; does **not** live-scroll.
   - **agent session with attached mux**: tmux `capture-pane`
     snapshot of the attached mux session. The mockup-review
     decision drops the `last_message_preview` caption that earlier
     drafts stacked above the capture; the header's `mux` row
     already carries the attachment context.
   - **agent session without mux**: the session's
     `last_message_preview` rendered with soft wrapping. When the
     preview pane has space beyond the ADR-0023 graph-resident
     snippet (200-char cap) and `--no-live-preview` is not set, the
     data adapter does a transcript-tail read on selection to fill
     the additional space, and an "open transcript" affordance for
     the future transcript viewer (ADR 0019) remains as a follow-on.
   - **PR** row: rendered in two stages so navigation never blocks
     on a `gh` call.
     - *Immediate stage* (synchronous, graph-only): the PR header
       fields the discovery pass already collected — owner/repo,
       number, state, draft, head ref shortname, the linked branch
       / checkout / session rows from the graph. Renders the first
       frame the row is selected.
     - *Enriched stage* (async, cached): once the row is selected,
       the data adapter kicks off a background `gh pr view` to
       fetch the v1 enrichment slice (check summary, review-comment
       count, latest activity). The right panel shows
       "loading checks…" until the call returns, then re-renders
       with the enriched fields. The result is cached in-memory
       keyed by PR id for the lifetime of the TUI session; `r`
       manual refresh invalidates the cache. Errors fall back to a
       single-line "gh: <reason>" note next to the enriched-stage
       section without losing the immediate-stage content.
     - The operator can move on to the next row before the
       enrichment returns; the background task is cancelled or
       allowed to complete-and-cache silently. Either way the UI
       stays responsive.
   - **fork** row: parent / child lineage, context effects, related
     checkouts and child sessions.
   - Empty/unavailable: a single dim line ("no preview available"
     plus the reason — disabled by flag, no tmux, no transcript,
     etc.).

The preview does **not** scroll on its own. The user can scroll it
manually with `J` / `K` (uppercase to distinguish from row navigation)
or PageUp/PageDown when focus is on the right panel.

### Keybindings

The v1 keybinding model follows agent-deck's posture: **direct row-
action keys**, not a modal picker or command palette. Each key acts on
the current selection — the key's meaning depends on the selected
row's kind, and unavailable actions are visibly disabled in the status
bar rather than offered through a separate menu.

#### Navigation (v1)

| Key                | Action                                       |
| ------------------ | -------------------------------------------- |
| `j` / `k` / arrows | Move down / up in the left panel             |
| PageDown / PageUp  | Page through the left panel                  |
| Home / End         | First / last visible row                     |
| `g` / `G`          | First / last row in the current view         |
| `Enter`            | Expand / collapse a parent row               |
| `Tab`              | Cycle focus: left panel → right panel → left |
| `J` / `K` (focus right) | Scroll preview down / up                |
| `1`–`5`            | Switch view: sessions, mux, union, prs, forks |
| `/`                | Open in-view fuzzy search overlay            |
| `r`                | Refresh discovery now                        |
| `R`                | Rename the selected agent session            |
| `?`                | Help overlay                                 |
| `q` / Ctrl-C       | Quit (restores terminal)                     |
| `v`                | Open the controls overlay (ADR 0031)         |
| `]` / `[`          | Cycle to next / previous view                |
| `f`                | Jump into the controls overlay's Filters section (ADR 0031) |
| `F`                | Clear all active filters in the current view |

#### Actions on the selected row (v1)

| Key   | Acts on                          | Meaning                                                |
| ----- | -------------------------------- | ------------------------------------------------------ |
| `a`   | mux row, or agent row with mux   | Attach to the mux target                               |
| `i`   | agent / mux session row          | Copy the selected session's full id to the clipboard |
| `o`   | PR row                           | Open the PR URL in `$BROWSER` (if available)           |
| `R`   | agent session row                | Open the rename overlay (ADR 0029 / ADR 0030); `Enter` commits the alias and the optional lockstep tmux rename, `Esc` cancels |

Single keystroke; status bar reflects the action's outcome. When the
selected row doesn't support a key, the status bar shows a one-line
"disabled because …" reason rather than swallowing the keystroke.

#### Reserved for later phases (do not bind in v1)

| Key   | Future action                                                       |
| ----- | ------------------------------------------------------------------- |
| `n`   | New agent / new mux session                                         |
| `c`   | Confirm a discovered candidate as a declared link                   |
| `d`   | Delete (mux session, checkout, declared link, …)                    |
| `m`   | Inline mux-picker when the selected agent has ambiguous LinkedToMux |
| `M`   | Merge (branch, checkout, fork)                                      |
| `s`   | AI session-name suggestion overlay (`H-AI-NAMING-003`)              |

These keys are deliberately unbound in v1 so muscle memory can map to
their final actions in later phases without rebinding. v1 is
read-mostly + attach.

`f` was previously reserved for a future fork-the-selected-session
action; ADR 0031 reallocates it to "jump into the controls overlay's
Filters section." A future fork accelerator will land on a different
key (TBD when the fork-action workstream begins).

### Actions (v1 scope)

The v1 action surface is intentionally narrow and read-only-with-attach:

- **Attach to existing mux session** (`a` / `Enter` on mux row).
- **Attach to the mux session linked to an agent row** (`a` / `Enter`
  on an agent row whose preferred `LinkedToMux` resolves).
- **Copy the selected agent or mux session's full id** (`i`).
- **Open the selected PR in `$BROWSER`** (`o`).
- **Rename the selected agent session** (`R`) via the Conspectus alias
  overlay, with optional tmux lockstep rename when the session has a
  single resolved mux.

Explicit non-goals for v1:

- creating mux sessions, windows, or panes
- starting new agent sessions (`n`)
- resuming an un-muxed agent into a new or existing mux
- creating checkouts
- merging branches
- creating or commenting on PRs
- mutating declared links
- deleting any state

Resume of un-muxed agents sounds like an obvious v1 inclusion but is
deferred because it requires harness-specific resume-command modeling
(claude-code's `claude --resume`, codex's rollout-id reattach,
opencode's session-id reopen) plus a confirmation/launch flow. Doing it
correctly across the supported harnesses is its own story (see
`P8-011` in the backlog).

### Semi-Live Data

The TUI is **polling-based** in v1 — not push, not streaming. The
initial implementation stays compatible with today's one-shot
discovery:

- Poll graph discovery on the `--refresh-interval` cadence (default
  30 seconds).
- Poll tmux capture for the selected mux row on the
  `--mux-preview-interval` cadence (default 2 seconds). Pause capture
  when the right panel isn't showing a mux row.
- Never block input handling on discovery, `gh`, tmux capture, or
  transcript reads. Run them on background tasks; surface their
  outcomes via the data adapter.
- On a provider failure, retain the prior good slice (per Phase 7's
  per-provider eviction model). Surface the failure in the status bar
  as a single-line "<provider>: <one-line reason>" string and apply
  exponential backoff up to a cap before retrying. Never spin.
- The data adapter is shaped so a Phase 7 server snapshot/push transport
  can replace in-process polling later without changing UI components.

### Empty, Loading, And Error States

Each state must have a defined visible presentation so the v1
implementation can be specified, snapshot-tested, and shipped without
ambiguity.

| Situation                            | Left panel                                      | Right panel                              | Status bar                              |
| ------------------------------------ | ----------------------------------------------- | ---------------------------------------- | --------------------------------------- |
| First-launch, discovery in flight    | "Loading…" centered, dim                        | empty                                    | "discovering…"                          |
| Discovery complete, no sessions      | "No sessions discovered. `?` for help."         | empty                                    | counts: `0 agents · 0 mux · 0 PRs`      |
| `CONSPECTUS_DISABLE_TMUX=1`          | mux view shows "tmux discovery disabled"        | for a selected agent: "no preview (tmux disabled)" | provider warning chip               |
| `--no-live-preview`                  | rendered normally                               | "preview disabled (`--no-live-preview`)" | unchanged                               |
| tmux missing / unreachable           | mux view rows degrade to "tmux unavailable"     | "no preview (tmux unavailable)"          | error chip with one-line reason         |
| `gh` provider error                  | prs view shows last good slice with stale marker | normal                                  | error chip "gh: <reason>"               |
| Refresh failed, prior data retained  | normal (stale marker on title bar)              | normal                                   | "last refresh failed; using …s ago snapshot" |
| Selected row removed by refresh      | selection snaps to nearest sibling              | re-renders against new selection         | "previous selection removed"            |

The status bar uses two zones: action hints on the left, status / error
chips on the right. Chips are colour-coded per ADR 0022 (error red,
warning yellow, info default).

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
  - repo/workspace/checkout/session lineage
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

## Locked v1 Decisions

These were open product questions; the answers are now part of the v1
contract.

### From the first PM walkthrough

- **Primary persona**: Returning Operator (sessions-first workflow).
- **Default view**: `sessions`, configurable via `--view` flag and
  `[tui].default_view` config.
- **Default sort**: `hierarchy`, configurable via `--sort` flag and
  `[tui].default_sort` config.
- **Refresh interval**: 30 s graph / 2 s mux capture, both
  flag-overridable.
- **Action UX**: direct single-key actions on the selected row
  (agent-deck style). No modal picker, no command palette in v1.
- **Right panel composition**: fixed header + fixed preview, no tabs,
  no panel toggles. Preview is polled, not live-scrolled.
- **Unsupported actions**: visibly disabled with a one-line "disabled
  because …" reason in the status bar.
- **Source of graph data in v1**: in-process polling; Phase 7 server
  mode replaces it later without UI changes.
- **`Enter` on a row**: expand / collapse parent rows. Attach is `a`
  (or `Enter` on a leaf row that has a single resolved mux target).
- **Resume an un-muxed agent**: deferred to its own story (`P8-011`).
  The `R` key is already assigned to rename.

### From the P8-001a walkthrough

- **"Project" grouping** in the sessions tree: configurable from
  day one. Default `graph` derives the tree from `WorkspaceContainsRepo` /
  `BelongsToRepo` / cwd-match relationships and follows resolved
  session lineage. `repo` remains the location-first grouping; other
  values include `checkout`, `scan-root`.
  Configurable via `[tui].sessions_grouping` and the
  `--sessions-grouping` flag. Orphan sessions always land in a
  single "Ungrouped" bucket.
- **Mux target granularity**: session-only in v1. The graph models
  `MuxSession` and nothing finer, and `tmux attach -t <session>`
  drops the operator at the session's last-active window. Window
  / pane targeting waits on a future story that expands mux
  discovery (likely after `H-MUXPROC-*` lands).
- **Ambiguous mux links**: `a` attaches to the resolver's preferred
  candidate. The `*` ambiguity marker stays visible in the row,
  the status bar surfaces a one-line "N candidates" note, and the
  right panel's `node show` view enumerates every candidate for
  diagnosis. The `m` key is **reserved** for a future inline
  mux-picker that opens only when ambiguity is real; v1 does not
  bind it. See "Sources of mux ambiguity" below for the scenarios
  this decision covers.
- **PR right-panel depth**: enriched. PR rows render in two stages
  so navigation never blocks on a `gh` call. The immediate stage
  uses only graph-resident PR data (state, draft, head ref,
  linked rows); the async stage runs `gh pr view` in the
  background and caches the result per PR id for the lifetime of
  the TUI session. `r` manual refresh invalidates the cache.

### From the sessions-view mockup review

These pin the visible behavior of the v1 default sessions view
(`docs/tui-sessions-mockup.md`):

- **Path rendering**: every path shown in the TUI uses `~`
  shortening for `$HOME`. No raw `/home/<user>/…` strings in the
  tree, right panel, or status bar.
- **Checkout-level depth**: render the checkout level only when a
  project has >= 2 checkouts. Projects with a single checkout
  collapse to one level (sessions hang directly off the project
  row).
- **Ambiguous-mux row expansion**: when a session has ≥ 2
  `LinkedToMux` candidates, the row becomes expandable; expanding
  reveals one child row per candidate, with the resolver-preferred
  candidate marked. Sessions with a single, definitive mux link
  (or no mux link at all) remain leaves. Selecting a candidate
  child row navigates the right panel to that mux; pressing `a`
  on a candidate child attaches to that specific candidate,
  overriding the resolver's pick. This is a passive tree view of
  the same `LinkedToMux` evidence; the reserved `m` modal
  (`P8-014`) remains the explicit picker for the same scenario.
- **Activity indicator dropped**: the recency column carries the
  "which one was I in" signal already; no per-row `●`/`○` prefix
  in v1.
- **Inline preview density**: session rows stay one physical line
  tall. When horizontal space remains after the mux indicator, show
  a dim same-line preview from `last_message_preview`; otherwise
  omit or crop it. Layout must be responsive to terminal width, not
  pinned to 80 columns.
- **Mux indicator**: `◉`/`◐`/`◯` glyphs carrying color signal
  (green attached / yellow ambiguous / dim un-muxed). Color is the
  primary differentiator; the glyph stays small.
- **Header bar freshness**: show `updated Ns ago` (time since the
  most recent successful refresh) alongside the discovery counts.
  Refresh cadences move into `?` help and out of the header.
- **Right-panel header field set**: the five existing fields
  (harness, cwd, mux, pr, lineage) stay, and a `title` row is
  added when the session has a non-empty title (opencode chat
  topics today, other harnesses later). Sessions without a title
  omit the row rather than render `—`.
- **Right-panel preview content**:
  - Un-muxed agent session: render `last_message_preview` from
    the graph. When the preview pane has space for more than the
    ADR-0023 graph-resident snippet (200-char cap) and
    `--no-live-preview` is not set, the data adapter does a
    transcript-tail read on selection and renders the fuller
    message. The same-line previews in the tree always use the
    graph-resident snippet — only the right-panel preview
    expands.
  - Muxed agent session: render the tmux `capture-pane` snapshot
    of the attached mux, polled at the mux-preview cadence. Do
    *not* duplicate `last_message_preview` in the preview zone
    when a pane capture is available; the header's mux row
    carries the attachment context, and the pane capture is the
    "what was it doing" answer.
  - Standalone mux row: pane capture, same as above.
- **Inline preview vs `--no-live-preview`**: `--no-live-preview`
  does **not** suppress same-line row previews or the graph-resident
  right-panel preview. It only suppresses live extras — mux pane
  capture and the transcript-tail read that fills the right-panel
  preview beyond the graph-resident snippet. The v1-deferrable
  question on this topic is now resolved.

### Controls overlay (ADR 0031)

ADR 0031 fronts filtering, view switching, per-view grouping, and the
sort toggle with a single navigable **Controls overlay**. Accelerator
keys (`v`, `1`–`5`, `]`/`[`, `f`, `F`, the grouping-cycle key) reach
the same outcomes for muscle-memory operators, but no capability in
this surface depends on memorizing a key.

The overlay opens with `v` (final key choice settled in `F8-005`) and
shows sections for View, Grouping (scoped to the active view), Filters
(scoped to the active view), and Sort (global). Arrow keys move the
cursor between actionable rows; Enter picks or drills into a sub-
editor; Esc backs out one level; mouse click selects when the
terminal supports it. Accelerator keys are surfaced inline (`[1]`,
`[2]`, …) so they remain discoverable.

Sub-editors:

- **Harness**: multi-select list (claude / codex / opencode / aider).
- **Max age**: text input (ADR 0030 primitive) seeded with the
  current value; parse errors render inline; empty commit clears.
- **Mux state**: multi-select (attached / ambiguous / unmuxed).

Active filters render as compact chips in a new status-bar zone left
of the provider chips, color-coded per ADR 0022:

```
… │ harness:claude · max-age:7d · mux:unmuxed │ updated 12s ago │ gh ⟳ │
```

Header counts reflect the filtered set with the unfiltered total in
parens: `12 of 47 agents · 3 of 8 mux · 0 PRs`. The filtered-zero
empty frame shows `No sessions match <chips>. F clears.`

Per ADR 0031, filter / grouping / selection / expanded / left-scroll
state is **per view**; sort is **global**. Switching `1 → 2 → 1`
restores the sessions view's prior state exactly. Defaults seed from
`[tui.views.<name>]` config sub-tables; the legacy `[tui]
.sessions_grouping` key continues to work as a deprecated alias.

CLI parity is via a shared `RowFilter` predicate type:

```sh
conspectus tui --view sessions --harness claude --max-age 7d \
               --mux-state unmuxed --grouping repo
conspectus table sessions --harness claude --max-age 7d
```

`/` fuzzy search (T8-017) remains a separate, transient overlay that
ranks within the active filter set rather than the full snapshot.

### Sources of mux ambiguity

Conspectus never invents agent ↔ mux links — every candidate comes
from an evidence source. Multiple sources can disagree, which is
what produces the ambiguity marker:

- **Multiple agent-mux orchestrators** (agent-deck, dmux, workmux,
  …). If two orchestrator state files reference the same agent
  session with different mux targets, Conspectus sees both as
  candidates. Common when experimenting with orchestrators
  side-by-side.
- **CWD-based heuristic matches multiple muxes**. The
  `cross_link::infer` pass correlates agent and mux sessions by
  shared cwd. Two tmux sessions opened in the same checkout both
  become candidates for any agent session running there. This is
  the most common cause in practice today.
- **Declared override + discovered**. A `conspectus declared`
  binding points at mux X while a discovered candidate also
  points at mux Y. The resolver gives declared links higher
  provenance, so it picks X — but Y survives as an active
  candidate that surfaces the `*` marker.
- **Future**: cached vs fresh (once `H-PROD-002` caching lands)
  and the process-tree linker (`H-MUXPROC-*`) will each add new
  evidence sources that can disagree with cwd inference.

The resolver picks deterministically by provenance tier →
confidence → id, so "preferred" is stable across runs and matches
what `node show` already exposes. v1's "attach to preferred"
behavior is the right call most of the time; the reserved `m` key
covers the edge case explicitly when it matters.

## Open Product Questions (v1-deferrable)

These can be answered later without invalidating in-flight work:

- Should global search include `last_message_preview` content, full
  transcripts, or only structural fields?
- Should mouse support land in v1 or wait?
- Should an in-TUI provider-toggle key let the operator disable a
  noisy provider for the rest of the session?

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
