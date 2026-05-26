-- Conspectus query-engine schema.
--
-- Mirrors the Rust model in src/model/mod.rs field-for-field. See:
--   ADR 0036 (engine selection)
--   ADR 0037 (persistence model)
--   ADR 0038 (CLI / server transport under WAL)
--   ADR 0039 (query feature gate)
--   ADR 0041 (resolver stays in Rust; SQL is a consumer of resolver output)
--
-- Conventions:
--   * `node_id TEXT` uses the `NodeId` Display form (e.g.
--     `agent_session:claude-code:default:abc`) as the stable
--     content-addressed primary key.
--   * Polymorphic blobs (`SourceMetadata.fields`,
--     `UnresolvedEndpoint.metadata`, `Vec<String>` collections) land in
--     `TEXT` columns holding JSON, queryable via `JSON_EXTRACT`.
--   * `discovery_provider` / `discovery_freshness_epoch` track per-row
--     producing-provider provenance per ADR 0037. Until P7-002 adds the
--     provider fields to the in-memory model, the loader writes sensible
--     defaults (`'unknown'`, `0`); these defaults disappear once P7-002
--     lands and the columns become genuinely required.
--   * `PRAGMA user_version` is set in Rust by `apply_schema()` to match
--     the `SCHEMA_VERSION` constant. Bumping the schema means bumping
--     both in lockstep.

-- =============================================================
-- Node tables (one per kind, plus a v_nodes union view)
-- =============================================================

