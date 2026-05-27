# ADR 0044: NodeId Foreign References Persisted as JSON

## Status

Accepted

## Context

ADR 0043 §"Schema work required" identified one schema gap as a
blocker for the consumer-side migration: `NodeId` foreign-reference
columns in `candidate_links`, `resolved_relationships`,
`diagnostics`, and `aliases` store only the `fmt::Display` form of
the typed `NodeId`. Recovering the structural pieces requires
parsing the Display string, and the Display format uses `:`, `@`,
`#`, and `/` as separators — characters that legitimately appear
inside `common_dir` paths, refnames, and similar structural fields.

ADR 0043 named two candidate fixes (structured-id columns; or
hardened `FromStr for NodeId` with escaping). A wider design pass
on `phase-9-sqlite-spike` enumerated four options and compared
them as concrete `schema.sql` diffs:

1. **Literal flattening** — replace `source_node_id TEXT` with a
   `*_kind` discriminator plus one nullable column per structural
   field across every NodeId variant. ~28 columns × 2 sides on
   `candidate_links`. No new tables. No parser.
2. **Per-kind sidecar tables** — strip the structural fields out of
   main tables into per-`(referring_table, side, kind)` sidecars.
   16 sidecars on `candidate_links`, 16 on `resolved_relationships`,
   8 each on `diagnostics` / `aliases`; ~48 new tables total.
   Narrowest main tables. No parser. PK rework needed for
   `resolved_relationships` and `aliases` (their natural keys were
   the dropped text columns).
3. **Keep text + add `*_kind` discriminator** — minimum schema
   delta; structural fields recovered via join into `node_<kind>`.
   Parser survives as the recovery path for orphan references
   whose node isn't in any `node_*` table.
4. **JSON-encoded foreign references** — store each endpoint as a
   single `TEXT` column holding a serde-serialized `NodeId`; use
   SQLite generated columns over `json_extract(col, '$.type')` to
   surface the kind discriminator for indexing.

Orphan references — link/alias/diagnostic rows whose endpoint
points at a `NodeId` not present in any `node_*` table — are
structurally normal in this system. They arise from declared
links to undiscovered nodes, aliases for sessions that haven't
been scanned, cross-provider references when one provider is
skipped, partial eviction, and lineage relations crossing
discovery boundaries. CLAUDE.md guardrails name sparse graphs as
the default expected shape, not an exceptional one.

Options 1, 2, and 4 all preserve structural pieces at write time
(the loader has typed values and writes them into typed storage,
typed-tagged storage, or JSON respectively). Option 3 discards
structural pieces at write time and relies on join-or-parse to
recover them later.

## Decision

Adopt **Option 4: JSON-encoded foreign references**. Each
endpoint column on the affected tables stores a serde-serialized
typed value (`NodeId` or `LinkEndpoint`). A sibling
`GENERATED ALWAYS AS (json_extract(..., '$.type')) STORED` column
exposes the kind discriminator for indexed filtering. Structural
filters use `json_extract` either directly in WHERE clauses or
through expression indexes.

### Schema shape per table

`candidate_links`:

```sql
source                 TEXT NOT NULL,    -- serde_json(&link.source) → NodeId JSON
source_kind            TEXT NOT NULL GENERATED ALWAYS AS
                           (json_extract(source, '$.type')) STORED,

target_kind            TEXT NOT NULL,    -- 'node' | 'unresolved'
target_node            TEXT,             -- serde_json(&id) when target_kind = 'node'
target_node_kind       TEXT GENERATED ALWAYS AS
                           (CASE WHEN target_kind = 'node'
                                 THEN json_extract(target_node, '$.type')
                            END) STORED,
-- UnresolvedEndpoint columns (target_node_type, target_harness_key, …)
-- remain unchanged for the `target_kind = 'unresolved'` variant.
```

`resolved_relationships` (PK becomes the JSON columns directly —
`(source, relation, target)` is unique by the same argument as the
prior `(source_node_id, relation, target_node_id)`):

