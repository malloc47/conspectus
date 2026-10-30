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

The frames show the left pane only, at a terminal width of 170 columns
unless a frame says otherwise, with ages measured at a fixed time. The
`►` at the start of a row marks the row the cursor is on. Colors are described in the tables
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
| 1 | Cursor gutter | 2 cells | `►` and a space on the cursor row, in the row's look; nothing on every other row | Every row of every view, focused or not, so no cell after it moves when the cursor does. The Search view's header line and the Pinned view's banner are not rows and have none |
| 2 | Indent | 2 cells per level | nothing | Flat Sessions (grouping `none`) has no indent |
| 3 | Disclosure | 2 cells | `▶` collapsed or `▼` expanded, in cyan | Blank when the row has nothing under it |
| 4 | Kind glyph | 1 cell and a space | what the row is (see [Glyphs](#glyphs)) | A group header that stands for no entity (the Ungrouped bucket, PR remote and state groups, and the Pinned, History, and Search buckets) has none: the cell is blank when another header in the tree has a glyph, so every header's label starts in one column, and omitted when none has |
| 5 | Pin marker | 2 cells | `◉` pin on a running entity, `◎` pin on one that isn't | Reserved on every row of the tree once any row in it has a marker, so columns hold still |

A column padded to its widest value (group labels, paths, and counts,
PR labels, and the label, status, and chip of a Pinned or History row)
is measured over every row of the tree, folded groups included, so
folding or unfolding a group never moves a column. A long name inside a
folded group still widens its column. A group header's label and a
Pinned or History row's label are padded to end in one column at every
depth (a nested row's is narrower by its indent), so what follows them, a
header's path and count or a row's state word, lines up with its
parent's.

The pin marker is the same pair in every view. In Sessions, Mux, and
PRs it appears on a row whose entity a pin points at. In Pinned it
appears on every top-level pin row: filled when the pinned entity is
running, hollow (in the ghost color) when it is stopped, removed,
missing, unknown, or not started yet. In History it is hollow for a
record a pin points at, shown only when History lists pinned records.

### Selection, pending refresh, and dimming

- The selected row starts with `►` in the cursor gutter, and is
  reversed and bold while the left pane has focus and bold only when it
  does not; the marker alone finds it on a terminal that draws no color
  or reverse video. A harness or program chip on it keeps the look it has
  on every other row, so it still reads as a chip inside the highlight.
  When a view lists one entity twice, only the row the cursor is on is
  marked and highlighted. The right pane and every menu, form, and list
  mark their cursor rows the same way.
- Right after an action in tmux (attach, kill, rebuild), the affected
  rows keep their old values, dimmed, with a spinner where their state
  glyph or word goes, until the refresh lands.
- A row for something stopped, removed, missing, planned, or unknown is
  drawn as a ghost. A group header for such an entity is too.

## Glyphs

Every glyph the left panel can draw. `Style` is the default theme; kind
glyphs can be overridden in the theme configuration.

Every glyph is one that DejaVu Sans Mono draws at exactly one cell, or one every common font carries. A glyph a terminal's monospace font lacks is borrowed from a wider font and spills over its neighbor, which cuts it in half under a selection highlight, so new glyphs are measured before they are used.

Two glyphs are kept although a font may draw them wider than a cell, because the operator prefers how they look. `▶`, the collapsed-row arrow, is U+25B6, which a terminal with an emoji font may draw wide (the cursor `►` is a different codepoint and has no such risk). The eight Braille spinner frames are in none of the common monospace fonts, so the terminal borrows a proportional one. The marks that several surfaces share (the cursor, the arrows, the check, failure, and warning marks, the no-value dash) are defined once, so a mark looks the same wherever it is drawn.

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
| `⋎` | fork | light magenta | Pinned fork pins |
| `⇄` | pull request | the style of its state glyph: open green, draft yellow, merged magenta, closed red, dim when the state isn't known | PR rows; Pinned and History PRs |
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
| `►` | the focused pane; or the cursor row of a list | cyan in a pane title, as is the focused pane's border; the row's look on a cursor row | Left of the tab bar; in the cursor gutter of the left pane's cursor row, and before the cursor row of the right pane, menus, forms, and lists |
| `▲` | at least one session below has an ambiguous mux link | yellow bold | Group headers |
| `◌` | a pull request with no local branch | dim | PR rows, in the tracking cell |
| `—` | an age or value that isn't known | dim | Age cells |
| `√` | a path or value that checked out | green | Forms |
| `×` | a failed check or value | red | Forms and check lists |
| `•` | an unchecked item | default | Multi-select lists, such as Clean up |
| `◄` | back | gray | The right pane's breadcrumb |
| `⠋` `⠙` `⠹` `⠸` `⠼` `⠴` `⠦` `⠧` | a refresh is pending for this row | gray | Where the state glyph or word goes |

The `…` character marks text cut to fit, and `·` separates parts of a
content string (`repo · branch`). Neither is a state. Every width on this
page counts terminal cells, not characters: a CJK character or an emoji
takes two, a combining accent none, so wide text is cut and padded at the
same columns as ASCII.

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
| `unknown` | the entity couldn't be checked, or a pull request has no state yet | `?` | Pinned, History |
| `open` | an open pull request | `○` | Pinned, History |
| `draft` | an open pull request marked draft | `◐` | Pinned, History |
| `merged` | a merged pull request | `⊕` | Pinned, History |
| `closed` | a closed, unmerged pull request | `⊗` | Pinned, History |

In Pinned, the word of a live pull request takes the style of its state
glyph (green, yellow, magenta, red, or dim for `unknown`); other live
words are green. Words of rows that are stopped, removed, missing,
planned, or unknown are ghost style. A pull request's word follows the
same rule as its glyph (see [PR row](#pr-row)): a closed or merged draft
reads `closed` or `merged`.

## Age

The age cell is four cells wide, right-aligned, in every view and every
row kind, the Search view's match and pane rows and the contents
search's results included. It reads
`Ns`, `Nm`, `Nh`, or `Nd` (days, with no larger unit), or `—` when no
time applies; an age too wide for the cell is cut with `…`. Where an age
is shown, it is colored by how recent it is: under five minutes is bright
green and bold, under an hour green, under a day yellow, and older dim.
Rows drawn as ghosts keep the ghost style instead. The same ladder writes
every age the TUI shows as prose (`5m ago` in the detail pane, Messages,
the header, the search status, and Clean up); the CLI's tables go on to
weeks (`3w`).

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

- **Harness chip.** A filled pill, 10 cells wide (the theme's
  `badge_width` of 8 and a cell of padding on each side), in the
  harness color: claude magenta, codex cyan, opencode green, aider red,
  pi light blue, anything else white. A longer name is cut with `…`. On
  a stopped row the chip stays its color but is dimmed.
- **Program chip.** For a mux whose pane runs something that isn't a
  known harness, a white-on-black pill of the same width shows the
  program. A mux with neither shows a dim ` no agent ` the same width.
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

## When a row is too narrow

Group headers, session rows, mux rows (running and stopped), PR rows,
Pinned and History rows, and the Search view's hit, pane, and pane hit
rows lay their cells out by one rule, in every view that lists them. A
row's room is the pane's width less the cursor gutter and the row's
indent. When the rows overflow the pane, the scrollbar takes its last
column, and every row is laid out as though the pane were still that
column wider: the cells that step with the width or take a share of it
(a mux name, a hit's name and tag) keep the widths they have while the
rows fit, and the preview, snippet, PR title, content, or directory
gives the bar its column; where that cell is already at its least, the
row gives the column up by the steps below.

1. The cells that line up down the tree (the prefix; a session's key,
   chip, age, and state; a mux's name, chips, age, and state; a PR's
   label, tracking mark, age, and state; every cell of a Pinned or
   History row; a header's label, path, count, and warning; every cell
   of a hit, pane, or pane hit row) keep the widths they have on every
   row: a padded column is as wide as its widest cell in the whole tree,
   folded groups included. When they do not fit, they give way in a fixed
   order, the same on every row at that width, so the columns that stay
   keep their places. Rows whose
   label ends in one column at every depth (headers, Pinned and History
   rows) give way alike at every depth too.
2. What follows them on a row (a session's label, `was in` note,
   project, hit count, and preview or snippet; a mux's hit count and
   preview, snippet, or directory; a PR's branch and title) takes the
   cells that row has left, and gives way in its own order, row by row.
3. A cell that gives way narrows, cut with `…`, or leaves the row with
   the gap before it. A label, path, project, or PR branch narrows to no
   fewer than 8 cells (unless its whole text is shorter) and leaves after
   that; a mux name narrows to 10 cells and stays; a PR's or Pinned or
   History row's label narrows to 8 cells and stays (a nested Pinned row's
   label may narrow further, so its state word stays in line). A PR label
   narrows its `owner/repo` and keeps its `#N` whole. A preview, snippet,
   PR title, the content of a Pinned or History row, or a stopped mux's
   directory shows only when it gets at least 7 cells, or its whole text.
   A hit's snippet keeps 20 cells while anything before it can give way,
   and narrows last.
4. A row that still does not fit is cut at the pane's edge with `…`. No
   cell is cut without a mark, and no column moves on one row but not
   another.

| Row | Gives way first to last | Never gives way |
|---|---|---|
| Group header | the path or members (narrowing first) | the label, the count, the warning |
| Session row, cells that line up | the key, the chip, the age | the state glyph |
| Session row, the rest | the preview or snippet, the `was in` note, the project (narrowing first), the hit count, the label (narrowing first) | |
| Mux row, cells that line up | the chips narrow to one chip and `+N`, the name narrows, the age, the chips | the name, the state glyph |
| Mux row, the rest | the preview, snippet, or directory, the hit count | |
| PR row, cells that line up | the label narrows (its number stays whole), the age, the tracking mark | the label, the state glyph |
| PR row, the rest | the title, the branch (narrowing first) | |
| Pinned or History row | the chip, the content, the label narrows, the age | the label, the state word |
| Hit, pane, or pane hit row | the chip or command, the age, the name narrows, the snippet narrows | the name, the turn, line, or hit count |

A Pinned row in a narrow pane keeps its name and its state word:

<!-- frame: keys="4jl" width=40 height=40 -->
```text
┌ ► sessions · mux · prs · pinned … ───┐
│  ▼ project  ~/repos/project          │
│►   ▼ ▣ ◉ project-main   live       5m│
│        ●   ambiguous …  in mux     1m│
│        ●   ambiguous …  in mux     1m│
│        ●   hook-super…  in mux     1m│
│        ●   claude-cod…  in mux     1h│
│      ▣ ◎ nightly        stopped    2d│
│      ● ◉ parser-sessi…  in mux     1h│
│      ◇ ◎ ledger-workt…  removed    3d│
│      ▣ ◎ weekly-review  planned     —│
│  ▼ Other                             │
│      ⇄ ◉ ledger-pr      merged     5d│
```

## Sessions view

Reached with `1`. Groupings (`Ctrl-G` cycles): **graph** (default),
**workspace**, **repo**, **checkout**, **scan-root** (same as repo), and
**none**. Within a group, sessions are newest activity first; a control
floats sessions that have a mux to the top.

### Group header

Workspace `▦`, repo `◆`, checkout `◇`, or the glyph-less Ungrouped
bucket, whose blank glyph cell keeps its label in the column of the
others.

| # | Cell | Source | Condition | Style |
|---|---|---|---|---|
| 1 | Label | workspace name, or the last part of the path | always | bold, padded so every header's label ends in one column, at every depth, folded groups included |
| 2 | Path | full path with `~`, or members and provider for a workspace | when it differs from the label | dim, cut in the middle to fit |
| 3 | `(cwd)` | the launch directory | only the group containing where Conspectus was started | cyan |
| 4 | Count | agent sessions beneath | N > 0 | dim, in one column across the tree's headers |
| 5 | `▲` | ambiguous links beneath | any | yellow bold |

A checkout header adds `(linked · locked · prunable)` for those states.
A checkout nested under its repo starts its path and ends its count in
the repo header's columns:

<!-- frame: keys="<C-g><C-g><C-g>jjjjjl" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│  ▼ ◆   repo-a                   ~/atelier-demo/repo-a     (2)                     │
│    ▼ ◇   repo-a                 ~/atelier-demo/repo-a     (2)                     │
│        ●   showcase   pi           2h  ∘  Showcase: pi in atelier member repo     │
│        ●   showcase   codex        3d  ∘  Showcase: codex parent in atelier fork …│
│  ▶ ◆   repo-b                   ~/atelier-demo/repo-b     (1)                     │
│► ▼ ◆   bare-project             ~/checkouts/bare-project  (1)                     │
│    ▶ ◇   bare-project (linked)  ~/checkouts/bare-project  (1)                     │
│  ▶ ◆   project                  ~/repos/project           (4)  ▲                  │
│  ▶     Ungrouped                                          (3)                     │
```

### Session row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | cursor gutter, indent, disclosure (only if the session has forks beneath it), `●`, pin marker | always | |
| 2 | Session key | the harness's own id | always | first 8 cells, dim, padded to 8 |
| 3 | Harness chip | the session's harness | always | 10 cells |
| 4 | Age | last activity | always | 4 cells |
| 5 | State | `⊸` linked to a definitive mux, `∘` not | always | 1 cell; a spinner while a refresh is pending |
| 6 | Label | alias, title, or mux name (see above) | when one applies | cut at 32 cells |
| 7 | `was in` chip | the session's last mux | only if that mux is stopped | dim |
| 8 | Project | repo or directory name | only in flat grouping | 16 cells, cut with `…` |
| 9 | Preview | last message | only if it gets at least 7 cells | dim italic |

While a content filter is active (`--contains TEXT`, or the Content row in
Controls), the rows are the sessions whose transcripts mention the text, and
each one carries a dim `N hits` chip between the state cell and the label.
The header count reads `matching/total sessions` and the footer shows
`find:"TEXT"` with `searching n/m` while the scan runs and `N sessions`
(plus `· M skipped`) once it is done.
The text lives only in memory. Pinned and History ignore the filter and say so.

<!-- frame: keys="" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ ▦   showcase-deck  atelier-repo-a+project  (agent-deck)  (1)                   │
│      ●   showcase   claude       1m  ⊸  Showcase: agent-deck composite launcher   │
│  ▶ ◆   repo-a         ~/atelier-demo/repo-a                 (3)                   │
│  ▶ ◆   repo-b         ~/atelier-demo/repo-b                 (1)                   │
│  ▶ ◆   bare-project   ~/checkouts/bare-project              (1)                   │
│  ▶ ◆   project        ~/repos/project                       (4)  ▲                │
│  ▶     Ungrouped                                            (1)                   │
```

<!-- frame: keys="jjjjjlj" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│  ▼ ▦   showcase-deck  atelier-repo-a+project  (agent-deck)  (1)                   │
│      ●   showcase   claude       1m  ⊸  Showcase: agent-deck composite launcher   │
│  ▶ ◆   repo-a         ~/atelier-demo/repo-a                 (3)                   │
│  ▶ ◆   repo-b         ~/atelier-demo/repo-b                 (1)                   │
│  ▶ ◆   bare-project   ~/checkouts/bare-project              (1)                   │
│  ▼ ◆   project        ~/repos/project                       (4)  ▲                │
│►     ●   showcase   claude       1m  ∘  ambiguous mux candidate A  Showcase: ambi…│
│      ●   showcase   claude       1m  ∘  ambiguous mux candidate B  Showcase: ambi…│
│      ●   showcase   claude       1m  ∘  hook-supersession current sessi…  Showcas…│
│      ● ◉ showcase   claude       1h  ∘  claude-code in project repo  I expanded `…│
│  ▶     Ungrouped                                            (1)                   │
```

<!-- frame: keys="<C-g><C-g><C-g><C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ●   showcase   claude       1m  ∘  ambiguous mux candidate A  project          │
│    ●   showcase   claude       1m  ∘  ambiguous mux candidate B  project          │
│    ●   showcase   claude       1m  ⊸  agent-deck composite launcher  showcase-dec…│
│    ●   showcase   claude       1m  ∘  hook-supersession current sessi…  project   │
│    ● ◉ showcase   claude       1h  ∘  claude-code in project repo  project        │
│    ●   showcase   pi           2h  ∘  repo-a            Showcase: pi in atelier m…│
│    ●   showcase   opencode     3h  ∘  repo-b            Showcase: opencode in ate…│
│    ●   showcase   codex       12h  ⊸  bare-work  bare-project      Showcase: code…│
│    ●   showcase   codex        2d  ∘  repo-a            Showcase: codex child for…│
│    ●   showcase   codex        3d  ∘  repo-a            Showcase: codex parent in…│
│    ●   showcase   codex        7d  ∘  orphan            Showcase: orphan session …│
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
| 1 | Prefix | cursor gutter, indent, disclosure (only with more than one agent attached), `▣`, pin marker | always | |
| 2 | Name | the mux name without its backend prefix | always | blue; a column of 18 cells, 20 once the row leaves it 37 cells, 24 from 71, and 30 from 103 (counted after the cursor gutter, the indent, and the prefix); long names keep their start and end |
| 3 | Chip | its agents' harness chips, as many as fit in a column of at most two, with `+N` for the rest; else the pane's harness; else a program chip; else ` no agent ` | always | 10 cells each; the column is as wide as the widest row's chips, so the age lines up |
| 4 | Age | the mux's own last activity | always | 4 cells |
| 5 | State | `⊸` a client is attached, `∘` none, `?` can't tell | always | 1 cell; a spinner while a refresh is pending |
| 6 | Preview | last message of the one agent | only with exactly one visible agent, and at least 7 cells for it | dim italic |

### Stopped mux row

A stopped mux that the view lists is drawn in the same columns, so it
lines up with the running ones, and as a ghost, its `▣` included.

| # | Cell | Source | Condition |
|---|---|---|---|
| 1 | Prefix | `▣` and the pin marker (`◎` for a pinned stopped mux) | always |
| 2 | Name | the mux name | always |
| 3 | Chip | the harness it last ran, dimmed; else ` no agent ` | always |
| 4 | Age | time since it was last seen running | always |
| 5 | State | `⊘` stopped, `?` unknown | always |
| 6 | Content | the directory it ran in | when it gets at least 7 cells |

<!-- frame: keys="2" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ▣   ambiguous                  claude      15m  ∘                              │
│    ▣   bare-work                  codex        2h  ∘  Showcase: codex resume via …│
│    ▣ ◉ project                    claude       5m  ⊸                              │
│    ▣   showcase-deck              claude      30m  ∘  Showcase: agent-deck compos…│
│    ▣ ◎ nightly-refactor           claude       2d  ⊘  ~/repos/project             │
```

<!-- frame: keys="2<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ ◆   bare-project  ~/checkouts/bare-project                                     │
│      ▣   bare-work                  codex        2h  ∘  Showcase: codex resume vi…│
│  ▶ ◆   project       ~/repos/project                                              │
│  ▶     Ungrouped                                                                  │
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
| PRs with no local repo | a blank glyph cell, so the name lines up with the others; the repo name in bold, the forge name dim |
| state | no glyph; `Open`, `Draft`, `Merged`, `Closed`, or `Other` |

Each ends with the count of sessions beneath its PRs, and `▲` when one of
them has an ambiguous mux link.

### PR row

| # | Cell | Source | Condition | Width and style |
|---|---|---|---|---|
| 1 | Prefix | cursor gutter, indent, disclosure (only with sessions beneath), `⇄` colored by state, pin marker | always | |
| 2 | Label | `#N`, or `owner/repo#N` | the short form only inside a group that holds one forge repo | bold, padded across the tree; when the row is narrow `owner/repo` is cut and `#N` stays whole |
| 3 | Tracking | `⌥` has a local branch, `◌` none | always | 1 cell |
| 4 | Age | last update | always | 4 cells |
| 5 | State | `○` open, `◐` draft, `⊕` merged, `⊗` closed, `?` no state | always | 1 cell |
| 6 | Branch | the local branch | only when tracked | dim, cut at 24 |
| 7 | Title | the PR title | when it gets at least 7 cells, or its whole title | gray, takes the rest |

One rule decides how a pull request's state shows, everywhere it shows:
the forge's state beats the draft flag. Only an open PR marked draft
reads as a draft (`◐`, the word `draft`, the `Draft` group). A closed or
merged PR that is still marked draft reads closed or merged, and a PR
with no state reads `?` (the word `unknown`, the `Other` group), draft
or not. The `⇄` kind glyph always takes its state glyph's style, so the
two never disagree.

<!-- frame: keys="3" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ ◇   conspectus/project  ~/repos/project  (4)  ▲                                │
│    ▶ ⇄   #7 ⌥    2h  ◐  feature/extra  Release notes for 0.3                      │
│  ▶ ◆   conspectus/project  ~/repos/project  no checkout                           │
│  ▶     project             conspectus/project                                     │
```

<!-- frame: keys="3<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼   Draft   (4)  ▲                                                               │
│      ⇄   conspectus/project#8 ⌥    1d  ◐  main  Pin the toolchain and refresh the…│
│    ▶ ⇄   conspectus/project#7 ⌥    2h  ◐  feature/extra  Release notes for 0.3    │
│  ▶   Merged                                                                       │
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
| 1 | Prefix | cursor gutter, indent, disclosure (only with rows beneath), the pin's kind glyph, pin marker | always | the glyph keeps its color even on a ghost row |
| 2 | Label | the pin's name | always | bold, cut at 28, padded so it ends in one column at every depth |
| 3 | Status | a word from [State words](#state-words) | always | cut at 9, padded; a spinner while a refresh is pending |
| 4 | Age | see [Age](#age) | always | 4 cells |
| 5 | Chip | the harness or program | mux and session pins | one width across the tree, so content lines up; the first cell to leave a narrow row |
| 6 | Content | by kind (below) | when it gets at least 7 cells, or its whole text | gray |

Content by kind: mux, its directory; session, the pin's title; worktree,
`repo · branch`; repo, its remote (else its path); branch, `repo · ref`;
PR, `owner/repo · title`; workspace, its root; fork, `provider · name`.

### Nested row

Rows beneath a pin use the same columns, with no pin marker, and their
state word starts in the pin row's column. A nested
mux reads `live`. A nested session's label is its alias, title, mux name,
or the first 8 characters of its key, its status is `in mux` or
`resumable`, and its content is the directory it ran in.

<!-- frame: keys="4" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ project  ~/repos/project                                                       │
│    ▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/proje…│
│      ▣ ◎ nightly                         stopped    2d   claude     ~/repos/proje…│
│      ● ◉ parser-session                  in mux     1h   claude     claude-code i…│
│      ◇ ◎ ledger-worktree                 removed    3d              project · fea…│
│      ▣ ◎ weekly-review                   planned     —   codex      ~/repos/proje…│
│  ▼ Other                                                                          │
│      ⇄ ◉ ledger-pr                       merged     5d              conspectus/pr…│
```

<!-- frame: keys="4<C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ Muxes                                                                          │
│    ▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/proje…│
│      ▣ ◎ nightly                         stopped    2d   claude     ~/repos/proje…│
│      ▣ ◎ weekly-review                   planned     —   codex      ~/repos/proje…│
│  ▼ Sessions                                                                       │
│      ● ◉ parser-session                  in mux     1h   claude     claude-code i…│
│  ▼ Worktrees                                                                      │
│      ◇ ◎ ledger-worktree                 removed    3d              project · fea…│
│  ▼ PRs                                                                            │
│      ⇄ ◉ ledger-pr                       merged     5d              conspectus/pr…│
```

<!-- frame: keys="4<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▶ ▣ ◉ project-main                    live       5m   claude     ~/repos/project │
│    ▣ ◎ nightly                         stopped    2d   claude     ~/repos/project │
│    ● ◉ parser-session                  in mux     1h   claude     claude-code in …│
│    ◇ ◎ ledger-worktree                 removed    3d              project · featu…│
│    ⇄ ◉ ledger-pr                       merged     5d              conspectus/proj…│
│    ▣ ◎ weekly-review                   planned     —   codex      ~/repos/project │
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
| 1 | Prefix | cursor gutter, indent, the record's kind glyph, pin marker (`◎` for a pinned record, when pinned records are listed) | always |
| 2 | Label | mux name; worktree name; repo name; PR number | cut at 28 |
| 3 | Status | `stopped` (mux), `removed` (worktree, repo), or the PR state | always |
| 4 | Age | time since it was last seen | always |
| 5 | Chip | the harness | muxes only |
| 6 | Content | mux, directory; worktree, `repo · branch`; repo, remote or path; PR, `owner/repo · title` | when it gets at least 7 cells, or its whole text |

<!-- frame: keys="5" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ project  ~/repos/project                                                       │
│      ▣ release-prep   stopped    4d   claude     ~/repos/project                  │
│      ◇ hotfix/login   removed    5d              project · hotfix/login           │
│      ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                  │
│      ▣ docs-sweep     stopped    9d   claude     ~/repos/project                  │
│      ◇ spike/cache    removed   12d              project · spike/cache            │
```

<!-- frame: keys="5<C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│► ▼ Muxes                                                                          │
│      ▣ release-prep   stopped    4d   claude     ~/repos/project                  │
│      ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                  │
│      ▣ docs-sweep     stopped    9d   claude     ~/repos/project                  │
│  ▼ Worktrees                                                                      │
│      ◇ hotfix/login   removed    5d              project · hotfix/login           │
│      ◇ spike/cache    removed   12d              project · spike/cache            │
```

<!-- frame: keys="5<C-g><C-g>" width=170 height=30 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────────────────────────┐
│►   ▣ release-prep   stopped    4d   claude     ~/repos/project                    │
│    ◇ hotfix/login   removed    5d              project · hotfix/login             │
│    ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project                    │
│    ▣ docs-sweep     stopped    9d   claude     ~/repos/project                    │
│    ◇ spike/cache    removed   12d              project · spike/cache              │
```

## Search view

Reached with `6`. It shows one of the content searches the process
remembers (the last ten; `[search] history` sets how many): the sessions,
and the mux panes, that matched a query typed in the contents modal
(`Ctrl-/`, or `s` where a terminal does not send it). Every search is
kept there as soon as it starts, newest last, so a new search does not
lose the one before it, and the view streams the results in while the
scan runs. Nothing is saved to disk; a restart forgets them.

The first line says which search is shown: its place among the remembered
ones, the query, whether it read the current view's sessions and muxes or
all of them, what it found, how long ago it ended, and `stopped` when it
was cut short. When more than one is remembered the right end names the
paging keys, `<` for an older search and `>` for a newer one; paging
shows what each search found without scanning again. Controls has a
`searches` row that lists them all. `filtering` ends the line when the
search shown is the one narrowing the Sessions, Mux, and PRs lists
(`Ctrl-F` makes it so; see the Content paragraph under the Sessions view),
and paging does not change which search that is. `F` clears the filter
and keeps every search.

With no search yet it says to press `s`. It is off when live preview is
disabled.

Groupings (`Ctrl-G` cycles): **none** (default, one ranked list),
**project**, and **harness**; panes, which belong to neither, sit under
**Mux panes**. Sessions and panes are ranked together by the number of
matches, then by the latest (a pane's latest is the time its scrollback
was read); the Recency sort puts the latest first. `Ctrl-L`, or the
`listed as` row in Controls, switches between **Sessions** (one row per
session or pane) and **Messages** (every match in one list, newest first;
the default comes from the `[search]` setting). Switching lists the same
matches another way and never searches again. `Ctrl-O` opens the search
options: whether tool calls and output, thinking, and pane scrollback are
searched.

Buckets look like the Pinned view's. `Enter` and `v` open the viewer at
the best match (or at the matching turn on a hit row); on a pane row or a
pane line `Enter` opens the pane's scrollback at that line in the
full-screen pane viewer. `Shift-Enter`/`Alt-Enter` reveals a session, mux, or PR
row in its own view, and `Backspace` on the left pane goes back. Every other
action works on the session, or on the pane's mux, as in the Sessions and
Mux views.

### Session row

The Sessions view's session row, taken from its flat layout, with the
`N hits` chip and the best snippet where a message preview would go. The age is the
session's last activity, as everywhere else, not the time of the match.

<!-- frame: keys="6sthe<C-f>" width=130 height=20 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/1 "the" · All · 2 sessions · 1 pane · 4 s… · filtering│
│►   ▣ ◉ project 0.0   2 hits            0s …once they pass.    │
│    ● ◉ showcase   claude       1h  ∘  project           2 hits│
│    ●   showcase   pi           2h  ∘  repo-a            2 hits│
```

<!-- frame: keys="6sthe<C-f><C-g>" width=130 height=20 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/1 "the" · All · 2 sessions · 1 pane · 4 s… · filtering│
│► ▼   Mux panes                                                │
│      ▣ ◉ project 0.0   2 hits          ready once they pass.  │
│  ▼   project    (1)  ▲                                        │
│      ● ◉ showcase   claude       1h  ∘  project         2 hits│
│  ▼   repo-a     (1)                                           │
│      ●   showcase   pi           2h  ∘  repo-a          2 hits│
```

### Hit row

A match in the Messages listing. It has no kind glyph. The hit, pane,
and pane hit rows take the columns of the contents search's results
(which put a `session` or `pane` word first), so a match reads the same
in the modal and in the view at one width. There is no session row above
a hit row, so it names its session first.

| # | Cell | Source | Condition | Style |
|---|---|---|---|---|
| 1 | Chip | the session's harness chip | gives way first | see [Labels, chips, counts, and warnings](#labels-chips-counts-and-warnings) |
| 2 | Name | the session's alias, title, or mux name, or the start of its id | always; narrows to 8 cells when the row is narrow | a fifth of the row's width, 12 to 32 cells |
| 3 | Turn | `turn N · kind`, counting from 1; the kind is `message`, `summary`, `tool call`, `tool output`, or `thinking` | always | dim; 22 cells in a row of 100 or more, 20 from 78, else 14 |
| 4 | Age | time since the turn was written, or a dash | gives way after the chip | see [Age](#age) |
| 5 | Snippet | the text around the match, cropped so the match stays on screen and cut on word boundaries; the match in the theme's match style (bold, underlined) | always: the cells before it give way so it keeps 20 cells | dim italic |

<!-- frame: keys="6sthe<BackTab><C-f>" width=130 height=20 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/1 "the" · All · 2 sessions · 1 pane · 4 s… · filtering│
│►   project 0.0   line 17           0s ready once they pass.   │
│    project 0.0   line 6            0s …the notes into a…      │
│    claude-code…  turn 3 · mess…  107d Expand the notes into a…│
│    claude-code…  turn 4 · mess…  107d …since the last release…│
│    pi in ateli…  turn 1 · mess…  107d …cover the fork index…  │
│    pi in ateli…  turn 2 · mess…  107d …the empty index,…      │
```

<!-- frame: keys="6sthe<C-f>f<Up><Space><Esc>" width=150 height=20 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ─────────────────────┐
│► ▼ project  ~/repos/project                                             │
│      ▣ release-prep   stopped    4d   claude     ~/repos/project        │
│      ◇ hotfix/login   removed    5d              project · hotfix/login │
│      ▣ bugfix-sprint  stopped    6d   claude     ~/repos/project        │
│      ▣ docs-sweep     stopped    9d   claude     ~/repos/project        │
│      ◇ spike/cache    removed   12d              project · spike/cache  │
```

### Pane row

A mux pane whose scrollback matched, where a session row would be. A mux
and its panes are one entity, so the row wears the mux glyph and acts on
the mux (`a` attaches). Hits the pane's own session already contains are
left out, so a pane that mirrors an agent does not repeat it.

It takes the [hit row](#hit-row)'s columns after the mux glyph.

| # | Cell | Source | Condition | Style |
|---|---|---|---|---|
| 1 | Prefix | cursor gutter, indent, disclosure | always | |
| 2 | Kind | the mux glyph `▣` | always | mux color |
| 3 | Command | what the pane was running, in the chip's column | when it is known; gives way first | dim |
| 4 | Name | the mux's name and the pane's `window.pane`, such as `work 0.1` | always; narrows to 8 cells | as the hit row's |
| 5 | Hits | `N hits` | always | dim, as the hit row's turn |
| 6 | Age | time since the scrollback was read | gives way after the command | see [Age](#age) |
| 7 | Snippet | the newest matching line, cropped around the match | always, at least 20 cells while anything else can give way | dim italic, match in the match style |

<!-- frame: keys="2<C-/>feature<C-s>" width=130 height=12 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/1 "feature" · All · 1 session · 1 pane · 4 skipped · …│
│►   ▣ ◉ project 0.0   4 hits            0s from feature/extra… │
│    ● ◉ showcase   claude       1h  ∘  project           4 hits│
```

### Pane hit row

A pane's match in the Messages listing. It has no kind glyph. It takes
the [hit row](#hit-row)'s columns: what the pane runs in the chip's
column, then the mux's name and the pane's place, then:

| # | Cell | Source | Condition | Style |
|---|---|---|---|---|
| 1 | Line | `line N`, counting from 1 in the scrollback, newest (lowest) first | always | dim, as the hit row's turn |
| 2 | Age | time since the scrollback was read | gives way after the command | see [Age](#age) |
| 3 | Snippet | the line, cropped around the match | always, at least 20 cells while anything else can give way | dim italic, match in the match style |

<!-- frame: keys="2<C-/>feature<BackTab><C-s>" width=130 height=14 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/1 "feature" · All · 1 session · 1 pane · 4 skipped · …│
│►   project 0.0   line 16           0s from feature/extra into…│
│    project 0.0   line 7            0s feature/extra.          │
│    project 0.0   line 3            0s …feature notes,…        │
│    project 0.0   line 1            0s …on feature/extra…      │
│    claude-code…  turn 1 · mess…  107d …on feature/extra…      │
│    claude-code…  turn 2 · mess…  107d …feature notes,…        │
│    claude-code…  turn 3 · mess…  107d …PR from feature/extra. │
│    claude-code…  turn 4 · mess…  107d …from feature/extra…    │
```

### Paging

<!-- frame: keys="6sthe<C-s>s<Backspace><Backspace><Backspace>release<C-s><<>" width=130 height=8 -->
```text
┌ ► sessions · mux · prs · pinned · history · search ───────────┐
│search 1/2 "the" · All · 2 sessions · 1 pane · 4 skipped · sca…│
│►   ▣ ◉ project 0.0   2 hits            0s …once they pass.    │
│    ● ◉ showcase   claude       1h  ∘  project           2 hits│
│    ●   showcase   pi           2h  ∘  repo-a            2 hits│
```
