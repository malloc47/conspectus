# TUI reference: columns, glyphs, and states

This is the living reference for what the left panel draws: every view,
every row kind, every column, every glyph and state word, and when each
appears. Read it to vet what a view includes; change it in the same
commit as any change to those things.

It stays true by test. The glyph table, the state-word table, and the
list of views and row kinds are checked against the code, and the
example frames are re-rendered from the built-in showcase world and
compared. After an intended change to the TUI, edit the prose and
regenerate the frames:

```sh
just tui-columns          # UPDATE_TUI_COLUMNS=1 on the frame test
```

The frames show the left pane only, at a terminal width of 170 columns,
with ages measured at a fixed time. Colors are described in the tables
because a frame is plain text. For how the views fit into the rest of
the product, see [design.md](design.md); for keys and menus, see
[operations.md](operations.md).

## Principles

1. **Views of one kind of thing show state as glyphs.** Sessions, Mux,
   and PRs each list one kind of entity, so state is a single-cell glyph
   and never a word. Pinned and History list entities of every kind side
   by side, so they spell state as a word.
2. **A state shows the same way for every value.** Linked and not linked
   are both glyphs, and so are a pin on a running entity and a pin on one
   that isn't. No state hides behind the absence of a glyph, and none
   depends on color alone: each value has its own shape.
3. **One word for one thing, everywhere.** A mux that isn't running is
   `stopped`. A worktree or repo that no longer exists is `removed`. The
   same word appears in Pinned, History, the detail pane, and the CLI.
