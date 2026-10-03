# ADR 0107: Related View At Neighbor Granularity

## Status

Accepted. Narrows ADR 0074 §3 and §6, ADR 0075 §1, and the
`competing_link_ids` semantics from ADR 0077.

## Context

The detail pane's `Related` section (ADR 0074) has a validated zone
for resolver winners and a collapsed `Other` zone for "everything
else": alternates, conflicts, and unresolved stubs. The intent was
that validated answers "what is true?" and Other answers "what else
could it be?"

In practice the same neighbor showed up in both zones. A mux could
list `attached session claude-code:8c7e…` as validated and again,
marked `⚠`, under Other. Two things combined to cause it:

1. **The explorer emits one row per candidate link, not per
   neighbor.** `build_explorer` walks `candidate_links` and turns
   every active link into a row. Independent producers routinely
   corroborate each other: a local pin, a hook-sidecar record, and a
   `cross_link` active-pane match can all say "session S is attached
   to mux M". That's three rows for one fact.
2. **The resolver calls every non-winner "competing".**
   `resolve_links` buckets single-target relations by
   `(source, relation)`, takes the top-ranked candidate, and puts the
   rest in `competing_link_ids`, whether or not they point at the
   same target. It then emits `Diagnostic::Conflict` for any
   non-empty list. The explorer maps `competing_link_ids` to
   `EdgeStateLabel::Conflict`, so evidence that agrees with the winner
   reads as disagreement.

On the operator's live graph both resolved `LinkedToMux` slots had
this shape: a pin won, and a hook-sidecar or `cross_link` candidate for
the *same* mux was listed as competing. Every pin whose harness is also
seen by hook or process discovery hits this case, so the Related
summary showed a false `1 ⚠` and `conspectus node` printed a false
conflict.

ADR 0074 didn't choose this; it assumed every non-winner was a
different answer. The data model never told agreeing candidates apart
from disagreeing ones.

## Decision

### 1. The resolver separates corroborating from competing candidates

`ResolvedRelationship` gains `corroborating_link_ids: Vec<String>`:

- **`corroborating_link_ids`**: non-winning candidates in the slot
  whose target equals the winner's target. They support the answer.
- **`competing_link_ids`**: non-winning candidates whose target
  differs from the winner's. They are alternative answers.

For multi-target relations the target is part of the slot key, so
every non-winner is corroborating by construction.

`Diagnostic::Conflict` is emitted only when `competing_link_ids` is
non-empty. Agreement among producers is not a conflict.

The no-winner case from ADR 0077 is unchanged in spirit: when
`suppress_ambiguous_cwd_mux_links` clears `selected_link_id`, every
candidate the slot considered, corroborating ones included, moves into
`competing_link_ids` and `corroborating_link_ids` is emptied. There is
no answer for them to corroborate.

`ResolutionExplanation` gains a matching `corroborating` score list so
`conspectus node show` and `graph --explain` can still show why one
producer outranked another. `decisive_axis` is computed against the
first competing candidate, falling back to the first corroborating
one, so the explainer keeps naming the axis that decided the ranking.

`snapshot::FORMAT_VERSION` bumps so `graph.bin` caches written with the
old shape cold-rebuild (ADR 0082).

### 2. The explorer emits one row per neighbor

`finalize_group` folds its candidate links by neighbor, so each
`(direction, relation, neighbor)` becomes one `RelationshipLink` row:

- If any of the neighbor's links is a resolver winner, the row is
  `Resolves` and lands in the validated zone. The winner is the row's
  representative link.
- Otherwise the row lands in Other. It is `Conflict` when any of its
  links is in a slot's `competing_link_ids` (a real different-target
  competitor, or a no-winner slot) and `AltOf` when none is (the
  neighbor's slot resolved somewhere this view isn't).
- Every active link backing the row is kept on it as
  `evidence: Vec<LinkEvidence>` (link id, provenance, confidence,
  adapter, and evidence kind), representative first.

So a neighbor appears at most once in `Related`, in exactly one zone.
Other holds only neighbors the resolver did not pick, plus unresolved
stubs. `⚠` counts mean real disagreement.

### 3. Supporting evidence lives in the Preview zone

The row itself stays `<verb> <glyph> <neighbor>`; no count chip is
added. When the cursor is on a link row, the Preview zone lists every
backing link under an `evidence` heading, one line per link: the
evidence kind (or the adapter when the producer stamped none), then
provenance and confidence, e.g.
`hook_session_path_match · strong_discovered · high`. The existing
`edge` line stays and describes the representative.

### 4. Row keys identify neighbors, not links

`ExplorerRowKey::ValidatedLink` and `OtherLink` carry
`(direction, relation, neighbor)` instead of a link id. The
representative link can change between refreshes (a pin gets authored,
a hook record goes stale) without the row changing identity, so the
cursor and breadcrumb restores from ADR 0074 §6 stay put.

## Consequences

- **The validated zone and the summary chip read honestly.** A
  pin-bound mux shows one `attached session` row and no Other zone.
- **Conflict diagnostics shrink to real disagreements.** `node` output,
  graph JSON, and the HTML inspector stop reporting agreeing producers
  as losers. The HTML inspector's "Lost candidates" list only shows
  different-target candidates; corroborating ones get their own list.
- **Graph JSON gains a field.** `corroborating_link_ids` is omitted
  when empty, so snapshots without corroboration keep their existing
  shape. Consumers that treated `competing_link_ids` as "all
  non-winners" must union the two lists.
- **The tree views are unaffected.** They already follow
  `selected_link_id` (ADR 0077 / `CSP-422`), and the no-winner
  fan-out still walks `competing_link_ids`.
- **Evidence is one step further away in the TUI.** Seeing which
  producers agreed means moving the cursor to the row and reading the
  Preview zone, rather than expanding Other. That trade is the point:
  provenance detail is diagnostic, not part of the routine scan.

## Alternatives Considered

### A. View-only fix: dedupe by neighbor in the explorer

Group by neighbor in `finalize_group` and classify by comparing
targets there, leaving the resolver alone. Rejected: the false
`Diagnostic::Conflict`, the mixed meaning of `competing_link_ids` in
graph JSON, and the HTML inspector's "Lost candidates" would all stay
wrong. Per the data-model-first guardrail the model should carry the
distinction rather than each consumer re-deriving it.

### B. Keep per-link rows but move corroborating links to validated

Show every agreeing link as its own validated row. Rejected: it moves
the duplicate from Other to validated instead of removing it, and
makes the validated zone grow with the number of producers instead of
the number of neighbors.

### C. Show corroboration as a row chip (`×3`)

Annotate the validated row with an evidence count. Rejected for now:
it adds a column of noise to the routine scan, and the count isn't
actionable without seeing which producers it counts. The Preview zone
already exists for exactly that question. Revisit if operators want
an at-a-glance evidence-strength signal.

### D. Drop corroborating candidates from the resolver output

Only record the winner and different-target competitors. Rejected: it
throws away evidence the explainer and debugging rely on, and
conflicts with "declared links win but are not destructive" in
`docs/design.md`.

## Open Questions Answered

- A neighbor appears at most once in `Related`.
- Other is scoped to neighbors the resolver did not pick, plus
  unresolved-evidence stubs. Corroborating evidence is not an Other
  row.
- `competing_link_ids` means different-target candidates (or every
  candidate, for a no-winner slot). Same-target candidates live in
  `corroborating_link_ids`.
- A conflict diagnostic requires a different-target competitor.
