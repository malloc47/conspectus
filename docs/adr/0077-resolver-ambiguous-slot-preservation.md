# ADR 0077: Resolver-Side Preservation Of Ambiguous Resolution Slots

## Status

Accepted. Narrowed 2026-10-02 by [ADR 0107](0107-related-view-neighbor-granularity.md):
when a winner exists, `competing_link_ids` lists only
different-target candidates; same-target candidates move to
`corroborating_link_ids`.

## Context

The resolver groups candidate links into slots keyed by
`(source, relation, target_key)` and picks one as the winner
(`compare_candidates`). The winner becomes a `ResolvedRelationship`
with `selected_link_id` set to the winning link id and
`competing_link_ids` listing the rejected candidates.

A second pass — `suppress_ambiguous_cwd_mux_links` in
`src/resolve/mod.rs` — sweeps the `LinkedToMux` slots and **removes**
the `ResolvedRelationship` entirely when:

1. The would-be-winner's evidence is cwd-based
   (`exact_cwd_match` / `cwd_prefix_match`).
2. The target mux is reachable from ≥2 distinct sessions via the
   same cwd evidence.

The removal is correct in spirit — the cwd alone cannot tell which
session "really" attaches to the mux — but it leaves downstream
consumers with no first-class signal that an ambiguity exists at
that slot. The resolver pushes a `Diagnostic::Conflict` in parallel,
but consumers that read `resolved_relationships` (the detail-pane
explorer, the SQLite tree-view joins, the dot / html projections)
do not consult diagnostics, so they each grew their own ad-hoc
fallback to re-derive ambiguity from the candidate set:

- **CSP-421** (`src/tui/explorer.rs`): `build_relationship_group`
  flags a group as ambiguous when there is no
  `ResolvedRelationship` *and* there are ≥2 distinct candidate
  targets among the active candidates. Keeps the `⚠` glyph alive
  in the explorer.
- **CSP-422** (`src/tui/rows/sessions.rs`):
  `mux_candidates_for_session` walks the candidates when there is
  no resolver winner *and* ≥2 distinct candidate targets are
  present, returning the full fan-out so the sessions tree still
  groups the session into the `MuxIndicator::Ambiguous` bucket.

Both fallbacks reach over the model boundary and reconstruct what
the resolver already knows. They drift over time: a future change
to the candidate set, or a new suppression rule for some other
relation, would force a parallel fallback in each consumer.
`CSP-420` was filed to retire the inference path by promoting the
ambiguity into the resolved-relationships layer itself.

## Decision

### Shape

`ResolvedRelationship.selected_link_id` becomes `Option<String>`:

```rust
pub struct ResolvedRelationship {
    pub source: NodeId,
    pub target: NodeId,
    pub relation: RelationKind,
    pub selected_link_id: Option<String>,        // changed
    pub competing_link_ids: Vec<String>,
}
```

When the resolver picks a winner, `selected_link_id = Some(id)` and
the slot behaves exactly as today. When the resolver explicitly
cannot pick — currently only `suppress_ambiguous_cwd_mux_links`
— the slot survives with `selected_link_id = None` and
`competing_link_ids` carrying *every* candidate that was considered
(including what would have been the tiebreak winner).

`target` continues to carry the would-have-been-winner's target so
the slot is still keyed and joinable on `(source, relation)`. The
field name now reads as a *placeholder* in the no-winner case;
consumers MUST check `selected_link_id` before treating `target` as
"the answer."

### Why `Option<String>` over an additive flag

Two alternatives were considered:

1. **`ambiguous_only: bool`** added alongside the existing
   `selected_link_id: String`. Additive, no model break. Rejected
   because it leaves `selected_link_id` carrying a tiebreak-arbitrary
   winner that consumers must remember to ignore based on a separate
   field. Joins on `selected_link_id = link_id` would attribute the
   "wrong" arbitrary winner if the consumer forgot the flag check —
   exactly the bug `suppress_ambiguous_cwd_mux_links` was added to
   prevent.
2. **A new `ambiguous_targets: Vec<NodeId>`** field plus dropping
   `target` from the no-winner case. Rejected because it bifurcates
   the shape between winner and no-winner cases, complicating every
   consumer that reads `target` today.

`Option<String>` is honest: when the resolver cannot pick, the field
literally has no value. SQL `JOIN ... ON rr.selected_link_id =
cl.link_id` naturally excludes NULL rows (the CSP-422 tree-view
joins want exactly that behavior — no row for an ambiguous slot —
so the join semantics survive without per-consumer awareness).
Rust pattern-match callers express the case directly with `if let
Some(id) = &rel.selected_link_id`. The model break is mostly
mechanical: 12 source files and a chunk of snapshot files touch
`selected_link_id` today; most are either SQL joins (NULL-safe) or
JSON serialization (Option round-trips cleanly).

### `competing_link_ids` semantic shift

