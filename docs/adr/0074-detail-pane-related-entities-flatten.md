# ADR 0074: Detail-Pane Related-Entities Flatten

## Status

Accepted. Narrowed 2026-10-02 by [ADR 0107](0107-related-view-neighbor-granularity.md):
rows are one per neighbor rather than one per candidate link, the
Other zone holds only neighbors the resolver did not pick, and row
keys identify neighbors.

## Context

The detail-pane relationship explorer landed under the `CSP-313` /
`CSP-314` / `CSP-315` mockup-driven workstream. That design split the
focused node's neighbors into two stacked sections —
`Upstream` (incoming edges, `link.target == focused`) and
`Downstream` (outgoing edges, `link.source == focused`) — each its
own [`RelationshipExplorer`](`src/tui/explorer.rs:208`) keyed by
`Direction`. Within each section, rows were grouped by
`(relation, neighbor_kind)` headers and the resolver-winner /
alt-of / conflict distinction surfaced as inline edge-state text
(`CSP-328`).

Operating against the showcase scenario revealed three friction
points:

1. **Direction is harder to read by section position than by verb.**
   Operators had to learn that "Upstream" meant "the focused node is
   on the receiving end" and "Downstream" meant "the focused node
   originates the relation." Once the verb catalog is in place
   (`forked from`, `contains`, `attached to`, `spawned by`, …), the
   verb carries the same information in plain English, and the
   section position becomes redundant scaffolding.
2. **The `(relation, neighbor_kind)` sub-headers add visual weight
   without helping the scan.** With one row per neighbor and the
   verb in the leftmost column, the relation kind is already named
   on the row. The sub-header line is just a third level of nesting
   above an already-narrow column.
3. **Validated and competing links mix in the same scan.** The
   resolver's chosen winner (`EdgeStateLabel::Resolves`) is the
   answer to "what relationship is true here." Alternates
   (`AltOf`), conflicts (`Conflict`), and unresolved-evidence stubs
   are all the question "what could it be" — useful for diagnosing
   the resolver but noise during routine scanning. The two
   audiences want different layouts.

ADR 0073 (per-node-kind visual identity) puts the kind glyph on
every row. That removes the strongest justification for keeping the
`neighbor_kind` sub-header at all: the glyph is already there.

The data model already encodes direction per
[`GraphLink`](`src/model/links.rs`) and the resolver classifies each
candidate as `Resolves` / `AltOf(rel)` / `Conflict`
(`src/tui/explorer.rs:804`). This is a view-layer flatten, not a
model change. ADRs 0001 (NodeId), 0002 (GraphLink), and 0050 (graph
visualization exports) all stay intact.

## Decision

### 1. Single `relationships` field on `NodeView`

`NodeView` replaces the prior pair:

```rust
pub upstream: RelationshipExplorer,
pub downstream: RelationshipExplorer,
```

with one combined surface:

```rust
pub relationships: RelationshipExplorer,
```

`Direction` (`Upstream` / `Downstream`) survives on
`RelationshipGroup` and `RelationshipLink` as per-row metadata so
the renderer can pick the right verb. The enum keeps its existing
shape; the only loss is the `Direction::label()` accessor for
section dividers, which no caller will need.

### 2. Verb catalog drives the leftmost row column

A new `RelationKind::directional_verb(direction: Direction) -> &'static str`
maps each `(RelationKind, Direction)` pair to a user-facing verb.
The verb takes the place the prior `relation.snake_case()` text held
in `render_group_header_line` and `render_single_link_composite`.

Initial catalog (subject to refinement during the implementation
pass; the catalog lives in one place so updates are local):

| RelationKind | Outgoing (focus = source) | Incoming (focus = target) |
|--------------|---------------------------|----------------------------|
| `AssociatedWith` | `associated with` | `associated with` |
| `BelongsToRepo` | `belongs to` | `checked out at` |
| `CheckedOutBranch` | `on branch` | `checked out by` |
| `WorkspaceContainsRepo` | `contains` | `member of` |
| `BranchHasForgePr` | `has PR` | `for branch` |
| `LinkedToMux` | `attached to` | `attached session` |
| `RootedIn` | `rooted in` | `hosts` |
| `ForksWorkspace` | `forks workspace` | `forked by` |
| `ForksRepo` | `forks repo` | `forked by` |
| `CreatedCheckout` | `created` | `created by` |
| `ReferencedCheckout` | `references` | `referenced by` |
| `ParentSession` | `parent of` | `child of` |
| `ChildSession` | `child of` | `parent of` |
| `CreatedBranch` | `created branch` | `created by` |
| `AssociatedBranch` | `associated branch` | `associated by` |
| `ParentFork` | `forked from` | `forked by` |
| `RootedAtPath` | `rooted at` | `hosts` |
| `MuxContainsProcess` | `contains process` | `in mux` |
| `ProcessIdentifiesSession` | `identifies` | `identified by` |
| `ProcessCandidatesSession` | `candidate for` | `candidate process` |

