# 0063: A stacked master-detail layout sizes its list to its rows

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/master_detail.md](../controls/master_detail.md) |

## Decision

- `StackedMasterDetail` lays out a list zone over a one-row divider over a
  detail area, in one pane. The list is as tall as its rows wrap to, at
  least three, within the room a minimum detail area leaves (six rows when
  the pane allows; the detail's minimum gives way first on a short pane).
- The list wraps at the full width while it fits and one column narrower
  when it scrolls, and is drawn in a window the host keeps, which follows
  every row of the cursor's line, with the shared scrollbar. The layout
  width is one column narrower than the pane, so a rule cannot wrap when
  the bar appears.
- It returns the window and the three areas; the detail is the host's to
  draw.
- `frame::SectionRule` is the rule that labels a zone: a muted rule, the
  label as a chip in the accent under the badge look, an optional summary
  before or after it, and a two-cell closing rule, so the labels down a
  pane line up.

## Rationale

The host drew this layout inline in its detail pane, with its own sizing
rule; as a control it gives any host a list over a detail with the same
scrolling and the same give-way.

## Consequences

- The explorer draws its zones in it (record [0062](0062-the-explorer.md)).

## Alternatives considered

- **Leaving the layout in the host.** The explorer's design moved it with
  the explorer; no other alternative was recorded.

## Origin

- host ADR 0188, "The explorer is a framework control over a host source,
  and the right pane's keys are one table": decision 7.