Today `competing_link_ids` lists the *losers*: candidates that
competed against `selected_link_id` and were dropped. The
no-winner case extends the semantic: when `selected_link_id` is
`None`, `competing_link_ids` lists *every* candidate that was
considered, including what would have been the arbitrary tiebreak
winner. Consumers that build a Conflict / AltOf row set walk
`competing_link_ids` regardless of whether `selected_link_id` is
`Some` or `None`; the only thing that changes between the two cases
is whether a `Resolves` row exists.

### `Diagnostic::Conflict` retention

The diagnostic stays. It is still useful for observers that want
the full ambiguity audit (the JSON `--explain` mode planned under
`CSP-096`, the test invariants in `tests/testing_replay.rs`), and
removing it would break wire-level snapshots. The resolved-side
signal becomes the primary read path for renderers; the diagnostic
becomes the secondary read path for observability.

## Consumers

| Surface | Change |
|---------|--------|
| `src/resolve/mod.rs` (resolver) | `suppress_ambiguous_cwd_mux_links` mutates the matched rel: sets `selected_link_id = None`, pushes the original winner id into `competing_link_ids`. No more `.retain()` removal. |
| `src/model/mod.rs` | `selected_link_id: Option<String>`; derive `Ord` updated so JSON sort stays stable. |
| `src/query/schema.sql` | `selected_link_id TEXT` (nullable, not `NOT NULL`). |
| `src/query/loader.rs` / `reader.rs` | Bind / read `Option<String>` via `rusqlite`'s `Option` impls. |
| `src/tui/rows/mux.rs` SQL | No change. The `JOIN ... ON rr.selected_link_id = cl.link_id` already excludes NULL rows; the CSP-422 invariant survives. |
| `src/tui/rows/sessions.rs` `mux_candidates_for_session` | Retire the candidate-fan-out fallback. Read `selected_link_id` directly; when `None`, return the links named in `competing_link_ids` so `MuxStateKey::Ambiguous` still fires. |
| `src/tui/rows/prs.rs` / `forks.rs` SQL | No change (same NULL-exclusion argument). |
| `src/tui/explorer.rs` `build_relationship_group` | Retire the CSP-421 fallback that infers ambiguity from candidate fan-out. Detect `selected_link_id.is_none()` to mark the group ambiguous and route every candidate to Conflict in the Other zone. |
| `src/output/dot.rs` / `html/mod.rs` / `node_show.rs` / `agent.rs` | Filter / pattern-match on `Option`. Selected-link sets skip `None`; node-show's "selected link" cell renders an em dash when no winner. |
| Tests + snapshots | Regenerate JSON / SQLite / showcase snapshots so the new `Option<String>` shape lands as data, not as a wire break in CI. |

## Consequences

- Two ad-hoc fallbacks retire (CSP-421 in `build_relationship_group`,
  CSP-422 in `mux_candidates_for_session`); the resolver becomes
  the single source of truth for ambiguity.
- The model break is contained: SQL joins survive without
  per-consumer awareness, JSON snapshots round-trip via `Option`,
  and the changed surfaces are small (mostly explicit pattern
  matches).
- The shape generalizes: future "resolver could not pick" cases
  (e.g. a hypothetical `suppress_ambiguous_*` rule for another
  relation) use the same `selected_link_id = None` form without
  inventing new types.
- `target` becomes "placeholder unless `selected_link_id` is set,"
  documented in the field doc. Consumers that join on target
  *without* checking selected (none exist today) would attribute
  ambiguity to the arbitrary tiebreak winner — flagged in the field
  comment so future readers don't reach for `target` blind.

## Alternatives Considered

- **Additive flag** — see "Why `Option<String>`" above.
- **Drop the suppression pass; rely on the CSP-421 fallback
  alone.** Rejected: two independent consumers (explorer renderer,
  tree-view SQL) already needed their own inference paths; a third
  would land the next time the model grew.
- **Move the ambiguity signal out of `ResolvedRelationship`
  entirely** (e.g. a new `AmbiguousSlot` type listed in
  `ResolveOutput`). Considered cleaner-typed but bifurcates the
  read surface — consumers would need to query two collections to
  answer "what's at this slot." `Option<String>` keeps the read
  pattern uniform.

## Open Questions

- Should the `Diagnostic::Conflict` entry the suppression pass
  emits have its `competing_link_ids` populated (it's `vec![]`
  today)? Out of scope for this ADR; the diagnostic is no longer
  the primary signal, and the cross-source semantics of "the
  competitors live on other sources' slots" make the population
  story non-obvious. Tracked as a followup if observability work
  needs it.
- Whether `target: NodeId` should also become `Option<NodeId>`
  someday. Rejected for v1: it would force every join site to
  handle `target IS NULL`, doubling the schema break for limited
  payoff. The "placeholder target" convention is documented and
  consumers already gate on `selected_link_id`.