```sql
source              TEXT NOT NULL,
source_kind         TEXT NOT NULL GENERATED ALWAYS AS (json_extract(source, '$.type')) STORED,
target              TEXT NOT NULL,
target_kind         TEXT NOT NULL GENERATED ALWAYS AS (json_extract(target, '$.type')) STORED,
relation            TEXT NOT NULL,
selected_link_id    TEXT NOT NULL,
competing_link_ids  TEXT NOT NULL DEFAULT '[]',
PRIMARY KEY (source, relation, target)
```

`diagnostics`:

```sql
conflict_source       TEXT,           -- JSON when kind = 'conflict'
conflict_source_kind  TEXT GENERATED ALWAYS AS
                          (json_extract(conflict_source, '$.type')) STORED,
```

`aliases` (PK becomes the JSON column):

```sql
node          TEXT PRIMARY KEY,
node_kind     TEXT NOT NULL GENERATED ALWAYS AS (json_extract(node, '$.type')) STORED,
display_name  TEXT NOT NULL
```

### Saved views

`v_mux_attachments`, `v_pr_by_branch`, `v_fork_ancestry`, and
`v_workspace_member_repos` previously joined on the dropped text
columns. They rewrite to structural joins using
`json_extract(..., '$.<field>')` against the typed `node_<kind>`
columns. Each gets a supporting expression index per the access
pattern the view uses (e.g.
`idx_candidate_links_target_node_native_id` for
`v_mux_attachments`). `v_sessions_with_repo` and `v_nodes` are
unaffected — neither referenced the dropped columns.

### Loader / reader

The loader writes JSON via `serde_json::to_string(&link.source)`
and so on. The reader uses `serde_json::from_str::<NodeId>` to
recover the typed value. `reader::parse_node_id` is deleted.

### SCHEMA_VERSION

Bumps from 2 to 3. ADR 0037 §"Schema versioning" governs upgrade
behavior: a binary refuses to open a database written by a newer
schema, and migrates forward from older versions. The first
release shipping ADR 0044 treats a v2 file as a cold rebuild (the
loader regenerates everything from discovery + resolver output;
no data preservation needed because the affected tables are
rebuildable cache).

## Consequences

- The fragile `parse_node_id` helper is removed. Structural
  recovery routes through serde, which already passes a
  round-trip test (`model::node_id_round_trips_through_json`)
  for one variant; a new test extends coverage to every variant.
- The round-trip equality test landed by P10-001 continues to
  pass byte-for-byte. The shape of the persisted endpoint
  changes but the semantic round trip does not.
- Schema width stays narrow. `candidate_links` gains 3 columns
  (`source`, `source_kind`, `target_node_kind`) and loses 2
  (`source_node_id`, `target_node_id`). Net +1 column.
  `resolved_relationships`, `aliases`, `diagnostics` change
  similarly.
- Renderer queries for structural filters become more verbose:
  `json_extract(source, '$.harness_key') = ?` rather than direct
  column equality. Expression indexes match the access patterns
  the saved views and renderers use, so the query planner picks
  them up.
- `sqlite3 .schema` stays compact; `SELECT … FROM candidate_links`
  shows endpoints as JSON objects instead of the Display text.
  Less crisp for casual inspection but unambiguous, and the
  `*_kind` generated column carries the type tag inline as a
  scannable hint.
- Adding a new structural field to a `NodeId` variant (e.g. P7-002's
  provider-provenance fields) no longer requires a schema change
  for the endpoint columns. Serde absorbs it. The typed `node_<kind>`
  tables still need column additions for the new field if it's
  query-relevant on its own; the link-side endpoint columns just
  carry the JSON through. SCHEMA_VERSION only bumps when the
  typed tables move, not when endpoint JSON shape evolves.
- Adding a new `NodeId` variant (a ninth node kind) requires
  schema work in the typed `node_<kind>` table area (per
  ADR 0037), but the link-side endpoint columns don't need
  changes — the new variant's JSON tag falls through the generated
  column expression unchanged.
- The compile-time exhaustiveness invariant on the loader weakens
  on the endpoint side. Today the loader's
  `let RepoNode { … } = repo;` destructure forces awareness of
  every field. After this ADR, the loader serializes whole
  `NodeId` values via serde — the loader no longer touches
  endpoint structural fields field-by-field. The typed
  `node_<kind>` tables retain their existing exhaustive
  destructures. The new test
  (`every_node_id_variant_round_trips_through_json`) catches
  serde-contract drift at test time; it is the symmetric guard
  for what the destructure used to enforce on the endpoint side.