4. **Age is the most expected time for the view, and the same for the
   same entity wherever it appears.** See [Age](#age).
5. **Stopped and removed things look like ghosts.** Their rows use the
   ghost style: dim gray italics, with the harness chip dimmed rather
   than recolored.

## Row anatomy

### Common row prefix

Every row starts with the same cells, left to right:

| # | Cell | Width | Shows | Condition |
|---|---|---|---|---|
| 1 | Indent | 2 cells per level | nothing | Flat Sessions (grouping `none`) has no indent |
| 2 | Disclosure | 2 cells | `▶` collapsed or `▼` expanded, in cyan | Blank when the row has nothing under it |
| 3 | Kind glyph | 1 cell and a space | what the row is (see [Glyphs](#glyphs)) | Omitted on rows that stand for no entity: the Ungrouped bucket, PR remote and state groups, and the Pinned and History buckets |
| 4 | Pin marker | 2 cells | `◉` pin on a running entity, `◎` pin on one that isn't | Reserved on every row of the tree once any row in it has a marker, so columns hold still |

The pin marker is the same pair in every view. In Sessions, Mux, and
PRs it appears on a row whose entity a pin points at. In Pinned it
appears on every top-level pin row: filled when the pinned entity is
running, hollow (in the ghost color) when it is stopped, removed,
missing, unknown, or not started yet. In History it is hollow for a
record a pin points at, shown only when History lists pinned records.

### Selection, pending refresh, and dimming

- The selected row is reversed and bold while the left pane has focus,
  and bold only when it does not.
- Right after an action in tmux (attach, kill, rebuild), the affected
  rows keep their old values, dimmed, with a spinner where their state
  glyph or word goes, until the refresh lands.
- A row for something stopped, removed, missing, planned, or unknown is
  drawn as a ghost. A group header for such an entity is too.

## Glyphs

Every glyph the left panel can draw. `Style` is the default theme; kind
glyphs can be overridden in the theme configuration.

Every glyph is one that DejaVu Sans Mono draws at exactly one cell, or one every common font carries. A glyph a terminal's monospace font lacks is borrowed from a wider font and spills over its neighbor, which cuts it in half under a selection highlight, so new glyphs are measured before they are used.

| Glyph | Meaning | Style | Where it appears |
|---|---|---|---|
| `▦` | workspace | light blue | Sessions and Mux group headers; Pinned workspace pins |
| `◆` | repo | blue | Group headers for a repo; Pinned and History repos |
| `◇` | checkout or worktree | cyan | Group headers for a checkout; Pinned and History worktrees |
| `●` | agent session | light green | Every session row, in every view, including nested ones |
| `▣` | mux | magenta | Mux rows; Pinned and History muxes |
| `◉` | pin on a running entity | magenta | The pin marker cell, in every view |
| `◎` | pin on an entity that is stopped, removed, missing, unknown, or not started | ghost | The pin marker cell, in every view |
| `◈` | runtime process | process color | Never drawn in the left panel |
| `⌥` | branch | green | Pinned branch pins; and on a PR row, a pull request whose branch exists locally |
| `⋎` | fork | light magenta | Fork rows; Pinned fork pins |
| `⇄` | pull request | by state: open green, draft yellow, merged magenta, closed red | PR rows; Pinned and History PRs |
| `⊸` | linked: a session with a definitive mux, or a mux with a client attached | green | Session rows, Mux rows, nested sessions |
| `∘` | not linked: a session with no definitive mux (none or ambiguous), or a mux with no client | dim | Session rows, Mux rows, nested sessions |
| `⊘` | a mux that has stopped | ghost | Stopped mux rows in the Mux view |
| `?` | can't be checked: a mux whose server didn't answer, or a PR with no state yet | dim | Mux rows, stopped mux rows, PR rows |
| `○` | an open pull request | green | PR rows |
| `◐` | a draft pull request (open and marked draft) | yellow | PR rows |
| `⊕` | a merged pull request | magenta | PR rows |
| `⊗` | a closed pull request | red | PR rows |
| `▼` | an expanded row | cyan | The disclosure cell |
| `▶` | a collapsed row | cyan | The disclosure cell |
| `►` | the focused pane, or a menu cursor | cyan | Left of the tab bar and menu rows |
| `▲` | at least one session below has an ambiguous mux link | yellow bold | Group headers |
| `◌` | a pull request with no local branch | dim | PR rows, in the tracking cell |
| `—` | an age or value that isn't known | dim | Age cells |
| `√` | a path or value that checked out | green | Forms |
| `×` | a failed check or value | red | Forms and check lists |
| `•` | an unchecked item | default | Multi-select lists, such as Clean up |
| `◄` | back | gray | The right pane's breadcrumb |
| `⠋` `⠙` `⠹` `⠸` `⠼` `⠴` `⠦` `⠧` | a refresh is pending for this row | gray | Where the state glyph or word goes |

The `…` character marks text cut to fit, and `·` separates parts of a
content string (`repo · branch`). Neither is a state.

Two glyphs have a second life by design. `⌥` is both the branch kind
and the "tracked locally" mark on a PR row, because both say "this is a
local branch". `?` means "couldn't be checked" for muxes and PRs alike.

## State words

Only Pinned and History use words. Views of one kind show the glyph in
the last column instead.

| Word | Meaning | Glyph in a single-kind view | Where it appears |
|---|---|---|---|
| `live` | the pinned entity is running or present (worktrees, repos, branches, workspaces, forks, and live muxes) | pin marker `◉` | Pinned |
| `in mux` | a pinned session that is running inside a mux | `⊸` | Pinned, and sessions nested under a mux pin |
| `resumable` | a pinned session with no mux | `∘` | Pinned |
| `stopped` | a mux that isn't running | `⊘` | Pinned, History |
| `removed` | a worktree or repo that no longer exists | none (no view lists these alone) | Pinned, History |
| `missing` | a pin whose session or target can't be found anywhere | none | Pinned |
| `planned` | a pin with no target yet; the first rebuild creates it | none | Pinned |
| `unknown` | the entity couldn't be checked | `?` | Pinned |
| `open` | an open pull request | `○` | Pinned, History |
| `draft` | an open pull request marked draft | `◐` | Pinned, History |
| `merged` | a merged pull request | `⊕` | Pinned, History |
| `closed` | a closed, unmerged pull request | `⊗` | Pinned, History |

In Pinned, the word of a live pull request takes the hue of its state
glyph (green, yellow, magenta, red); other live words are green. Words of
rows that are stopped, removed, missing, planned, or unknown are ghost
style.

## Age

The age cell is four cells wide, right-aligned, in every view. It reads
`Ns`, `Nm`, `Nh`, or `Nd` (days, with no larger unit), or `—` when no
time applies. Where an age is shown, it is colored by how recent it is:
under five minutes is bright green and bold, under an hour green, under
a day yellow, and older dim. Rows drawn as ghosts keep the ghost style
instead.

What time the age counts from depends on the entity, and is the same in
every view that shows that entity:

| Entity | Age | Notes |
|---|---|---|
| Agent session | time since its last activity | `—` if the harness records none |
| Mux, running | time since the mux's own last activity | Agents inside it don't change this |
| Mux, stopped | time since it was last seen running | Mux view, Pinned, and History agree |
| Pull request | time since it was last updated | |
| Worktree or repo, running | `—` | Pinned only; nothing to count |
| Worktree or repo, removed | time since it was last seen | |
| Planned or missing pin | `—` | |

Sorting by recency uses these same times.

## Labels, chips, counts, and warnings

- **Harness chip.** A filled pill, 10 cells wide, in the harness color:
  claude magenta, codex cyan, opencode green, aider red, pi light blue,
  anything else white. A longer name is cut with `…`. On a stopped row
  the chip stays its color but is dimmed.
- **Program chip.** For a mux whose pane runs something that isn't a
  known harness, a white-on-black pill of the same width shows the
  program. A mux with neither shows a dim ` no agent `.
- **Session label.** The alias if there is one (bold); otherwise the
  title, but only when another session of the same harness in the same
  group would look the same; otherwise, for a session with no title,
  the name of the mux it runs in (or last ran in); otherwise nothing.
  Because the group decides, flat grouping shows more titles than
  grouped views. The detail pane and search always use alias, then
  title, then mux name, then the short id.
- **`was in <mux>` chip.** After a session's label, dim: the session's
  last mux no longer runs. When the label already is that mux's name it
  reads `(mux stopped)`. Sessions view only.
- **Counts.** A group header ends with `(N)`: the number of agent
  sessions anywhere beneath it, shown only when N is more than zero.
  Pinned and History buckets have none.
- **Warning.** `▲` ends a group header when a session beneath it has an
  ambiguous mux link.
- **Previews.** The last message of a session (or of the one agent in a
  mux) shows in dim italics at the end of the row, if room remains.

## Sessions view

Reached with `1`. Groupings (`Ctrl-G` cycles): **graph** (default),
**workspace**, **repo**, **checkout**, **scan-root** (same as repo), and
**none**. Within a group, sessions are newest activity first; a control
floats sessions that have a mux to the top.

### Group header

Workspace `▦`, repo `◆`, checkout `◇`, or the glyph-less Ungrouped
bucket.

| # | Cell | Source | Condition | Style |
|---|---|---|---|---|
| 1 | Label | workspace name, or the last part of the path | always | bold, padded to the widest in the tree |
| 2 | Path | full path with `~`, or members and provider for a workspace | when it differs from the label | dim, cut in the middle to fit |
| 3 | `(cwd)` | the launch directory | only the group containing where Conspectus was started | cyan |
| 4 | Count | agent sessions beneath | N > 0 | dim, right-aligned across groups |
| 5 | `▲` | ambiguous links beneath | any | yellow bold |

A checkout header adds `(linked · locked · prunable)` for those states.

### Session row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | indent, disclosure (only if the session has forks beneath it), `●`, pin marker | always | |
| 2 | Session key | the harness's own id | always | first 8 characters, dim, padded to 8 |
| 3 | Harness chip | the session's harness | always | 10 cells |
| 4 | Age | last activity | always | 4 cells |
| 5 | State | `⊸` linked to a definitive mux, `∘` not | always | 1 cell; a spinner while a refresh is pending |
| 6 | Label | alias, title, or mux name (see above) | when one applies | cut at 32 cells |
| 7 | `was in` chip | the session's last mux | only if that mux is stopped | dim |
| 8 | Project | repo or directory name | only in flat grouping | padded to 16 |
| 9 | Preview | last message | only if at least 8 cells remain | dim italic |

<!-- frame: keys="" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ ▦   showcase-deck  atelier-repo-a+project  (agent-deck)  (1)                     │
│    ●   showcase   claude       1m  ⊸  Showcase: agent-deck composite launcher     │
│▶ ◆   repo-a         ~/atelier-demo/repo-a               (3)                       │
│▶ ◆   repo-b         ~/atelier-demo/repo-b               (1)                       │
│▶ ◆   bare-project   ~/checkouts/bare-project            (1)                       │
│▶ ◆   project        ~/repos/project                     (4)  ▲                    │
│▶   Ungrouped                                            (1)                       │
```

<!-- frame: keys="jjjjjlj" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ ▦   showcase-deck  atelier-repo-a+project  (agent-deck)  (1)                     │
│    ●   showcase   claude       1m  ⊸  Showcase: agent-deck composite launcher     │
│▶ ◆   repo-a         ~/atelier-demo/repo-a               (3)                       │
│▶ ◆   repo-b         ~/atelier-demo/repo-b               (1)                       │
│▶ ◆   bare-project   ~/checkouts/bare-project            (1)                       │
│▼ ◆   project        ~/repos/project                     (4)  ▲                    │
│    ●   showcase   claude       1m  ∘  ambiguous mux candidate A  Showcase: ambigu…│
│    ●   showcase   claude       1m  ∘  ambiguous mux candidate B  Showcase: ambigu…│
│    ●   showcase   claude       1m  ∘  hook-supersession current sessi…  Showcase:…│
│    ● ◉ showcase   claude       1h  ∘  claude-code in project repo  I expanded `no…│
│▶   Ungrouped                                            (1)                       │
```

<!-- frame: keys="<C-g><C-g><C-g><C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│  ●   showcase   claude       1m  ∘  ambiguous mux candidate A  project            │
│  ●   showcase   claude       1m  ∘  ambiguous mux candidate B  project            │
│  ●   showcase   claude       1m  ⊸  agent-deck composite launcher  showcase-deck-c│
│  ●   showcase   claude       1m  ∘  hook-supersession current sessi…  project     │
│  ● ◉ showcase   claude       1h  ∘  claude-code in project repo  project          │
│  ●   showcase   pi           2h  ∘  repo-a            Showcase: pi in atelier mem…│
│  ●   showcase   opencode     3h  ∘  repo-b            Showcase: opencode in ateli…│
│  ●   showcase   codex       12h  ⊸  bare-work  bare-project      Showcase: codex …│
│  ●   showcase   codex        2d  ∘  repo-a            Showcase: codex child forke…│
│  ●   showcase   codex        3d  ∘  repo-a            Showcase: codex parent in a…│
│  ●   showcase   codex        7d  ∘  orphan            Showcase: orphan session wi…│
```

## Mux view

Reached with `2`. Groupings: **session** (default, a flat list), **host**
(the same), **repo**, and **workspace**. Muxes are listed by name;
with recency sorting they sort by their own activity, creation, or last
attach, as chosen in the controls, and a control floats muxes that have an
attached agent to the top. Stopped muxes always sort below the
running ones, most recently seen first.

By default the view lists running muxes, plus stopped muxes that a pin
points at. A control shows every stopped mux; any narrowing filter
hides all stopped ones. The header counts stopped muxes either way.

Group headers are the same as in Sessions, and carry a count only when
sessions are listed beneath a mux.

### Mux row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | indent, disclosure (only with more than one agent attached), `▣`, pin marker | always | |
| 2 | Name | the mux name without its backend prefix | always | blue; padded to a column of 10 to 30 cells depending on the pane width; long names keep their start and end |
| 3 | Chip | up to two harness chips (one if narrow) with `+N` for more; else the pane's harness; else a program chip; else ` no agent ` | always | 10 cells each |
| 4 | Age | the mux's own last activity | always | 4 cells |
| 5 | State | `⊸` a client is attached, `∘` none, `?` can't tell | always | 1 cell; a spinner while a refresh is pending |
| 6 | Preview | last message of the one agent | only with exactly one visible agent and room left | dim italic |

### Stopped mux row

A stopped mux that the view lists uses the same columns, so it lines up
with the running ones, drawn as a ghost.

| # | Cell | Source | Condition |
|---|---|---|---|
| 1 | Prefix | `▣` and the pin marker (`◎` for a pinned stopped mux) | always |
| 2 | Name | the mux name | always |
| 3 | Chip | the harness it last ran, dimmed; else ` no agent ` | always |
| 4 | Age | time since it was last seen running | always |
| 5 | State | `⊘` stopped, `?` unknown | always |
| 6 | Content | the directory it ran in | when room remains |

<!-- frame: keys="2" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│  ▣   ambiguous                  claude      15m  ∘                                │
│  ▣   bare-work                  codex        2h  ∘  Showcase: codex resume via /p…│
│  ▣ ◉ project                    claude       5m  ⊸                                │
│  ▣   showcase-deck              claude      30m  ∘  Showcase: agent-deck composit…│
│  ▣ ◎ nightly-refactor           claude       2d  ⊘  ~/repos/project               │
```

<!-- frame: keys="2<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ ◆   bare-project  ~/checkouts/bare-project                                       │
│    ▣   bare-work                  codex        2h  ∘  Showcase: codex resume via …│
│▶ ◆   project       ~/repos/project                                                │
│▶   Ungrouped                                                                      │
```

Under a mux with more than one agent attached, each agent is a nested
[Session row](#session-row).

## PRs view

Reached with `3`. Groupings: **checkout** (default), **repo**, and
**state**. Within a group, PRs are ordered by repository and then newest
number first; a control floats PRs that have a local branch to the top.
Sessions working on a PR's branch are nested beneath it.

### PR group header

| Grouping | Header |
|---|---|
| checkout | `◇`, the forge name (`owner/repo`) in bold, the checkout path dim |
| checkout, no checkout but a local repo | `◆`, the forge name, the path, and `no checkout` |
| repo | `◆`, the forge name, the path |
| PRs with no local repo | no glyph; the repo name in bold, the forge name dim |
| state | no glyph; `Open`, `Draft`, `Merged`, `Closed`, or `Other` |

Each ends with the count of sessions beneath its PRs, and `▲` when one of
them has an ambiguous mux link.

### PR row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | indent, disclosure (only with sessions beneath), `⇄` colored by state, pin marker | always | |
| 2 | Label | `#N`, or `owner/repo#N` | the short form only inside a group that holds one forge repo | bold, padded across the tree |
| 3 | Tracking | `⌥` has a local branch, `◌` none | always | 1 cell |
| 4 | Age | last update | always | 4 cells |
| 5 | State | `○` open, `◐` draft, `⊕` merged, `⊗` closed, `?` no state | always | 1 cell |
| 6 | Branch | the local branch | only when tracked | dim, cut at 24 |
| 7 | Title | the PR title | always | gray, takes the rest |

<!-- frame: keys="3" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ ◇   conspectus/project  ~/repos/project  (4)  ▲                                  │
│  ▶ ⇄   #7 ⌥    2h  ◐  feature/extra  Release notes for 0.3                        │
│▶ ◆   conspectus/project  ~/repos/project  no checkout                             │
│▶   project             conspectus/project                                         │
```

<!-- frame: keys="3<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼   Draft   (4)  ▲                                                                 │
│    ⇄   conspectus/project#8 ⌥    1d  ◐  main  Pin the toolchain and refresh the l…│
│  ▶ ⇄   conspectus/project#7 ⌥    2h  ◐  feature/extra  Release notes for 0.3      │
│▶   Merged                                                                         │
```

## Pinned view

Reached with `4`. Groupings: **project** (default; a pin's repo comes
from its target or directory, and pins with none go in `Other`),
**kind**, and **none**. Pins keep the order they were created in; sorting
by recency orders them by their age instead. A mux pin lists the sessions
it hosts or last hosted, and a worktree pin lists the muxes running in
it and their sessions.

### Pin bucket header

A bucket has no glyph and no count: for project grouping, the repo name
in bold and its path dim; for kind grouping, `Muxes`, `Sessions`,
`Worktrees`, `Repos`, `Branches`, `PRs`, `Workspaces`, or `Forks`.

### Pin row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | indent, disclosure (only with rows beneath), the pin's kind glyph, pin marker | always | the glyph keeps its color even on a ghost row |
| 2 | Label | the pin's name | always | bold, cut at 28, padded across the tree |
| 3 | Status | a word from [State words](#state-words) | always | cut at 9, padded; a spinner while a refresh is pending |
| 4 | Age | see [Age](#age) | always | 4 cells |
| 5 | Chip | the harness or program | mux and session pins | one width across the tree, so content lines up |
| 6 | Content | by kind (below) | when room remains | gray |

Content by kind: mux, its directory; session, the pin's title; worktree,
`repo · branch`; repo, its remote (else its path); branch, `repo · ref`;
PR, `owner/repo · title`; workspace, its root; fork, `provider · name`.

### Nested row

Rows beneath a pin use the same columns, with no pin marker. A nested
mux reads `live`. A nested session's label is its alias, title, mux name,
or the first 8 characters of its key, its status is `in mux` or
`resumable`, and its content is the directory it ran in.

<!-- frame: keys="4" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ project  ~/repos/project                                                         │
│  ▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/project │
│    ▣ ◎ nightly                         stopped    2d   claude     ~/repos/project │
│    ● ◉ parser-session                  in mux     1h   claude     claude-code in …│
│    ◇ ◎ ledger-worktree                 removed    3d              project · featu…│
│    ▣ ◎ weekly-review                   planned     —   codex      ~/repos/project │
│▼ Other                                                                            │
│    ⇄ ◉ ledger-pr                       merged     5d              conspectus/proj…│
```

<!-- frame: keys="4<C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ Muxes                                                                            │
│  ▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/project │
│    ▣ ◎ nightly                         stopped    2d   claude     ~/repos/project │
│    ▣ ◎ weekly-review                   planned     —   codex      ~/repos/project │
│▼ Sessions                                                                         │
│    ● ◉ parser-session                  in mux     1h   claude     claude-code in …│
│▼ Worktrees                                                                        │
│    ◇ ◎ ledger-worktree                 removed    3d              project · featu…│
│▼ PRs                                                                              │
│    ⇄ ◉ ledger-pr                       merged     5d              conspectus/proj…│
```

<!-- frame: keys="4<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/project   │
│  ▣ ◎ nightly                         stopped    2d   claude     ~/repos/project   │
│  ● ◉ parser-session                  in mux     1h   claude     claude-code in pr…│
│  ◇ ◎ ledger-worktree                 removed    3d              project · feature…│
│  ⇄ ◉ ledger-pr                       merged     5d              conspectus/projec…│
│  ▣ ◎ weekly-review                   planned     —   codex      ~/repos/project   │
```

## History view

Reached with `5`. Groupings: **project** (default; a worktree belongs to
the repo it was made from, PRs go in `Other`), **kind** (`Muxes`,
`Worktrees`, `Repos`, `PRs`), and **none**. Rows are newest sighting
first. The view lists what Conspectus has seen stop or disappear, plus
pull requests that left the listing. A control, on by default, hides
records that a pin points at. There are no session records.

Buckets look like Pinned buckets.

### History row

Every row is drawn as a ghost, with the same columns as a pin row.

| # | Cell | Source | Condition |
|---|---|---|---|
| 1 | Prefix | indent, the record's kind glyph, pin marker (`◎` for a pinned record, when pinned records are listed) | always |
| 2 | Label | mux name; worktree name; repo name; PR number | cut at 28 |
| 3 | Status | `stopped` (mux), `removed` (worktree, repo), or the PR state | always |
| 4 | Age | time since it was last seen | always |
| 5 | Chip | the harness | muxes only |
| 6 | Content | mux, directory; worktree, `repo · branch`; repo, remote or path; PR, `owner/repo · title` | when room remains |

<!-- frame: keys="5" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ project  ~/repos/project                                                         │
│    ▣ release-prep   stopped    4d   claude     ~/repos/project                    │
│    ◇ hotfix/login   removed    5d              project · hotfix/login             │
│    ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                    │
│    ▣ docs-sweep     stopped    9d   claude     ~/repos/project                    │
│    ◇ spike/cache    removed   12d              project · spike/cache              │
```

<!-- frame: keys="5<C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│▼ Muxes                                                                            │
│    ▣ release-prep   stopped    4d   claude     ~/repos/project                    │
│    ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                    │
│    ▣ docs-sweep     stopped    9d   claude     ~/repos/project                    │
│▼ Worktrees                                                                        │
│    ◇ hotfix/login   removed    5d              project · hotfix/login             │
│    ◇ spike/cache    removed   12d              project · spike/cache              │
```

<!-- frame: keys="5<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history ────────────────────────────────────────┐
│  ▣ release-prep   stopped    4d   claude     ~/repos/project                      │
│  ◇ hotfix/login   removed    5d              project · hotfix/login               │
│  ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                      │
│  ▣ docs-sweep     stopped    9d   claude     ~/repos/project                      │
│  ◇ spike/cache    removed   12d              project · spike/cache                │
```

## Union view

Reachable only with `--view union`; it is not in the tab bar. It lists
every session, then every mux, flat, with the Session and Mux columns
above. It shows no pin marker, and its ages read `—` because it does not
measure time.

## Forks view

Reachable only with `--view forks`; it is not in the tab bar. Groupings:
by provider or by parent (the choice currently does not change the
tree).

### Fork row

| # | Cell | Source | Style |
|---|---|---|---|
| 1 | Prefix | indent, disclosure, `⋎` | |
| 2 | Label | `provider:name`, or the source key | bold |
| 3 | Parent | `parent:<label>` | gray |
| 4 | Scope | where the fork lives (a workspace, say) | dim |
| 5 | Count | `(N)` child sessions | dim |

Child sessions nest beneath as [Session rows](#session-row).
