# 0056: Readers are one pager over content the host lays out

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/pager.md](../controls/pager.md) |

## Decision

- **The content is a trait.** A host's content implements `PagerContent`:
  `layout(width, theme)` lays its rows out and counts them (kept while the
  width and its state are the same), `rows(range, theme)` returns a range;
  a header, chips, the units a find matches and their rows, a painter for
  matches, words of its own for pager keys on the help page, an empty note,
  keys of its own (resolved after the pager's) answered by `apply`
  (`Relayout`, `Close`, or a request for the host), and a copy hook
  complete it, most with defaults.
- **Three modes.** `Screen` fills the screen with a header, rules, and a
  footer led by the `[ n/m ]` position chip; `Modal(size)` draws in a named
  modal frame with the shared scrollbar and its footer kept down; `Embedded`
  is the body and the scrollbar in an area the host lays out.
- **One window.** The pager keeps the offset asked for and the window the
  last draw showed: page keys move by its rows and half pages by half;
  every draw brings the offset inside the content, so a key past an end is
  undone by the next. `G` makes the window keep to the end as the content
  grows, until a key moves it.
- **One find** (house rule 18). `/` opens a prompt on the line editor;
  `Enter` finds the units that hold the query and always scrolls the match
  to one row below the window's top; `n` and `N` step, wrapping with a
  note. The current match takes the selection look over the text; the
  others the match look. `land` opens at a match found elsewhere.
- **Levels and leaving.** `Esc` steps back: the help page, the prompt, the
  active find, then the pager. `q` and `Ctrl-C` leave from any level, and
  `q` types in the prompt.
- **Keys, help, and footer.** A scroll fragment, a pager table, a reader's
  table that adds find and help, a help level's table, and a find prompt's.
  A host gives a reader opened by a key its own table over the pager's
  actions. `?` opens a help page from the pager's table and the content's,
  read in a pager of its own. The full-screen footer drops the scroll keys,
  then the other keys, then the chips, then the content's keys, keeping the
  keys that leave.

## Rationale

The host had seven scrolling readers of one shape, each with its own
scrolling, find, help, and footer. One pager over a content trait gives
every reader the same keys and leaves only the content to its host.

## Consequences

- Help pages and long values are pager contents (record
  [0035](0035-generated-help-pages.md)); a log list's detail is an embedded
  pager (record [0057](0057-log-lists.md)).

## Alternatives considered

- **Scrolling long lines sideways.** A find match past the edge would need
  the view to move; wrapping is the default and a column offset is a seam.
- **Scrolling only when a match is off screen.** Steadier for near matches,
  but house rule 18 says a confirmed find scrolls to its match.
- **Marking the current match in a gutter.** It would move every row two
  cells in a reader with no cursor.
- **Counting lines, not rows, in the position chip.** The window scrolls by
  rows.
- **A separate action enum for the pager.** Two enums for one family of
  readers.
- **Resolving the content's keys first.** A content could take a pager key
  that the footer and the conventions cannot see.

## Origin

- host ADR 0177, "Readers are one pager over content the host lays out":
  decisions 1 to 8.
- host ADR 0178, "Help pages and long values are pager contents, and a
  framed footer scrolls what does not fit": decision 2 (a reader's help
  level is a pager of its own).