Open question (a) from the backlog stays open: for relations whose
verbs read identically in both directions (`AssociatedWith` is the
canonical case), the renderer **does not** add an arrow suffix in
v1. If real operator confusion appears on a specific relation, a
follow-up commit refines that row's verb pair rather than adding a
fallback glyph.

### 3. Two-zone layout: validated rows on top, "Other" below the fold

The flattened explorer renders in two zones under one `Related`
divider:

1. **Validated zone.** A flat, header-less list of rows whose
   `edge_state == EdgeStateLabel::Resolves`. Each row reads
   `<verb-column 22w> <kind-glyph> <neighbor_label>` (with the
   ADR 0073 glyph in the neighbor-kind color). Every row is
   selectable; cursor navigation walks the validated zone as one
   contiguous space.
2. **Other zone.** A single collapsible header `▶ Other (N)` (or
   `▼ Other (N)` when expanded) whose children are every non-
   `Resolves` row: `AltOf(_)`, `Conflict`, and unresolved-evidence
   stubs. Rows inside the Other zone share the same
   `<verb-column> <glyph> <neighbor_label>` shape so a focused row
   reads the same way in either zone. The Other header carries a
   `⚠` suffix when any descendant is `Conflict` and a `—` suffix
   when any descendant is an unresolved stub, matching the
   ADR 0071 / 0072 ambiguity language.

The prior per-`(relation, neighbor_kind)` sub-headers are removed
entirely. The kind glyph (ADR 0073) plus the directional verb
carry the same information per row; a sub-header would be a third
identification of the same fact.

The Other zone is collapsed by default. The reducer remembers the
per-detail Other-zone expansion state so navigating into a neighbor
and returning via the breadcrumb preserves the operator's choice.

### 4. Sort order: neighbor kind → verb → neighbor label

Rows in both zones sort by `(NodeKind ordinal, verb, neighbor_label)`.
The kind ordinal matches `NodeKind::ALL` order from `src/tui/icons.rs`
so the visual scan reads `▦ workspaces → ◆ repos → ◇ checkouts → ●
sessions → ▣ muxes → ⚙ processes → ⎇ branches → ⑂ forks → ⇄ PRs`.
Within a kind, the verb sort clusters semantically similar rows;
within a verb, alphabetical neighbor label keeps the order stable
across snapshot refreshes.

