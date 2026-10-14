# ADR 0119: The Compass-Rose Logo

## Status

Accepted.

## Context

Conspectus is about to go public and be presented in a talk. It had no
mark, so the README, the slides, and any future site had nothing to show
but the name.

A bake-off drew six candidates from what Conspectus does:

- a constellation shaped like a C;
- an evidence fork;
- a survey reticle;
- linked tmux panes;
- a prompt glyph trailing a graph;
- a panorama.

All six used one palette. Ideas that wouldn't scale or said nothing about
the tool were set aside, as was an eye with a graph iris, which reads as
surveillance against the project's never-touch-your-agents stance. The
operator chose the survey reticle, but its crosshair read as a gun sight.
Three compass treatments were drawn, and the compass rose read clearly as
a compass at every size.

## Decision

1. **The mark is a compass rose around a small evidence graph.** A ring
   carries four rose points; north is the long teal point. Inside the ring:
   - a teal focal node;
   - a solid navy link to a resolved neighbour, to the north-east;
   - a dashed amber link to a hollow node to the south-west, the candidate
     the resolver kept but didn't choose.

   The two links bend slightly in an S through the centre, so they read as
   a graph rather than a straight needle.
2. **The colors carry meaning.**
   - Navy `#14213D` is what is resolved, and the ring.
   - Teal `#1FA2B8` is north and the node in focus.
   - Amber `#E3A33B` is a kept candidate, echoing the TUI's ambiguity
     markers.
   - The dark variant swaps navy for `#E6ECF4`.
3. **The assets live in `docs/images/logo/`**: `conspectus-mark.svg` and
   `conspectus-mark-dark.svg` on a 64-unit grid, and 512 px PNG renders of
   each for slides. The README header shows the mark through a `<picture>`
   that follows the reader's color scheme.
4. **No wordmark file.** Text set in an SVG falls back to whatever font
   the viewer has. Lockups set the name in type beside the mark; the
   bake-off used IBM Plex Mono.

## Consequences

- The mark tells the project's story in one glance: a survey, a graph,
  and evidence kept alongside the choice.
- Below 32 px the bend and the dashes merge. The ring, the rose points,
  and the teal north still carry it, so no separate favicon cut is needed
  yet.
- A change to the mark's geometry or palette updates both SVGs and
  re-renders the PNGs. The bake-off's generator scripts were scratch work
  and are not kept.

## Alternatives Considered

- **Evidence fork.** The clearest at 16 px and the most literal statement
  of "resolve, but keep the losers". Not chosen: it lacks the survey half
  of the name.
- **Survey reticle as drawn.** Not chosen: the crosshair ticks crossing
  the ring read as a gun sight.
- **Reticle with a north pointer, or with bezel ticks.** Not chosen: the
  first reads as a map pin, the second as a clock or gauge.
- **Straight links.** Not chosen: they read as a single needle bar rather
  than a graph.
