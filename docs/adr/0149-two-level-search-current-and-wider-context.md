# ADR 0149: Two-Level Search: Current Context And Wider Context

## Status

Accepted. Supersedes the parts of ADR 0147 that put a contents scope
inside the `/` overlay and that named its promote keys and the Search
view's relation to the content filter (see the amendment note in ADR
0147). Everything else in ADR 0147 stands: the matcher, the scanner, the
content filter, `conspectus search`, the payload rules, and the thresholds
for a persistent index. Amends ADR 0086 (Tier 2 gains mux scrollback as a
second content source) and builds on ADR 0112 (in-memory pane captures,
`--no-live-preview`). Amended by ADR 0157: the chip column of item 25 is
a chip's footprint from the theme.

## Context

ADR 0147 gave `/` two scopes, rows and contents, switched with `Tab`. In
use that mixes two different questions in one modal. "Take me to that
thing" is a rapid-select gesture over what is already on screen, answered
instantly from metadata. "Which session or terminal said X?" is a
background scan of payload that streams, can be promoted into a filter or
a view, and wants room for long snippets. The operator asked on
2026-10-26 to separate them:

- `/` searches the **current context**: a modal over the current filter,
  a rapid-select mechanism like search has always been, where Enter selects
  the chosen item in the left pane. `Tab`, with a visible tab identifier,
  widens it to **all entities**; selecting one either switches the view and
  selects it in the left pane, or finds the shortest path to it from the
  current view and highlights it in the right pane.
- `Ctrl+/` searches the **content** of all entities (session history for
  sessions, mux buffer history for muxes, and so on), deduplicating so that
  a mux buffer showing a session's history does not surface the same
  matches as the session history itself. `Tab` toggles current and all
  entities.
- The last `Ctrl+/` search is always kept in the Search view (`6`);
  `Ctrl+S` jumps there from the modal. The last N searches are remembered
  and paged in the Search view, so a new search does not lose the previous
  results.
- The search modals use much more width: on a wide terminal the 60-cell
  modal is unnecessarily narrow (`centered_modal_rect` caps it at 60).

Facts established while writing this decision:

- **Ctrl+/ on a terminal.** Legacy terminals send Ctrl+/ as the single
  byte `0x1F`, the same byte as Ctrl+_ and Ctrl+7. crossterm 0.29.0
  (`src/event/sys/unix/parse.rs`) decodes bytes `0x1C` to `0x1F` as
  `Char('4')` to `Char('7')` with the `CONTROL` modifier, so Ctrl+/ arrives
  as `(Char('7'), CONTROL)`. Only a terminal using the kitty keyboard
  protocol (or tmux `extended-keys`) sends a CSI-u sequence that decodes as
  `(Char('/'), CONTROL)`. The app never requests the protocol today (no
  `PushKeyboardEnhancementFlags` anywhere), and `ratatui::init` only
  enables raw mode and the alternate screen.
- **Ctrl+S.** crossterm's raw mode is `cfmakeraw`, which clears `IXON`, so
  Ctrl+S and Ctrl+Q reach the app instead of pausing the terminal. A tmux
  end-to-end test guards this.
- **Right pane.** The explorer shows one focused node with its resolved
  neighbors in Related zones. `Enter` on a neighbor row drills (pushes a
  `BreadcrumbHop`, rebuilds the view on the neighbor, and mirrors the left
  pane if the neighbor has a row); Backspace pops. Drill history already is
  a path, and an unfocused pane shows no highlight, so "highlight in the
  right pane" means: focus the right pane and put its cursor on the row.
- **Pane capture.** `capture_pane` runs `tmux capture-pane -p -J -e -S -100`
  for the live preview. `docs/capabilities.toml` already allows
  `capture-pane ...` as a read; it does not allow `list-panes`.
- **Today's `/` rows scope** searches `visible_rows()`, which skips rows
  under collapsed groups.

## Decision

### Two modals, one widget

