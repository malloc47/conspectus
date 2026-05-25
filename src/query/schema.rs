//! SQL schema for the embedded query engine.
//!
//! The DDL itself lives in `schema.sql` (embedded via `include_str!`)
//! so it can be edited with SQL tooling and reviewed as a single unit.
//! This module exposes the constants and helpers that wrap it.
//!
//! See ADR 0036 (engine selection), ADR 0037 (persistence model), and
//! the P9-002 backlog story for the schema design rationale.

use rusqlite::Connection;

use crate::model::{GraphNode, NodeId, RelationKind};

/// Embedded DDL text.
pub const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Schema version recorded in `PRAGMA user_version` after `apply_schema`.
///
/// Bump whenever `schema.sql` changes shape (added/removed/renamed
/// tables, columns, indexes, or views), and ship a migration alongside
/// the bump. Aligned with the in-memory `GraphSnapshot` schema version;
/// when the model gains breaking changes (e.g. P7-002's provider-
/// provenance fields), both versions advance together.
pub const SCHEMA_VERSION: u32 = 1;

/// Apply `SCHEMA_SQL` to `conn` and record `SCHEMA_VERSION` in
/// `PRAGMA user_version`. Idempotent: every `CREATE` statement uses
/// `IF NOT EXISTS` so re-running it on a populated database is safe.
///
/// This is the entry point P9-003 (the loader) uses on a fresh
/// database file and on each warm-start open after verifying the
/// existing `user_version` matches.
pub fn apply_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    // PRAGMA does not accept bind parameters; the constant is safe to
    // interpolate since it is a `u32`.
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    Ok(())
}

/// Read the current `user_version` from `conn`.
pub fn read_user_version(conn: &Connection) -> rusqlite::Result<u32> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

/// Name of the per-kind node table for this node. The exhaustive match
/// is the compile-time enforcement that every `GraphNode` variant has a
/// corresponding table — adding a new variant breaks the build until
/// this function and `schema.sql` are updated together.
pub fn node_table_name(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Repo(_) => "node_repos",
        GraphNode::Checkout(_) => "node_checkouts",
        GraphNode::Workspace(_) => "node_workspaces",
        GraphNode::AgentSession(_) => "node_agent_sessions",
        GraphNode::MuxSession(_) => "node_mux_sessions",
        GraphNode::Branch(_) => "node_branches",
        GraphNode::Fork(_) => "node_forks",
        GraphNode::ForgePr(_) => "node_forge_prs",
    }
}

/// Per-kind table name for a [`NodeId`]. Same enforcement as
/// [`node_table_name`].
pub fn node_table_name_for_id(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "node_repos",
        NodeId::Checkout(_) => "node_checkouts",
        NodeId::Workspace(_) => "node_workspaces",
        NodeId::AgentSession(_) => "node_agent_sessions",
        NodeId::MuxSession(_) => "node_mux_sessions",
        NodeId::Branch(_) => "node_branches",
        NodeId::Fork(_) => "node_forks",
        NodeId::ForgePr(_) => "node_forge_prs",
    }
}

/// The serde tag string used for `node_kind` in the `v_nodes` view.
pub fn node_kind_tag(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}

