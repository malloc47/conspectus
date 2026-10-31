# 0064: The tab bar and the priority line fit by the one rule

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/tab_bar.md](../controls/tab_bar.md) |

## Decision

- **`TabBar`** draws the names of the views a pane can show, one active (in
  `accent` and bold) and the rest `inert`, joined by the spaced separator
  mark, on the strict `text::elide` with the mark at the end: every tab
  while they fit; otherwise the inactive tabs give way from the end of the
  bar and the active one never; a shortened bar ends in ` …` with a blank
  cell after it, so the mark stays off the pane's corner; and when even the
  active tab and the mark do not fit, the active tab shows alone, cut with
  `…` only when it alone is too wide.
- **`PriorityLine`** draws segments of styled spans (a status chip among
  them), each with a rank or none and an optional gap of its own, joined by
  a separator (` · ` in `muted` by default), on `text::elide_filled` with
  ` …` at the end. A segment with no rank never gives way and is cut last.
  Parts of a line that must never move are spans the host draws around the
  line, giving it the cells they leave.

## Rationale

The host's tab strip, header, and status line each had a fit of its own, or
none. Built on one rule (record
[0018](0018-pieces-of-a-line-give-way-by-one-rule.md)), they give way alike:
the least important pieces first, one mark where something went.

## Consequences

- The status bar's keys and segments are a priority line (record
  [0066](0066-the-status-bar.md)).

## Alternatives considered

- **The strict rule for every line.** A narrow status line would show fewer
  hints than fit.
- **Filling back by the greedy order alone.** A group of one rank could show
  a later piece after an earlier one went.
- **One separator for every line.** A line that fits would change for no
  reason.
- **A host's fixed chips as unranked segments.** Their spacing belongs to the
  chips, and which came first would change the gap before it.

## Origin

- host ADR 0190, "The tab strip, the header, and the status bar give way by
  one rule, and an empty slot says so one way": decisions 3 and 4, and its
  alternatives.
