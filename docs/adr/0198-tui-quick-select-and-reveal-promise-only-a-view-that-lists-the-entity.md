# ADR 0198: Quick Select And Reveal Promise Only A View That Lists The Entity

## Status

Accepted. Amends
[ADR 0149](0149-two-level-search-current-and-wider-context.md) decision
6 (a fourth outcome, `not listed`, after `in <View> view`) and
[ADR 0144](0144-pinned-and-history-views.md) (a pin's menu offers
**Reveal in its native view** only where that view lists the entity).
Follows the consistency-first rule of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md). Carried
out by CSP-769.

## Context

Retiring the Forks view (CSP-711) left a live fork with no view, but
`actions::natural_view` still answered Sessions for it. Quick select's
wider reach indexes forks, so a fork that nothing focused leads to read
`in Sessions view` and then failed on `Enter` with "isn't listed in the
Sessions view"; a fork's pin offered **Reveal in its view**, which failed
the same way. The task left the choice to the operator among four
options: leave unreachable forks out of the index, give the open plan a
"not listed" outcome, hide Reveal for kinds no view lists, or give forks
a home.

The promise was wrong for more than forks. A view lists an entity only
as its own grouping and filter build it: Sessions shows a repo, a
checkout, or a workspace only as the group of its sessions, a branch or
a process has no row anywhere, and a filter hides rows. So a repo with no
sessions read `in Sessions view` too, and a gone mux's pin offered a
reveal the Mux view could not satisfy where it lists no ghost. Revealing
a pin from another view (`in Pinned view`) also failed, because the
reveal looked for rows by row id and a pin row's id names no node.

## Decision

1. **One rule: a gesture is promised or offered only where it lands.**
   Whether a view lists an entity is decided on the tree that view would
   show after a reveal switches to it: its saved grouping and filter (the
   current ones for the active view), History showing pinned records, and
   the recency sort a flat Sessions view switches to. `App::reveal_view`
   answers which view a reveal lands in, if any; `App::listing` answers it
   for every view at once.
2. **`natural_view` names no view for the kinds no view lists** (forks,
   branches, processes): it answers `None`, and a reveal of one says it
   "isn't listed in any view".
3. **Quick select gets a fourth outcome.** After `here`, `via <kind>` or
   `N hops`, and `in <View> view` (now only when that view lists the
   entity) comes `not listed`: `Enter` says why and leaves the operator
   where they were. The listing is built once when the reach widens, with
   the entity index, so drawing the words builds no tree; `Enter` checks
   the one view it needs again.
4. **The pin menu offers Reveal only where it lands**: when the pin's
   entity is listed by its view and that view is not the current one.
5. **A reveal finds a row by the entity it stands for** (the same test
   `here` and the right pane's mirror use), so a pin row is found by its
   pin.

## Consequences

- A fork is still found by name and still opens along a relationship
  path (`via repo`); with none it reads `not listed`, as a repo with no
  sessions does.
- A pin of a fork, a branch, or an entity no view lists (a gone mux where
  the Mux view lists no ghost, a checkout with no sessions) shows no
  **Reveal in its view**; its reveal keys (`O`, `Shift+Enter`) still say
  why they cannot.
- Widening quick select builds each view's tree once; a refresh while it
  is open leaves the words on the snapshot the index was built from, as
  the index itself already is.
- A consistency check holds the words to what `Enter` does for every
  entity of a world with every kind, and another holds the menu's Reveal
  to what revealing does for every pin.

## Alternatives Considered

- **Leave unreachable forks out of the index.** Whether a fork is
  reachable depends on what the right pane is on, so results would come
  and go as the focus moves, and a repo with no sessions would still
  promise Sessions.
- **Hide Reveal for the kinds no view lists, and keep the rest.** Smaller,
  but it leaves the same broken promise for entities of listed kinds that
  their view does not list.
- **Give forks a home** (rows in the Sessions graph, or a view again). A
  product change the retirement of the Forks view decided against; it
  belongs to its own story.
- **Drop unlisted results from quick select.** Honest, but the operator
  searching for a fork by name would find nothing and not learn it exists.

## Open Questions Answered

- *What does `Enter` do on a `not listed` result?* It says why (no view
  lists the kind, or the view it belongs in does not list it) and stays.
- *Is the listing per kind or per entity?* Per entity, on the tree the
  view would show.
