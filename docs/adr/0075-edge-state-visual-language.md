# ADR 0075: Edge-State Visual Language In The Detail-Pane Other Zone

## Status

Accepted

## Context

ADR 0074 collapsed the detail-pane relationship explorer into a
single `Related` zone with a flat validated list on top and a
collapsible `Other` zone below. The validated zone is the resolver-
winner zone by construction: every row there carries
`EdgeStateLabel::Resolves`. The Other zone holds the rest —
`EdgeStateLabel::AltOf(_)`, `EdgeStateLabel::Conflict`, and
unresolved-evidence stubs.

Today (post-H-UI-003) those three Other-zone categories all render
with the same shape: 6-space indent, `theme.link_id` color,
optionally bold via a `resolved_winner` path that can never fire
(Resolves rows are routed to the validated zone before the renderer
sees them). The visual signal that an `AltOf` is meaningfully
different from a `Conflict` is missing.

The legacy `★` resolver-winner marker, originally introduced to
highlight winners in mixed lists, is now functionally dead. The
validated zone never needs it (every row is a winner); the Other
zone never reaches it (Resolves rows don't land there).

H-UI-005 in the backlog requested per-row glyph or chip plus color
hooks per `EdgeStateLabel` variant. With the validated / Other split
already in place, the remaining work is the *per-row* distinction
inside Other.

Existing TUI vocabulary anchors the choice:

- `⚠` means "the resolver flagged this as ambiguous" (ADR 0071's
  group-detail catalog, ADR 0072's group-row warning).
- `—` (em dash) marks unresolved-evidence rows (current rendering).
- Color signals: `theme.warning` (orange/yellow) for ambiguity-
  family signals, `theme.secondary_text` for "secondary content,"
  `theme.placeholder` (DIM modifier) for inert / placeholder rows.

## Decision

### 1. Per-row visual treatment in the Other zone

Each Other-zone row is dispatched on `EdgeStateLabel`:

| Edge state | Indent | Prefix | Label color | Modifier |
|------------|--------|--------|-------------|----------|
| `AltOf(_)` | 6 spaces | none | `theme.edge_alt_of` (default: `secondary_text`) | none |
| `Conflict` | 4 spaces + `⚠ ` | `⚠ ` in `theme.edge_conflict` | `theme.edge_conflict` (default: `warning`) | BOLD |
| Unresolved | 6 spaces | `— ` | `theme.placeholder` (DIM) | none |

The Conflict glyph reuses the `⚠` vocabulary so a row-level conflict
and the Other header's `K ⚠` summary count share one symbol. The
prefix replaces two cells of indent (so the kind glyph column still
aligns across all three row kinds: indent + 2 cells of prefix / 6
cells of plain indent both anchor the verb column at column 6).

### 2. Drop the `★` resolver-winner marker

`render_validated_link_line` and `render_other_link_line` no longer
emit the `★` glyph. The validated zone already encodes "this is the
resolver's pick" via zone membership; the Other zone never holds
Resolves rows. The `resolved_winner: bool` field on
`RelationshipLink` stays in the data model (it backs the resolver
diagnostics layer) but the row renderer no longer reads it.

### 3. New theme keys: `edge_alt_of` and `edge_conflict`

ADR 0032's flat `[tui.theme]` map gains two color fields with the
existing color-spec grammar:

```toml
[tui.theme]
edge_alt_of   = "gray"      # defaults to `theme.secondary_text` value
edge_conflict = "yellow"    # defaults to `theme.warning` value
```

Defaults:

- `edge_alt_of` → `Color::DarkGray` (matches the
  `theme.secondary_text` default; operators can shift independently).
- `edge_conflict` → `Color::Yellow` (matches the `theme.warning`
  default; operators who paint conflict in a louder color can do so
  without changing the broader warning vocabulary).

Both go through the standard `known_keys` + `set_color` machinery.
The `Theme::default()` byte-for-byte invariant still holds for every
other field.

### 4. No group-level chip for candidate-only fan-outs

The H-UI-007 backstop (and the still-open H-UI-006) ensures
candidate-only fan-outs surface as Other-zone rows in the
`Conflict` state. The per-row `⚠ ` prefix plus the warning color
make those rows scream "conflict" without a separate group chip.
The Other header's `K ⚠` summary already carries the count for
group-level awareness.

This answers the backlog's open question about a distinct
candidate-only group header chip: no — the per-row treatment plus
the header summary cover it.

### 5. `NO_COLOR` legibility

Under `NO_COLOR` / `--color never`:

- AltOf rows are visually identical to plain links (no color hint,
  no prefix). The zone position (under `▶ Other`) carries the
  signal.
- Conflict rows still carry the `⚠ ` prefix glyph — color-blind
  and `NO_COLOR` operators see the conflict marker. The BOLD
  modifier survives under `NO_COLOR` for most terminals.
- Unresolved rows still carry the `— ` prefix and the DIM modifier
  (which may or may not render depending on terminal, but the
  prefix is unambiguous).

The Conflict glyph is therefore the only edge-state signal that
survives `NO_COLOR` reliably. This is acceptable because Conflict
is the actionable category (the resolver wants help); AltOf is
"alternative for context" and reads acceptably as "an unstyled
Other row."

## Consequences

- **The Other zone scans by color.** An operator skimming a
  detail pane sees Conflict rows jump out (warning + `⚠`),
  Unresolved rows recede (DIM + `—`), and AltOf rows sit in
  between (secondary text, no prefix). The three categories
  separate visually without re-reading the edge-meta toggle's
  trailing text.
- **The `★` marker exits the renderer.** One less glyph in the
  vocabulary, one less branch in `render_related_row`.
  `RelationshipLink.resolved_winner` stays available for non-
  renderer consumers (diagnostics, JSON / DOT under H-VIS-005).
- **Two new flat `[tui.theme]` keys.** Operators get
  independently-themable edge-state colors. The schema growth is
  small enough that ADR 0032's flat-map decision still holds.
- **Snapshot churn is concentrated in the explorer Other zone.**
  Tests that pinned the old `★` text remove it; new tests pin the
  per-edge-state color + prefix.
- **The Conflict glyph is the cross-mode anchor.** Color-blind,
  `NO_COLOR`, and ASCII-only operators all see `⚠` on conflict
  rows. The AltOf and Unresolved categories rely on a mix of
  color and DIM modifier, neither of which is reliable on every
  terminal; the `⚠` is the one signal we guarantee.

## Alternatives Considered

### A. Reuse existing `secondary_text` + `warning` without new keys

Color the rows using the existing theme vocabulary directly.
Rejected: any operator who wants to differentiate edge-state
colors (orange conflict but yellow warning chip on the header,
say) would have to override the broader keys too, leaking the
change into status-bar warnings and group ambiguity chips. The
two-key cost is a fair price for independent themability.

### B. Color only, no `⚠` prefix on Conflict rows

Drop the glyph and rely on `theme.edge_conflict` alone. Rejected:
under `NO_COLOR` the row reads identical to an AltOf, and the
conflict category is the one that actually wants the operator's
attention. The two-cell cost of `⚠ ` is acceptable.

### C. Per-edge-state kind-style glyph (e.g. `⊕` for AltOf)

Introduce a new glyph for every variant. Rejected: the kind-glyph
column already carries the neighbor kind; adding a second small
glyph per row crowds the indent and competes with the existing
visual rhythm. The `⚠`-on-Conflict-only approach reserves the
extra cell for the one category that needs to read at a glance.

### D. Move the `★` marker to a candidate-only group chip

Keep `★` and repurpose it to mean "this group has no resolver
winner." Rejected: the validated / Other zone split already
answers "is there a winner?" at the row level. A group chip
duplicates the answer the Other zone's existence already gives.

### E. New `edge_resolves` color for the validated zone

Add a third edge-state color for completeness. Rejected: the
validated zone already uses `theme.link_id` + BOLD as its
identity treatment. Adding `edge_resolves` would either duplicate
`link_id` (pointless) or break the operator's mental model that
"validated rows look like other id-bearing rows in the TUI." Keep
the validated zone's existing color scheme.

### F. Group-level "(no winner)" header chip per candidate-only group

Surface candidate-only fan-outs with a dedicated chip on the
group row. Rejected: the flattened detail pane has no per-group
header anymore (ADR 0074 §3). The per-row `⚠` plus the Other
zone's summary count cover the same operator question without
re-introducing the sub-header structure.

## Open Questions Answered

- The `★` resolver-winner marker is removed from the renderer.
- Per-edge-state row treatment lives in the Other zone only —
  the validated zone reads as `theme.link_id` + BOLD as before.
- `edge_alt_of` and `edge_conflict` are new flat `[tui.theme]`
  color fields; ADR 0032's flat-map invariant survives.
- Conflict rows carry `⚠ ` as a row prefix so the signal survives
  `NO_COLOR` / `--color never`.
- Candidate-only group fan-outs do not get a dedicated group
  chip; the per-row treatment plus the Other header's `K ⚠`
  summary cover the use case.
- The backlog's "distinct group header chip vs reusing ADR 0072's
  `⚠`" question collapses to "reuse `⚠` at the row level."