CREATE TABLE IF NOT EXISTS node_repos (
    node_id                   TEXT PRIMARY KEY,
    common_dir                TEXT NOT NULL,
    source_paths              TEXT NOT NULL DEFAULT '[]',   -- JSON array
    remotes                   TEXT NOT NULL DEFAULT '[]',   -- JSON array
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_checkouts (
    node_id                        TEXT PRIMARY KEY,
    repo_common_dir                TEXT NOT NULL,   -- denormalized from CheckoutId.repo
    root                           TEXT NOT NULL,
    git_dir                        TEXT,
    current_branch_repo_common_dir TEXT,
    current_branch_refname         TEXT,
    discovery_provider             TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_workspaces (
    node_id                   TEXT PRIMARY KEY,
    root                      TEXT NOT NULL,
    provider_name             TEXT,                          -- WorkspaceNode.provider
    name                      TEXT,
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_agent_sessions (
    node_id                   TEXT PRIMARY KEY,
    harness_key               TEXT NOT NULL,
    state_scope               TEXT NOT NULL,
    session_key               TEXT NOT NULL,
    cwd                       TEXT,
    title                     TEXT,
    last_message_preview      TEXT,
    last_active_epoch         INTEGER,
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_node_agent_sessions_last_active
    ON node_agent_sessions(last_active_epoch DESC);

CREATE TABLE IF NOT EXISTS node_mux_sessions (
    node_id                       TEXT PRIMARY KEY,
    native_id                     TEXT NOT NULL,
    backend                       TEXT NOT NULL,
    cwd                           TEXT,
    active_pane_command           TEXT,
    active_pane_pid               INTEGER,
    active_pane_current_path      TEXT,
    active_pane_start_command     TEXT,
    activity_epoch                INTEGER,
    created_epoch                 INTEGER,
    discovery_provider            TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_branches (
    node_id                   TEXT PRIMARY KEY,
    repo_common_dir           TEXT NOT NULL,    -- denormalized from BranchId.repo
    refname                   TEXT NOT NULL,
    current_commit            TEXT,
    upstream                  TEXT,
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_forks (
    node_id                   TEXT PRIMARY KEY,
    provider_source_key       TEXT NOT NULL,    -- from ForkId
    provider_name             TEXT NOT NULL,    -- ForkNode.provider (atelier / agent-deck / ...)
    name                      TEXT,
    scope                     TEXT,
    capabilities              TEXT NOT NULL DEFAULT '[]',   -- JSON array
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS node_forge_prs (
    node_id                   TEXT PRIMARY KEY,
    provider_name             TEXT NOT NULL,    -- ForgePrNode.provider (github / ...)
    host                      TEXT NOT NULL,
    owner                     TEXT NOT NULL,
    repo                      TEXT NOT NULL,
    number                    INTEGER NOT NULL,
    state                     TEXT,
    url                       TEXT,
    updated_epoch             INTEGER,
    is_draft                  INTEGER NOT NULL DEFAULT 0,   -- boolean
    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

-- Union view over all node tables. node_kind matches the serde tag on
-- GraphNode / NodeId (snake_case).
CREATE VIEW IF NOT EXISTS v_nodes AS
    SELECT node_id, 'repo'          AS node_kind, discovery_provider, discovery_freshness_epoch FROM node_repos
    UNION ALL
    SELECT node_id, 'checkout',      discovery_provider, discovery_freshness_epoch FROM node_checkouts
    UNION ALL
    SELECT node_id, 'workspace',     discovery_provider, discovery_freshness_epoch FROM node_workspaces
    UNION ALL
    SELECT node_id, 'agent_session', discovery_provider, discovery_freshness_epoch FROM node_agent_sessions
    UNION ALL
    SELECT node_id, 'mux_session',   discovery_provider, discovery_freshness_epoch FROM node_mux_sessions
    UNION ALL
    SELECT node_id, 'branch',        discovery_provider, discovery_freshness_epoch FROM node_branches
    UNION ALL
    SELECT node_id, 'fork',          discovery_provider, discovery_freshness_epoch FROM node_forks
    UNION ALL
    SELECT node_id, 'forge_pr',      discovery_provider, discovery_freshness_epoch FROM node_forge_prs;

-- =============================================================
-- Candidate links (all evidence-level GraphLinks)
-- =============================================================

CREATE TABLE IF NOT EXISTS candidate_links (
    link_id                   TEXT PRIMARY KEY,             -- GraphLink.id
    source_node_id            TEXT NOT NULL,

    -- LinkEndpoint: discriminator + node-id variant + unresolved-evidence variant.
    target_kind               TEXT NOT NULL,                -- 'node' | 'unresolved'
    target_node_id            TEXT,                         -- populated when target_kind = 'node'
    target_node_type          TEXT,                         -- UnresolvedEndpoint.node_type when 'unresolved'
    target_harness_key        TEXT,
    target_native_id          TEXT,
    target_state_scope        TEXT,
    target_path               TEXT,
    target_metadata           TEXT NOT NULL DEFAULT '{}',   -- JSON; UnresolvedEndpoint.metadata

    relation                  TEXT NOT NULL,                -- RelationKind serde tag (snake_case)
    provenance                TEXT NOT NULL,                -- Provenance serde tag
    confidence                TEXT NOT NULL,                -- Confidence serde tag
    freshness                 TEXT NOT NULL,                -- Freshness serde tag

    -- LinkState: discriminator + variant fields.
    state                     TEXT NOT NULL,                -- 'active' | 'ignored' | 'overridden'
    state_reason              TEXT,
    state_overridden_by       TEXT,

    -- SourceMetadata
    source_adapter            TEXT NOT NULL,
    source_evidence           TEXT,
    source_fields             TEXT NOT NULL DEFAULT '{}',   -- JSON

    discovery_provider        TEXT NOT NULL DEFAULT 'unknown',
    discovery_freshness_epoch INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_candidate_links_source_relation
    ON candidate_links(source_node_id, relation);
CREATE INDEX IF NOT EXISTS idx_candidate_links_target_relation
    ON candidate_links(target_node_id, relation);
CREATE INDEX IF NOT EXISTS idx_candidate_links_provider_fresh
    ON candidate_links(discovery_provider, discovery_freshness_epoch);

-- =============================================================
-- Resolved relationships (resolver output, ADR 0041)
-- =============================================================

CREATE TABLE IF NOT EXISTS resolved_relationships (
    source_node_id     TEXT NOT NULL,
    target_node_id     TEXT NOT NULL,
    relation           TEXT NOT NULL,
    selected_link_id   TEXT NOT NULL,
    competing_link_ids TEXT NOT NULL DEFAULT '[]',          -- JSON array of link ids
    PRIMARY KEY (source_node_id, relation, target_node_id)
);

CREATE INDEX IF NOT EXISTS idx_resolved_relationships_source_relation
    ON resolved_relationships(source_node_id, relation);
CREATE INDEX IF NOT EXISTS idx_resolved_relationships_target_relation
    ON resolved_relationships(target_node_id, relation);

-- =============================================================
-- Diagnostics
-- =============================================================
--
-- Polymorphic over the three Diagnostic variants. `kind` discriminates;
-- per-variant columns are nullable.
CREATE TABLE IF NOT EXISTS diagnostics (
    kind                         TEXT NOT NULL,             -- 'unresolved_endpoint' | 'config' | 'conflict'
    -- UnresolvedEndpoint
    link_id                      TEXT,
    relation                     TEXT,                      -- shared with Conflict
    -- Config
    config_path                  TEXT,
    config_message               TEXT,
    -- Conflict
    conflict_source_node_id      TEXT,
    conflict_selected_link_id    TEXT,
    conflict_competing_link_ids  TEXT                       -- JSON array
);

-- =============================================================
-- Alias overlay (ADR 0029)
-- =============================================================

CREATE TABLE IF NOT EXISTS aliases (
    node_id      TEXT PRIMARY KEY,
    display_name TEXT NOT NULL
);

-- =============================================================
-- Provider state (per ADR 0037)
-- =============================================================

CREATE TABLE IF NOT EXISTS provider_state (
    provider     TEXT PRIMARY KEY,
    last_run_at  INTEGER NOT NULL,                          -- Unix epoch seconds
    last_outcome TEXT NOT NULL,                             -- 'success' | 'error' | 'skipped'
    last_error   TEXT                                       -- nullable detail
);

-- =============================================================
-- Embeddings (P9-008 / ADR 0042)
-- =============================================================
--
-- One row per (node, embedded source field, model) triple. The
-- vector is stored as little-endian float32, dim * 4 bytes. The
-- loader does not touch this table — embeddings are an additive
-- overlay maintained outside the discovery / resolver lifecycle.
-- See `docs/vector-search.md` for ingestion and `--similar-to`
-- usage.

CREATE TABLE IF NOT EXISTS embeddings (
    node_id      TEXT    NOT NULL,
    source_field TEXT    NOT NULL,                          -- e.g. 'last_message_preview'
    model        TEXT    NOT NULL,                          -- model identifier, opaque to conspectus
    dim          INTEGER NOT NULL,                          -- vector dimensionality
    vector       BLOB    NOT NULL,                          -- float32 little-endian, dim * 4 bytes
    PRIMARY KEY (node_id, source_field, model)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_source_field
    ON embeddings(source_field, model);

-- =============================================================
-- Saved views (P9-006)
-- =============================================================
--
-- A small curated library of named joins that the in-Rust view code
-- already encodes. Surfaced through `conspectus query --list-views`.
-- The set is curated, not a contract — bumping `SCHEMA_VERSION` is
-- required when a view's column shape changes. See
-- `docs/query-guide.md` for prose descriptions and example queries.

-- Agent sessions joined to the deepest checkout whose root contains
-- the session's cwd. Mirrors the join the TUI sessions view does;
-- sessions without a matching checkout still appear with NULL
-- checkout columns (LEFT JOIN preserves them).
CREATE VIEW IF NOT EXISTS v_sessions_with_repo AS
SELECT
    s.node_id            AS session_node_id,
    s.harness_key,
    s.state_scope,
    s.session_key,
    s.cwd,
    s.last_active_epoch,
    c.node_id            AS checkout_node_id,
    c.root               AS checkout_root,
    c.repo_common_dir    AS repo_common_dir
FROM node_agent_sessions s
LEFT JOIN node_checkouts c
    ON c.node_id = (
        SELECT c2.node_id
        FROM node_checkouts c2
        WHERE s.cwd IS NOT NULL
          AND (s.cwd = c2.root OR s.cwd LIKE c2.root || '/%')
        ORDER BY length(c2.root) DESC
        LIMIT 1
    );

-- One row per active `LinkedToMux` candidate link, joined to its
-- mux. Each mux can carry multiple attached agent sessions (one row
-- per attachment).
CREATE VIEW IF NOT EXISTS v_mux_attachments AS
SELECT
    m.node_id            AS mux_node_id,
    m.backend,
    m.native_id,
    cl.source_node_id    AS agent_session_node_id,
    cl.link_id,
    cl.provenance,
    cl.confidence,
    cl.freshness
FROM node_mux_sessions m
JOIN candidate_links cl
    ON cl.target_node_id = m.node_id
    AND cl.relation = 'linked_to_mux'
    AND cl.state = 'active';

-- Branches joined to their forge PRs via the `BranchHasForgePr`
-- relation. Branches without a PR still appear with NULL PR
-- columns (LEFT JOIN).
CREATE VIEW IF NOT EXISTS v_pr_by_branch AS
SELECT
    b.node_id            AS branch_node_id,
    b.repo_common_dir,
    b.refname,
    pr.node_id           AS pr_node_id,
    pr.provider_name     AS pr_provider,
    pr.host              AS pr_host,
    pr.owner             AS pr_owner,
    pr.repo              AS pr_repo,
    pr.number            AS pr_number,
    pr.state             AS pr_state,
    pr.is_draft          AS pr_is_draft,
    pr.url               AS pr_url
FROM node_branches b
LEFT JOIN candidate_links cl
    ON cl.source_node_id = b.node_id
    AND cl.relation = 'branch_has_forge_pr'
    AND cl.state = 'active'
LEFT JOIN node_forge_prs pr
    ON pr.node_id = cl.target_node_id;

-- Transitive `ParentFork` closure: for each Fork node, one row per
-- (self, ancestor, depth) triple. Depth 0 is the fork itself; each
-- next row follows one `parent_fork` candidate link further up.
CREATE VIEW IF NOT EXISTS v_fork_ancestry AS
WITH RECURSIVE ancestry(fork_node_id, ancestor_node_id, depth) AS (
    SELECT node_id, node_id, 0 FROM node_forks
    UNION ALL
    SELECT
        a.fork_node_id,
        cl.target_node_id,
        a.depth + 1
    FROM ancestry a
    JOIN candidate_links cl
        ON cl.source_node_id = a.ancestor_node_id
        AND cl.relation = 'parent_fork'
        AND cl.state = 'active'
)
SELECT fork_node_id, ancestor_node_id, depth FROM ancestry;

-- Workspaces with their member repos via the
-- `WorkspaceContainsRepo` relation. Inner join — workspaces that do
-- not contain any repos do not appear.
CREATE VIEW IF NOT EXISTS v_workspace_member_repos AS
SELECT
    w.node_id            AS workspace_node_id,
    w.root               AS workspace_root,
    w.provider_name      AS workspace_provider,
    r.node_id            AS repo_node_id,
    r.common_dir         AS repo_common_dir
FROM node_workspaces w
JOIN candidate_links cl
    ON cl.source_node_id = w.node_id
    AND cl.relation = 'workspace_contains_repo'
    AND cl.state = 'active'
JOIN node_repos r
    ON r.node_id = cl.target_node_id;
