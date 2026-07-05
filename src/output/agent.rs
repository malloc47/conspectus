//! In-memory agent projection renderer (P11-011b / ADR 0082).
//!
//! Cells from ADR 0006:
//! `id` `agent` `cwd` `mux` `mux-conf` `pr` `pr-conf` `lineage`
//! `workspace` `checkout` `branch` `repo` `fork` `declared`
//! `preview` `title` `activity`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use super::render::{
    self, RenderOptions, SESSIONS_COLUMNS, current_epoch, format_relative_age, header_label,
    strip_branch_prefix,
};
use super::table::{agent_session_key_for_label, short_session_id};
use crate::filter::{MuxStateKey, SessionMatchInputs};
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutNode, ForgePrNode, ForkNode, GraphLink, GraphNode,
    GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance, RelationKind,
    WorkspaceNode, path_is_ancestor_of, pick_preferred,
};

type SessionKey = (String, String, String);

fn session_key_of(agent: &AgentSessionId) -> SessionKey {
    (
        agent.harness_key.clone(),
        agent.state_scope.clone(),
        agent.session_key.clone(),
    )
}

#[derive(Debug, Clone)]
struct SessionRow<'a> {
    agent: &'a AgentSessionNode,
    alias_display_name: Option<String>,
    /// Display-form `NodeId` of the deepest checkout whose root
    /// contains the session's cwd. `None` when the session has no
    /// cwd or no matching checkout.
    checkout_node_id: Option<NodeId>,
    checkout_root: Option<String>,
    repo_common_dir: Option<String>,
}

impl SessionRow<'_> {
    fn key(&self) -> SessionKey {
        session_key_of(&self.agent.id)
    }
}

#[derive(Debug, Clone)]
struct MuxInfo {
    backend: String,
    native_id: String,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

#[derive(Debug, Clone)]
struct PrInfo {
    owner: String,
    repo: String,
    number: u64,
    state: Option<String>,
    is_draft: bool,
    provenance: String,
    confidence: String,
    candidate_count: usize,
}

#[derive(Debug, Clone)]
struct LineageInfo {
    label: String,
    has_grandparent: bool,
}

#[derive(Debug, Clone)]
struct DeclaredInfo {
    state_label: String,
}

#[derive(Debug, Clone)]
struct ForkInfo {
    label: String,
}

// -----------------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------------

pub fn build_agent_rows_from_snapshot(
    snapshot: &GraphSnapshot,
    columns: &[&'static str],
    options: &RenderOptions,
) -> Vec<Vec<String>> {
    let sessions = collect_session_rows(snapshot);
    let mux_lookup = collect_mux_lookup(snapshot);
    let branch_lookup = collect_branch_lookup(snapshot);
    let lineage_lookup = collect_lineage_lookup(snapshot);
    let workspace_lookup = collect_workspace_lookup(snapshot);
    let fork_lookup = collect_fork_lookup(snapshot);
    let declared_lookup = collect_declared_lookup(snapshot);
    let pr_global = collect_global_pr(snapshot);

    let filtered: Vec<&SessionRow<'_>> = sessions
        .iter()
        .filter(|row| row_matches(options, row, &mux_lookup))
        .collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        columns
            .iter()
            .map(|key| header_label(SESSIONS_COLUMNS, key))
            .collect(),
    );

    for row in filtered {
        let branch_refname = row
            .checkout_node_id
            .as_ref()
            .and_then(|id| branch_lookup.get(id));
        let ctx = CellCtx {
            row,
            mux: mux_lookup.get(&row.key()),
            branch_refname,
            lineage: lineage_lookup.get(&row.key()),
            workspace: workspace_lookup.get(&row.key()),
            fork: fork_lookup.get(&row.key()),
            declared: declared_lookup.get(&row.key()),
            pr: pr_global.as_ref().filter(|_| row.agent.cwd.is_some()),
        };
        rows.push(columns.iter().map(|key| cell(key, &ctx)).collect());
    }

    rows
}

struct CellCtx<'a> {
    row: &'a SessionRow<'a>,
    mux: Option<&'a MuxInfo>,
    branch_refname: Option<&'a String>,
    lineage: Option<&'a LineageInfo>,
    workspace: Option<&'a String>,
    fork: Option<&'a ForkInfo>,
    declared: Option<&'a DeclaredInfo>,
    pr: Option<&'a PrInfo>,
}

