use super::*;
use crate::filter::PrBranchFilter;
use crate::model::{
    BranchId, BranchNode, CheckoutNode, Confidence, Freshness, GraphLink, Provenance, RepoNode,
    SourceMetadata,
};
use crate::resolve::resolve_snapshot;

/// A repo at `/r` with a checkout on `feature`, PR #1 from `feature`,
/// and merged PR #2 whose branch isn't local.
fn world() -> GraphSnapshot {
    let mut snapshot = sparse_world();
    // As forge discovery leaves it: the PR whose branch isn't local
    // keeps an unresolved link naming the local repo it was queried for.
    let mut metadata = crate::model::Metadata::new();
    metadata.insert(
        "local_repo".to_string(),
        serde_json::Value::String("/r/.git".to_string()),
    );
    snapshot.candidate_links.push(GraphLink {
        id: "pr-no-branch".to_string(),
        source: NodeId::ForgePr(ForgePrId::new("github", "github.com", "o", "r", 2)),
        target: LinkEndpoint::Unresolved {
            evidence: crate::model::UnresolvedEndpoint {
                node_type: "branch".to_string(),
                harness_key: None,
                native_id: Some("gone".to_string()),
                state_scope: None,
                path: None,
                metadata,
            },
        },
        relation: RelationKind::BranchHasForgePr,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    resolve_snapshot(snapshot)
}

/// [`world`] without the evidence that names PR #2's local repo, as a
/// graph from a declared or older source would be.
fn sparse_world() -> GraphSnapshot {
    let repo = RepoId::new("/r/.git");
    let checkout = CheckoutId::new(repo.clone(), "/r");
    let branch = BranchId::new(repo.clone(), "refs/heads/feature");
    let mut snapshot = GraphSnapshot::empty();
    let mut repo_node = RepoNode::new(repo);
    repo_node.common_dir = "/r/.git".to_string();
    snapshot.nodes.push(GraphNode::Repo(repo_node));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: checkout.clone(),
        root: "/r".to_string(),
        git_dir: None,
        current_branch: Some(branch.clone()),
        worktree: None,
    }));
    snapshot.nodes.push(GraphNode::Branch(BranchNode {
        id: branch.clone(),
        refname: "refs/heads/feature".to_string(),
        current_commit: None,
        upstream: None,
    }));
    for (number, state) in [(1, "OPEN"), (2, "MERGED")] {
        snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "o", "r", number),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            number,
            state: Some(state.to_string()),
            url: None,
            updated_epoch: None,
            is_draft: false,
            head_ref: None,
        }));
    }
    let link = |id: &str, source: NodeId, target: NodeId, relation| GraphLink {
        id: id.to_string(),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    };
    snapshot.candidate_links.push(link(
        "checked-out",
        NodeId::Checkout(checkout),
        NodeId::Branch(branch.clone()),
        RelationKind::CheckedOutBranch,
    ));
    snapshot.candidate_links.push(link(
        "pr-branch",
        NodeId::ForgePr(ForgePrId::new("github", "github.com", "o", "r", 1)),
        NodeId::Branch(branch),
        RelationKind::BranchHasForgePr,
    ));
    snapshot
}

fn build(snapshot: &GraphSnapshot, grouping: PrsGrouping, cwd: Option<&Path>) -> RowTree {
    build_prs_tree(PrsBuildInputs {
        snapshot,
        home: None,
        grouping,
        filter: RowFilter::default(),
        now: None,
        cwd,
    })
}

/// Each row as `depth label`: groups by display path, PRs by number.
fn outline(tree: &RowTree) -> Vec<String> {
    tree.rows
        .iter()
        .map(|row| {
            let label = match &row.kind {
                RowKind::Group(group) => group.display_path.clone(),
                RowKind::Pr(pr) => format!("#{}", pr.pr_number),
                other => format!("{other:?}"),
            };
            format!("{} {label}", row.depth)
        })
        .collect()
}

#[test]
fn checkout_grouping_files_each_pr_under_the_checkout_of_its_branch() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Checkout, None);
    assert_eq!(outline(&tree), ["0 /r", "1 #1", "0 No checkout", "1 #2"]);
    assert_eq!(
        tree.rows[0].id,
        RowId::Group(NodeId::Checkout(CheckoutId::new(
            RepoId::new("/r/.git"),
            "/r"
        )))
    );
}

#[test]
fn checkout_grouping_marks_the_launch_checkout() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Checkout, Some(Path::new("/r/src")));
    let launch: Vec<bool> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Group(group) => Some(group.is_launch_context),
            _ => None,
        })
        .collect();
    assert_eq!(launch, [true, false]);
}

#[test]
fn repo_grouping_files_every_pr_under_its_local_repo() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    // The PR without a local branch sits beside the one with, not in a
    // catch-all group. Newest number first within a group.
    assert_eq!(outline(&tree), ["0 /r", "1 #2", "1 #1"]);
}

#[test]
fn repo_grouping_falls_back_to_the_forge_repo_when_the_graph_is_sparse() {
    let snapshot = resolve_snapshot(sparse_world());
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    assert_eq!(outline(&tree), ["0 /r", "1 #1", "0 o/r", "1 #2"]);
    assert_eq!(tree.rows[2].id, RowId::Remote("o/r".to_string()));
}

fn tracked_by_number(tree: &RowTree) -> Vec<(u64, bool)> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Pr(pr) => Some((pr.pr_number, pr.tracked)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_pr_is_tracked_when_it_resolves_to_a_local_branch() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    assert_eq!(tracked_by_number(&tree), [(2, false), (1, true)]);
}

fn build_with(snapshot: &GraphSnapshot, filter: RowFilter) -> RowTree {
    build_prs_tree(PrsBuildInputs {
        snapshot,
        home: None,
        grouping: PrsGrouping::Repo,
        filter,
        now: None,
        cwd: None,
    })
}

#[test]
fn the_branch_filter_includes_or_excludes_by_tracking() {
    let snapshot = world();
    let only = |pr_branch| {
        let filter = RowFilter {
            pr_branch: Some(pr_branch),
            ..RowFilter::default()
        };
        tracked_by_number(&build_with(&snapshot, filter))
    };
    assert_eq!(only(PrBranchFilter::Tracked), [(1, true)]);
    assert_eq!(only(PrBranchFilter::Untracked), [(2, false)]);
    // The default keeps every PR, with or without sessions attached.
    assert_eq!(
        tracked_by_number(&build_with(&snapshot, RowFilter::default())).len(),
        2
    );
}

#[test]
fn a_filter_that_leaves_a_group_empty_drops_its_header() {
    let snapshot = resolve_snapshot(sparse_world());
    let filter = RowFilter {
        pr_branch: Some(PrBranchFilter::Untracked),
        ..RowFilter::default()
    };
    let tree = build_with(&snapshot, filter);
    assert_eq!(outline(&tree), ["0 o/r", "1 #2"]);
}

#[test]
fn floating_tracked_prs_lifts_them_above_the_rest_within_a_group() {
    let snapshot = world();
    let floated = RowFilter {
        float_tracked_prs_top: true,
        ..RowFilter::default()
    };
    assert_eq!(
        tracked_by_number(&build_with(&snapshot, floated)),
        [(1, true), (2, false)]
    );
    assert_eq!(
        tracked_by_number(&build_with(&snapshot, RowFilter::default())),
        [(2, false), (1, true)]
    );
}

#[test]
fn state_grouping_files_prs_by_state() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::State, None);
    assert_eq!(outline(&tree), ["0 Open", "1 #1", "0 Merged", "1 #2"]);
}