- The schema-drift detection from P10-001
  (`schema_columns_match_constants`) catches column shape changes
  but not JSON content drift. A serde rename or field removal
  would slip past the schema test. The variant-coverage
  round-trip test added here is the matching guard.

## Alternatives Considered

- **Option 1 (literal flattening)**: rejected. The structural
  recovery property is identical to Option 4, but the schema
  width balloons (~28 columns × 2 sides on `candidate_links`)
  for no compensating benefit. Schema-evolution friction is
  higher — every `NodeId` field addition requires a column.
- **Option 2 (per-kind sidecar tables)**: rejected. The narrow
  main tables are appealing but the 48-table fan-out plus
  surrogate-key rework on `resolved_relationships` and `aliases`
  is heavyweight for an internal model-mirroring schema. The
  routing complexity in the loader is real (per-kind branches in
  4 insert paths). The expressive power gained over Option 4 is
  marginal: structural joins are easy in both shapes; the
  sidecar shape happens to use relational JOINs while Option 4
  uses `json_extract`.
- **Option 3 (keep text + add `*_kind`)**: rejected. The parser
  survives, which means orphan references can't be reconstructed
  without it, and the parser's fragility to separator characters
  in path components is the original problem we set out to fix.
  Promoting `parse_node_id` to a hardened `FromStr for NodeId`
  with escaping was the alternative path; the assessment was
  that storing the structural pieces explicitly is the cleaner
  invariant than maintaining a robust parser for a format we
  control.
- **Use SQLite JSONB (binary JSON, SQLite ≥ 3.45)**: deferred. JSONB
  is faster to access but loses human readability with `sqlite3`
  inspection. The endpoint columns are small (<200 bytes each
  typically); the JSON1 (text) representation is fine at this scale.
  Migration to JSONB is a future optimization, not blocking.
- **Sit on `parse_node_id`'s current limitations**: rejected.
  The renderer migration in P10-004..012 reads endpoint structural
  fields enough that a fragile parser would either need hardening
  (real work) or be papered over with renderer-side workarounds
  (worse). Solving it once at the storage layer is cleaner.

## Open Questions Answered

- **Does this change affect the JSON export?** The export
  (`conspectus dump --format json`) reads from SQLite via
  `read_snapshot()` and serializes the resulting `GraphSnapshot`.
  `GraphSnapshot`'s JSON shape is governed by the model's existing
  serde derives and is independent of the SQLite encoding. The
  export wire format does not change.
- **What about partial JSON queries with non-typed `node_id`
  values?** The schema is closed-world: every endpoint column is
  serde-serialized from a typed `NodeId`. The loader is the only
  writer. An external SQL query that synthesizes a malformed JSON
  blob and writes it to the database is out of scope — the
  writer connection lives in the server (ADR 0038), and
  read-only client connections cannot mutate.
- **How does this interact with P10-001's
  `schema_columns_match_constants`?** The column list constants
  in `query::schema::TABLE_COLUMNS` need updating for the new
  shape (`source`, `source_kind`, `target_node`, `target_node_kind`
  on `candidate_links`, etc.). The test catches the schema
  changes that follow this ADR; it doesn't catch JSON shape
  drift, which is what
  `every_node_id_variant_round_trips_through_json` covers.
- **Does this require touching the resolver?** No. The resolver
  consumes typed `GraphLink` values and emits typed
  `ResolvedRelationship` values. The serialization to JSON
  happens entirely in the loader (write path) and reader (read
  path); the resolver does not see the SQLite layer.
- **What if a future need wants O(1) structural lookup by
  `NodeId`?** The generated `*_kind` columns plus expression
  indexes on `json_extract` paths give the planner the same
  access shape as direct typed columns. Concrete renderer
  patterns added in P10-004..012 will decide which expression
  indexes pay for themselves; the ADR commits to the encoding,
  not a fixed index set.
