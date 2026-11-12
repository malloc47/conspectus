# ADR 0202: Each View Reopens With The Rows It Had Expanded And Selected

## Status

Accepted. Amends [ADR 0031](0031-tui-filter-view-switching-and-per-view-state.md)
(section 3, per-view state): of a view's state, the filter and grouping
already outlived the process in `tui-state.json`; the expanded rows and
the selected row now do too, for every view but Search. The left-panel
scroll offset still does not: the pane scrolls to the restored selection.
Writes stay inside [ADR 0143](0143-mutation-envelope-with-history-pins-and-clone.md) category 2: the
file is a rebuildable observation under `$XDG_STATE_HOME/conspectus/`,
and no write path is added.

## Context

`conspectus tui` restores the last view and each view's filter, grouping,
sort, and preview settings from `$XDG_STATE_HOME/conspectus/tui-state.json`,
but every run started with the default folds and the cursor on the first
row (or the launch directory's group). Each view keeps its expanded rows
and selection in its tree state (`tuikit::tree::TreeState<RowId>`, ADR
0182) for the life of the process, so an operator who drops in and out of
the TUI re-opened the same workspaces, repos, and muxes and found their
place again every time.

Three things make the tree harder to keep than a filter:

1. **Its identities can go stale.** A row is keyed by a graph node id, a
   pin id, a History record id, a bucket label, or a forge slug. Between
   runs a session can end, a pin be removed, a repo be deleted, or a
   daemon restart change what is listed.
2. **The rows are not there when the file is read.** The state is read
   at startup; the first rows arrive with the first refresh, and the
   other views' rows only when the operator switches to them.
3. **The expanded set only grows** if nothing removes the groups that
   went away.

## Decision

1. **What is kept.** Each view's entry in `view_states` gains `expanded`,
   a list of row keys, and `selected`, an object with the selected row's
   key (`row`) and the keys of the rows above it, top-level first
   (`path`). Every expanded row the view lists is kept, not only group
   rows: a pin, a session with nested sessions, or a PR the operator
   opened is as deliberate as a group. The Search view keeps neither,
   since its rows answer a query that is not restored, and nothing keeps
   the right pane's drill or the preview's scroll.
2. **One string encoding for rows.** A row key is the row's kind
   (`group`, `session`, `mux`, `pr`, `history`, `pin`, `nested`,
   `bucket`, `remote`, `synthetic`), a colon, and what names it: a node
   id in its text form, a record or pin id, a label, or a slug; a nested
   row is its pin id and its node's text, split by a line break (a pin id
   is one line). The file's top-level `row_keys` names the encoding
   (`1`). Keys are never decoded: a later run computes the keys of the
   rows it lists and compares text, so a key that names no row simply
   matches nothing. To keep two rows from sharing a key, a node id whose
   text reads back as another id (`NodeId::parse`) gets no key, and
   neither do the Search view's turn, pane, and line rows; a row with no
   key is not kept, and a selected row with none is kept as the nearest
   row above it that has one.
3. **Each view's saved tree applies once, when the view first lists
   rows.** That is the first refresh for the starting view and the first
   switch to each other view; a refresh that lists no rows leaves it
   waiting. Applying it: the expanded rows become the saved keys that
   name a listed row; the cursor goes to the saved row, else to the
   deepest row of its saved path still listed, else to the first row;
   between copies of a row listed twice, to the copy whose rows above it
   match the saved path best; and the groups above the cursor are opened
   when the saved folds hid it. The pane's window follows the cursor on
   the next frame, as it does for every jump. It is then forgotten, so a
   later refresh, the operator's own moves, a jump into the view (the
   jump's target wins; the view's folds are still the saved ones), or a
   regroup before the rows arrive (which drops it) are never undone by
   it. A gone row or group is neither an error nor a warning.
4. **Writing prunes.** A view's `expanded` is written as the expanded
   rows its tree lists at the time, so a group that went away leaves the
   file on the next write and the set stays bounded by what the view
   shows. The active view is written from its tree; a view the operator
   left, as it was when they left it (its rows are not at hand later);
   a view whose saved tree is still waiting, unchanged.
5. **When it is written** is the existing policy: on a view switch and
   on each settings change, and at the end of the run. Moving the cursor
   and folding never write by themselves. Two writes are added to that
   policy, because the tree changes with every key and lands only at
   the end: before an effect hands the terminal to another program (an
   attach, a viewer, a launch), since a closed terminal ends the TUI
   too, and when the event loop ends in an error. A crash still loses
   what changed since the last write.
6. **Reading is lenient per entry.** An entry of `expanded` or `path`
   that is not a string is skipped; an `expanded` that is not a list, or
   a `selected` that is not an object with a string `row`, is dropped
   for that view; a `path` that is not a list is an empty path. A
   `row_keys` that is missing or not `1` drops every view's `expanded`
   and `selected` and keeps the rest. A file from an earlier build has
   neither field, and each view opens as before. Unknown fields round-trip
   at the top level and, newly, in each view.
7. **`--no-resume-view` and `--snapshot` reopen no rows.**
   `RunConfig::resume_view` carries the flag into the TUI, which drops
   each saved tree unapplied when its view first lists rows; a view the
   run never showed is written back unchanged. `--snapshot` reads and
   writes nothing, as before.

## Consequences

- An operator who quits and restarts sees each view as they left it,
  and a stale selection lands near where it was instead of at the top.
- The restore uses two additions to the framework's tree: `select_at`,
  which opens a row's ancestors and puts the cursor on that copy of it,
  and the `ancestors` walk `reveal` already used, made public. Both are
  generic and name nothing of Conspectus.
- A saved selection beats the launch directory's group, which only
  picks the first load's row when the file has none for the view.
- Under `--no-resume-view` the run still writes its own views' state,
  as it already did for the view and settings; whether that flag should
  stop writes is open work (backlog CSP-835).
- The file's keys lean on `NodeId`'s text form. A change to it that
  keeps its parse in step keeps the keys; one that changes what an id's
  text says should bump `row_keys`.

## Alternatives Considered

- **Persist only group rows' expansion.** Simpler to explain, but a pin
  slot or a session the operator opened would close on every restart for
  no gain: their ids are as stable as a group's.
- **Store row ids as JSON objects (the `NodeId` serde form).** Fully
  round-trippable, but verbose, tied to the graph's machine-readable
  output, and still needing a tag per row kind; keys compared as text
  need no decoder.
- **Decode keys back into row ids.** Needs a parser per kind and a table
  of the synthetic group names; comparing the keys of the listed rows
  needs neither and ignores an unknown kind for free.
- **Apply the saved tree after every message, or on every refresh until
  it matches.** Either would override a jump or the operator's moves; once
  per view at its first rows cannot.
- **Prune each view's in-memory folds on every rebuild.** A filter that
  hides a group for a moment would close it for the rest of the run;
  pruning only what is written leaves the run alone.
- **Build every view's tree at each write to prune them all.** A view
  switch would pay for five tree builds; the tree a view had when it was
  left is close enough, and the next visit prunes it.
- **Write on every move.** The file would churn with every key; the
  existing write points plus the hand-off and error writes cover the
  ways a run ends short of a crash.

## Open Questions Answered

- *Does a saved selection beat the launch directory's group?* Yes; the
  file has none for a view the operator never used, and then the launch
  directory still picks the first row.
- *Is a malformed key fatal to the view's tree?* No: each entry stands
  alone, and a key that names nothing is ignored like a gone row.
