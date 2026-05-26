# Conspectus Query Guide

`conspectus query <sql>` runs read-only SQL against the resolved
graph database (per ADR 0036). The schema mirrors the in-Rust model
field-for-field; see `src/query/schema.sql` for the full DDL.

This guide covers two things: the saved views shipped with the
binary, and the conventions to keep in mind when writing your own
queries against the schema.

## Running queries

```
conspectus query 'SELECT count(*) FROM v_nodes'
conspectus query --format json 'SELECT * FROM v_sessions_with_repo'
conspectus query --format csv  'SELECT harness_key, count(*) FROM node_agent_sessions GROUP BY harness_key'
conspectus query --width 80    'SELECT * FROM v_pr_by_branch'
```

The `query` command opens the database read-only and rejects every
mutation (`INSERT`, `UPDATE`, `DELETE`, `CREATE`, `DROP`, writable
`ATTACH`) at the SQLite layer. Mutating graph state remains the job
of the structured commands (`rename`, `declared`, …) which route
through the server's writer connection (see ADR 0038).

Output formats:

- `--format table` (default) — width-aware columnar text. Truncates
  with `…` when cells overflow the budget. Headers are bold when
  stdout is a TTY (`--color auto|always|never`).
- `--format json` — one JSON object per line, keyed by column name.
- `--format csv` — RFC 4180 CSV (comma-separated, CRLF row
  terminator, quoted on need).
- `--format tsv` — tab-separated with `\t`/`\n`/`\r`/`\\` escaped
  inside cells so output stays line-oriented.

## Saved views

`conspectus query --list-views` enumerates the curated set below.
These views are **not a stable contract** — column shapes and
inclusion may evolve in lock-step with `SCHEMA_VERSION`. They name
the joins users would otherwise write by hand.

### `v_sessions_with_repo`

Agent sessions joined to the deepest checkout whose root contains
the session's cwd. Sessions without a matching checkout still
appear (LEFT JOIN) with NULL checkout columns.

Columns: `session_node_id`, `harness_key`, `state_scope`,
`session_key`, `cwd`, `last_active_epoch`, `checkout_node_id`,
`checkout_root`, `repo_common_dir`.

```sql
SELECT harness_key, COUNT(*) AS n
FROM v_sessions_with_repo
GROUP BY harness_key
ORDER BY n DESC;
```

### `v_mux_attachments`

Active `linked_to_mux` candidate links joined to their mux session.
One row per attachment; muxes with multiple attached agent sessions
appear with one row each.

Columns: `mux_node_id`, `backend`, `native_id`, `agent_session_node_id`,
`link_id`, `provenance`, `confidence`, `freshness`.

```sql
SELECT native_id, COUNT(*) AS attached_agents
FROM v_mux_attachments
GROUP BY mux_node_id
HAVING attached_agents > 1;
```

### `v_pr_by_branch`

Branches joined to their forge PRs via `branch_has_forge_pr`.
Branches without a PR still appear with NULL PR columns.

Columns: `branch_node_id`, `repo_common_dir`, `refname`,
`pr_node_id`, `pr_provider`, `pr_host`, `pr_owner`, `pr_repo`,
`pr_number`, `pr_state`, `pr_is_draft`, `pr_url`.

```sql
SELECT refname, pr_state, pr_url
FROM v_pr_by_branch
WHERE pr_state IS NOT NULL
ORDER BY refname;
```

### `v_fork_ancestry`

Transitive `parent_fork` closure as a recursive CTE. One
`(fork_node_id, ancestor_node_id, depth)` row per chain step.
Depth 0 is the fork itself; depth 1 is its direct parent; and so on.

Columns: `fork_node_id`, `ancestor_node_id`, `depth`.

```sql
-- All forks descended from a given root.
SELECT fork_node_id, depth
FROM v_fork_ancestry
WHERE ancestor_node_id = 'fork:my-root'
ORDER BY depth;
```

### `v_workspace_member_repos`

Workspaces joined to their member repos via
`workspace_contains_repo`. Inner join — workspaces that do not
contain any repos do not appear.

Columns: `workspace_node_id`, `workspace_root`,
`workspace_provider`, `repo_node_id`, `repo_common_dir`.

```sql
SELECT workspace_root, COUNT(*) AS member_count
FROM v_workspace_member_repos
GROUP BY workspace_node_id
ORDER BY member_count DESC;
```

## Schema conventions

A few things to know when writing queries directly against the
underlying tables:

- **Node identity**: every `*.node_id` is the `NodeId::Display`
  form (e.g. `agent_session:claude-code:default:abc`). The
  per-kind tables (`node_repos`, `node_agent_sessions`, …) all
  carry this as their primary key, and the `v_nodes` view unions
  them so `SELECT node_id, node_kind FROM v_nodes` walks every
  node.
- **Relation strings**: the `relation` column on `candidate_links`
  and `resolved_relationships` uses the serde snake_case tags
  (`linked_to_mux`, `branch_has_forge_pr`, `parent_fork`, …).
  The `RelationKind` enum in `src/model/mod.rs` is the canonical
  list.
- **`candidate_links` vs `resolved_relationships`**: candidate
  links carry every evidence-level link the providers found, with
  provenance and confidence. `resolved_relationships` carries only
  the resolver-selected winner per `(source, relation, target_key)`
  tuple. For exploration, query `resolved_relationships`. For
  audit / diagnostic work, query `candidate_links` to see the
  competing alternatives.
- **`state` column**: only `state = 'active'` links inform the
  in-Rust views. The saved views above all filter on `'active'`.
  When writing your own query, decide whether you want to see
  ignored / overridden links too.
- **JSON-text columns**: `source_paths`, `remotes`, `capabilities`,
  `competing_link_ids`, `source_fields`, `target_metadata`,
  `conflict_competing_link_ids` hold JSON. Use `JSON_EXTRACT(col,
  '$.key')` or `JSON_EACH(col)` to crack them.
- **Provenance columns**: `discovery_provider` and
  `discovery_freshness_epoch` are forward-looking columns. Until
  P7-002 lands provider provenance on the in-memory model the
  loader writes the defaults (`'unknown'`, `0`); afterward, these
  will carry real per-row provider attribution.

## Composing recursive queries

SQLite supports `WITH RECURSIVE` natively. `v_fork_ancestry` is the
canonical example. For session-lineage chains, use the same shape
against the `parent_session` relation:

```sql
WITH RECURSIVE chain(child, parent, depth) AS (
    SELECT s.node_id, NULL, 0 FROM node_agent_sessions s
    UNION ALL
    SELECT
        c.child,
        cl.target_node_id,
        c.depth + 1
    FROM chain c
    JOIN candidate_links cl
        ON cl.source_node_id = COALESCE(c.parent, c.child)
        AND cl.relation = 'parent_session'
        AND cl.state = 'active'
    WHERE c.depth < 32       -- safety bound
)
SELECT * FROM chain
WHERE depth > 0
ORDER BY child, depth;
```

The deepest known cycle in conspectus's data is a few hops, so a
small explicit depth bound is friendly defensive practice rather
than load-bearing.

## See also

- `docs/design.md` § Query Surface — design rationale.
- ADR 0036 — engine selection.
- ADR 0037 — persistence model and schema versioning.
- ADR 0038 — CLI / server transport under WAL.
- ADR 0039 — `query` Cargo feature gate.
- `src/query/schema.sql` — full DDL.