1. **Quick select (`/`)** and **content search (`Ctrl+/`)** are two
   entry points to one wide search widget with two modes. Each shows its
   title and a segmented tab identifier on the top border:
   `/ quick select  [Current] All  (12)` and
   `Search contents  [Current] All  (3 sessions)`. The active tab is bold
   inside brackets; `Tab` and `Shift+Tab` switch. Both modals open on
   `Current` every time (predictable for a rapid-select gesture); the
   Search view's history entries record the scope that produced them.
2. The old contents scope inside `/` goes away. `Tab` in `/` no longer
   means "search contents"; it means "all entities".

### Quick select: current versus all (`/`)

3. **Current context** is every row in the active view's built tree after
   its filters, including rows under collapsed groups and nested rows. The
   scope is the filter, not the fold. Selecting a row expands its ancestors
   (the existing `expand_to_and_select`). This widens today's rule, which
   searched only visible rows, because a rapid-select should reach a row
   whatever its group's fold state.
4. **All entities** is every node in the snapshot that has an operator-
   visible name, plus the gone entities history projects:

   | Kind | Matched fields |
   |---|---|
   | Session | alias, title, session key, working directory, last-message preview |
   | Mux | name, working directory, pane command, harness |
   | Repo | path, remote URLs, default branch |
   | Checkout / worktree | path, branch, repo |
   | PR | `owner/repo#N`, title, head branch |
   | Pin | name, label, target label |
   | History record (gone) | label, recipe fields it shows (name, cwd, branch) |
   | Workspace, fork | path or provider, name |

   Runtime processes are never results. The message preview is payload, so
   it joins the haystack only when live preview is enabled (ADR 0112); under
   `--no-live-preview` the other fields still match. Ranking is the existing
   matcher's (ASCII case-insensitive fuzzy rank), with ties broken by kind
   order (session, mux, pin, checkout, repo, PR, history, other) and then
   recency.
5. **Result row**: kind glyph, label, dim context (path, branch, or repo),
   and a right-aligned outcome word saying what Enter will do.

### How an All result opens

6. **Rule order**, evaluated when the result is highlighted so the row can
   show it:
   1. **`here`**: the entity has a row in the current view's tree. Select
      it in the left pane (ancestors expanded) and keep focus where it is.
   2. **`via <kind>` or `N hops`**: the entity is related to the current
      selection within K = 3 hops. Focus the right pane on the selection's
      explorer view, drill along the shortest path (each drill pushes a
      breadcrumb hop, so Backspace retraces it), and leave the cursor on
      the target's row in the Related zone of the last hop, so it is the
      highlighted row. The outcome word names the first hop's neighbor kind
      (`via mux`, `via repo`) for one hop away or fewer, and `3 hops`
      beyond.
   3. **`in <View> view`**: otherwise switch to the view where the entity
      lives, using the existing `reveal_node` (gone entities go to Mux
      ghosts or History as today) and select its row there.
7. **Path finding.** Breadth-first over resolved relationships treated as
   undirected, using only resolver winners (`selected_link_id` set), the
   same edges the explorer's validated zone shows. Excluded: candidate and
   unresolved edges, `MuxContainsProcess`, `ProcessIdentifiesSession`, and
   `ProcessCandidatesSession`, because runtime processes are ephemeral and
   never targets. Ties between equal-length paths prefer, in order, the path
   whose first hop uses a structural relation (`BelongsToRepo`, `RootedIn`,
   `WorkspaceContainsRepo`, `CheckedOutBranch`, `BranchHasForgePr`,
   `LinkedToMux`, `PinTargets`, parent and child sessions) over the rest,
   then the lexicographically smallest node id sequence. The start is the
   node the right pane is focused on (the left selection's node, or the
   current drill focus when the right pane has focus).
8. **Explaining the choice.** The outcome word is always shown; nothing
   moves the operator without it being visible first.

### Content search (`Ctrl+/`)

9. **Content sources.** A `ContentSource` seam in `content_search` lists
   searchable units, scans one unit against a query with a cancel token,
   and says how its hits relate to other sources' hits. Two sources ship:
   - **Session transcripts** (the scanner of ADR 0147, unchanged).
   - **Mux pane scrollback** (new). Enumerate panes with one
     `tmux list-panes -a -F ...` per socket; capture each with
     `tmux capture-pane -p -J -S -5000 -t <pane id>` (joined wrapped lines,
     no color escapes, at most 5,000 lines and 2 MiB per pane), on worker
     threads, cancellable between panes, off the UI thread. A pane that
     fails to capture is counted as skipped. Gone muxes have no scrollback.

   Later sources (PR bodies, commit messages) implement the same trait.
