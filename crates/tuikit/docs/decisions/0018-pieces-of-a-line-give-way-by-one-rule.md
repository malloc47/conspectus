# 0018: The pieces of a line give way by one rule

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/text.md](../controls/text.md) |

## Decision

- `text::elide` fits a line of ranked pieces to its room. Pieces of the
  lowest rank give way first, the later of two at one rank first; each run
  of pieces that gave way leaves one mark, in its gap or once at the end; the
  last piece that cannot give way is cut last, with `…`.
- A piece may carry its own gap, used in place of the line's separator when
  something precedes it, so a line whose pieces are not all joined alike is
  measured exactly.
- `text::elide_filled` applies `elide` and then brings pieces back, the
  highest rank first, while the line still fits; pieces of one rank come
  back in order, so the kept pieces of a rank are always its first ones. It
  is for lines whose pieces stand alone (a status line). A line whose pieces
  read in sequence (a breadcrumb, a tab strip) keeps the strict rule:
  filling would show a later piece past a gap the mark does not place.

## Rationale

A breadcrumb, a tab strip, a header, and a status line each had a fit of
its own, or none. One rule makes them give way alike and keeps a title from
needing a fixed floor.

## Consequences

- `Breadcrumb`, `TabBar`, and `PriorityLine` are built on it (records
  [0062](0062-the-explorer.md) and
  [0064](0064-tab-bar-and-priority-line.md)).

## Alternatives considered

- **The strict rule for status lines too.** One behavior everywhere, but one
  hint fewer at some widths.
- **Filling back by the greedy order alone**, with no order within a rank.
  Simpler, but the kept pieces of a rank could be any of them.
- **One separator for every piece.** Lines with mixed gaps would be
  mismeasured.

## Origin

- host ADR 0188, "The explorer is a framework control over a host source,
  and the right pane's keys are one table": decision 6 (one fit rule for the
  pieces of a line).
- host ADR 0190, "The tab strip, the header, and the status bar give way by
  one rule, and an empty slot says so one way": decisions 1 and 2 (a gap per
  piece, and the fill pass).
