# ADR 0041: Resolver Stays in Rust

## Status

Accepted

## Context

ADR 0036 adopts SQLite as the embedded query engine for the
`conspectus query <sql>` surface. A reasonable follow-up question
once SQL is available: should the resolver — the code in
`src/resolve/` that ranks candidate `GraphLink` records by provenance
precedence, applies relation-specific tie-breakers, and emits
typed `ResolvedRelationship` records — be reimplemented as SQL
queries that operate on the SQLite tables directly?

The case for moving it: SQL is already the engine, recursive CTEs
plus window functions can express ranking and tie-break logic, and
the resolver's output is just another query result.

The case against: the resolver is heavily unit-tested with stable
JSON fixtures. Provenance precedence (`Provenance::precedence` in
`src/model/mod.rs:390-400`) is small but load-bearing — silently
changing a winner on a tie case would cascade through every
downstream view. The resolver also produces diagnostics for
unresolved endpoints and competing candidates that are awkward to
emit cleanly from SQL.

This ADR records the non-decision so it does not get re-raised.

## Decision

The resolver stays in Rust. SQLite is the *consumer* of resolver
output, not a replacement for it.

Concretely:

- `src/resolve/mod.rs` continues to own `resolve_snapshot` and the
  per-relation precedence rules. Its inputs remain `GraphSnapshot`'s
  `candidate_links` and node set; its output remains
  `Vec<ResolvedRelationship>` plus the diagnostics it already emits.
- The SQLite loader (P9-003) populates a `resolved_relationships`
  table from the resolver's output, preserving the (source,
  relation, target) winner selection and carrying the diagnostics
  into a sibling `diagnostics` table.
- SQL queries against the schema may read either layer:
  `resolved_relationships` for the resolver-chosen winners or
  `candidate_links` for the full evidence trail. The saved-view
  library (P9-006) leans on the resolved layer by default; ad-hoc
  exploration freely walks either.

This is recorded as an ADR so the question is closed rather than
re-litigated each time a new view or query pattern is designed.

## Consequences

- The resolver test suite stays in Rust and keeps its byte-for-byte
  JSON-snapshot stability. No risk of SQL ranking diverging from the
  Rust comparator on tie cases.
- The SQLite schema mirrors the in-Rust model rather than encoding
  resolver semantics in DDL. Migrating the storage layer in the
  future (DuckDB, CozoDB, or anything else) is a loader-rewrite
  rather than a model rewrite.
- Diagnostics for unresolved endpoints, competing-candidate
  ambiguity, and conflict tie-breaks remain Rust-side data with
  rich types; they are surfaced through the `diagnostics` table for
  query access without rebuilding the type system in SQL.
- The line between "resolver decision" and "query result" is clear:
  if a column in the SQL schema requires precedence reasoning, it is
  populated by the resolver. If it can be derived by a SELECT, it
  lives in a view.

## Alternatives Considered

- **Move the resolver into SQL.** Reasons not to:
  - The current resolver's tie-break order
    (`Provenance::precedence` then `Confidence` then link id) is
    deterministic in Rust and equally deterministic in SQL via
    `ROW_NUMBER() OVER (PARTITION BY ... ORDER BY ...)` — but the
    *equivalence* is unverified, and verifying it would require
    rebuilding the resolver's full fixture corpus against SQL
    output, byte-for-byte.
  - SQLite recursive CTEs (without `USING KEY`) are not the
    cheapest way to express the resolver's per-relation logic.
    Some tie-breaks involve looking at sibling rows, which is
    easier in Rust than in correlated subqueries.
  - The resolver emits typed diagnostics that downstream code
    consumes structurally (`Diagnostic::CompetingCandidates`,
    `Diagnostic::UnresolvedEndpoint`, etc.). Reproducing this
    through SQL would either flatten the types into JSON text or
    require multiple round trips per query.
- **Move part of the resolver — e.g. just `LinkedToMux`
  precedence — into SQL.** Rejected because partial migration is
  the worst of both worlds: two implementations to keep in sync,
  same fixture-equivalence risk, no clear gain.
- **Run the resolver in SQL on the side and assert equality with
  the Rust output as a CI check.** Interesting as a verification
  technique but not adopted at v1; revisit if a specific resolver
  rule becomes unwieldy in Rust.

## Open Questions Answered

- **Does this foreclose ever moving the resolver into SQL?** No.
  This ADR records "not now," not "never." If a resolver rule grows
  unwieldy in Rust, or if SQLite gains a feature that makes ranking
  semantics first-class (e.g. a future `MERGE INTO` pattern), the
  decision is revisitable. The trigger would be a concrete pain
  point, not aesthetic preference.
- **What about the rename and declared-link CRUD path?** Those are
  *writers* into the candidate-link space, not resolver
  reimplementations. They stay in Rust, and the resolver re-runs
  over the updated candidate set on the next snapshot build.
- **Does this affect the SQL schema design?** Only by setting
  expectation. The schema (P9-002) carries a
  `resolved_relationships` table populated by the resolver; it does
  not carry views that *reimplement* the resolver from
  `candidate_links` alone. Saved views may join or filter on the
  resolved layer but should not duplicate its precedence logic.
