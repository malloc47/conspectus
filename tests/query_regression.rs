//! Query regression corpus (P9-007).
//!
//! Five named fixtures × a battery of canned queries, each
//! snapshotted as deterministic JSON so any schema or saved-view
//! drift surfaces as a snapshot diff on the next `cargo nextest`.
//!
//! Fixtures are programmatic so the assertions are independent of
//! on-disk state. Queries that walk multi-row results use explicit
//! `ORDER BY` clauses so insta sees a stable byte sequence.

use conspectus::model::{
    AgentSessionId, AgentSessionNode, BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence,
    ForgePrId, ForgePrNode, ForkId, ForkNode, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId, Provenance, RelationKind,
    RepoId, RepoNode, SourceMetadata, WorkspaceId, WorkspaceNode,
};
use conspectus::query::{OutputFormat, run_query_against_snapshot};

// =============================================================
// Fixture builders
// =============================================================

fn fixture_sparse_orphan_session() -> GraphSnapshot {
    // One agent session with no checkout and no mux. The fixture
    // exists to confirm orphan sessions still appear in
    // node_agent_sessions / v_sessions_with_repo (with NULL
    // checkout columns) and produce zero rows in v_mux_attachments.
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new("claude-code", "default", "orphan"),
        harness_key: "claude-code".into(),
        cwd: Some("/tmp/scratch".into()),
        title: Some("orphan-session".into()),
        last_message_preview: None,
        last_active_epoch: Some(1_700_000_000),
    }));
    snap
}

fn fixture_multi_checkout_repo() -> GraphSnapshot {
    // One repo with three checkouts at different roots. Tests that
    // each checkout appears in node_checkouts joined back to the
    // single shared repo_common_dir.
    let mut snap = GraphSnapshot::empty();
    let repo_common = "/r/.git";
    snap.nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_common))));
    for root in ["/r", "/r/sub", "/r/other"] {
        snap.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(RepoId::new(repo_common), root),
            root: root.into(),
            git_dir: None,
            current_branch: None,
        }));
    }
    snap
}

