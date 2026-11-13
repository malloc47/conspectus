# 0062: The explorer is a cursor and a drill stack over a source the host supplies

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/explorer.md](../controls/explorer.md) |

## Decision

- **The host supplies a source.** `ExplorerModel` names the kinds of value
  the explorer keeps: an `Id` it can show, a row `Key` stable across
  rebuilds, the host's own `Row`, the `Content` for one thing, and what the
  host `Saved` with a frame. `ExplorerSource` is built from the host's data
  for each call: `load` an id, the `id` of a content, its `zones` for a
  view, where a link row `follow`s, a breadcrumb tag, what to `save` with a
  frame, and the grid its rows draw in. The explorer holds owned values
  only, so it lives in the host's model.
- **The explorer keeps the state and its rules.** `Explorer<M>` holds the
  content shown, the cursor (by index and by row key), the open sections and
  the every-property switch, the drill frames, the arm, and the zone's
  window. After every rebuild the cursor goes back to the same key, else to
  the first row of the first zone that may take it, else to the first row.
  `refresh` reloads what is shown and keeps the drill. `Back` restores a
  frame's view and cursor and skips frames whose thing is gone; with no
  frame left it arms, and again it leaves. The cursor stops at the ends and
  pages by the rows shown.
- **Keys answer with an outcome.** `handle(key, &source)` does the
  explorer's part and answers `None`, `Copy`, `Inspect`, `Toggle`, `Drill`,
  `Follow` (a link whose far end does not load), `Back` (with the frame's
  saved value), `Armed`, `Leave`, or a `Refusal`. The property grid's
  intents feed it. The host does every effect and words every hint and
  refusal.
- **One key table.** `EXPLORER_KEYS` binds the moves, the page keys, the
  ends, `Enter` exactly (so its Shift and Alt chords stay free), the section
  keys, inspect, the every-property switch, and the back keys. A help page
  and a status hint whose `Enter` verb follows the cursor's row come from
  it, and a host takes it in as a pane's scope.

## Rationale

The host's detail pane kept its drill stack and zoned cursor as methods on
its whole application state, only a small part of it domain-free, and its
rules could not be tested alone. A source rebuilt per call keeps the
explorer free of borrowed host data, and outcomes keep it free of effects.

## Consequences

- The breadcrumb that shows the drill and the stacked layout are controls of
  their own (records [0018](0018-pieces-of-a-line-give-way-by-one-rule.md)
  and [0063](0063-stacked-master-detail.md)).

## Alternatives considered

- **Keeping the explorer in the host.** Lower risk, but the untestable state
  stays; it was the fallback if the move could not keep the frames.
- **Content kept outside the explorer.** A drill and a `Back` change the
  content, so the host would do half of each move.
- **One source trait over the host's data.** A source that borrows has a
  lifetime, which would end up in the host's field type; splitting the
  stored types from the access keeps it plain.
- **The explorer as an overlay.** It is a pane: drawn with or without focus,
  and it must not own every key.
- **Reusing the tree's state for the cursor.** Its fallback to the same
  position suits a tree, not zones.
- **A static `Enter` verb in the hint.** `Enter` copies, follows, or folds by
  row; the hint can say which.

## Origin

- host ADR 0188, "The explorer is a framework control over a host source,
  and the right pane's keys are one table": decisions 1 to 5.