fn cell(key: &str, ctx: &CellCtx<'_>) -> String {
    let dash = || "—".to_string();
    let agent = ctx.row.agent;
    match key {
        "id" => agent_session_key_for_label(&agent.id.session_key),
        "agent" => format!(
            "{}:{}",
            agent.harness_key,
            agent_session_key_for_label(&agent.id.session_key)
        ),
        "cwd" => agent.cwd.clone().unwrap_or_else(dash),
        "mux" => match ctx.mux {
            Some(m) => format!("{}:{}", m.backend, m.native_id),
            None => dash(),
        },
        "mux-conf" => match ctx.mux {
            Some(m) => {
                render::indicator_from_tags(&m.provenance, &m.confidence, m.candidate_count > 1)
            }
            None => dash(),
        },
        "pr" => match ctx.pr {
            Some(p) => forge_pr_label(p),
            None => dash(),
        },
        "pr-conf" => match ctx.pr {
            Some(p) => {
                render::indicator_from_tags(&p.provenance, &p.confidence, p.candidate_count > 1)
            }
            None => dash(),
        },
        "lineage" => match ctx.lineage {
            Some(l) if l.has_grandparent => format!("{}←", l.label),
            Some(l) => l.label.clone(),
            None => dash(),
        },
        "workspace" => ctx.workspace.cloned().unwrap_or_else(dash),
        "checkout" => ctx.row.checkout_root.clone().unwrap_or_else(dash),
        "branch" => ctx
            .branch_refname
            .map_or_else(dash, |r| strip_branch_prefix(r).to_string()),
        "repo" => ctx.row.repo_common_dir.clone().unwrap_or_else(dash),
        "fork" => ctx.fork.map_or_else(dash, |f| f.label.clone()),
        "declared" => ctx.declared.map_or_else(dash, |d| d.state_label.clone()),
        "preview" => agent.last_message_preview.clone().unwrap_or_else(dash),
        "title" => ctx
            .row
            .alias_display_name
            .clone()
            .or_else(|| agent.title.clone())
            .unwrap_or_else(dash),
        "activity" => agent
            .last_active_epoch
            .map_or_else(dash, |epoch| format_relative_age(epoch, current_epoch())),
        _ => dash(),
    }
}

fn forge_pr_label(pr: &PrInfo) -> String {
    let state = pr.state.as_deref().unwrap_or("?");
    let draft = if pr.is_draft { " draft" } else { "" };
    format!("{}/{}#{} ({state}{draft})", pr.owner, pr.repo, pr.number)
}

fn row_matches(
    options: &RenderOptions,
    row: &SessionRow<'_>,
    mux_lookup: &HashMap<SessionKey, MuxInfo>,
) -> bool {
    if !options.filter.has_narrowing_predicates() {
        return true;
    }
    let candidate_count = mux_lookup.get(&row.key()).map_or(0, |m| m.candidate_count);
    let inputs = SessionMatchInputs {
        harness_key: &row.agent.harness_key,
        now_epoch: options.now_epoch,
        last_active_epoch: row.agent.last_active_epoch,
        mux_state: MuxStateKey::from_candidate_count(candidate_count),
    };
    options.filter.matches_session(&inputs)
}

// -----------------------------------------------------------------------------
// In-memory collectors
// -----------------------------------------------------------------------------

fn collect_session_rows(snapshot: &GraphSnapshot) -> Vec<SessionRow<'_>> {
    // Pre-index checkouts so we can find the deepest matching one for
    // each session's cwd in O(C) per session. Same logic as the
    // `v_sessions_with_repo` view.
    let checkouts: Vec<&CheckoutNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Checkout(checkout) => Some(checkout),
            _ => None,
        })
        .collect();

    let mut rows: Vec<SessionRow<'_>> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(agent) => {
                let alias = snapshot
                    .aliases
                    .get(&NodeId::AgentSession(agent.id.clone()))
                    .map(|s| s.to_string());
                let (checkout_node_id, checkout_root, repo_common_dir) =
                    deepest_checkout_for(&checkouts, agent.cwd.as_deref());
                Some(SessionRow {
                    agent,
                    alias_display_name: alias,
                    checkout_node_id,
                    checkout_root,
                    repo_common_dir,
                })
            }
            _ => None,
        })
        .collect();
    rows.sort_by(|a, b| {
        a.agent
            .id
            .harness_key
            .cmp(&b.agent.id.harness_key)
            .then_with(|| a.agent.id.state_scope.cmp(&b.agent.id.state_scope))
            .then_with(|| a.agent.id.session_key.cmp(&b.agent.id.session_key))
    });
    rows
}

