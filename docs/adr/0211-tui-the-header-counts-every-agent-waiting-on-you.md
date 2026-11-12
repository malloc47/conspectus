# ADR 0211: The Header Counts Every Agent Waiting On You

## Status

Accepted. Carries out the operator's decision 3 in section 7.1 of the
live-state plan (`docs/plans/live-state/README.md`). Amends
[ADR 0078](0078-tui-surface-division-of-labor.md) decision 1 (the top
header carries a second triage chip) and
[ADR 0190](0190-tui-the-chrome-gives-way-by-one-rule-and-an-empty-slot-says-so-one-way.md)
decision 5 (the header's ranks). Draws the group headers' chip of
[ADR 0210](0210-tui-group-headers-count-the-agents-waiting-beneath-them.md);
adds no theme role (ADR 0153), no glyph (ADR 0154), and no key.

## Context

A group header now says where an agent waits, but only for the view on
screen and only once the operator looks down the tree. The decision: a
header chip `! N`, the waiting count over the whole graph, hidden at
zero, ranked after freshness and counts and before `▲`. What it left
open: what "the whole graph" counts, where the chip sits relative to the
opt-in harness chips, how it reads, and how it stays current between
snapshots.

The header is a `PriorityLine` (ADR 0190): the session count never gives
way; freshness, the mux count, the `▲ N` triage chip, the stopped count,
and the harness chips give way by rank. Live values are read once per
moment into the rows by one pass (`rows::live::annotate`), at each build
and at each overlay refill; the header must not disagree with the rows
it sits above.

## Decision

1. **Every agent anywhere.** The chip counts the live-state entries whose
   mark is waiting: every session with live state and every agent in a
   pane no session matches, whatever the active view lists and whatever
   the filter hides. Exited, stale, and unknown agents, and sessions with
   no evidence, count nothing.

2. **Read with the rows.** The pass that fills the rows' live values
   counts them too, from the same tables at the same moment, and leaves
   the number on the row tree (`RowTree::waiting_anywhere`). The header
   reads it there, so a store change or time moving a state changes the
   chip in place with the rows' marks, without a rebuild.

3. **The group headers' chip.** `! N` is the same `StatusChip` a group
   header draws (the waiting mark and the number in the waiting mark's
   tone, ADR 0210), set apart from what precedes it by the chip-section
   gap `  ·  `. It is not bold, as the marks it counts are not.

4. **After the counts and the harness chips, before `▲ N`.** The two
   triage chips sit together at the end of the line, waiting first.

5. **Its rank.** From the last to go: freshness (6), the mux count (5),
   the waiting count (4), `▲ N` (3), the stopped count (2), the harness
   chips (1). The session count still has no rank. The ranks are
   renumbered; the order of the others is unchanged.

6. **No key.** Nothing acts on the chip yet; the State filter will.

## Consequences

- The showcase header reads `updated 0s ago · 11 sessions · 4 mux · 4
  stopped  ·  ! 2  ·  ▲ 4`; at 50 columns `updated 0s ago · 11 sessions ·
  4 mux  ·  ! 2 …`, where it used to keep `▲ 4`.
- A header with agents waiting is up to eight cells longer, so on a
  narrow screen the stopped count and `▲ N` leave sooner.
- The count can exceed what the view lists, by design: a filter that
  hides a waiting session leaves it counted.
- docs/tui-columns.md gains a header section whose frames the reference
  test renders, through a `pane=` attribute on its frame markers.

## Alternatives Considered

- **Count the filtered view.** It would match `N/M sessions`, but the
  question the chip answers ("does anything need me?") is not about the
  filter.
- **Count waiting, ready, and working (`! 2 › 3 » 4`).** Exact, but wide
  everywhere; the decision chose the waiting count alone.
- **Compute it in the header from the live tables each frame.** It could
  run a moment ahead of the rows' marks, which the overlay refills once
  a second.
- **Bold it, as `▲` is.** The marks it counts are not bold, and the
  group headers' count is the same chip.
- **Put it before the harness chips.** The two triage chips would be
  split by the opt-in ones.

## Open Questions Answered

- *Does the Search view count only what it found?* No: the chip is the
  same in every view.
