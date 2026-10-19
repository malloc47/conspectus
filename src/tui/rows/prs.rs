//! In-memory PRs-view row-tree builder (ADR 0082), grouped by
//! checkout, repo, or state (ADR 0114, 0130).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::filter::RowFilter;
use crate::model::{
    CheckoutId, ForgePrId, ForgePrNode, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, NodeId,
    RelationKind, RepoId, path_is_ancestor_of,
};
use crate::output::render::{node_short_id_from_display, strip_branch_prefix, unique_prefix_len};
use crate::tui::PrsGrouping;
use crate::tui::rows::{
    GroupRow, PrRow, Row, RowId, RowKind, RowTree, ViewLabel, format_recency, shorten_home,
};

pub struct PrsBuildInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub grouping: PrsGrouping,
    pub filter: RowFilter,
    pub now: Option<i64>,
    /// Launch directory; the checkout containing it is the launch
    /// context, so its group opens expanded.
    pub cwd: Option<&'a Path>,
}

#[derive(Clone, Debug)]
struct PrData<'a> {
    node_id: String,
    id: ForgePrId,
    node: &'a ForgePrNode,
}

#[derive(Clone, Debug)]
struct BranchLink {
    branch_node_id: String,
    repo: RepoId,
    refname: String,
}

/// The group a PR sits under (ADR 0114). Ordered so groups sort by
/// display order: checkouts by path, then repos, then forge repos with
/// no local repo.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Checkout {
        root: String,
        id: CheckoutId,
    },
    /// A repo's PRs that are checked out nowhere, filed under the repo
    /// instead of one catch-all group (ADR 0130).
    Unchecked {
        path: String,
        id: RepoId,
    },
    Repo {
        path: String,
        id: RepoId,
    },
    /// A forge repo (`owner/repo`) with no local repo known for its
    /// PRs, so the graph is too sparse to file them anywhere else.
    Remote(String),
    State(StateBucket),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum StateBucket {
    Open,
    Draft,
    Merged,
    Closed,
    Other,
}

impl StateBucket {
    fn of(pr: &ForgePrNode) -> Self {
        let state = pr.state.as_deref().unwrap_or_default().to_ascii_lowercase();
        match state.as_str() {
            "open" if pr.is_draft => Self::Draft,
            "open" => Self::Open,
            "merged" => Self::Merged,
            "closed" => Self::Closed,
            _ => Self::Other,
        }
    }

    fn row(self) -> (&'static str, &'static str) {
        match self {
            Self::Open => ("prs-open", "Open"),
            Self::Draft => ("prs-draft", "Draft"),
            Self::Merged => ("prs-merged", "Merged"),
            Self::Closed => ("prs-closed", "Closed"),
            Self::Other => ("prs-other", "Other"),
        }
    }
}