/// Deepest checkout whose root is `cwd` or an ancestor of `cwd`,
/// mirroring `v_sessions_with_repo`'s SELECT ... ORDER BY
/// length(c2.root) DESC LIMIT 1. Returns the Display-form NodeId,
/// the checkout's root, and the underlying repo's common_dir.
fn deepest_checkout_for(
    checkouts: &[&CheckoutNode],
    cwd: Option<&str>,
) -> (Option<NodeId>, Option<String>, Option<String>) {
    let Some(cwd) = cwd else {
        return (None, None, None);
    };
    let cwd_path = Path::new(cwd);
    let mut best: Option<&CheckoutNode> = None;
    for candidate in checkouts {
        let root_path = Path::new(&candidate.root);
        if root_path == cwd_path || path_is_ancestor_of(root_path, cwd_path) {
            best = match best {
                None => Some(candidate),
                Some(current) if candidate.root.len() > current.root.len() => Some(candidate),
                Some(current) => Some(current),
            };
        }
    }
    match best {
        Some(checkout) => (
            Some(NodeId::Checkout(checkout.id.clone())),
            Some(checkout.root.clone()),
            Some(checkout.id.repo.common_dir.clone()),
        ),
        None => (None, None, None),
    }
}

/// Per-checkout preferred `checked_out_branch` refname.
fn collect_branch_lookup(snapshot: &GraphSnapshot) -> HashMap<NodeId, String> {
    let mut per_checkout: HashMap<NodeId, Vec<&GraphLink>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::CheckedOutBranch) {
            continue;
        }
        let NodeId::Checkout(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::Branch(_),
        } = &link.target
        else {
            continue;
        };
        per_checkout
            .entry(link.source.clone())
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (checkout_id, candidates) in per_checkout {
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        if let LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        } = &best.target
        {
            out.insert(checkout_id, branch_id.refname.clone());
        }
    }
    out
}

/// Per-agent preferred `linked_to_mux` candidate's mux info plus
/// the count of distinct active mux targets (for the ambiguity
/// `*` marker).
fn collect_mux_lookup(snapshot: &GraphSnapshot) -> HashMap<SessionKey, MuxInfo> {
    let mux_lookup: HashMap<NodeId, &MuxSessionNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some((NodeId::MuxSession(mux.id.clone()), mux)),
            _ => None,
        })
        .collect();
    let mut per_session: HashMap<NodeId, Vec<&GraphLink>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::LinkedToMux) {
            continue;
        }
        let NodeId::AgentSession(_) = &link.source else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::MuxSession(_),
        } = &link.target
        else {
            continue;
        };
        per_session
            .entry(link.source.clone())
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (source, candidates) in per_session {
        let NodeId::AgentSession(agent_id) = &source else {
            continue;
        };
        let candidate_count = candidates
            .iter()
            .filter_map(|link| match &link.target {
                LinkEndpoint::Node { id } => Some(id.to_string()),
                LinkEndpoint::Unresolved { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        let Some(best) = pick_preferred(&candidates) else {
            continue;
        };
        let LinkEndpoint::Node {
            id: target_id @ NodeId::MuxSession(_),
        } = &best.target
        else {
            continue;
        };
        let Some(mux) = mux_lookup.get(target_id) else {
            continue;
        };
        out.insert(
            session_key_of(agent_id),
            MuxInfo {
                backend: mux.backend.clone(),
                native_id: mux.native_id.clone(),
                provenance: best.provenance.snake_case().to_string(),
                confidence: best.confidence.snake_case().to_string(),
                candidate_count,
            },
        );
    }
    out
}

/// Global preferred PR. Mirrors `preferred_pr_for_session`'s
/// intentionally loose behavior: find the first preferred
/// `branch_has_forge_pr` candidate (BTreeMap-by-branch order)
/// and use its source PR. Production discovery emits these with
/// source=ForgePr, target=Branch.
fn collect_global_pr(snapshot: &GraphSnapshot) -> Option<PrInfo> {
    let pr_lookup: HashMap<NodeId, &ForgePrNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some((NodeId::ForgePr(pr.id.clone()), pr)),
            _ => None,
        })
        .collect();
    // Group active branch_has_forge_pr links by target branch
    // key. Take the first branch's candidates (BTreeMap order on
    // (repo_common_dir, refname)).
    let mut by_branch: BTreeMap<(String, String), Vec<&GraphLink>> = BTreeMap::new();
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
        by_branch
            .entry((branch_id.repo.common_dir.clone(), branch_id.refname.clone()))
            .or_default()
            .push(link);
    }
    let (_, first_branch_candidates) = by_branch.into_iter().next()?;
    let candidate_count = first_branch_candidates.len();
    let best = pick_preferred(&first_branch_candidates)?;
    let NodeId::ForgePr(_) = &best.source else {
        return None;
    };
    let pr = pr_lookup.get(&best.source)?;
    Some(PrInfo {
        owner: pr.owner.clone(),
        repo: pr.repo.clone(),
        number: pr.number,
        state: pr.state.clone(),
        is_draft: pr.is_draft,
        provenance: best.provenance.snake_case().to_string(),
        confidence: best.confidence.snake_case().to_string(),
        candidate_count,
    })
}

