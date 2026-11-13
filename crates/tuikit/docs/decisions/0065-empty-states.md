# 0065: An empty slot says so one way

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/empty_state.md](../controls/empty_state.md) |

## Decision

- `EmptyState` is what a slot says when it has nothing to show: a message
  and an optional hint on rows of their own, wrapped at word edges, in the
  `muted` look, left-aligned from the slot's corner by default or centered
  across and down it. It is drawn in an area or returned as lines for a
  region that lays out its own. The tree's placeholder is one.
- A slot whose content failed to load with a reason is showing content, not
  an empty state, and keeps the body's look.
- A minimum-size screen (one centered empty state over the whole screen
  below a size the host names) is a seam, not built.

## Rationale

Empty messages in the host had several looks and alignments. One control
makes "nothing here" read the same in every pane, and keeps a reason for a
failure readable as content.

## Consequences

- Every empty slot a host draws takes the same look, with its own words.

## Alternatives considered

- **Centered by default.** Common for an empty screen, but every pane's
  empty state would move; centering stays an option for a slot with nothing
  around it.
- **Muting failures too.** A reason something failed is content the user
  needs to read.
- **A minimum-size screen now.** Left as the seam.

## Origin

- host ADR 0190, "The tab strip, the header, and the status bar give way by
  one rule, and an empty slot says so one way": decisions 7 and 8.
