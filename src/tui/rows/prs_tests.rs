use super::*;
use crate::model::{
    BranchId, BranchNode, CheckoutNode, Confidence, Freshness, GraphLink, Provenance, RepoNode,
    SourceMetadata,
};
use crate::resolve::resolve_snapshot;

/// A repo at `/r` with a checkout on `feature`, PR #1 from `feature`,
/// and merged PR #2 whose branch isn't local.
fn world() -> GraphSnapshot {
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
    resolve_snapshot(snapshot)
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
fn repo_grouping_files_each_pr_under_its_branchs_repo() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    assert_eq!(outline(&tree), ["0 /r", "1 #1", "0 No local repo", "1 #2"]);
}

#[test]
fn state_grouping_files_prs_by_state() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::State, None);
    assert_eq!(outline(&tree), ["0 Open", "1 #1", "0 Merged", "1 #2"]);
}