10. **Current versus All.** *Current* is the sessions and muxes that have a
    row (including nested rows) in the active view's tree after its filters;
    *All* is every agent session and every live mux. In views that list
    neither (for example PRs with no nested sessions) *Current* is empty:
    the modal says so and stays on *Current* until `Tab`.
11. **Deduplication.** A pane normally mirrors the transcript of the agent
    session it runs. The pane's attributed sessions are those with a
    resolved `LinkedToMux` relation to its mux whose harness has a parser.
    Normalize text as the matcher does (ASCII fold, whitespace collapsed),
    and strip box-drawing and border characters from a pane line. A pane hit
    is **dropped** if the pane line (or the line with its neighbors joined,
    when the line is shorter than 12 characters) is contained in the
    normalized body of any matching turn of any attributed session. The
    comparison uses every turn kind, not only the kinds being searched, so a
    pane never surfaces text the session search deliberately withheld (tool
    output is on screen in the pane but off by default in the search).
    Panes with no attributed session (a plain shell, build output) keep all
    their hits, and so do panes whose attributed sessions cannot be parsed,
    because there is nothing to deduplicate against. Hits are never merged
    across two panes.
12. **Result shape.** Results are heterogeneous, so rows carry a source word
    (`session` or `pane`) per the representation rules for mixed views
    (ADR 0145). Session-level layout shows one row per session and one row
    per mux with pane hits; message layout nests hit rows; flat layout lists
    hits newest first (a pane hit has no timestamp and uses the capture
    time, so it sorts as of the scan). The `Ctrl-R` layout cycle, `Ctrl-T`
    tool content, and `Ctrl-Y` thinking content keep their meaning from ADR
    0147 and apply to sessions only.
13. **Opening a hit.** A session hit opens the viewer at the message. A pane
    hit selects the mux (revealed in the Mux view) and opens a read-only
    scrollback pager on a fresh capture, scrolled to the matched line with
    the match highlighted, using the pager key conventions. `a` attaches to
    the mux as for any mux subject.
14. **Payload and capabilities.** Pane scrollback is Tier 2 payload read
    under an operator gesture, with the same lifetime rules as transcript
    hits: memory only, capped and normalized snippets, never persisted,
    never in the graph, never over the daemon socket. ADR 0112 already
    accepts in-memory pane captures. Content search is unavailable under
    `--no-live-preview`, with the same message as today. The capability
    manifest gains one read, `list-panes ...`, citing this ADR;
    `capture-pane ...` is already allowed. Nothing writes.
15. **CLI.** `conspectus search --source sessions|panes|all` follows the TUI
    as a separate story, with `sessions` the default so current invocations
    do not change, and `all` applying the same deduplication.

### Keys