fn fixture_fork_ancestry_chain() -> GraphSnapshot {
    // Four-node ParentFork chain a -> b -> c -> d. v_fork_ancestry
    // should produce the closure for each fork including the root.
    let mut snap = GraphSnapshot::empty();
    for key in ["a", "b", "c", "d"] {
        snap.nodes.push(GraphNode::Fork(ForkNode {
            id: ForkId::new(key),
            provider: "atelier".into(),
            provider_source_key: key.into(),
            name: None,
            scope: None,
            capabilities: vec![],
        }));
    }
    for (child, parent, id) in [("b", "a", "L_ba"), ("c", "b", "L_cb"), ("d", "c", "L_dc")] {
        snap.candidate_links.push(GraphLink {
            id: id.into(),
            source: NodeId::Fork(ForkId::new(child)),
            target: LinkEndpoint::Node {
                id: NodeId::Fork(ForkId::new(parent)),
            },
            relation: RelationKind::ParentFork,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    snap
}

fn fixture_workspace_with_prs() -> GraphSnapshot {
    // One workspace, two repos contained, one branch per repo, one
    // PR per branch. Exercises the v_workspace_member_repos and
    // v_pr_by_branch joins together.
    let mut snap = GraphSnapshot::empty();
    snap.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new("/w"),
        root: "/w".into(),
        provider: Some("atelier".into()),
        name: Some("ws".into()),
    }));
    let workspace_id = NodeId::Workspace(WorkspaceId::new("/w"));

    for (idx, (repo_common, refname, pr_number)) in [
        ("/repo1/.git", "refs/heads/main", 1u64),
        ("/repo2/.git", "refs/heads/feature", 2u64),
    ]
    .into_iter()
    .enumerate()
    {
        snap.nodes
            .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_common))));
        let branch_id = BranchId::new(RepoId::new(repo_common), refname);
        snap.nodes.push(GraphNode::Branch(BranchNode {
            id: branch_id.clone(),
            refname: refname.into(),
            current_commit: None,
            upstream: None,
        }));
        let pr = ForgePrNode {
            id: ForgePrId::new(
                "github",
                "github.com",
                "owner",
                &repo_common[1..6],
                pr_number,
            ),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "owner".into(),
            repo: repo_common[1..6].into(),
            number: pr_number,
            state: Some("open".into()),
            url: Some(format!("https://github.com/owner/r/pull/{pr_number}")),
            updated_epoch: Some(1_700_000_000 + (pr_number as i64) * 60),
            is_draft: false,
        };
        snap.nodes.push(GraphNode::ForgePr(pr.clone()));

        snap.candidate_links.push(GraphLink {
            id: format!("L_wcr_{idx}"),
            source: workspace_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(RepoId::new(repo_common)),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snap.candidate_links.push(GraphLink {
            id: format!("L_bpr_{idx}"),
            source: NodeId::Branch(branch_id),
            target: LinkEndpoint::Node {
                id: NodeId::ForgePr(pr.id.clone()),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    snap
}

fn fixture_ambiguous_mux_candidates() -> GraphSnapshot {
    // One agent session, two mux sessions, two active linked_to_mux
    // candidate links. v_mux_attachments returns two rows for this
    // session — the ambiguity case the resolver has to break in
    // the in-Rust pipeline.
    let mut snap = GraphSnapshot::empty();
    let agent_id = AgentSessionId::new("claude-code", "default", "ambiguous");
    snap.nodes.push(GraphNode::AgentSession(AgentSessionNode {
        id: agent_id.clone(),
        harness_key: "claude-code".into(),
        cwd: None,
        title: None,
        last_message_preview: None,
        last_active_epoch: Some(1_700_000_000),
    }));
    let agent_node_id = NodeId::AgentSession(agent_id);

    for (native, link_id, provenance) in [
        ("tmux:0", "L_mux_0", Provenance::StrongDiscovered),
        ("tmux:1", "L_mux_1", Provenance::Discovered),
    ] {
        let mux_id = MuxSessionId::new(native);
        snap.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            native_id: native.into(),
            backend: "tmux".into(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snap.candidate_links.push(GraphLink {
            id: link_id.into(),
            source: agent_node_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
            relation: RelationKind::LinkedToMux,
            provenance,
            confidence: Confidence::Medium,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    snap
}

// =============================================================
// Canned query runner
// =============================================================

/// Run `sql` against `snapshot` in JSON format and return the output
/// verbatim. JSON ordering is deterministic (BTreeMap), so this is
/// safe to feed straight into insta.
fn json(snapshot: &GraphSnapshot, sql: &str) -> String {
    run_query_against_snapshot(snapshot, sql, OutputFormat::Json)
        .unwrap_or_else(|err| panic!("query failed: {sql}\n{err:#}"))
}

// =============================================================
// Snapshots, one test per (fixture, query) combination
// =============================================================

#[test]
fn sparse_orphan_session_node_counts() {
    let snap = fixture_sparse_orphan_session();
    insta::assert_snapshot!(
        "sparse_orphan_session_node_counts",
        json(
            &snap,
            "SELECT node_kind, COUNT(*) AS n FROM v_nodes GROUP BY node_kind ORDER BY node_kind"
        )
    );
}

#[test]
fn sparse_orphan_session_appears_in_v_sessions_with_repo_with_null_checkout() {
    let snap = fixture_sparse_orphan_session();
    insta::assert_snapshot!(
        "sparse_orphan_session_v_sessions_with_repo",
        json(
            &snap,
            "SELECT harness_key, session_key, cwd, checkout_node_id, repo_common_dir \
             FROM v_sessions_with_repo ORDER BY session_key"
        )
    );
}

#[test]
fn sparse_orphan_session_has_no_mux_attachments() {
    let snap = fixture_sparse_orphan_session();
    insta::assert_snapshot!(
        "sparse_orphan_session_no_mux",
        json(
            &snap,
            "SELECT COUNT(*) AS attachments FROM v_mux_attachments"
        )
    );
}

#[test]
fn multi_checkout_repo_has_three_checkouts_sharing_one_repo() {
    let snap = fixture_multi_checkout_repo();
    insta::assert_snapshot!(
        "multi_checkout_repo_checkouts",
        json(
            &snap,
            "SELECT repo_common_dir, root FROM node_checkouts ORDER BY root"
        )
    );
}

#[test]
fn multi_checkout_repo_node_counts() {
    let snap = fixture_multi_checkout_repo();
    insta::assert_snapshot!(
        "multi_checkout_repo_node_counts",
        json(
            &snap,
            "SELECT node_kind, COUNT(*) AS n FROM v_nodes GROUP BY node_kind ORDER BY node_kind"
        )
    );
}

#[test]
fn fork_ancestry_chain_full_closure() {
    let snap = fixture_fork_ancestry_chain();
    insta::assert_snapshot!(
        "fork_ancestry_chain_closure",
        json(
            &snap,
            "SELECT fork_node_id, ancestor_node_id, depth \
             FROM v_fork_ancestry \
             ORDER BY fork_node_id, depth"
        )
    );
}

#[test]
fn fork_ancestry_chain_root_has_only_self() {
    let snap = fixture_fork_ancestry_chain();
    insta::assert_snapshot!(
        "fork_ancestry_chain_root_only_self",
        json(
            &snap,
            "SELECT ancestor_node_id, depth FROM v_fork_ancestry \
             WHERE fork_node_id = 'fork:a' ORDER BY depth"
        )
    );
}

#[test]
fn workspace_with_prs_member_repos() {
    let snap = fixture_workspace_with_prs();
    insta::assert_snapshot!(
        "workspace_with_prs_member_repos",
        json(
            &snap,
            "SELECT workspace_root, repo_common_dir FROM v_workspace_member_repos \
             ORDER BY repo_common_dir"
        )
    );
}

#[test]
fn workspace_with_prs_pr_by_branch() {
    let snap = fixture_workspace_with_prs();
    insta::assert_snapshot!(
        "workspace_with_prs_pr_by_branch",
        json(
            &snap,
            "SELECT refname, pr_number, pr_state FROM v_pr_by_branch \
             ORDER BY pr_number"
        )
    );
}

#[test]
fn workspace_with_prs_branch_to_pr_join_via_candidate_links() {
    // Raw join over the tables rather than the saved view — exercises
    // the schema directly so a v_pr_by_branch refactor that drifts
    // from the underlying tables surfaces on either path. Joins are
    // structural via json_extract over the JSON endpoint columns
    // (ADR 0044).
    let snap = fixture_workspace_with_prs();
    insta::assert_snapshot!(
        "workspace_with_prs_raw_branch_pr_join",
        json(
            &snap,
            "SELECT b.refname, pr.number AS pr_number, pr.state AS pr_state \
             FROM node_branches b \
             JOIN candidate_links cl \
               ON cl.source_kind = 'branch' \
               AND json_extract(cl.source, '$.repo.common_dir') = b.repo_common_dir \
               AND json_extract(cl.source, '$.refname') = b.refname \
               AND cl.relation = 'branch_has_forge_pr' \
               AND cl.state = 'active' \
             JOIN node_forge_prs pr \
               ON cl.target_node_kind = 'forge_pr' \
               AND json_extract(cl.target_node, '$.provider') = pr.provider_name \
               AND json_extract(cl.target_node, '$.host') = pr.host \
               AND json_extract(cl.target_node, '$.owner') = pr.owner \
               AND json_extract(cl.target_node, '$.repo') = pr.repo \
               AND json_extract(cl.target_node, '$.number') = pr.number \
             ORDER BY pr.number"
        )
    );
}

#[test]
fn ambiguous_mux_candidates_returns_two_rows() {
    let snap = fixture_ambiguous_mux_candidates();
    insta::assert_snapshot!(
        "ambiguous_mux_candidates_rows",
        json(
            &snap,
            "SELECT native_id, provenance, confidence \
             FROM v_mux_attachments ORDER BY native_id"
        )
    );
}

#[test]
fn ambiguous_mux_candidates_candidate_links_table_directly() {
    let snap = fixture_ambiguous_mux_candidates();
    insta::assert_snapshot!(
        "ambiguous_mux_candidates_links_table",
        json(
            &snap,
            "SELECT link_id, target_node_kind, \
                    json_extract(target_node, '$.native_id') AS target_native_id, \
                    provenance, confidence, state \
             FROM candidate_links WHERE relation = 'linked_to_mux' ORDER BY link_id"
        )
    );
}

// =============================================================
// Cross-cutting: every saved view is queryable on every fixture
// =============================================================

/// Smoke that catches the case where a saved view stops being
/// selectable against a particular fixture shape (e.g. a recursive
/// CTE that diverges, a JOIN that fails type coercion). Each
/// (fixture, view) pair runs SELECT COUNT(*); the snapshot pins the
/// expected row count so a join-shape regression shows up here.
#[test]
fn every_saved_view_returns_a_stable_row_count_per_fixture() {
    let fixtures: &[(&str, GraphSnapshot)] = &[
        ("sparse_orphan_session", fixture_sparse_orphan_session()),
        ("multi_checkout_repo", fixture_multi_checkout_repo()),
        ("fork_ancestry_chain", fixture_fork_ancestry_chain()),
        ("workspace_with_prs", fixture_workspace_with_prs()),
        (
            "ambiguous_mux_candidates",
            fixture_ambiguous_mux_candidates(),
        ),
    ];
    let mut report = String::new();
    for (fixture_name, snap) in fixtures {
        for view in conspectus::query::SAVED_VIEWS {
            let sql = format!("SELECT COUNT(*) AS n FROM {}", view.name);
            let out = json(snap, &sql);
            // `out` is one JSON line like `{"n":3}\n`; strip the
            // trailing newline so the report stays tight.
            let trimmed = out.trim_end();
            report.push_str(&format!("{fixture_name}.{} -> {trimmed}\n", view.name));
        }
    }
    insta::assert_snapshot!("every_saved_view_row_counts_per_fixture", report);
}
