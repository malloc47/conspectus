# 0021: A cursorless scroll region counts its offset from an anchor

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/scroll.md](../controls/scroll.md) |

## Decision

- `scroll::ScrollRegion` is a `Copy` window with no cursor, kept by its
  host, whose offset counts from an `Anchor`: `Top`; `Bottom`, which keeps
  its distance from the end as rows arrive; or `Mark`, a row the content
  marks (a match's first row less some context).
- A step toward the anchor's own end stops there at once; a step past an end
  the anchor does not stop at is taken back by the next placement. Setting
  another anchor, or placing from another, starts the region at that anchor.
  `place` clamps it over the rows a frame laid out.
- It draws either lines it wraps to the area, a leading banner scrolling
  with them, or rows its host cut at the placed window under a banner that
  stays put. Both draw the shared scrollbar and return the region drawn.
- It is not the pager's embedded mode, which owns its content, counts from
  the top with a sticky end, and paints its own find.

## Rationale

A preview whose source changes with the selection is drawn from a shared
borrow on every frame and wants to follow the newest output or a match.
Embedding a pager would keep a second copy of every source's rows and change
the frames; a region over the host's rows does neither.

## Consequences

- The framework has two scrolled readers, one per shape: the pager for
  content it owns, the region for content its host draws.

## Alternatives considered

- **The pager's embedded mode for previews.** One reader, but a copy of the
  rows and different counting.
- **Two counters inside the region.** It would restore an older offset after
  a match comes and goes, but carries two meanings of one number into the
  framework; a region holds one place.
- **An unclamped match offset.** Identical frames, but it keeps dead keys
  after scrolling past an end, which house rule 12 rules out.

## Origin

- host ADR 0181, "Rich text is a framework feature, and the preview scrolls
  in a region counted from an anchor": decisions 5 and 6.
