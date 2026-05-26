# Vector Search

`conspectus query --similar-to <node-id>` runs nearest-neighbor
search over the `embeddings` table introduced in ADR 0042. The
feature ships in two flavors that share the same SQL surface:

- **Linear-scan fallback** (default). Pure-Rust cosine-distance scan
  over the embeddings table. No setup beyond writing rows. Suitable
  for the graph sizes Conspectus targets (low-thousands of vectors).
- **Indexed via `sqlite-vec`**. Load the extension at connection
  open time and write SQL that calls `vec_distance_cosine` or
  queries a `vec0` virtual table you created yourself. Required for
  fast KNN at scale.

Conspectus does not compute embeddings. You ingest them from
whatever model you choose; the schema is opaque to model identity.

## CLI quickstart

```sh
# Linear-scan KNN against the default field (last_message_preview),
# returning the 10 nearest neighbors.
conspectus query \
    --similar-to 'agent_session:claude-code:default:abc123'

# Pick a different embedded field and a smaller result set.
conspectus query \
    --similar-to 'agent_session:claude-code:default:abc123' \
    --field last_message_preview \
    --limit 5

# JSON output for downstream tooling.
conspectus query \
    --similar-to 'agent_session:claude-code:default:abc123' \
    --format json

# Load sqlite-vec and run a raw query that uses its functions
# directly. The flag is independent of --similar-to: it works with
# any SQL query path.
conspectus query \
    --load-extension /usr/local/lib/vec0.so \
    "SELECT node_id, vec_distance_cosine(vector,
       (SELECT vector FROM embeddings WHERE node_id = 'agent_session:...:abc')) AS d
     FROM embeddings WHERE source_field = 'last_message_preview'
     ORDER BY d LIMIT 10"
```

## Result schema

`--similar-to` returns four columns:

| Column         | Type    | Notes                                         |
| -------------- | ------- | --------------------------------------------- |
| `node_id`      | TEXT    | The neighbor's NodeId Display form.           |
| `source_field` | TEXT    | The field used for the comparison.            |
| `model`        | TEXT    | Embedding model identifier (opaque).          |
| `distance`     | REAL    | Cosine distance. 0 ≤ d ≤ 2; lower is closer.  |

Rows are ordered by ascending `distance`. The target node itself is
excluded from the result.

## The `embeddings` table

```sql
CREATE TABLE embeddings (
    node_id      TEXT    NOT NULL,
    source_field TEXT    NOT NULL,   -- e.g. 'last_message_preview'
    model        TEXT    NOT NULL,   -- opaque to conspectus
    dim          INTEGER NOT NULL,
    vector       BLOB    NOT NULL,   -- float32 LE, dim * 4 bytes
    PRIMARY KEY (node_id, source_field, model)
);
```

The table is **not cleared by the loader**. Embeddings persist
across snapshot rebuilds. Stale rows (vectors for nodes that no
longer exist in the graph) survive until you prune them:

```sql
DELETE FROM embeddings
 WHERE node_id NOT IN (SELECT node_id FROM v_nodes);
```

ADR 0042 deferred a built-in garbage-collection command.

## Ingestion

There is no ingest-from-model path inside Conspectus. The choice of
model, the cost of computing embeddings, GPU access, and provider
rate-limits are all yours. The minimum viable ingestion script is a
shell command per row:

```sh
# Compute the vector with whatever you have on hand. The example
# below is a placeholder.
vector_json=$(my_embed_script "$session_text")
sqlite3 ~/.local/share/conspectus/graph.sqlite <<SQL
  INSERT INTO embeddings (node_id, source_field, model, dim, vector)
  VALUES ('$node_id', 'last_message_preview', 'all-MiniLM-L6-v2',
          384,
          (SELECT VEC_FROM_JSON('$vector_json')));
SQL
```

`VEC_FROM_JSON` is a `sqlite-vec` helper that parses a JSON array
into a float32 BLOB. Without the extension you compose the BLOB
yourself (four little-endian bytes per float). A future
`conspectus query --import-embeddings` will read JSON Lines in this
shape:

```
{"node_id": "...", "source_field": "...", "model": "...", "vector": [...]}
{"node_id": "...", "source_field": "...", "model": "...", "vector": [...]}
```

## Same-dim assumption

The linear-scan fallback silently skips rows whose `dim` does not
match the target's. ADR 0042 punts multi-model storage with
different dims to a follow-up. v1 expects a database to contain
embeddings from a single model (or from multiple models that all
happen to use the same dimension).

## Loading `sqlite-vec`

Conspectus does not bundle the extension. To use the indexed path:

1. Build `sqlite-vec` from <https://github.com/asg017/sqlite-vec>
   (the README has prebuilt binaries for major platforms).
2. Note the path to the resulting `vec0.so` / `vec0.dylib` /
   `vec0.dll`.
3. Pass it via `--load-extension PATH` on any `conspectus query`
   invocation that wants the extension's functions or virtual
   tables.

Loading an extension is unsafe in the same sense `LD_PRELOAD` is
unsafe: the extension can execute arbitrary native code. Treat the
path argument as you would any other system-administration setting.

## See also

- ADR 0036 — SQLite engine selection.
- ADR 0042 — vector search via sqlite-vec (this feature's design
  rationale, scope, and follow-up roadmap).
- `docs/query-guide.md` — general `conspectus query` usage.
