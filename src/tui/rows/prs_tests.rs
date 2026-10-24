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
        head_commit: None,
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
            title: None,
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
    // The PR checked out nowhere sits under its repo, not a catch-all.
    assert_eq!(
        outline(&tree),
        ["0 o/r  /r", "1 #1", "0 o/r  /r  no checkout", "1 #2"]
    );
    assert_eq!(
        tree.rows[0].id,
        RowId::Group(NodeId::Checkout(CheckoutId::new(
            RepoId::new("/r/.git"),
            "/r"
        )))
    );
}

#[test]
fn checkout_grouping_files_unchecked_prs_under_their_repo_node() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::Checkout, None);
    assert_eq!(
        tree.rows[2].id,
        RowId::Group(NodeId::Repo(RepoId::new("/r/.git")))
    );
    match &tree.rows[2].kind {
        RowKind::Group(group) => assert_eq!(
            group.primary_node,
            Some(NodeId::Repo(RepoId::new("/r/.git")))
        ),
        other => panic!("expected a group, got {other:?}"),
    }
}

#[test]
fn checkout_grouping_falls_back_to_the_forge_repo_when_the_graph_is_sparse() {
    let snapshot = resolve_snapshot(sparse_world());
    let tree = build(&snapshot, PrsGrouping::Checkout, None);
    assert_eq!(outline(&tree), ["0 o/r  /r", "1 #1", "0 o/r", "1 #2"]);
}

#[test]
fn checkout_grouping_never_emits_a_no_checkout_group() {
    for snapshot in [world(), resolve_snapshot(sparse_world())] {
        let tree = build(&snapshot, PrsGrouping::Checkout, None);
        assert!(
            !outline(&tree).iter().any(|row| row.contains("No checkout")),
            "{:?}",
            outline(&tree)
        );
    }
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
    assert_eq!(outline(&tree), ["0 o/r  /r", "1 #2", "1 #1"]);
}