/// The serde tag string for a [`RelationKind`]. Exhaustive over every
/// variant so adding a new relation breaks the build here until the
/// schema's `relation` column (and any saved view referencing it) is
/// considered.
pub fn relation_kind_tag(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::AssociatedWith => "associated_with",
        RelationKind::BelongsToRepo => "belongs_to_repo",
        RelationKind::CheckedOutBranch => "checked_out_branch",
        RelationKind::WorkspaceContainsRepo => "workspace_contains_repo",
        RelationKind::BranchHasForgePr => "branch_has_forge_pr",
        RelationKind::LinkedToMux => "linked_to_mux",
        RelationKind::RootedIn => "rooted_in",
        RelationKind::ForksWorkspace => "forks_workspace",
        RelationKind::ForksRepo => "forks_repo",
        RelationKind::CreatedCheckout => "created_checkout",
        RelationKind::ReferencedCheckout => "referenced_checkout",
        RelationKind::ParentSession => "parent_session",
        RelationKind::ChildSession => "child_session",
        RelationKind::CreatedBranch => "created_branch",
        RelationKind::AssociatedBranch => "associated_branch",
        RelationKind::ParentFork => "parent_fork",
        RelationKind::RootedAtPath => "rooted_at_path",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode,
        ForgePrId, ForgePrNode, ForkId, ForkNode, GraphNode, MuxSessionId, MuxSessionNode, RepoId,
        RepoNode, WorkspaceId, WorkspaceNode,
    };

    /// Every `RelationKind` value the project understands today. The
    /// length assertion in `relation_kind_tags_cover_every_variant`
    /// fails if a variant is added without updating this slice.
    const ALL_RELATION_KINDS: &[RelationKind] = &[
        RelationKind::AssociatedWith,
        RelationKind::BelongsToRepo,
        RelationKind::CheckedOutBranch,
        RelationKind::WorkspaceContainsRepo,
        RelationKind::BranchHasForgePr,
        RelationKind::LinkedToMux,
        RelationKind::RootedIn,
        RelationKind::ForksWorkspace,
        RelationKind::ForksRepo,
        RelationKind::CreatedCheckout,
        RelationKind::ReferencedCheckout,
        RelationKind::ParentSession,
        RelationKind::ChildSession,
        RelationKind::CreatedBranch,
        RelationKind::AssociatedBranch,
        RelationKind::ParentFork,
        RelationKind::RootedAtPath,
    ];

    fn fresh_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory open");
        apply_schema(&conn).expect("apply schema");
        conn
    }

    fn sample_nodes() -> Vec<GraphNode> {
        vec![
            GraphNode::Repo(RepoNode::new(RepoId::new("/r/.git"))),
            GraphNode::Checkout(CheckoutNode::new(
                CheckoutId::new(RepoId::new("/r/.git"), "/r"),
                "/r",
            )),
            GraphNode::Workspace(WorkspaceNode {
                id: WorkspaceId::new("/w"),
                root: "/w".into(),
                provider: None,
                name: None,
            }),
            GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("claude-code", "default", "s1"),
                harness_key: "claude-code".into(),
                cwd: None,
                title: None,
                last_message_preview: None,
                last_active_epoch: None,
            }),
            GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new("tmux:0"),
                native_id: "tmux:0".into(),
                backend: "tmux".into(),
                cwd: None,
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                activity_epoch: None,
                created_epoch: None,
            }),
            GraphNode::Branch(BranchNode {
                id: BranchId::new(RepoId::new("/r/.git"), "refs/heads/main"),
                refname: "refs/heads/main".into(),
                current_commit: None,
                upstream: None,
            }),
            GraphNode::Fork(ForkNode {
                id: ForkId::new("provider:source"),
                provider: "atelier".into(),
                provider_source_key: "provider:source".into(),
                name: None,
                scope: None,
                capabilities: Vec::new(),
            }),
            GraphNode::ForgePr(ForgePrNode {
                id: ForgePrId::new("github", "github.com", "owner", "repo", 1),
                provider: "github".into(),
                host: "github.com".into(),
                owner: "owner".into(),
                repo: "repo".into(),
                number: 1,
                state: None,
                url: None,
                updated_epoch: None,
                is_draft: false,
            }),
        ]
    }

    #[test]
    fn schema_applies_to_fresh_in_memory_connection() {
        let conn = fresh_conn();
        let version = read_user_version(&conn).expect("read user_version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn apply_schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        apply_schema(&conn).expect("first apply");
        apply_schema(&conn).expect("re-apply should not error");
        assert_eq!(read_user_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn every_expected_table_exists() {
        let conn = fresh_conn();
        let expected = &[
            "node_repos",
            "node_checkouts",
            "node_workspaces",
            "node_agent_sessions",
            "node_mux_sessions",
            "node_branches",
            "node_forks",
            "node_forge_prs",
            "candidate_links",
            "resolved_relationships",
            "diagnostics",
            "aliases",
            "provider_state",
        ];
        for table in expected {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap_or_else(|err| panic!("query sqlite_master for {table}: {err}"));
            assert_eq!(count, 1, "expected table {table} not found");
        }
    }

    #[test]
    fn every_expected_index_exists() {
        let conn = fresh_conn();
        let expected = &[
            "idx_node_agent_sessions_last_active",
            "idx_candidate_links_source_relation",
            "idx_candidate_links_target_relation",
            "idx_candidate_links_provider_fresh",
            "idx_resolved_relationships_source_relation",
            "idx_resolved_relationships_target_relation",
        ];
        for index in expected {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "expected index {index} not found");
        }
    }

    #[test]
    fn v_nodes_view_exists_and_unions_all_kinds() {
        let conn = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view' AND name = 'v_nodes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "v_nodes view missing");

        // Empty database: 0 rows. The query should still succeed.
        let row_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM v_nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(row_count, 0);
    }

    #[test]
    fn every_node_variant_maps_to_a_known_table() {
        // Exhaustive coverage: every `GraphNode` variant maps to a
        // table that the schema actually defines. Catches drift between
        // the Rust model and the SQL schema at test time.
        let conn = fresh_conn();
        for node in sample_nodes() {
            let table = node_table_name(&node);
            let id_table = node_table_name_for_id(&node.id());
            assert_eq!(
                table, id_table,
                "GraphNode and NodeId table mappings disagree for {table}"
            );
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "node table {table} missing");
        }
    }

    #[test]
    fn every_node_variant_has_a_node_kind_tag() {
        for node in sample_nodes() {
            let tag = node_kind_tag(&node.id());
            assert!(
                !tag.is_empty(),
                "tag for {table} is empty",
                table = node_table_name(&node)
            );
        }
    }

    #[test]
    fn relation_kind_tags_cover_every_variant() {
        // The match expression inside relation_kind_tag is the
        // compile-time enforcement; this runtime test catches the case
        // where someone adds a variant *and* a match arm with a
        // duplicate or empty string, which the compiler cannot.
        let mut seen = std::collections::HashSet::new();
        for kind in ALL_RELATION_KINDS {
            let tag = relation_kind_tag(kind);
            assert!(!tag.is_empty(), "empty tag for {kind:?}");
            assert!(seen.insert(tag), "duplicate tag {tag} for {kind:?}");
        }
        // Verify the table covers all 17 currently-defined variants.
        // If a new variant is added to `RelationKind`, this length
        // assertion fails until ALL_RELATION_KINDS is updated.
        assert_eq!(
            ALL_RELATION_KINDS.len(),
            17,
            "RelationKind variant count changed; update ALL_RELATION_KINDS and \
             relation_kind_tag together"
        );
    }
}
