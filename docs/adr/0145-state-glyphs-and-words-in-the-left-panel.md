# ADR 0145: State Glyphs And Words In The Left Panel

## Status

Accepted. Amends ADR 0072 (the attached/unattached glyph pair changes
from `◉`/`◯` to a link pair), ADR 0073 (state glyphs get their own
vocabulary beside the node-kind glyphs), and ADR 0144 (status words in
the Pinned and History views follow one vocabulary).

## Context

A column-by-column audit of the left panel found one state drawn
several ways. `◉` meant "pinned" in one cell and "attached" in the cell
beside it, in the Mux and Sessions views. A stopped mux was `gone` in
the Mux view, `stopped` in the Pinned view, and `gone` again in History,
while a running mux in the Mux view was a `◯`/`◉` glyph and in the
Pinned view the word `live`. Age meant last activity in one place, time
since a mux disappeared in another, and nothing at all for some kinds,
and only some views colored it by recency.

## Decision

### Glyphs for one kind of thing, words for a mix

A view of a single primary entity (Sessions, Mux, PRs) shows state only
as glyphs, in a state cell after the age. A view that lists several
kinds (Pinned, History) shows status as a word, because one glyph can't
mean the same thing for a mux, a worktree, and a PR.

### Every state has a glyph for each of its values

A state shown as a glyph is shown for all its values, so no value hides
behind an empty cell, and the values differ in shape and not only color:

| State | Glyphs |
|---|---|
| Link to a mux (session) or client attached (mux) | `⚯` linked, `⚬` not linked |
| Mux that stopped, or couldn't be checked | `⊘` stopped, `?` unknown |
| Pull request | `○` open, `◐` draft, `✓` merged, `✗` closed, `?` unknown |
| Pin marker | `◉` pin on a running entity, `◎` pin on one that isn't running |

The pin marker uses the same pair in every view, including live pins in
the Pinned view and pinned records in History. No state glyph reuses a
node-kind glyph, and no glyph carries two meanings.

### One vocabulary for words

A mux that isn't running is `stopped`. A worktree or repo that vanished
is `removed`. A session whose record is gone is `missing`. A pin with no
target is `planned`. `gone` is not an interface word; it stays an
internal name for the fact.

### One age per entity

The age cell is the most expected time axis for the view and the same
for the same entity everywhere: last activity for a session and for a
mux (the mux's own activity, not blended with its agents'), last update
for a PR, and for a stopped or removed entity the time since it was last
seen. An unknown age is `—`. The recency color applies wherever an age
appears and is computed from the clock the age text uses.

## Consequences

- The attached/unattached glyphs change for everyone who learned `◉` and
  `◯`; the help legend and operator guide explain the new ones.
- A mux's age no longer rises when one of its agents works.
- A reference document of columns, glyphs, and states is kept beside the
  design so these rules stay checkable.

## Alternatives Considered

- **Words everywhere.** Readable, but a column of words costs width and
  can't be scanned the way a glyph column can.
- **Glyphs everywhere.** The mixed views would need a glyph per kind and
  state, past what operators can remember.
- **Keep `◉`/`◯` for attachment and move the pin marker.** The pin
  marker is the one that appears in every view, so it keeps its glyph.
