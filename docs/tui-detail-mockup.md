# `conspectus tui` Detail Pane — Mockup For Review

This document is a **review-and-refine artifact**, not a contract. It
sketches what the v1 right-panel detail looks like once it becomes the
focused node inspector plus relationship explorer described in the
TUI Detail Navigation section of [`docs/design.md`](design.md) and
in stories `CSP-313` through `CSP-317` in
[`docs/backlog.md`](backlog.md).

The plan-of-record stays in the backlog stories; this mockup feeds
the locked layout and behavior decisions back into them when the
modeling story (`CSP-313`) starts.

Companion to [`docs/tui-sessions-mockup.md`](tui-sessions-mockup.md),
which covers the left-pane row tree. The interaction between the two
panes — selection in the tree drives the detail; `Enter` on a
relationship row drills the detail into a new focused node without
moving the tree selection — is called out where it matters.

## The Scenario

The cluttered case the `CSP-313` preamble flags: an agent session
with one preferred mux, two runtime-process observations (one
`process_identifies` winner, one `process_candidates` runner-up),
two known child sessions, one unresolved parent session, and a
checkout root. Resolved relationships and competing candidates are
both visible in the snapshot.

All paths render with `~`-shortening for `$HOME`. Short ids use the
existing CSP-127 FNV-1a hex.

## The Mockup

