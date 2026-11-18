# ADR 0249: A Mux That Can Never Report State Shows No Live Mark

## Status

Accepted. Amends
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
(the live cell is always filled) for muxes that no agent can report on.
No new glyph, role, or write path.

## Context

ADR 0209 fills the live cell of every session and mux row so no state hides
behind an empty cell, and draws `·` (no state) when an entity has no
evidence. For a session that is the right word: its harness may report once
its hooks are on. A mux that holds no agent, such as a shell, an editor, or
the daemon's own `conspectus-server`, has nothing that could ever report, and
the operator (2026-11-17) found the `·` on those rows noise: it says an agent
is there with nothing to report, which is not so.

## Decision

1. **Such a mux draws a blank live cell.** A mux row whose live cell would
   be `·` draws nothing there when nothing about it can report state: no
   session is linked to it, no agent (running or exited) is counted on it,
   and its pane runs no harness (the observed process, the pane's command,
   or its pin's launch command names none). A stopped mux in Mux, Pinned, or
   History follows the same rule from its last run: no agent chip, no
   agent counted. The column keeps its width, so the row lines up.
2. **Anything else keeps the mark.** A mux with an agent linked, an agent
   in a pane no session matches (even an exited one), or a harness in its
   pane keeps `·` when it has no state yet, because it may report once its
   hooks are on. Session rows are unchanged: every session belongs to a
   harness.
3. **The rule is the row's, not the glyph's.** The judgment is a method of
   the mux row (`never_reports_state`) and the stopped row's chip, and the
   live cell reads it where it draws; the model and the live state are
   untouched, so the filter, the header count, and the detail pane read as
   before.

## Consequences

- A column of `·` no longer decorates shells and servers; the muxes that
  show it are the ones where an agent is expected.
- The "always filled" rule of the live cell now reads "on every row that
  can report", in `docs/tui-columns.md` and the help legend. Design rule 2
  (no state hides behind the absence of a glyph) stands: these rows have no
  state.
- A mux whose pane starts a harness later gains the mark when its process
  is observed or its hooks report.

## Alternatives Considered

- **Hide the mark for every mux with no live state.** Hides what tells the
  operator an agent is there but not reporting, the case the hooks'
  installation exists to fix.
- **A new glyph for "no agent".** Another mark in a one-cell column for
  what is better shown as nothing; the chip already says ` no agent `.
- **Decide it in the model** (a field on the mux node). The model has the
  evidence already; the judgment is how a row draws it.