#[test]
fn repo_grouping_falls_back_to_the_forge_repo_when_the_graph_is_sparse() {
    let snapshot = resolve_snapshot(sparse_world());
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    assert_eq!(outline(&tree), ["0 o/r  /r", "1 #1", "0 o/r", "1 #2"]);
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

fn labels(tree: &RowTree) -> Vec<String> {
    tree.rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Pr(pr) => Some(pr.label.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_group_of_one_forge_repo_names_it_and_its_rows_show_only_the_number() {
    let snapshot = world();
    for grouping in [PrsGrouping::Checkout, PrsGrouping::Repo] {
        let tree = build(&snapshot, grouping, None);
        let mut seen = 0;
        for row in &tree.rows {
            if let RowKind::Pr(pr) = &row.kind {
                seen += 1;
                assert_eq!(pr.label, format!("#{}", pr.pr_number));
                // Search still matches the full name.
                assert_eq!(pr.repo_display, format!("o/r#{}", pr.pr_number));
            }
        }
        assert_eq!(seen, 2);
    }
}

#[test]
fn state_groups_span_repos_so_their_rows_keep_owner_and_repo() {
    let snapshot = world();
    let tree = build(&snapshot, PrsGrouping::State, None);
    assert_eq!(labels(&tree), ["o/r#1", "o/r#2"]);
}

#[test]
fn a_group_with_several_forge_repos_keeps_owner_and_repo_on_its_rows() {
    // A fork's PRs into its upstream sit in the same local repo group.
    let mut snapshot = sparse_world();
    snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", "up", "r", 9),
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: "up".to_string(),
        repo: "r".to_string(),
        number: 9,
        state: Some("OPEN".to_string()),
        title: None,
        url: None,
        updated_epoch: None,
        is_draft: false,
        head_ref: None,
    }));
    snapshot.candidate_links.push(GraphLink {
        id: "pr-up".to_string(),
        source: NodeId::ForgePr(ForgePrId::new("github", "github.com", "up", "r", 9)),
        target: LinkEndpoint::Node {
            id: NodeId::Branch(BranchId::new(RepoId::new("/r/.git"), "refs/heads/feature")),
        },
        relation: RelationKind::BranchHasForgePr,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    let tree = build(&snapshot, PrsGrouping::Repo, None);

    let group = match &tree.rows[0].kind {
        RowKind::Group(group) => group.display_path.clone(),
        other => panic!("expected a group, got {other:?}"),
    };
    assert_eq!(group, "/r", "no single forge repo to name the group after");
    let mixed: Vec<String> = tree.rows[1..]
        .iter()
        .take_while(|row| !matches!(row.kind, RowKind::Group(_)))
        .filter_map(|row| match &row.kind {
            RowKind::Pr(pr) => Some(pr.label.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(mixed, ["o/r#1", "up/r#9"]);
}

#[test]
fn rows_carry_the_title_and_no_attached_count() {
    let mut snapshot = world();
    for node in &mut snapshot.nodes {
        if let GraphNode::ForgePr(pr) = node
            && pr.number == 1
        {
            pr.title = Some("Fix the login redirect".to_string());
        }
    }
    let tree = build(&snapshot, PrsGrouping::Repo, None);
    let titles: Vec<(u64, Option<String>)> = tree
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::Pr(pr) => Some((pr.pr_number, pr.title.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        titles,
        [(2, None), (1, Some("Fix the login redirect".to_string()))]
    );
}

fn world_with_states() -> GraphSnapshot {
    // #1 open (tracked), #2 merged, plus #3 open draft, #4 closed.
    let mut snapshot = world();
    for (number, state, draft) in [(3, "OPEN", true), (4, "CLOSED", false)] {
        snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "o", "r", number),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "o".to_string(),
            repo: "r".to_string(),
            number,
            state: Some(state.to_string()),
            title: None,
            url: None,
            updated_epoch: None,
            is_draft: draft,
            head_ref: None,
        }));
    }
    snapshot
}

fn numbers_with_state(
    snapshot: &GraphSnapshot,
    grouping: PrsGrouping,
    keys: &[crate::filter::PrStateKey],
) -> Vec<u64> {
    let filter = RowFilter {
        pr_state: Some(crate::filter::PrStateFilter::from_values(
            keys.iter().copied(),
        )),
        ..RowFilter::default()
    };
    build_prs_tree(PrsBuildInputs {
        snapshot,
        home: None,
        grouping,
        filter,
        now: None,
        cwd: None,
    })
    .rows
    .iter()
    .filter_map(|row| match &row.kind {
        RowKind::Pr(pr) => Some(pr.pr_number),
        _ => None,
    })
    .collect()
}

#[test]
fn the_state_filter_keeps_only_prs_in_the_chosen_states_under_every_grouping() {
    use crate::filter::PrStateKey::{Closed, Draft, Merged, Open};
    let snapshot = world_with_states();
    for grouping in [PrsGrouping::Checkout, PrsGrouping::Repo, PrsGrouping::State] {
        let mut open = numbers_with_state(&snapshot, grouping, &[Open]);
        open.sort_unstable();
        assert_eq!(open, [1, 3], "open includes the draft, {grouping:?}");
        assert_eq!(numbers_with_state(&snapshot, grouping, &[Draft]), [3]);
        let mut done = numbers_with_state(&snapshot, grouping, &[Merged, Closed]);
        done.sort_unstable();
        assert_eq!(done, [2, 4]);
    }
}

#[test]
fn the_state_filter_drops_groups_it_empties_and_the_default_keeps_every_pr() {
    use crate::filter::PrStateKey::Merged;
    let snapshot = world_with_states();
    let filter = RowFilter {
        pr_state: Some(crate::filter::PrStateFilter::from_values([Merged])),
        ..RowFilter::default()
    };
    let tree = build_prs_tree(PrsBuildInputs {
        snapshot: &snapshot,
        home: None,
        grouping: PrsGrouping::State,
        filter,
        now: None,
        cwd: None,
    });
    assert_eq!(outline(&tree), ["0 Merged", "1 #2"]);
    assert_eq!(
        build(&snapshot, PrsGrouping::State, None)
            .rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::Pr(_)))
            .count(),
        4
    );
}
