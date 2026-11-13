# ADR 0210: Group Headers Count The Agents Waiting Beneath Them

## Status

Accepted. Carries out the group-header part of the operator's decision 2
in section 7.1 of the live-state plan (`docs/plans/live-state/README.md`;
the mux rollup half landed with
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
decision 3, whose precedence this record keeps). Amends
[ADR 0072](0072-mux-indicator-attachable-binary.md): a group row ends
with its session count, then the waiting count, then the ambiguity
glyph. Reads the row models' live values as ADR 0209 decision 7 does;
adds no theme role (ADR 0153) and no glyph (ADR 0154).

## Context

ADR 0209 put each agent's state in a cell of its row. A folded group
hides those cells, and in the Mux view an agent that is a mux's only one
has no row at all, so the operator can't tell from the headers where an
agent waits. The decision was "group headers show the waiting count next
to `(N)` when it is above zero". It left open what a header counts in
each view, how an agent listed twice is counted, how the count looks, and
what happens on a narrow pane.

What a header can count differs by view. Sessions, PRs, and Search list
sessions, and a session can be listed twice beneath one header (a PR
view checkout with two pull requests on one branch lists its sessions
under each). The Mux view lists muxes, and only nests a mux's agents when
there are several; a mux's counts already hold every agent it hosts, its
agents in panes no session matches among them, each agent counted on one
mux (its preferred one). Pinned and History buckets carry no counts.

## Decision

1. **A header counts waiting agents beneath it, each once.** It walks the
   whole tree under it, folded rows included, as the session count does.
   An entity listed more than once beneath one header counts once; a
   nested header counts what is beneath it, so a checkout's waiting
   session counts in the checkout's header and in its repo's. A session
   listed under two sibling headers counts in each, since each says what
   is beneath it.

2. **By the rows of the view's own kind.** In Sessions, PRs, and Search a
   header counts its session rows whose mark is waiting. In the Mux view
   it counts, for each mux row beneath it (running or stopped), the
   mux's agents that are waiting, from the mux's counts; the agents
   nested under a mux are among them and add nothing. A Mux view header
   can therefore show a waiting count with no `(N)`, which counts rows.
   Pinned and History count nothing.

3. **It reads as the marks it counts.** `! N`: the waiting mark, a space,
   and the number, one `StatusChip` in the waiting mark's tone (the
   theme's `warning`), so a header's count and the rows' `!` look alike.
   It follows `(N)`, and `▲` follows it. Each live mark gains a `tone`
   that its style is drawn from, so the chip and the marks share one
   source.

4. **One column, never giving way.** The count is a lined-up column of
   the header schema, left-aligned so every `!` is in one column, between
   the session count and the warning. It takes no cells while no header
   in the tree has a count. Like the count and the warning it never gives
   way: a narrow header's path narrows and leaves first.

5. **In place.** The count is worked out from the rows' live values each
   frame, so a store change refills it with the rows, without a rebuild.

## Consequences

- In the showcase, `project` reads `(4)  ! 2  ▲` in Sessions and PRs,
  and `! 2` alone in the Mux view, where its `ambiguous` mux counts the
  two waiting sessions that prefer it.
- Once any header has a count, every header of the tree reserves its
  width and gap (five cells for a one-digit count), so on a narrow pane
  every header's path narrows, and leaves, that much sooner.
- `(N)` still counts rows, so a session listed twice beneath a header
  counts twice there but once in its waiting count.
- The help page's "Agent state" legend gains `! N`, which widens that
  section's mark column.

## Alternatives Considered

- **Count rows, as `(N)` does.** Simpler, but a session under two pull
  requests would wait twice.
- **Count only listed session rows in the Mux view.** A mux whose one
  agent waits would show `!` on its row and nothing on its header.
- **Count each session in one header only.** It would keep a sum across
  sibling headers honest, but a header could hide a waiting row listed
  beneath it.
- **A dim count, like `(N)`.** It would recede beside the session count,
  where the point is to be seen; the waiting tone ties it to the marks.
- **Right-align the number, as `(N)` is.** `! 2` and `! 12` would put
  their marks in different columns.
- **Let the count give way before the path.** It would keep paths longer
  on narrow panes and drop the one thing the header adds for live state.

## Open Questions Answered

- *Does the warning still need a session count?* Yes, unchanged: `▲`
  says a listed session's mux link is ambiguous, and shows only with
  `(N)`.
