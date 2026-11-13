# ADR 0213: The Detail Pane Says What A Node's Agents Are Doing

## Status

Accepted. Carries out the detail-pane part of the operator's decisions 7,
8, and 12 in section 7.1 of the live-state plan
(`docs/plans/live-state/README.md`), and the recommendation of section 8
of `tui-options.md` there: the explorer's Node gets `State`, `Since`,
`Source`, and `Evidence` fields. Extends the per-kind field reference of
`docs/tui-detail-mockup.md` and the field previews of
[ADR 0112](0112-node-specific-previews.md) decision 2 (the new fields
preview by their value types). Draws the marks of
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md);
adds no theme role (ADR 0153), no glyph (ADR 0154), and no key.

## Context

Session and mux rows show what their agents are doing as a one-cell
mark, Pinned rows as a word, and group headers and the header line count
the agents that wait. The detail pane said nothing about it, so the
facts the mark compresses (the detail, since when, where the report
came from, and how far it can be trusted) were only in `conspectus node
show` and the graph JSON.

The decisions name the fields and say quiet states dim and stale ones
read `?`; decision 8 says the detail pane lists a session's subagents.
They leave open the labels, which fields a node shows by default, how
the fields look, what a mux shows, what a node with no evidence shows,
where the values come from, and how the fields stay current between
snapshots. The store keeps a count of subagents only: no subagent ids or
names are kept (ADR 0086), so there is nothing to list one by one.

## Decision

1. **Five session fields after `status`, in the Node summary.** A
   session with live state lists `state`, `since`, `source`, and
   `evidence`, then `subagents` while its agent runs any, after its
   `status` and before the Related rule, in the summary the pane shows
   by default and in the full view. The labels are lowercase like the
   zone's others.

2. **`state` leads with the row's mark.** The value is the state and its
   detail in the vocabulary's words joined by ` · ` (`waiting ·
   permission`, `ready · finished`, `unknown · stale`), led by the mark
   the session's row draws, in that row's style: the same shape and
   color, dimmed when quiet, `?` when the evidence is too old to trust,
   `·` once the session exited. It previews the glossary entries of both
   words.

3. **`since` is when the shown state began,** on the TUI's age ladder
   (`5m ago`), previewing the time: the recorded start, or the latest
   evidence when the read rules put another state in the recorded one's
   place (a gone process, evidence too old), as `node show` says.

4. **`source` says what reported it** (`hook`), and **`evidence` how far
   it can be trusted**: `fresh`, `quiet`, or `stale`, the observation's
   confidence. Both preview their glossary entries.

5. **Quiet and stale recede.** When the evidence is quiet or stale, the
   words of `state` and the value of `evidence` take the theme's muted
   look. They are real values, not placeholders, so they still copy.

6. **`subagents` is a count.** `N active`, shown by the rule the row's
   `+N` chip follows (one or more, and a state that is neither `exited`
   nor `unknown`), uncapped. It is the detail pane's listing of
   decision 8: the store keeps how many, not which.

7. **A mux shows its agents.** `agents`: its counts in the rollup's
   order, most urgent first (`2 waiting · 1 error · 1 starting`), led by
   the mux row's mark (never dimmed, as on the row), previewing the
   glossary entries of the states it counts. `pane_agents`: the agents
   in its panes that no session matches yet, each as its pane and state,
   read as a list field reads: the first and how many more (`%4 waiting ·
   input  (+1)`), previewing one per line. They count in `agents` too,
   but have no row or node of their own.

8. **No evidence, no fields.** A session or mux with no live state lists
   none of them, the way an optional field that has no value is left
   out. The row's `·` already says there is no evidence; a column of
   dashes would add four rows to every session whose harness reports
   nothing.

9. **The rows' values, at the rows' moment.** The pass that fills the
   rows' live values keeps the whole set on the row tree
   (`RowTree::live`), and the explorer reads it, so a field never says
   something its row does not. When the overlay refills the rows (a
   store change, a process found gone, time moving a state), the
   explorer reloads its node in place, keeping its cursor, fold, and
   frames.

10. **Pinned and History keep their words.** A Pinned session reads
    `waiting`, `working`, `ready`, or `error` in place of its link word
    (ADR 0209 decision 4). The pin's own detail is unchanged; the
    session, with its mux links and its live fields, is one step away
    through the pin's `targets` row. Rows drawn as ghosts (stopped,
    removed, missing) never take a live word, and History lists no
    sessions.

## Consequences

- A session with live state shows up to ten summary fields, so its
  Preview has up to five fewer lines; on the showcase's pinned session
  the transcript's first question no longer fits at a height of 34, and
  a key script that walks onto the first Related row needs five more
  `j` (the screenshot recipe's does).
- `evidence` names two things in the pane: a live state's freshness in
  the Node fields and a link's producers in a Related row's Preview.
- The fields of a neighbor that has no preview of its own, which the
  Preview lists, carry its live fields too.
- `since` is as current as the node's load or the overlay's last
  refill, like the pane's other ages.

## Alternatives Considered

- **`source` and `evidence` in the full view only.** Two rows fewer, but
  the decision names them as Node fields, and `evidence` is how the pane
  says a state is quiet or stale in words.
- **One combined field** (`! waiting · permission · 5m · hook · fresh`).
  Dense, and each part previews differently.
- **Dashes for a node without evidence.** Four dim rows on every session
  whose harness reports nothing, saying what the row's `·` says.
- **Reading the live tables in the explorer.** The pane could read a
  moment apart from the rows, and the mark and the field would disagree
  until the next refill.
- **Capitalized labels** (`State`), as the decision wrote them. The
  zone's other labels are lowercase.
- **Listing each subagent.** The store keeps no subagent ids or names,
  and keeping them would widen what it stores.

## Open Questions Answered

- *Does a pin's detail show its session's live state?* No: it shows the
  pin, as before, and its `targets` row leads to the session.
- *Does `pane_agents` show when every agent of a mux has a session?* No:
  its agents are one step away under Related.
