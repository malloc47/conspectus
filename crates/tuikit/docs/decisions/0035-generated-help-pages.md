# 0035: Help pages are generated from key tables and read in a pager

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/help.md](../controls/help.md) |

## Decision

- **Described by the action enum.** An action enum implements
  `HelpAction`: `LISTED`, its actions in the page's order, and `help`, an
  exhaustive `match` giving each a section and one sentence, so a new action
  fails to compile until it is described.
- **Generated from the table.** `sections_for(table)` makes one row per
  listed action the table binds, with every key bound to it (included tables
  and aliases too: a help page lists every key where a footer shows one),
  plain keys before chords, joined with ` / `. An action no key binds is
  left out; a row is dimmed when none of its keys is available. The
  listing, not the table, orders the page, since a table's order is the
  order keys resolve in.
- **Nothing undescribed.** `undescribed(table)` returns the keys a page
  would not describe; a host asserts it is empty.
- **A program's page from its keymap.** `keymap_sections` lists one row per
  binding under the section it names. What a binding cannot say is a
  `HelpAnnotation` anchored by its id: prose after a row, a row of its own
  after a binding, or a section of its own (a legend of marks).
  `unanchored` finds an annotation whose binding is gone.
- **One page.** `HelpPage` has bold, wrapped headings; key rows indented two
  cells with the keys in `accent` in a 16-cell column and descriptions
  wrapped beside them; legend rows whose marks and names line up. A help
  page is a pager's content, read at the `WIDE` size, and its footer is
  generated from the table that scrolls and closes it.

## Rationale

Hand-written help drifted from the keys, and the host had three help pages
with three renderers. Generating the page from what resolves the keys makes
it complete by construction, and an exhaustive description makes a new key
impossible to leave unexplained.

## Consequences

- Readers open help on `?` as a level of their own (record
  [0056](0056-one-pager.md)).

## Alternatives considered

- **Closures instead of a trait.** As capable, but every call site would
  pass the listing and descriptions again.
- **Rows in the table's order.** Pages would open with `Ctrl-C` and change
  whenever a row moves for resolution's sake.
- **Only the shown keys.** Shorter rows, but a page that leaves out `j`
  beside `↓` hides keys.
- **A description column sized per page.** Tighter, but pages would stop
  lining up.
- **Cutting headings with the ellipsis mark.** Drops the end of a heading.

## Origin

- host ADR 0162, "Help pages are one page, generated from the key tables":
  decisions 1 to 4.
- host ADR 0178, "Help pages and long values are pager contents, and a
  framed footer scrolls what does not fit": decisions 1 and 2.
- host ADR 0192, "The main screen's keys are one scoped keymap, and the help
  page is generated from it": decision 9.