pub fn build_prs_tree(inputs: PrsBuildInputs<'_>) -> RowTree {
    let snapshot = inputs.snapshot;
    let filter = &inputs.filter;
    let now = inputs.now;

    let prs = collect_prs(snapshot);
    let agents = collect_agents(snapshot);
    let candidate_counts = crate::model::SnapshotIndex::new(snapshot)
        .agent_mux_candidate_counts()
        .clone();
    let branches = collect_preferred_branch_per_pr(snapshot);
    let local_repos = collect_local_repo_per_pr(snapshot);
    let checkouts = collect_checkouts_per_branch(snapshot);
    let repo_paths = collect_repo_paths(snapshot);

    // Floating tracked PRs is a stable partition, so each group keeps
    // the owner, repo, and number order `collect_prs` gave it.
    let mut prs = prs;
    if filter.float_tracked_prs_top {
        prs.sort_by_key(|pr| !branches.contains_key(&pr.node_id));
    }

    let mut node_ids: Vec<String> = prs.iter().map(|pr| pr.node_id.clone()).collect();
    node_ids.extend(agents.iter().map(|agent| agent.node_id.clone()));
    let full_ids: Vec<String> = node_ids
        .iter()
        .map(|id| node_short_id_from_display(id))
        .collect();
    let id_len = unique_prefix_len(&full_ids);
    let short_ids: HashMap<&str, String> = node_ids
        .iter()
        .zip(full_ids.iter())
        .map(|(node_id, full)| (node_id.as_str(), full[..id_len].to_string()))
        .collect();

    // Each PR's rows (the PR, then the sessions working in its
    // checkouts), filed under its bucket.
    let mut buckets: BTreeMap<Bucket, Vec<Row>> = BTreeMap::new();
    for pr in &prs {
        let branch = branches.get(&pr.node_id);
        if let Some(wanted) = filter.pr_branch
            && !wanted.admits(branch.is_some())
        {
            continue;
        }
        let pr_checkouts: &[(String, CheckoutId)] = branch
            .and_then(|branch| checkouts.get(&branch.branch_node_id))
            .map_or(&[], Vec::as_slice);
        let roots: Vec<String> = pr_checkouts.iter().map(|(root, _)| root.clone()).collect();
        let visible_agents: Vec<&super::AgentData<'_>> = agents
            .iter()
            .filter(|agent| agent_attached_to_roots(agent, &roots))
            .filter(|agent| super::session_matches_filter(agent, &candidate_counts, now, filter))
            .collect();
        if filter.has_narrowing_predicates() && visible_agents.is_empty() {
            continue;
        }

        // The local repo a PR belongs to: its branch's, else the one it
        // was queried for (ADR 0129).
        let repo_group = || {
            let repo = branch
                .map(|branch| &branch.repo)
                .or_else(|| local_repos.get(&pr.node_id))?;
            let path = repo_paths
                .get(repo)
                .cloned()
                .unwrap_or_else(|| repo.common_dir.clone());
            Some((path, repo.clone()))
        };
        let remote_group = || Bucket::Remote(format!("{}/{}", pr.node.owner, pr.node.repo));
        let bucket = match inputs.grouping {
            PrsGrouping::Checkout => match pr_checkouts.first() {
                Some((root, id)) => Bucket::Checkout {
                    root: root.clone(),
                    id: id.clone(),
                },
                None => repo_group()
                    .map_or_else(remote_group, |(path, id)| Bucket::Unchecked { path, id }),
            },
            PrsGrouping::Repo => {
                repo_group().map_or_else(remote_group, |(path, id)| Bucket::Repo { path, id })
            }
            PrsGrouping::State => Bucket::State(StateBucket::of(pr.node)),
        };

        let rows = buckets.entry(bucket).or_default();
        let node_id = NodeId::ForgePr(pr.id.clone());
        rows.push(Row {
            id: RowId::Pr(node_id.clone()),
            depth: 1,
            expandable: !visible_agents.is_empty(),
            kind: RowKind::Pr(PrRow {
                pr_number: pr.node.number,
                repo_display: format!("{}/{}#{}", pr.node.owner, pr.node.repo, pr.node.number),
                state: pr.node.state.clone(),
                is_draft: pr.node.is_draft,
                branch_name: branch
                    .map(|branch| branch.refname.as_str())
                    .map(strip_branch_prefix)
                    .map(str::to_string),
                tracked: branch.is_some(),
                updated_recency: format_recency(now, pr.node.updated_epoch),
                attached_count: visible_agents.len(),
                url: pr.node.url.clone(),
                primary_node: node_id,
            }),
        });
        for agent in visible_agents {
            rows.push(super::agent_row(
                agent,
                2,
                &candidate_counts,
                short_ids
                    .get(agent.node_id.as_str())
                    .cloned()
                    .unwrap_or_default(),
                inputs.home,
                now,
            ));
        }
    }

    let launch_root = launch_checkout_root(buckets.keys(), inputs.cwd);
    let mut tree = RowTree {
        view: ViewLabel::Prs,
        ..RowTree::default()
    };
    for (bucket, rows) in buckets {
        tree.rows
            .push(group_row(&bucket, launch_root.as_deref(), inputs.home));
        tree.rows.extend(rows);
    }
    tree
}

/// The root of the checkout group the launch directory sits in, the
/// deepest when checkouts nest.
fn launch_checkout_root<'b>(
    buckets: impl Iterator<Item = &'b Bucket>,
    cwd: Option<&Path>,
) -> Option<String> {
    let cwd = cwd?;
    buckets
        .filter_map(|bucket| match bucket {
            Bucket::Checkout { root, .. } if path_is_ancestor_of(Path::new(root), cwd) => {
                Some(root.clone())
            }
            _ => None,
        })
        .max_by_key(String::len)
}

fn group_row(bucket: &Bucket, launch_root: Option<&str>, home: Option<&Path>) -> Row {
    let (id, display_path, primary_node, is_launch_context) = match bucket {
        Bucket::Checkout { root, id } => {
            let node = NodeId::Checkout(id.clone());
            (
                RowId::Group(node.clone()),
                shorten_home(root, home),
                Some(node),
                launch_root == Some(root.as_str()),
            )
        }
        Bucket::Repo { path, id } => {
            let node = NodeId::Repo(id.clone());
            (
                RowId::Group(node.clone()),
                shorten_home(path, home),
                Some(node),
                false,
            )
        }
        Bucket::Unchecked { path, id } => {
            let node = NodeId::Repo(id.clone());
            (
                RowId::Group(node.clone()),
                format!("{}  no checkout", shorten_home(path, home)),
                Some(node),
                false,
            )
        }
        Bucket::Remote(slug) => (RowId::Remote(slug.clone()), slug.clone(), None, false),
        Bucket::State(state) => {
            let (id, label) = state.row();
            (RowId::Synthetic(id), label.to_string(), None, false)
        }
    };
    Row {
        id,
        depth: 0,
        expandable: true,
        kind: RowKind::Group(GroupRow {
            display_path,
            primary_node,
            is_launch_context,
        }),
    }
}