16. **Opening.** `/` opens quick select. `Ctrl+/` opens content search;
    the app accepts the three forms terminals send: `(Char('/'), CONTROL)`,
    `(Char('7'), CONTROL)` (what legacy terminals' `0x1F` decodes to), and
    `(Char('_'), CONTROL)`. The **guaranteed key** reachable in every
    terminal is plain `s` ("search contents"), checked for conflicts by the
    key-table tests (`S` already resumes a session, and a different matcher
    keeps the two apart), plus a Controls entry and a help row. The app does
    not enable the kitty keyboard protocol in this version (see Alternatives).
17. **Both modals.** `Tab` / `Shift+Tab` switch Current and All; `Up`,
    `Down`, `Ctrl+N`, `Ctrl+P` move; `Enter` picks (select / open); `Esc`
    closes (cancelling a running scan); `Ctrl+C` aborts. Footers are
    generated from the key tables, as for every modal.
18. **Content modal only.** `Ctrl+S` keeps the search and jumps to the
    Search view; `Ctrl+F` applies the query as the app-wide content filter
    and closes (ADR 0147); `Ctrl+R` cycles the layout; `Ctrl+T` and
    `Ctrl+Y` toggle tool and thinking content. `Ctrl+O` (open Search view)
    is retired in favor of `Ctrl+S`. Quick select has no options beyond
    `Tab`. In the main views `Ctrl+G` keeps its meaning (cycle grouping).
19. **Search view history keys.** `<` shows the previous (older) search and
    `>` the next (newer), checked against the key tables for conflicts; a
    Controls row opens a picker listing the remembered searches. `/` in the
    Search view is quick select over its rows like everywhere else, and
    `Ctrl+/` or `s` opens the content modal prefilled with the displayed
    entry's query and scope.

### The Search view and history

20. **Every content search is kept.** When a `Ctrl+/` scan starts, it
    becomes the newest history entry and the Search view displays it
    (streaming into it while it runs, even when the modal is open over
    another view, so `Ctrl+S` shows progress). An identical query, scope,
    and kinds as the newest entry updates it in place instead of adding one.
21. **N = 10 entries, in memory only**, configurable as `[search] history`
    (0 keeps only the displayed search). The oldest entry is evicted when
    the limit is reached. Entries hold hit summaries (count, latest time,
    best snippet per session; capped pane hits) but not per-message
    details; per-message details are re-read from transcripts when an entry
    is displayed in a message or flat layout, the global detail cap of
    ADR 0147 applying to the displayed entry only. Pane hits keep their own
    capped snippets (at most 200 per entry), since scrollback may have
    scrolled by the time the entry is displayed. A hard bound of 2,000
    retained snippets per entry (20,000 in total) keeps memory predictable.
22. **Paging does not rescan.** The header reads
    `search 2/5 "query" · Current · 12m ago` with the paging keys. Rerunning
    is opening the modal (prefilled) and pressing Enter, which makes a new
    entry.
23. **The filter is independent of paging.** `Ctrl+F` applies the displayed
    entry as the app-wide content filter (ADR 0147); paging afterward does
    not change the filter. When the displayed entry is the one the filter
    came from, the header says `filtering`. `F` clears the filter and does
    not delete history. The previous rule that the Search view lists "the
    sessions matching the active content search" becomes "the displayed
    history entry".
24. **Nothing is persisted.** Neither results nor queries are written to
    disk in this version (ADR 0147 keeps query state out of saved TUI
    state). Persisting query text alone is the first open question below.

### Sizing

25. **Modal rect.** Width is `min(W - 2, clamp(0.85 W, 60, 200))` and
    height `min(H - 2, clamp(0.8 H, 12, 50))`, centered, where `W` and `H`
    are the terminal's. Quick select and content search share it. Columns
    scale with the inner width `n`: marker 2, kind glyph 2, chip (a
    harness chip's footprint, `badge_width + 2`, 10 by default; ADR 0157)
    or source word 7, label `clamp(0.2 n, 12, 32)`, age 4, and the context
    or snippet takes the rest, never less than 20. When the terminal is
    narrower than 100 columns or `n` is small, columns drop in this order:
    chip, source word, age, context; the snippet is cropped around the
    match and ends with an ellipsis, text is never clipped mid-word, and
    footers wrap. Below 60 columns the modal fills the width.

## Consequences

- `/` is fast and predictable again, and the heavy, streaming, payload-
  reading search has its own entry point, keys, and room.
- Quick select over All entities makes every entity reachable from
  anywhere: by selection, by a visible path, or by switching views, and the
  outcome word keeps those three behaviors from feeling arbitrary.
- Mux scrollback becomes searchable, with a stated deduplication rule that
  can be tested and that errs toward keeping non-transcript output.
- Memory use is bounded by a per-entry and global snippet cap and by the
  existing detail cap; reading scrollback is bounded by 5,000 lines and
  2 MiB per pane.
- `Ctrl+/` depends on the terminal. The three accepted forms cover legacy
  and protocol-aware terminals, and `s` plus the Controls entry cover the
  rest, so no one is locked out.
- The retired contents scope, `Ctrl+O`, and the Search view's old tie to
  the content filter require migration in key tables, help, docs, tui-
  columns, and tests. The `SEARCH_CONTENT_KEYS` table becomes the content
  modal's table, and quick select gets its own.
- The capability manifest grows by one read.

## Alternatives Considered

- **Keep contents as a scope of `/`.** Rejected: it mixes a metadata
  selector with a payload scan, so `Tab` meant different things and the
  modal had to serve both a one-line selector and a snippet list.
- **Scope switching by query prefix** (VS Code's `>` `@` `#`). Attractive
  and familiar; deferred, not rejected. It could later alias the tabs, but
  prefixes are not discoverable without help and the tab identifier is.
- **Enable the kitty keyboard protocol to receive a real Ctrl+/.**
  Rejected for now. It changes the encoding of Ctrl+I versus Tab, Ctrl+[
  versus Esc, and Alt chords for every binding, needs tmux `extended-keys`
  to pass through, and is untested against the existing tables. The
  aliases and the plain key avoid the risk. Revisit if more chords need it.
- **Jump straight to the target in the right pane** (no drilling). Rejected:
  a jump shows the entity but not why it is related, and the breadcrumb
  makes the path visible and reversible.
- **Skip panes that run an attributed session** instead of deduplicating.
  Rejected: it loses build logs, errors, and any pane whose session cannot
  be parsed. **Deduplicate by timestamps.** Rejected: pane lines carry none.
- **Persist the last searches to disk.** Rejected for results (payload,
  ADR 0086). Queries alone are left to an operator decision.
- **Merge hits that appear in two panes.** Rejected: they are distinct
  places, and merging would hide one.

## Open Questions Answered

- *What is the current context for `/`?* Every row in the active view's
  tree after filters, including collapsed and nested rows.
- *What do All results match on?* The fields in the table above, with the
  message preview only when live preview is on.
- *How does an All result open, predictably?* The rule order above, with
  the outcome word shown on the row.
- *How is the path chosen?* Breadth-first over resolver winners, at most
  three hops, structural relations first.
- *How are pane and session matches deduplicated?* By containment of the
  normalized pane line in a matching turn of an attributed session, over all
  turn kinds.
- *Can Ctrl+/ be relied on?* No. Accept three encodings; guarantee `s`.
- *Does Ctrl+S work?* Yes: raw mode clears flow control.
- *How many searches are kept, and where?* Ten, in memory, configurable.
- *Does paging rescan?* No.
- *Does paging change the filter?* No; `Ctrl+F` applies the displayed entry.
- *How wide is the modal?* 85% of the terminal, between 60 and 200 columns.
- *Does the CLI get panes?* Later, as `--source`, defaulting to sessions.

## Open Questions Resolved By The Operator

1. **Persisting query text across restarts?** No. Search history stays in
   memory; no `remember_queries` option is added.
2. **Guaranteed key for content search?** `s` stays.
3. **Rows under collapsed groups in Current?** Yes. Current reads and
   selects them, expanding the groups above a picked row.
4. **Pinned and History in Current and All?** Quick select on Current
   picks any row of the Pinned or History view when it is the current
   view. All does not separate pinned from remembered entities: they are
   one set of entities, told apart only by their glyphs. Content scope in
   those views stays the rows' own entities plus nested ones.
5. **Scrollback depth?** 5,000 lines per pane is kept.
6. **Kitty keyboard protocol?** Not adopted for now; `Alt+Enter`, `O` and
   `s` remain the portable keys.

## Implementation Notes

Where the build settled details the decision left open, or departed from
its wording:

- **Opening the modal does not scan.** Opened from the Search view (or
  with a content filter active) the modal is prefilled and lists the
  results of the search it was filled from; the first change (typing,
  `Tab`, a search option) starts a new search. Rerunning is therefore a
  change to the query or options, not `Enter`, which opens the hit under
  the cursor. A rescan on open would have replaced the remembered search
  it was opened on.
- **Current in the Search view** is the scope the shown search was made in
  (or everything, if it read everything), not the Search view's own rows,
  which are results and no place to look for something else. The Search
  view with no search opens the modal on All.
- **Content search opens on All.** Operator preference departs from the
  decision's "opens on `Current` every time" for the contents modal
  (`Ctrl+/`, `s`, the Search view's `s`): a fresh content search reads
  everything, and `Tab` narrows it to the view. Quick select (`/`) still
  opens on `Current`. A modal opened onto an existing search keeps that
  search's scope.
- **A repeated search is not a new entry.** A content search that equals
  one already remembered (same trimmed text, kinds, and scope; layout is
  not part of it) takes that entry's place as the newest, shown entry
  instead of adding another, wherever the old one sat, and the new scan
  refreshes its results and age. Whether pane scrollback was read is part
  of the identity. Because a repeat adds nothing, it never pushes the
  oldest entry out.
- **One row per pane**, not per mux: a mux with several panes that match
  lists a row for each, wearing the mux glyph and acting on the mux.
- **The Search view stays fixed while the modal is open.** The search the
  modal is typing is the modal's own; it becomes the newest remembered
  search only when the modal exits (`Esc`, `Ctrl-S`, `Ctrl-F`, or opening
  a hit), through the same dedup. A search that never started, or whose
  query was emptied, is not remembered. `Ctrl-S` on an unchanged
  remembered search just shows it. (This replaces the earlier draft
  entries that the view showed as they were typed.)
- **Two granularities, and the options behind a modal.** The layout cycle
  (`Ctrl-R`) is gone. The modal's title carries a second choice,
  **Sessions** or **Messages**, switched with `Shift-Tab` in the modal (and in
  the Search view with `Ctrl-L`, since `m` and `o` are taken); Messages is the
  flat newest-first list. The nested per-message layout is no longer
  reachable in the TUI (`[search] group = "message"` means Messages; the
  CLI still has `--group message`). Changing the granularity regroups the
  shown matches and never scans again, and is not part of a search's
  identity. Tool calls, thinking, and pane scrollback are options in a
  pane behind `Ctrl-O` (also in the Search view and Controls), instead of
  `Ctrl-T` and `Ctrl-Y` in the key list; pane scrollback is new as an
  option and, like the other two, starts a new search when changed.
- **Stopped searches.** A search cut short (modal closed, or cleared while
  running) stays in the history marked `stopped`.
- **Snippet caps** are enforced when a scan ends: the lowest-ranked
  sessions beyond 2,000 per entry keep their counts but lose their text;
  the oldest entries go while all together exceed 20,000.
- **Controls** gained a `contents` row (opens the modal) and, in the Search
  view, a `searches` row (lists the remembered searches in a picker).
- Filter flows that do not use the modal (`f` → contains, `--contains`)
  read sessions only and are remembered like any search.
- **Clean slate, string paging, Shift-Tab (CSP-707).** `Ctrl-/` and `s`
  always open the contents modal with an empty query on All, whatever
  search the view shows or the lists filter by. `Ctrl-R` pages back through
  the remembered search strings (newest first, restoring each entry's
  reach and options and searching again) and `Ctrl-T` pages forward, to
  the empty field past the newest; only the older key is in the footer. The
  paging list is a snapshot taken when the modal opens. No path records an
  entry whose query is blank. In the contents modal `Shift-Tab` cycles
  Sessions and Messages and `Ctrl-L` is no longer bound there; `Tab` alone
  switches Current and All. Quick select keeps `Tab`/`Shift-Tab` for the
  reach.
- **Reveal and back (CSP-709).** `Shift+Enter` in the Search, History, and
  Pinned views runs the pin menu's "Reveal in its view" for the row's
  session, mux, or PR. Terminals mostly send it as `Enter`, and the kitty
  keyboard protocol stays off (see Alternatives), so `Alt+Enter` and a plain
  `O` do the same. Every cross-view jump records the view and selected row
  it left in a back list of at most 20 entries; `Backspace` with the left
  pane focused pops it, and the right pane keeps popping the explorer
  breadcrumb. A follow-up the operator did not ask for (selecting an entity
  after an action moved it) records nothing.
- **Full-screen pane viewer (CSP-708).** A pane hit opens a viewer that
  fills the screen and shares the session viewer's frame, key conventions,
  find prompt, and generated help page, instead of the small pager. `a`
  attaches to the pane's mux and closes the viewer; a stopped or gone mux,
  or the session Conspectus runs inside, refuses with a footer note and
  keeps the viewer open.
