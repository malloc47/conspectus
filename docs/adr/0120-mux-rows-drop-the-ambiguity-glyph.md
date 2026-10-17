# ADR 0120: Mux Rows Drop The Ambiguity Glyph

## Status

Accepted. Extends ADR 0072.

## Context

ADR 0072 made the session-row mux chip attachable-binary (`◉` / `◯`)
and moved the ambiguity signal to a single `⚠` on group rows and the
header. It didn't cover mux rows. A mux row in the mux and union views
still added a yellow `◐` after its attach-state glyph whenever any
visible agent session under that mux had more than one `LinkedToMux`
candidate.

Using it showed two problems:

1. **It broke the columns.** The glyph and its leading space appeared
   only on some rows, so those rows were two cells wider and the pin
   marker and preview moved right. The mux list stopped lining up.
2. **Nobody acted on it.** The operator had never made a decision based
   on whether it was there and couldn't remember what it meant. The
   `?` help modal only explained node-kind icons, so the mux-state
   glyphs had no in-app legend.

A mux row's ambiguity is also secondhand. It says a session *under*
this mux has other candidates, and the place to settle that is the
session's group detail (ADR 0071), not the mux row.

## Decision

1. **Mux rows render only the attach-state glyph.** `◉` (a client is
   attached), `◯` (no client), `?` (unknown), `◌` (pin placeholder), or
   the handoff spinner. Nothing follows it before the pin marker. The
   column is exactly one glyph wide in every state.
2. **`◐` leaves the TUI.** ADR 0072 had already dropped it from session
   rows, group rows, and the header, so mux rows were its last use.
   The `mux_ambiguous` theme key and `MuxSessionRow::ambiguous_count`
   go with it. A config that still sets `[tui.theme].mux_ambiguous`
   gets the standard unknown-key diagnostic.
3. **The help modal gets a "Mux state" legend** below the node-kind
   legend. It lists `◉ ◯ ◌ ? ⚠`, and where a glyph answers a different
   question on session rows than on mux rows, it says what each means.

## Consequences

- The attach-state glyph, pin marker, and preview line up on every
  mux row.
- The mux list no longer hints that a session under a mux is
  ambiguous. The same information is still in the sessions view's `⚠`
  group glyph and header count, the filter modal's ambiguous bucket,
  and the detail pane's candidate listing.
- `MuxIndicator::Ambiguous` is unchanged. Filters, header counts,
  attach defaults, and the detail pane still read it.
- The operator can look up every glyph in the attach-state column
  with `?` instead of in the docs.

## Alternatives Considered

### A. Reserve a fixed-width slot for `◐`

Always add two cells after the attach glyph and leave them blank when
nothing is ambiguous. This fixes the alignment but keeps a signal that
nobody acts on, and every mux row pays two cells for it. Rejected.

### B. Fold ambiguity into the attach-state glyph

Draw `◐` in place of `◉` / `◯` on ambiguous mux rows. Rejected: the
attach glyph on a mux row describes the mux itself (is a client
attached?), and replacing it would hide that state to show a property
of a session under the mux.

### C. Use the group-row `⚠` on mux rows

Rejected for the same reason as keeping `◐`: it puts a session-level
concern on a mux-level row and adds width the operator didn't use.