fn agent_attached_to_roots(agent: &super::AgentData<'_>, roots: &[String]) -> bool {
    let Some(cwd) = agent.node.cwd.as_deref() else {
        return false;
    };
    let cwd = Path::new(cwd);
    roots
        .iter()
        .any(|root| path_is_ancestor_of(Path::new(root), cwd))
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_prs(snapshot: &GraphSnapshot) -> Vec<PrData<'_>> {
    let mut prs: Vec<PrData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(PrData {
                node_id: NodeId::ForgePr(pr.id.clone()).to_string(),
                id: pr.id.clone(),
                node: pr,
            }),
            _ => None,
        })
        .collect();
    prs.sort_by(|a, b| {
        a.node
            .owner
            .cmp(&b.node.owner)
            .then_with(|| a.node.repo.cmp(&b.node.repo))
            .then_with(|| b.node.number.cmp(&a.node.number))
    });
    prs
}

fn collect_agents(snapshot: &GraphSnapshot) -> Vec<super::AgentData<'_>> {
    let mut agents: Vec<super::AgentData<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) if agent.cwd.is_some() => {
                let id = agent.id.clone();
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(id.clone()))
                    .map(std::string::ToString::to_string);
                Some(super::AgentData {
                    node_id: NodeId::AgentSession(id.clone()).to_string(),
                    id,
                    node: agent,
                    alias,
                })
            }
            _ => None,
        })
        .collect();
    agents.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    agents
}

/// The preferred branch per PR comes from the
/// resolver's `branch_has_forge_pr` winner. Production
/// discovery emits these with source=ForgePr, target=Branch.
fn collect_preferred_branch_per_pr(snapshot: &GraphSnapshot) -> HashMap<String, BranchLink> {
    let selected_link_ids: HashSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::BranchHasForgePr))
        .filter_map(|r| r.selected_link_id.as_deref())
        .collect();
    let mut out = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::BranchHasForgePr) {
            continue;
        }
        let NodeId::ForgePr(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &link.target
        else {
            continue;
        };
        if !selected_link_ids.contains(link.id.as_str()) {
            continue;
        }
        let branch_node_id = NodeId::Branch(branch_id.clone()).to_string();
        out.entry(link.source.to_string())
            .or_insert_with(|| BranchLink {
                branch_node_id,
                repo: branch_id.repo.clone(),
                refname: branch_id.refname.clone(),
            });
    }
    out
}

/// The local repo an untracked PR was queried for, from the unresolved
/// branch link's evidence (ADR 0129). A PR seen from several local
/// clones gets the first by path, so its row is stable.
fn collect_local_repo_per_pr(snapshot: &GraphSnapshot) -> HashMap<String, RepoId> {
    let mut out: HashMap<String, RepoId> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active)
            || !matches!(link.relation, RelationKind::BranchHasForgePr)
        {
            continue;
        }
        let NodeId::ForgePr(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Unresolved { evidence } = &link.target else {
            continue;
        };
        let Some(local) = evidence
            .metadata
            .get("local_repo")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let repo = RepoId::new(local);
        out.entry(link.source.to_string())
            .and_modify(|known| {
                if repo.common_dir < known.common_dir {
                    *known = repo.clone();
                }
            })
            .or_insert(repo);
    }
    out
}

/// Each branch's checkouts, `(root, id)` sorted by root.
fn collect_checkouts_per_branch(
    snapshot: &GraphSnapshot,
) -> HashMap<String, Vec<(String, CheckoutId)>> {
    let checkout_roots: HashMap<NodeId, String> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Checkout(checkout) => {
                Some((NodeId::Checkout(checkout.id.clone()), checkout.root.clone()))
            }
            _ => None,
        })
        .collect();
    let mut sorted: Vec<(String, String, CheckoutId)> = Vec::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::CheckedOutBranch) {
            continue;
        }
        let NodeId::Checkout(checkout_id) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &link.target
        else {
            continue;
        };
        let Some(root) = checkout_roots.get(&link.source) else {
            continue;
        };
        let branch_node_id = NodeId::Branch(branch_id.clone()).to_string();
        sorted.push((branch_node_id, root.clone(), checkout_id.clone()));
    }
    sorted.sort();
    sorted.dedup();
    let mut out: HashMap<String, Vec<(String, CheckoutId)>> = HashMap::new();
    for (branch_node_id, root, id) in sorted {
        out.entry(branch_node_id).or_default().push((root, id));
    }
    out
}

/// Each repo's display path: its working tree, so `~/src/project`
/// rather than `~/src/project/.git`.
fn collect_repo_paths(snapshot: &GraphSnapshot) -> HashMap<RepoId, String> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Repo(repo) => Some((
                repo.id.clone(),
                repo.common_dir
                    .strip_suffix("/.git")
                    .unwrap_or(&repo.common_dir)
                    .to_string(),
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "prs_tests.rs"]
mod tests;