Shown at ~70 cols of right-pane content. Real layouts narrower than
this drop content per the rules in
[Narrow-terminal Behavior](#narrow-terminal-behavior) below; the
80×24 frame at this split is too cramped for the relationship
explorer to be useful, which is the trigger for the follow-up
responsive-layout story (`CSP-319`).

```
  agent_session · claude-code:7f3c…ad04                          7f3c
  ◀ workspace · sysadmin › agent_session

  ──────────────────────────────────────────────────────────  Node  ──
  id           7f3c2a91…ad04
  harness      claude-code
  alias        refactor pass
  cwd          ~/src/conspectus
  status       active · last 2m

  ─────────────────────  Upstream  · 4 groups · 5 links · 1 ⚠  ──
  ▶ process_identifies      runtime_process                    1
      proc:claude · pid 82310  ★
      strong_discovered · high · active
  ▼ process_candidates      runtime_process                  1 ⚠
    ▸ proc:claude-sub · pid 82412
      discovered · medium · active
  ▶ child_session           agent_session                      2
    rooted_in               checkout:conspectus               ★
                            discovered · high · active

  ─────────────────────  Downstream  · 2 groups · 1 link · 1 —  ──
    linked_to_mux           tmux:work-claude                   ★
                            discovered · high · active
    parent_session          — unresolved (1 evidence)
                            claude-code:9a1f…  ·  discovered · low

  ──────────────────────────────  Preview  · proc:claude · pid 82310  ──
  pid          82310
  parent       1     pane 82300
  command      /usr/bin/claude --resume 7f3c…  (truncated · o)
  cwd          ~/src/conspectus
  role         human_agent
  observed     8s ago
  edge         strong_discovered · high · active   ·   resolves

  ?  controls  ·  Enter focus  ·  ⌫ back  ·  e expand  ·  o open value
```

## What Each Element Means

### Title and breadcrumbs

```
  agent_session · claude-code:7f3c…ad04                          7f3c
  ◀ workspace · sysadmin › agent_session
```

Two lines at the top of the pane.

- **Title line**: the focused node's kind label and display label. The
  right-side short id is the CSP-127 short id. Both stay
  identical to what `conspectus node show` already prints.
- **Breadcrumb line**: drill path from the original selection to the
  focused node, separated by `›`. The leading `◀` is a hint that
  `Backspace` returns one hop. When the breadcrumb fits, intermediate
  hops use `kind · short-label` form; tighter widths collapse to the
  kind name alone. The original (root) breadcrumb hop is the row
  selected in the left tree.

When the focused node *is* the selected tree row (no drilldown yet),
the breadcrumb line is omitted to recover one row.

### Node zone

```
  ──────────────────────────────────────────────────────────  Node  ──
  id           7f3c2a91…ad04
  harness      claude-code
  alias        refactor pass
  cwd          ~/src/conspectus
  status       active · last 2m
```

A focused inspector for the selected node only. The divider is the
existing right-anchored chip style (ADR 0033) so it lines up
visually with the section dividers below.

The default render shows the five Core-marked fields per the
[Node Core-Summary Fields Reference](#node-core-summary-fields-reference).
A "full node" toggle (see
[Expanded Node Detail View](#expanded-node-detail-view)) replaces the
top-5 with every field the node carries, for operators who want the
full set without leaving the detail explorer.

### Upstream / Downstream sections

```
  ─────────────────────  Upstream  · 4 groups · 5 links · 1 ⚠  ──
  ─────────────────────  Downstream  · 2 groups · 1 link · 1 —  ──
```

Direction is encoded by section, not by per-row arrows. **Upstream**
groups incoming edges (where the focused node is the *target* of the
link); **Downstream** groups outgoing edges (where the focused node
is the *source*). For an agent session, `process_identifies` is
upstream because processes point at sessions; `linked_to_mux` is
downstream because the session points at the mux.

Header suffix carries `<group count> · <link count> · <ambiguity /
unresolved counts>` so the operator can tell at a glance how dense
each direction is without expanding anything.

Empty sections are suppressed entirely — a focused node with no
upstream edges renders only Downstream.

### Group row

```
  ▶ process_identifies      runtime_process                    1
  ▼ process_candidates      runtime_process                  1 ⚠
  ▶ child_session           agent_session                      2
```

Multi-link groups (2+ edges sharing the same `(direction, relation,
neighbor kind)` triple) render as a header row:

- `▼`/`▶` expansion glyph
- relation kind in full snake_case (per locked decision 3)
- neighbor kind in full snake_case
- link count, right-aligned
- trailing annotations: `⚠` for resolver ambiguity, `—` followed by
  an unresolved-evidence count when the group also has unresolved
  endpoints not represented as concrete neighbors

`e` and `Enter` both toggle the group's expansion when the cursor
rests on the header — `Enter` is the universal "do the obvious
thing" key, and for a collapsed tree row the obvious thing is to
reveal the children, not drill into one that isn't visible yet. Once
the group is expanded, `Enter` on a highlighted child row drills
into that specific neighbor. `e` on an expanded header collapses it
back.

### Composite single-link row

```
  ▶ process_identifies      runtime_process                    1
      proc:claude · pid 82310  ★
      strong_discovered · high · active

    rooted_in               checkout:conspectus               ★
                            discovered · high · active
```

When a `(direction, relation, neighbor kind)` triple has exactly
**one** link, the row collapses to a two-line composite:

- first line: relation kind on the left, neighbor label in the middle,
  resolved-winner `★` on the right
- second line: provenance · confidence · state, indented to align
  with the neighbor label column

No expand glyph and no group-count column, because there's nothing to
expand. The cursor selects the composite as a unit; `Enter` drills
into the neighbor.

This handles the common single-link cases (mux per session, checkout
rooted-in, branch-has-pr, parent_session per child) without forcing
operators through a header + child expansion they'd never collapse.

A multi-link header that happens to be expanded to one row stays in
its `▼` form — the collapse rule only applies when the count is
intrinsically 1, not when ambiguity filtering trims a group down.

### Resolved-winner sort and the `★` glyph

Within every group, resolver-preferred candidates (the link the
resolver chose for the corresponding `ResolvedRelationship`) sort
first and carry a trailing `★`. Non-resolved candidates render below
in provenance-then-confidence-then-link-id order — the same ordering
`preferred_link` already uses (`src/tui/detail.rs:797`).

The glyph stays for redundancy: sort order tells the eye, `★` tells
the cursor.

In the composite single-link form the `★` sits inline because the
group header is gone. In the multi-link expanded form the `★` is
inside each child row.

### Evidence vocabulary

Provenance, confidence, and state render with full
`snake_case` names (`strong_discovered`, `local_declared`,
`discovered`, `convention`, `cached`; `high`, `medium`, `low`;
`active`, `ignored`, `overridden`). No abbreviation, no jargon
codes.

Wraps within the row's content column when the value column gets
narrow; never gets truncated in the relationship rows themselves
because evidence is the load-bearing signal for whether to act on a
link.

### Preview zone

```
  ──────────────────────────────  Preview  · proc:claude · pid 82310  ──
  pid          82310
  parent       1     pane 82300
  command      /usr/bin/claude --resume 7f3c…  (truncated · o)
  cwd          ~/src/conspectus
  role         human_agent
  observed     8s ago
  edge         strong_discovered · high · active   ·   resolves
```

Always present when the right pane has focus and a relationship row
or group header is highlighted. Shows the **neighbor** node's core
summary (its top-5 fields from the
[Node Core-Summary Fields Reference](#node-core-summary-fields-reference)
table below) plus a trailing **`edge`** row summarizing the link
itself.

The `edge` row carries: provenance, confidence, state, and the
resolved-vs-candidate tag (`resolves`, `alt of <relation>`, or
`conflict` — see [Glossary](#glossary)). When the cursor is on a
group header rather than an inner row, the preview targets the
resolver winner for that group so selecting the header still gives
the operator something to read.

Long values such as commands, mux names, observation keys, and
transcript paths truncate with a `(truncated · o)` hint. Pressing
`o` opens the full value in the focused-value modal (`CSP-316`).

### Hint footer

```
  ?  controls  ·  Enter focus  ·  ⌫ back  ·  e expand  ·  o open value
```

Single line of context-sensitive accelerators. The `?` and
`controls` chip route to the existing Controls overlay (ADR 0031),
which is the discoverable, navigable surface for every detail
action; the inline accelerators are layered on top, not required.

When the right pane is unfocused, the hint footer shows the keys
needed to *enter* the detail explorer (focus-pane, then drill);
when it is focused, the hints reflect the current cursor position
(group header vs row, expanded vs not, drillable vs leaf).

## Single-Link Collapse Rule

Multi-link group, expanded:

```
  ▼ child_session           agent_session                      2
    ▸ claude-code:9a1f…   discovered · high · active           ★
      claude-code:1c0e…   discovered · high · active
```

Multi-link group, collapsed:

```
  ▶ child_session           agent_session                      2
```

Single-link composite (no header, no expansion):

```
    rooted_in               checkout:conspectus               ★
                            discovered · high · active
```

Same selectable unit in all three cases. `Enter` drills into the
neighbor.

Edge case — unresolved-only group:

```
    parent_session          — unresolved (1 evidence)
                            claude-code:9a1f…  ·  discovered · low
```

Renders as a single-link composite because there is one piece of
evidence, but the neighbor label slot carries an `— unresolved (N
evidence)` placeholder instead of a concrete neighbor. `Enter` is
inert (no node to drill into); `o` opens the evidence detail.

## Expanded Node Detail View

The Node zone's default render shows the five Core-marked fields per
the [Node Core-Summary Fields Reference](#node-core-summary-fields-reference).
A "full node" toggle replaces the top-5 with every field the node
carries — the union of Core and non-Core rows in the per-kind table
above. Pressing the same toggle again returns to the top-5 default.

Default accelerator: `F` (uppercase). Primary surface is the
Controls overlay per ADR 0031; the accelerator is layered on top.

Mockup of the agent_session case expanded:

```
  ──────────────────────────────────────────────────────────  Node  ──
  id                    7f3c2a91…ad04
  full id               agent_session:claude-code:~/.claude:7f3c2a91…
  harness               claude-code
  state_scope           ~/.claude
  session_key           7f3c2a91-…
  alias                 refactor pass
  title                 detail pane navigation
  cwd                   ~/src/conspectus
  status                active · last 2m
  last_active_epoch     1748707200
  last_message_preview  could you give me a bit more co…  (truncated · o)
  session_kind          human
```

Behavioral notes:

- The toggle is per-focused-node. Drilling into a neighbor (`Enter`
  on a relationship row) resets the new focused node back to the
  top-5 default. Backspace returns the previous focused node with
  whichever toggle state it was in before the drill. This keeps the
  expanded view a deliberate "I want everything for *this* node"
  action rather than a sticky pane mode.
- Long values (`command`, `url`, deep `cwd` paths,
  `last_message_preview`) truncate with the existing
  `(truncated · o)` hint; the `o` open-value modal (`CSP-316`) picks
  them up identically to the top-5 render.
- The Relationships and Preview sections render unchanged below the
  expanded Node zone. The right pane scrolls when the expanded Node
  zone plus the Relationships and Preview sections exceed pane
  height; section dividers keep operators oriented during the
  scroll.
- For node kinds whose entire field set already fits in the top-5
  (`Repo`, `Workspace`, `Branch`, `Checkout`, `Fork`), expanded and
  default render to the same content. The toggle still works, so
  operators don't have to know which kinds bother.

## Alternative / Addition: Navigable Graph View

A `g` toggle replaces the Upstream / Downstream linear lists with an
ASCII context map for the focused node and its 1-hop neighbors.
Multi-hop topology stays out — it doesn't survive ASCII rendering at
terminal widths.

```
  ──────────────────  Graph  · 6 neighbors · 9 links · 1 ⚠ · g linear  ──

       ↑  proc:claude · pid 82310       process_identifies       ★
       ↑  proc:claude-sub · pid 82412   process_candidates     ⚠
       ↑  claude-code:9a1f…             child_session              ★
       ↑  claude-code:1c0e…             child_session
       ↑  checkout:conspectus           rooted_in                  ★

                [  agent_session · claude-code:7f3c…ad04  ]

       ↓  tmux:work-claude              linked_to_mux              ★
       ↓  — unresolved                  parent_session     (1 ev)
```

Selection moves vertically through the neighbor list; the focal node
block is decoration, not selectable. `Enter` drills the same way it
does in the linear view; `g` returns to the linear view; `Backspace`
returns through the breadcrumb stack.

Tradeoffs vs the linear view:

- **Wins**: direction is encoded by position (above / below the
  focal node), so even an unfamiliar operator can read it as a
  graph. Resolved-vs-candidate sits in the same `★` column. Reads
  faster than scanning two header rows for groups vs counts.
- **Costs**: drops the relation-kind grouping the linear view
  provides (every neighbor is its own row, and a 10-child-session
  group eats 10 rows here vs 1 collapsed header row in the linear
  view). Evidence has to fit in a compact form or wrap; full
  `strong_discovered · high · active` doesn't quite fit at 70 cols
  with the neighbor + relation columns.
- **Placement (locked)**: `g` toggles the graph view; the linear
  view is the default. The graph view is not rendered alongside
  the linear list (see locked decision 7).

A heavier visualization with edge lines between neighbors (e.g. so a
`linked_to_mux` neighbor and a `process_identifies` neighbor that
share a runtime context could be drawn with a connecting arc) was
considered and rejected for v1 — laying out an arbitrary 1-hop graph
in ASCII without crossings gets expensive fast, and the linear view
already exposes the same data.

## Left / Right Pane Synchronization

Drilling through relationship rows on the right pane (`Enter` on a
neighbor) moves the right pane's focused node. By default the left
pane's selection follows that focus so the two panes stay in
agreement about which node is the current "where am I" — the right
pane is the detail surface for whatever the left pane points at, and
that contract should hold under drilldown too.

The behavior is governed by `[tui.detail].left_pane_sync` in config.
Three modes:

### `mirror` (default)

The left pane scrolls to and selects the tree row that corresponds
to the right pane's focused node, expanding group rows along the
ancestor path as needed. The left pane's *view* (`sessions` / `mux`
/ `union` / `prs` / `forks`) does not change.

When the focused node has no row in the current view — drilling to a
`runtime_process` neighbor while the sessions view is active, for
instance — the left pane keeps its previous selection rather than
blanking it. The breadcrumb stack on the right pane stacks left-pane
selection state, so `Backspace` restores both panes.

Examples:

- **Sessions view, drill session → parent_session**: left pane
  expands the parent's repo/checkout tree and selects the parent
  session row. View unchanged.
- **Sessions view, drill session → child_session in another repo**:
  left pane expands the child's repo group (which may have been
  collapsed) and selects the child row. View unchanged.
- **Mux view, drill session_in_mux_A → child_session_in_mux_B**:
  left pane selects `mux_B` (the mux that owns the child) and
  expands it to reveal the child session row, then selects the
  child. View unchanged.
- **Sessions view, drill session → runtime_process**: process nodes
  have no top-level view in v1, so the left pane keeps its previous
  selection. The right pane is the only place the process detail
  lives until the operator either drills back or selects something
  else in the tree.

### `follow` (opt-in)

When `mirror` would keep the left pane's previous selection because
the focused node has no row in the current view, `follow` instead
switches the left pane to a view where the focused node *does* have
a row, then selects it. The view-change is intentionally
opt-in because changing the left-pane view is more
context-disruptive than changing the selection alone — operators
who turn it on are accepting that drilldown can rearrange the
left pane.

Examples:

- **Sessions view, drill session → linked_to_mux**: left pane
  switches to the Mux view and selects the linked mux row. The
  operator is now exploring the mux, not the session, and the left
  pane reflects that.
- **Mux view, drill mux → contained_process**: process has no
  top-level view, so `follow` falls back to `mirror` for this hop —
  left pane keeps the mux selected.
- **Backspace**: restores the previous view *and* the previous
  selection (the breadcrumb stack carries view state too).

Operators can flip the mode mid-session through the Controls overlay
without editing the TOML config.

### `none`

The left pane stays exactly where it is during right-pane drilldown.
Useful for operators who want the left pane to keep showing the row
they originally selected so they can come back to it after exploring
the graph.

### Manual left-tree navigation cancels the drill

If the operator moves the left-tree selection manually (j/k/arrows
or other tree navigation) while the right pane is in a drilled
state, the right pane's focused node is replaced by the node
corresponding to the new tree selection and the breadcrumb stack is
collapsed. The two panes are back in sync at the new selection.

This keeps the panes consistent without fighting the operator who
decides to abandon the current drill path and start fresh, and it
matches the existing TUI behavior where left-tree selection drives
the detail pane.

### Config

```toml
[tui.detail]
left_pane_sync = "mirror"  # "mirror" (default), "follow", "none"
```

## Node Core-Summary Fields Reference

Every available field per node kind. The `Core` column marks the
top-5 picks for the Node zone (and, by reuse, for the neighbor
section of the Preview zone). Renderers can show more when terminal
height allows; this table is the comparison surface.

Surfaces represent fields from `src/model/mod.rs` plus the alias
overlay (ADR 0029) and the existing `last_active_epoch`-derived
status string.

### Repo (`RepoNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | CSP-127 short id |
| `common_dir` | `common_dir` | ✓ | `~`-shortened path |
| `remotes` | `remotes[]` | ✓ | First entry; count suffix when > 1 |
| `source_paths` | `source_paths[]` | ✓ | First entry; count suffix when > 1 |
| `id` (full) | `id` | ✓ | Repository identity in expanded form |

Repo carries only four model fields; everything available makes the
top 5.

### Checkout (`CheckoutNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `root` | `root` | ✓ | `~`-shortened |
| `current_branch` | `current_branch.refname` | ✓ | When set |
| `repo` | `id.repo` | ✓ | Owning repo display id |
| `git_dir` | `git_dir` | ✓ | `~`-shortened; differs for linked worktrees |

### Workspace (`WorkspaceNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `name` | `name` | ✓ | When set |
| `root` | `root` | ✓ | `~`-shortened |
| `provider` | `provider` | ✓ | `atelier`, generic, etc. |
| `id` (full) | `id` | ✓ | Workspace identity in expanded form |

### AgentSession (`AgentSessionNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | `harness:state_scope:session_key`, shortened |
| `harness` | `harness_key` | ✓ | |
| `alias` | aliases overlay | ✓ | When set; precedes title per ADR 0029 |
| `cwd` | `cwd` | ✓ | `~`-shortened |
| `status` | derived from `last_active_epoch` | ✓ | Relative recency string (`active · last 2m`) |
| `title` | `title` |   | Shown only when no alias overrides it |
| `last_active_epoch` | `last_active_epoch` |   | Raw epoch behind the status string |
| `last_message_preview` | `last_message_preview` |   | Shown in the Preview zone, not the inspector |
| `session_kind` | `session_kind` |   | `human` / `subagent` — surfaced via badge when set |
| `state_scope` | `id.state_scope` |   | Visible in the full id only |
| `session_key` | `id.session_key` |   | Visible in the full id only |

### MuxSession (`MuxSessionNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `backend · native_id` | `backend`, `native_id` | ✓ | Combined header label |
| `cwd` | `cwd` | ✓ | `~`-shortened |
| `attached` | `client_attached` | ✓ | `yes` / `no` from client-attached state |
| `last_active` | `activity_epoch` | ✓ | Relative recency string |
| `created` | `created_epoch` |   | Relative; less load-bearing than last_active |
| `active_pane_command` | `active_pane_command` |   | Live pane hint — Preview-relevant |
| `active_pane_pid` | `active_pane_pid` |   | Resolver input |
| `active_pane_current_path` | `active_pane_current_path` |   | Surfaces when cwd drifts |
| `active_pane_start_command` | `active_pane_start_command` |   | Session-key candidate; resolver input |

### RuntimeProcess (`RuntimeProcessNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `pid` | `pid` | ✓ | With `parent_pid` / `root_pane_pid` annotation |
| `command` | `command` | ✓ | Truncated; `o` opens full |
| `role` | `role` | ✓ | `human_agent` / `subagent` / `shell` / `unknown` |
| `observed` | `observed_epoch` | ✓ | Relative recency string |
| `cwd` | `cwd` |   | When set |
| `harness_key` | `harness_key` |   | When the role classifier set it |
| `depth` | `depth` |   | Process-tree depth from pane root |
| `parent_pid` | `parent_pid` |   | Folds into the `pid` annotation |
| `root_pane_pid` | `root_pane_pid` |   | Folds into the `pid` annotation |
| `observation_key` | `observation_key` |   | Full key behind the short id |

### Branch (`BranchNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `refname` | `refname` | ✓ | |
| `current_commit` | `current_commit` | ✓ | Short sha when available |
| `upstream` | `upstream` | ✓ | `origin/main` etc. |
| `repo` | `id.repo` | ✓ | Owning repo display id |

### Fork (`ForkNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `name` | `name` | ✓ | When set |
| `provider` | `provider` | ✓ | `atelier`, etc. |
| `scope` | `scope` | ✓ | Provider-defined scope label |
| `capabilities` | `capabilities[]` | ✓ | Count + first entry when set |
| `provider_source_key` | `provider_source_key` |   | Visible in the full id only |

### ForgePr (`ForgePrNode`)

| Field | Source | Core | Notes |
|-------|--------|------|-------|
| `id` (short) | `id` | ✓ | |
| `pr` | `owner/repo#number (state)` | ✓ | Composite label, mirrors `node show` |
| `draft` | `is_draft` | ✓ | When true |
| `updated` | `updated_epoch` | ✓ | Relative recency string |
| `url` | `url` | ✓ | Truncated; `o` opens full |
| `provider` | `provider` |   | Visible in the id only; `github` for v1 |
| `host` | `host` |   | Visible in the id only |
| `state` | `state` |   | Folds into the `pr` composite |
| `owner` / `repo` / `number` | `owner` / `repo` / `number` |   | Fold into the `pr` composite |

## Narrow-terminal Behavior

The mockup above assumes ~70 cols of right-pane content. At the
existing 80×24 sessions-view split the right pane has roughly 36
cols, which is too narrow for the relationship explorer as drawn. v1
narrow-pane rules (subject to operator review during `CSP-315`):

- The breadcrumb line drops first; the title's short id stays as the
  only "where am I" cue.
- Section header suffixes (`· 4 groups · 5 links · 1 ⚠`) drop next;
  the bare `Upstream` / `Downstream` labels stay.
- Composite single-link rows fall back to one line per row,
  truncating evidence to `provenance · state` (confidence drops).
- The graph view is unavailable below the threshold width and the
  `g` accelerator is dimmed in the hint footer.

A future layout pass should let the operator widen the right pane
when the detail explorer is in focus (cf. `CSP-190` on the contextual
status bar); that work is out of scope for `CSP-313`-`CSP-317`.

## Glossary

Codifies the relationship-explorer vocabulary so the renderer, the
tests, the snapshot fixtures, and operator-facing docs all use the
same words. Pair this with the model glossary in `src/model/mod.rs`
when the existing terminology needs surfacing.

**provenance** — where a link came from. Values: `local_declared`,
`global_declared`, `strong_discovered`, `discovered`, `convention`,
`cached`. Ordered by precedence per
`Provenance::precedence` (`src/model/mod.rs:492`).

**confidence** — strength of the evidence. Values: `high`, `medium`,
`low`.

**state** — lifecycle state. Values: `active`, `ignored`,
`overridden`.

**upstream** — incoming edges, where the focused node is the link's
*target*. Examples: `process_identifies` and `process_candidates`
for an agent session (processes point at sessions);
`workspace_contains_repo` for a repo (workspaces point at repos).

**downstream** — outgoing edges, where the focused node is the
link's *source*. Examples: `linked_to_mux` for an agent session;
`belongs_to_repo` for a checkout.

**resolved** — the link is the resolver's chosen winner for the
corresponding `ResolvedRelationship`. The `edge` row in the Preview
zone shows the literal word `resolves`; the inline `★` glyph mirrors
the same fact in relationship rows.

**candidate** — any active `GraphLink`, resolver winner or not. The
relationship explorer renders all active candidates; only the
winner sorts first and carries `★`.

**alt of `<relation>`** — a non-winning candidate competing for the
same `(source, relation)` slot as the resolved winner. The Preview
zone's `edge` row uses this form when the highlighted link lost its
resolver vote.

**conflict** — multiple candidates with non-trivial provenance or
confidence are competing for the same `(source, relation)` slot and
the resolver hasn't chosen cleanly. Mirrors the `⚠` annotation on
relationship rows. Distinct from `alt of`: `alt of` is what each
non-winning candidate is *labeled*; `conflict` is the *condition*
the group is in.

**ambiguous (⚠)** — group-level annotation: this group has at least
one resolver-flagged conflict. Equivalent to "the group contains at
least one link whose Preview `edge` row would say `conflict`".

**unresolved (N evidence)** — link evidence exists in the graph but
no concrete neighbor node has been discovered yet. Renders as a
single-link composite with `— unresolved (N evidence)` in the
neighbor label slot. Drilldown is inert in v1; see `CSP-318` for the
follow-up.

## Locked Decisions From This Review

These came out of the two inline-mockup passes on 2026-05-31 and are
recorded here so the next round can build on them without
re-deriving the reasoning.

1. **Upstream / Downstream split.** Direction is encoded by section,
   not per-row arrows; arrows weren't clear enough on a single row.
2. **Resolved sort first, glyph retained.** Resolved candidates sort
   to the top of each group, and the `★` glyph stays inline for
   redundancy.
3. **Full-name evidence vocabulary.** Provenance, confidence, and
   state render with their full `snake_case` names. No `H/M/L`,
   `disc`, `act`, or other compactions.
4. **Preview shows neighbor + edge together.** The Preview zone
   carries the neighbor's core fields *and* an `edge` summary row.
   No mode swap between neighbor-preview and edge-preview.
5. **Single-link composite row.** Groups with exactly one link
   collapse to a two-line composite row with no group header. The
   header form (with `▼`/`▶`) is reserved for 2+ links.
6. **Node Core-Summary Fields Reference as the comparison surface.**
   The fields table in this document is the canonical place to
   review which fields land in the Node zone; renderer changes that
   touch the field set update the table.
7. **`g` toggles the navigable graph view; linear is the default.**
   The graph view is a peer projection of the same data, reachable
   through `g` (and through the Controls overlay). It is not
   rendered alongside the linear view. The graph view is
   unavailable below the narrow-pane threshold.
8. **`Enter` on a group header expands; `Enter` on a child drills.**
   `Enter` is the universal "do the obvious thing" key, and the
   obvious thing on a collapsed tree row is to reveal the children.
   `e` remains the explicit expand/collapse accelerator and is
   redundant on the header (intentional — operators don't have to
   memorize which key does which job).
9. **Edge-state vocabulary stays as drafted (`resolves`, `alt of
   <relation>`, `conflict`)** and is codified in the
   [Glossary](#glossary) above. The mapping to `LinkStateLabel` is
   diagonal — `active`/`ignored`/`overridden` is a lifecycle
   axis, `resolves`/`alt of`/`conflict` is a resolver-outcome
   axis. The Preview `edge` row carries both when they're not
   redundant.
10. **`★` glyph stays.** The existing detail render already uses
    `⚠` and other non-ASCII glyphs, so the realistic terminal
    surface already supports `★`. An ASCII-fallback theme can
    rebind the glyph through the existing `Theme` plumbing
    (ADR 0032) if a concrete need surfaces.
11. **Expanded Node Detail toggle.** The Node zone has a "full
    node" toggle that swaps the top-5 render for every field the
    node carries (default accelerator `F`, primary surface
    Controls overlay). State is per-focused-node and resets on
    drill, so the operator opts in for each node deliberately.
12. **Left-pane mirror sync is the default.** Drilling on the
    right pane moves the left pane's selection along, expanding
    group rows as needed; the left-pane *view* does not change.
    Operators get the consistency contract — "the right pane is
    the detail surface for whatever the left pane points at" —
    preserved under drilldown without losing the original view.
13. **Left-pane view follow is opt-in via config.** A
    `[tui.detail].left_pane_sync = "follow"` mode switches the
    left-pane view when drilldown lands on a node kind absent
    from the current view (session → linked_to_mux switches to
    the Mux view). Off by default because the view change is
    more context-disruptive than the selection change. A `none`
    mode is also available for operators who want the left pane
    completely untouched.

### Filed As Follow-up Stories

- **`CSP-318`** — first-class evidence inspector and link-promotion
  flow for unresolved-evidence rows. v1 composite-row + `o`
  evidence open is the entry point; the long-term target is an
  inspector that supports promoting evidence to a declared link
  from inside the TUI, alongside the manual-link commands in
  `docs/design.md`.
- **`CSP-319`** — TUI responsive-layout design. Covers the
  thresholds where the right pane auto-expands on focus, where
  panes stack vertically, and where the right pane is hidden and
  swapped in via a tab; pairs with `CSP-190` and with this
  document's [Narrow-terminal Behavior](#narrow-terminal-behavior)
  rules.
- **`CSP-320`** — Expanded Node Detail toggle. Implements the
  `F`-toggle that swaps the Node zone between top-5 and full
  field set; reuses the existing `HeaderField` infrastructure
  and the per-kind fields-reference tables in this document.
- **`CSP-321`** — Left-pane mirror sync (default). Implements
  selection mirroring across drilldown, including breadcrumb-
  stacked left-pane selection state, and the manual-tree-
  navigation "checkout" behavior.
- **`CSP-322`** — Left-pane follow sync (opt-in). Implements the
  `follow` view-switching mode plus the Controls overlay entry
  for flipping between `mirror`, `follow`, and `none` mid-
  session. Stacked behind `CSP-321` so the mirror baseline ships
  first.

## Things The Mockup Doesn't Show

The mockup is a *layout* sketch. Everything visual beyond
glyph-and-text positioning is intentionally absent:

- **Text foreground color** — per-section, per-relation, per-state,
  per-harness palettes. Per-section colorization already exists in
  the current detail render (`field_value_style`,
  `src/tui/ui.rs:1662`); the relationship explorer extends that to
  rows and edge-state tags.
- **Text background color** — selection highlight, focus indicator,
  hover-style cues for keyboard navigation.
- **Text modifiers** — bold for labels and identity values, italic
  for placeholders, reverse-video for the selected row, dim for
  unresolved-evidence stubs.
- **Chip fills on dividers** — the right-anchored chip dividers
  (`──── Label ──`) carry a filled background per ADR 0032; the
  mockup's plain Unicode line stands in for that.
- **Pane edges** — the surrounding ratatui `Block` border that
  separates the detail pane from the rest of the TUI frame.
- **Provider-status / freshness chips** — the right-zone chips that
  already render in the status bar.
- **Per-harness color coding** — claude / codex / opencode / aider
  per the CSP-154 palette.

All visual differentiation in the layout above relies on glyphs and
spacing alone, so a black-and-white render of this mockup would still
be navigable but would feel undifferentiated. The final render goes
through the existing `Theme` (ADR 0032) so every label, value,
divider, glyph, and state tag carries the proper foreground /
background / modifier treatment for readability.

Other things still out of scope for this mockup:

- The Controls overlay (ADR 0031) entries for detail-pane actions.
- Empty / loading / error frames for the detail pane.
- Mouse interactions. Deferred per phase-08.
- Per-kind rendered variations of the Node zone for non-agent-
  session kinds. The fields table above is the v1 spec; rendered
  mockups for mux / process / repo / checkout / fork / pr nodes are
  good candidates for a follow-up review pass once CSP-313 modeling
  is stable.