/// Per-session lineage info: `label` from the preferred
/// `parent_session` candidate, plus `has_grandparent` when the
/// parent itself has a preferred `parent_session`.
fn collect_lineage_lookup(snapshot: &GraphSnapshot) -> HashMap<SessionKey, LineageInfo> {
    // First pass: pick preferred parent_session per source agent.
    let mut per_session: HashMap<NodeId, Vec<&GraphLink>> = HashMap::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if !matches!(link.relation, RelationKind::ParentSession) {
            continue;
        }
        let NodeId::AgentSession(_) = &link.source else {
            continue;
        };
        per_session
            .entry(link.source.clone())
            .or_default()
            .push(link);
    }
    // For each source, identify the preferred candidate and the
    // target's session key (when the target is a known
    // agent_session).
    let mut preferred: HashMap<SessionKey, (&GraphLink, Option<SessionKey>)> = HashMap::new();
    for (source, candidates) in &per_session {
        let NodeId::AgentSession(source_id) = source else {
            continue;
        };
        let Some(best) = pick_preferred(candidates) else {
            continue;
        };
        let parent_key = match &best.target {
            LinkEndpoint::Node {
                id: NodeId::AgentSession(parent_id),
            } => Some(session_key_of(parent_id)),
            _ => None,
        };
        preferred.insert(session_key_of(source_id), (best, parent_key));
    }

    let mut out = HashMap::new();
    for (key, (link, parent_key)) in &preferred {
        let label = match &link.target {
            LinkEndpoint::Node {
                id: NodeId::AgentSession(parent_id),
            } => short_session_id(&parent_id.session_key),
            LinkEndpoint::Node { .. } => continue,
            LinkEndpoint::Unresolved { evidence } => evidence.native_id.as_deref().map_or_else(
                || "?".to_string(),
                |native| format!("?{}", short_session_id(native)),
            ),
        };
        let has_grandparent = parent_key
            .as_ref()
            .is_some_and(|pk| preferred.contains_key(pk));
        out.insert(
            key.clone(),
            LineageInfo {
                label,
                has_grandparent,
            },
        );
    }
    out
}

