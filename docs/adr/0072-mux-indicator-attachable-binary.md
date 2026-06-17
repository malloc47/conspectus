# ADR 0072: Mux Indicator Becomes Attachable-Binary; Group Rows Own The Ambiguity Glyph

## Status

Accepted

## Context

ADR 0071 moved the per-session `AgentSessionMuxCandidate` rows out of
the Sessions view and surfaced the ambiguous-mux catalog on the
shared-ancestor group's detail pane. That change kept the per-session
`MuxIndicator::Ambiguous` chip (`◐`) on the session row "so the eye
still sees the flag."

In practice the per-session chip is still noisy under group-by views.
The showcase scenario reproduces the failure mode: every member
session of an ambiguous group renders the same `◐` glyph, so a
workspace with four ambiguous sessions reads as four warning chips
that all point at the same underlying ambiguity. The repetition adds
no information beyond the first chip — the ambiguity is a property of
the *group*, not of each member.

Three further observations from the rendered showcase:

1. The per-session chip's most actionable meaning is "can I attach
   from here without making a choice?" — `◉` answers yes, `◐` and `◯`
   both answer no. Operators reading the chip as a Boolean discover
   this on their own; the three-way coloring just slows that read.
2. The group-row summary chip block (`(N)  ◉ a ◐ b ◯ c`) repeats the
   same three buckets for every group row, eating right-edge width
   the cwd / project / preview columns could use, and the muxed and
   unmuxed counts are recoverable from `(N total)` minus a single
   ambiguity count.
3. The filter modal, in contrast, *does* benefit from three buckets —
   "show me only ambiguous sessions" is a useful query even when the
   row chip doesn't carry the distinction. Filter and row rendering
   should decouple.

## Decision

1. **The per-session row chip becomes attachable-binary.** The
   renderer maps `MuxIndicator` to one of two glyphs:
   - `◉` (theme `mux_attached`) when the session has exactly one
     definitive `LinkedToMux` candidate.
   - `◯` (theme `mux_unmuxed`) when the session has zero candidates
     **or** has ambiguous candidates. The chip's meaning is "there
     is a definitive mux here that is attachable"; ambiguous
     candidate sets fail that test, so they render as no-mux.
   The `MuxIndicator` enum **keeps** the
   `Ambiguous { candidate_count }` variant. Filter, status-bar
   hints, header counts, and the detail pane still need the
   distinction — only the row-chip *rendering* collapses.
2. **Group rows drop the per-bucket count chips and gain a single
   ambiguity warning glyph.** The group summary block becomes:
   `(N total)` plus `⚠` (theme `mux_ambiguity_warning`, an orange
   foreground) when any descendant session is in the
   `Ambiguous` state. No `◉ a ◐ b ◯ c` triplet. No count of
   ambiguous sessions on the group row — the detail pane's
   `Ambiguous muxes` section (ADR 0071) is the catalog.
3. **The filter modal keeps three buckets** (`attached`,
   `ambiguous`, `unmuxed`) and the original glyph vocabulary in
   its chip pills. `MuxStateKey` and the
   `MuxStateKey::from_candidate_count` derivation are unchanged.
4. **The pane-level header counter (top-left of the sessions pane)
   keeps the three-glyph chip section** for now. It's one global
   summary, not a per-group repeat, and serves as a discovery
   affordance for the filter modal. Revisit if the global chip
   block proves redundant after the row/group simplification.

## Consequences

- **Group-by views read cleanly.** A workspace with four ambiguous
  member sessions renders one `⚠` on the workspace row instead of
  four `◐` chips on the children plus a redundant `◐ 4` on the
  group summary.
- **The row chip carries a single decision-relevant signal.** When
  the operator sees `◉` they know `Enter`/`a` attaches without
  prompting; when they see `◯` they know the answer is either
  "nothing to attach to" or "open the group's detail to choose,"
  with the group row's `⚠` distinguishing the two cases at a
  glance.
- **The model keeps the three-bucket distinction.** Filter
  matching, status-bar hints ("Enter/a attach preferred · m
  choose"), the global header chip section, and the per-group
  `Ambiguous muxes` detail listing all continue to read
  `MuxIndicator::Ambiguous { candidate_count }` directly.
- **The row visual loses one prior cue.** An operator who relied on
  seeing `◐` on a session row to know "this specific session is in
  an ambiguous set" must now look up at the enclosing group row
  (single `⚠`) or into the detail pane. The compensation is the
  ADR 0071 group-detail catalog plus the new orange warning glyph;
  the cost is one indirection step for that question.
- **Snapshot churn.** Sessions / mux / union / prs / forks view
  snapshots all change. The showcase fixture re-renders without
  the per-session `◐` cluster and with a single `⚠` on the
  `atelier-demo` workspace row.
- **`MuxIndicator::Ambiguous` variant survives.** No model-layer
  rename, no migration; just two render sites change.

## Alternatives Considered

### A. Drop the row chip entirely

Remove the per-session mux glyph; rely on the group row's
ambiguity warning and the status bar's "Enter/a attach" hint to
answer "is there a mux here." Rejected: the "attachable from
here" Boolean is the single most-asked question per row in the
sessions view; losing the inline answer forces a status-bar scan
on every selection change.

### B. Introduce a new glyph pair (e.g. `■` / `□`)

Switch to a different visual vocabulary so the binary chip reads
clearly as "a different thing than the old three-way chip."
Rejected: the operator already reads `◉` as "muxed, attached" and
`◯` as "no mux" today; recycling the existing glyphs costs no
re-learning. The semantic shift (ambiguous → renders as `◯`) is
small enough to absorb without a new symbol.

### C. Keep the three-way row chip and add deduplication

Detect when multiple sibling sessions share the same ambiguous
mux set and suppress the duplicate `◐` chips. Rejected: more
clever logic than the problem deserves, and the chip is still
noise on the *first* visible ambiguous session even before any
duplicate kicks in. The simpler answer — "ambiguity is a group
concern" — generalizes better.

### D. Use a warning text chip instead of a glyph

Surface group ambiguity as `(! N ambiguous)` rather than a single
`⚠`. Rejected: heavier visually, eats right-edge width the
secondary path label often wants, and the count is recoverable
in the detail pane's catalog. The single warning glyph reads as
"there is ambiguity here; open the detail pane to learn how
much."

### E. Collapse the filter modal's buckets to two

Reduce `MuxStateFilter` to muxed / unmuxed so the filter UI
matches the row rendering. Rejected: "find every ambiguous
session" is a working operator query even when the row chip
hides ambiguity; the filter surface is a separate discovery
affordance from the row rendering.

## Open Questions

- Should the pane-level global header chip section eventually
  also collapse to a two-bucket form plus the warning glyph?
  Defer until the per-row / per-group change ships and the
  global header's redundancy can be assessed against real use.
- Does the ambiguity warning glyph want a count tooltip on hover
  (when a future mouse-aware mode lands), or stay strictly
  visual? Out of scope for V1.
- Should the `⚠` glyph respect `NO_COLOR` by switching to a
  bracketed text marker (`(!)`) instead of an orange-fg glyph?
  Probably yes; track under the broader `NO_COLOR` audit
  alongside the H-VIS workstream rather than this ADR.