This differs from the prior implicit sort (per-direction, then
relation kind, then candidate order). The kind-first sort matches
the operator's most common scan question (`what does this node
touch?`) better than the prior relation-first sort.

### 5. `Related` divider with summary chip

One divider line above the validated zone, chip on the right,
summary on the left:

```
────────── N validated · M other Related ──
```

The summary segments are:

- `N validated` — count of `Resolves` rows.
- `M other` — count of every non-`Resolves` row plus unresolved
  stubs, suppressed when `M == 0`.

Ambiguity (`⚠`) and unresolved (`—`) glyphs from ADR 0072 follow
the operator-facing pattern from the prior `Upstream`/`Downstream`
dividers, but attached to the `M other` segment (`M other · K ⚠ · L —`)
since by definition none of those signals can appear in the
validated zone.

### 6. Cursor + breadcrumb preservation

`ExplorerRowKey` collapses to one axis per zone:

```rust
pub enum ExplorerRowKey {
    Title,
    NodeField { label: &'static str },
    ValidatedLink { index: usize },
    OtherHeader,
    OtherChild { index: usize },
}
```

`Direction` exits the cursor key — the renderer reads it off the
row's `RelationshipGroup` for verb selection but never for cursor
identity. `Enter` still drills into the selected neighbor; the
breadcrumb stack and `Backspace` flow are unchanged. The
`expanded_groups` map collapses to a single `other_expanded: bool`
on the explorer state.

## Consequences

- **Operators get one place to look for the answer.** "What
  relationships does this node have?" reads as one ordered list,
  and the resolver-winner story is the default surface. Diagnostic
  data (alternates, conflicts, unresolved stubs) stays accessible
  via the `Other` zone but does not compete for vertical real
  estate.
- **Cursor navigation simplifies.** The cursor walks the validated
  zone as one space, then the Other header, then the Other
  children when expanded. Direction-aware tests (`agent_session_groups_link_to_mux_downstream`,
  the `linked session now surfaces upstream of the …` test) rewrite
  to assert on the combined list and the appropriate zone.
- **Snapshot churn is concentrated.** The right-pane explorer body
  is the only surface that changes. Header, status bar, left-pane,
  and node-fields are untouched. Updated snapshots collapse to one
  per node kind rather than the prior up/down pair.
- **The verb catalog becomes a maintained artifact.** Adding a new
  `RelationKind` variant requires picking both direction's verbs
  before the renderer compiles. An exhaustive match in
  `directional_verb` enforces this at build time.
- **Direction stays in the model.** `GraphLink`, `ResolvedRelationship`,
  the SQLite projections, and the JSON / DOT outputs all keep their
  source/target semantics. Only the right-pane render flattens; the
  rest of the project still asks "is this edge inbound or
  outbound?" through the existing data fields.
- **CSP-419 (resolved-vs-candidate visual separation)** lands
  cleanly on top: the validated zone *is* the resolver-winner row
  treatment, and the Other zone's per-row glyph + color treatment
  can carry the `EdgeStateLabel::AltOf` / `Conflict` distinction
  without re-litigating section placement.
- **`★` resolver-winner marker becomes redundant in the validated
  zone.** Every row there *is* a resolver winner. The marker either
  goes away entirely (preferred) or moves to the Other zone as the
  "this alternate is the resolver's *would-be* pick if its
  candidate fired" marker. The implementation pass picks one;
  default is to drop `★` from validated rows and leave it as-is in
  the Other zone for now.

## Alternatives Considered

### A. Keep the two-section layout but rename the dividers

Rename `Upstream` / `Downstream` to `Inbound` / `Outbound` and stop
there. Rejected: the verb catalog work needs to happen regardless
(operators read `forked from` more naturally than `upstream
parent_fork`), and once verbs are in place the section dividers are
redundant. Smaller change, smaller payoff.

### B. Keep the per-relation sub-headers and just collapse direction

Drop `Upstream` / `Downstream` but keep `(relation, neighbor_kind)`
sub-headers within the flat list. Rejected: the per-row glyph (ADR
0073) plus the verb (this ADR's decision 2) already identify the
relation and the neighbor kind. The header would say `▶ contains
(2)` above two `contains` rows that already start with `contains`.
Redundant scaffolding.

### C. Single flat list with no zone for alternates / conflicts

Treat every link the same — render `Resolves`, `AltOf`, `Conflict`,
and unresolved stubs in one flat list with a per-row state chip.
Rejected: routine scans become noisier on graphs with conflicts.
The validated/Other split matches the operator's two distinct
questions (`what is true?` vs `what could be?`) and respects ADR
0071 / 0072's stance that ambiguity is a separate surface from the
primary answer.

### D. Sort by verb, then neighbor label (drop the kind-first sort)

Sort within zones by `(verb, neighbor_label)` only, so rows with
the same verb cluster regardless of neighbor kind. Rejected: the
operator's most common scan question is `what does this node
touch?` (which kinds are reachable). The kind-first sort surfaces
that answer in one pass; the verb cluster is preserved as a
secondary axis within each kind. Verb-first sort makes mixed-kind
flows (e.g. a workspace that contains both repos and forks) read
as one long list of `contains` instead of two short ones grouped
by glyph.

### E. Drop the `Related` divider

Flow node-field rows straight into the related-entities rows with
no chip divider. Rejected: the divider carries the summary chip
(`N validated · M other`) which is the fastest answer to `how
many neighbors does this node have?` Without it, the operator has
to count rows. One divider line is a cheap price for that
affordance.

### F. Add a `→` / `←` arrow suffix to every row to disambiguate
direction

Append the arrow regardless of whether the verb is symmetric.
Rejected: most verbs read unambiguously already (`forked from`
cannot be confused with `forked by`). Adding the arrow everywhere
adds noise. The narrow case of an actually-symmetric verb
(`AssociatedWith`) is rare enough that the v1 plan accepts the
ambiguity and revisits if it becomes a real problem.

### G. Render the Other zone as a side-by-side panel rather than
below-the-fold

Split the right pane horizontally between validated and Other.
Rejected: the right pane is already split vertically between the
node-fields zone and the explorer; a second horizontal split would
crowd the chip dividers and force narrow columns. Below-the-fold
collapse matches the existing detail-pane vertical rhythm.

## Open Questions Answered

- The detail pane has one `Related entities` zone, not two
  `Upstream` / `Downstream` zones. Section position no longer
  encodes direction.
- The directional verb catalog lives on `RelationKind` as
  `directional_verb(direction)` so adding a new relation kind is a
  one-stop edit.
- Per-`(relation, neighbor_kind)` sub-headers are removed entirely.
  The kind glyph + verb subsume them.
- Validated rows (`EdgeStateLabel::Resolves`) sit in a flat,
  always-visible list. Alternates, conflicts, and unresolved stubs
  sit under a single collapsible `Other` header below the
  validated zone.
- Sort order is neighbor-kind ordinal → verb → neighbor label,
  matching the operator's scan question.
- The `Related` divider is kept and carries an `N validated · M
  other` summary plus ambiguity / unresolved glyphs on the `other`
  segment.
- `Direction` stays in the data model. Only the renderer flattens.
- Arrow-suffix disambiguation is **not** introduced in v1; the
  open backlog question stays open until a specific verb proves
  insufficient.