/// Per-session workspace label. Reads
/// `resolved_relationships(associated_with, agent_session ->
/// workspace)`. When the workspace has ≥2 selected
/// `workspace_contains_repo` members, the label is `+`-joined
/// basenames; otherwise the workspace root.
fn collect_workspace_lookup(snapshot: &GraphSnapshot) -> HashMap<SessionKey, String> {
    let workspace_lookup: HashMap<NodeId, &WorkspaceNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Workspace(workspace) => {
                Some((NodeId::Workspace(workspace.id.clone()), workspace))
            }
            _ => None,
        })
        .collect();
    let link_by_id: HashMap<&str, &GraphLink> = snapshot
        .candidate_links
        .iter()
        .map(|link| (link.id.as_str(), link))
        .collect();
    // Per workspace: list of basenames extracted from each
    // selected workspace_contains_repo link's source_metadata
    // logical_path field.
    let mut members: HashMap<NodeId, Vec<String>> = HashMap::new();
    let mut sorted_resolved: Vec<&crate::model::ResolvedRelationship> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| matches!(r.relation, RelationKind::WorkspaceContainsRepo))
        .filter(|r| matches!(r.source, NodeId::Workspace(_)))
        .filter(|r| r.selected_link_id.is_some())
        .collect();
    sorted_resolved.sort_by(|a, b| {
        a.source
            .to_string()
            .cmp(&b.source.to_string())
            .then_with(|| a.target.to_string().cmp(&b.target.to_string()))
    });
    for resolved in sorted_resolved {
        let Some(link_id) = resolved.selected_link_id.as_deref() else {
            continue;
        };
        let Some(link) = link_by_id.get(link_id) else {
            continue;
        };
        if let Some(display) = link
            .source_metadata
            .fields
            .get("logical_path")
            .and_then(|v| v.as_str())
            .and_then(|p| {
                Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
        {
            members
                .entry(resolved.source.clone())
                .or_default()
                .push(display);
        }
    }
    for displays in members.values_mut() {
        displays.sort();
        displays.dedup();
    }

    // Per session: each `associated_with` resolved relationship
    // pointing to a workspace contributes one entry. Multi-entry
    // sessions render as comma-joined; per-workspace display is
    // members.join('+') when ≥2, else workspace.root. The
    // workspace's root is read from the WorkspaceNode when one
    // exists; when it doesn't, fall back to the WorkspaceId's
    // structural `root`.
    let mut per_session: HashMap<SessionKey, Vec<(NodeId, String)>> = HashMap::new();
    for resolved in &snapshot.resolved_relationships {
        if !matches!(resolved.relation, RelationKind::AssociatedWith) {
            continue;
        }
        let NodeId::AgentSession(source_id) = &resolved.source else {
            continue;
        };
        let NodeId::Workspace(workspace_id) = &resolved.target else {
            continue;
        };
        let root = workspace_lookup
            .get(&resolved.target)
            .map_or_else(|| workspace_id.root.clone(), |w| w.root.clone());
        per_session
            .entry(session_key_of(source_id))
            .or_default()
            .push((resolved.target.clone(), root));
    }
    let mut out = HashMap::new();
    for (key, mut entries) in per_session {
        entries.sort_by(|a, b| a.1.cmp(&b.1));
        entries.dedup();
        let displays: Vec<String> = entries
            .into_iter()
            .map(|(ws_id, ws_root)| {
                members
                    .get(&ws_id)
                    .filter(|names| names.len() >= 2)
                    .map_or(ws_root, |names| names.join("+"))
            })
            .collect();
        out.insert(key, displays.join(","));
    }
    out
}

/// Per-session fork label from `child_session` candidates whose
/// source is a fork and target is the agent. First-wins.
fn collect_fork_lookup(snapshot: &GraphSnapshot) -> HashMap<SessionKey, ForkInfo> {
    let fork_lookup: HashMap<NodeId, &ForkNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Fork(fork) => Some((NodeId::Fork(fork.id.clone()), fork)),
            _ => None,
        })
        .collect();
    let mut sorted_links: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| matches!(link.relation, RelationKind::ChildSession))
        .filter(|link| matches!(link.source, NodeId::Fork(_)))
        .filter(|link| {
            matches!(
                link.target,
                LinkEndpoint::Node {
                    id: NodeId::AgentSession(_)
                }
            )
        })
        .collect();
    sorted_links.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out: HashMap<SessionKey, ForkInfo> = HashMap::new();
    for link in sorted_links {
        let Some(fork) = fork_lookup.get(&link.source) else {
            continue;
        };
        let LinkEndpoint::Node {
            id: NodeId::AgentSession(target_id),
        } = &link.target
        else {
            continue;
        };
        let display = fork
            .name
            .clone()
            .unwrap_or_else(|| fork.provider_source_key.clone());
        let info = ForkInfo {
            label: format!("{}:{}", fork.provider, display),
        };
        // First-wins.
        out.entry(session_key_of(target_id)).or_insert(info);
    }
    out
}

/// Strongest LocalDeclared / GlobalDeclared candidate per agent
/// session; surfaces its `LinkState` as a label.
fn collect_declared_lookup(snapshot: &GraphSnapshot) -> HashMap<SessionKey, DeclaredInfo> {
    let mut per_session: HashMap<SessionKey, Vec<&GraphLink>> = HashMap::new();
    for link in &snapshot.candidate_links {
        let NodeId::AgentSession(source_id) = &link.source else {
            continue;
        };
        if !matches!(
            link.provenance,
            Provenance::LocalDeclared | Provenance::GlobalDeclared
        ) {
            continue;
        }
        per_session
            .entry(session_key_of(source_id))
            .or_default()
            .push(link);
    }
    let mut out = HashMap::new();
    for (key, candidates) in per_session {
        // Rank by provenance precedence only; declared selection has
        // no confidence/id tiebreak here.
        let Some(best) = candidates
            .into_iter()
            .max_by(|a, b| a.provenance.precedence().cmp(&b.provenance.precedence()))
        else {
            continue;
        };
        let label = match &best.state {
            LinkState::Active => "declared",
            LinkState::Ignored { .. } => "ignored",
            LinkState::Overridden { .. } => "overridden",
        };
        out.insert(
            key,
            DeclaredInfo {
                state_label: label.to_string(),
            },
        );
    }
    out
}
