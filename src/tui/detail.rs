//! Selected-node detail view-models.
//!
//! Right-panel header data for a single node, in a non-string form
//! that the renderer can lay out at its own width. Mirrors the
//! content of `conspectus node show <id>` (and reuses
//! [`crate::output::table::node_short_id`] for the leading short id)
//! while exposing the locked mockup-review behavior:
//!
//! - For agent sessions, the header shows `harness`, `cwd`, `title`
//!   (when set), `mux`, `pr`, and `lineage` rows in that order.
//!   Sessions without a `title` omit the row rather than render a
//!   placeholder.
//! - Mux row carries the candidate count when ambiguous, so the
//!   renderer can surface "— (2 candidates) ⚠" inline.
//! - PR row is the immediate-stage label (state, draft); the async
//!   enrichment from `P8-012a` overlays on top later.
//! - Paths render with `~` shortening when a home directory is
//!   passed in.
//!
//! v1 implements agent-session and mux-session details fully; the
//! remaining node kinds emit a minimal field list pulled directly
//! from the node, so the right panel can render *something* for
//! every selectable row. Richer fork/PR detail is layered in by the
//! enrichment stories (`P8-012a`, `P8-012b`).

use std::path::Path;

use crate::model::{
    AgentSessionNode, BranchNode, Confidence, Diagnostic, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId, Provenance,
    RelationKind, RepoNode, ResolvedRelationship, WorkspaceNode, WorktreeNode,
};
use crate::output::table::node_short_id;
use crate::tui::rows::shorten_home;

/// Inputs to the detail builder.
#[derive(Debug, Clone)]
pub struct DetailInputs<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub target: &'a NodeId,
    /// User home directory used for `~`-shortening. Pass `None` to
    /// leave paths in full form.
    pub home: Option<&'a Path>,
}

/// Build the detail view-model for the given node id. Returns
/// `None` when the node isn't in the snapshot (e.g. selection
/// pointed at a row that was just removed by a refresh).
pub fn build_node_detail(inputs: DetailInputs<'_>) -> Option<NodeDetail> {
    let node = inputs
        .snapshot
        .nodes
        .iter()
        .find(|n| n.id() == *inputs.target)?;
    let id = node.id();

    let kind_label = kind_label(node);
    let title_line = title_line(node);
    let short_id = node_short_id(&id);
    let header_fields = header_fields(inputs.snapshot, node, inputs.home);
    let (outgoing_links, incoming_links) = link_summaries(inputs.snapshot, &id, inputs.home);
    let resolved = resolved_summaries(inputs.snapshot, &id);
    let diagnostics = diagnostic_summaries(inputs.snapshot, &id);

    Some(NodeDetail {
        kind_label,
        title_line,
        short_id,
        full_id: id,
        header_fields,
        outgoing_links,
        incoming_links,
        resolved,
        diagnostics,
    })
}

// -----------------------------------------------------------------------------
// View-model types
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct NodeDetail {
    /// `agent_session`, `mux_session`, `repo`, etc. Matches the
    /// snake-case label `node show` already prints.
    pub kind_label: &'static str,
    /// Compact identity line for the top of the right panel
    /// (e.g. `codex:…b4fdee8`, `tmux:editor`, `forge_pr:octo/repo#7`).
    pub title_line: String,
    /// FNV-1a 64-bit hex short id, floored at the H-TBL-002 length.
    pub short_id: String,
    pub full_id: NodeId,
    /// Header field rows in display order. Renderer prints
    /// `label    value annotation` per row.
    pub header_fields: Vec<HeaderField>,
    /// Active outgoing candidate links from this node.
    pub outgoing_links: Vec<LinkSummary>,
    /// Active incoming candidate links to this node.
    pub incoming_links: Vec<LinkSummary>,
    /// Resolved relationships involving this node, in stable order.
    pub resolved: Vec<ResolvedSummary>,
    /// Diagnostics that mention this node.
    pub diagnostics: Vec<DiagnosticSummary>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderField {
    /// Label shown in the leftmost column (`harness`, `cwd`, `mux`,
    /// `pr`, `lineage`, …). Static strings only.
    pub label: &'static str,
    /// Rendered value, with `~`-shortening already applied to paths.
    pub value: String,
    /// True when `value` is a placeholder (e.g. `— (no attach)`),
    /// so the renderer can dim it.
    pub placeholder: bool,
    /// Optional trailing annotation: `⚠` for ambiguity, `⟳` for an
    /// async-enrichment in-flight, `(preferred)` markers, etc.
    pub annotation: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkSummary {
    pub relation: RelationKind,
    pub other: LinkOther,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub state: LinkStateLabel,
    pub link_id: String,
    pub adapter: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LinkOther {
    Node(NodeId),
    Unresolved {
        node_type: String,
        harness_key: Option<String>,
        native_id: Option<String>,
        path: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStateLabel {
    Active,
    Ignored,
    Overridden,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSummary {
    pub relation: RelationKind,
    pub source: NodeId,
    pub target: NodeId,
    pub selected_link_id: String,
    pub competing_link_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagnosticSummary {
    UnresolvedEndpoint {
        link_id: String,
        relation: RelationKind,
    },
    Conflict {
        relation: RelationKind,
        selected_link_id: String,
        competing_link_ids: Vec<String>,
    },
    Config {
        path: String,
        message: String,
    },
}

// -----------------------------------------------------------------------------
// Per-kind header builders
// -----------------------------------------------------------------------------

fn kind_label(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Repo(_) => "repo",
        GraphNode::Worktree(_) => "checkout",
        GraphNode::Workspace(_) => "workspace",
        GraphNode::AgentSession(_) => "agent_session",
        GraphNode::MuxSession(_) => "mux_session",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

fn title_line(node: &GraphNode) -> String {
    let id = node.id();
    let short = node_short_id(&id);
    let short_tail: String = short.chars().rev().take(7).collect();
    let short_tail: String = short_tail.chars().rev().collect();
    match node {
        GraphNode::AgentSession(session) => {
            format!("{}:…{}", session.harness_key, short_tail)
        }
        GraphNode::MuxSession(mux) => mux_display_label(mux),
        GraphNode::ForgePr(pr) => format!("forge_pr:{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(fork) => match &fork.name {
            Some(name) => format!("fork:{name}"),
            None => format!("fork:{}", fork.provider_source_key),
        },
        GraphNode::Repo(repo) => format!("repo:{}", repo.common_dir),
        GraphNode::Worktree(worktree) => format!("checkout:{}", worktree.root),
        GraphNode::Workspace(workspace) => format!("workspace:{}", workspace.root),
        GraphNode::Branch(branch) => format!("branch:{}", branch.refname),
    }
}

fn header_fields(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
) -> Vec<HeaderField> {
    match node {
        GraphNode::AgentSession(session) => agent_session_fields(snapshot, session, home),
        GraphNode::MuxSession(mux) => mux_session_fields(snapshot, mux, home),
        GraphNode::ForgePr(pr) => forge_pr_fields(pr),
        GraphNode::Fork(fork) => fork_fields(fork),
        GraphNode::Repo(repo) => repo_fields(repo, home),
        GraphNode::Worktree(worktree) => worktree_fields(worktree, home),
        GraphNode::Workspace(workspace) => workspace_fields(workspace, home),
        GraphNode::Branch(branch) => branch_fields(branch),
    }
}

fn agent_session_fields(
    snapshot: &GraphSnapshot,
    session: &AgentSessionNode,
    home: Option<&Path>,
) -> Vec<HeaderField> {
    let mut fields = Vec::new();
    fields.push(plain("harness", session.harness_key.clone()));
    let cwd_value = match &session.cwd {
        Some(cwd) => HeaderField {
            label: "cwd",
            value: shorten_home(cwd, home),
            placeholder: false,
            annotation: None,
        },
        None => placeholder("cwd", "— (unknown)"),
    };
    fields.push(cwd_value);
    if let Some(title) = session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        fields.push(plain("title", title.to_string()));
    }

    let session_id = NodeId::AgentSession(session.id.clone());
    fields.push(session_mux_field(snapshot, &session_id));
    fields.push(session_pr_field(snapshot, &session_id, home));
    fields.push(session_lineage_field(snapshot, &session_id));

    fields
}

fn session_mux_field(snapshot: &GraphSnapshot, session: &NodeId) -> HeaderField {
    let candidates = active_links_from(snapshot, session, RelationKind::LinkedToMux);
    match candidates.len() {
        0 => placeholder("mux", "— (no attach)"),
        1 => {
            let preferred = candidates[0];
            let value = link_target_label(snapshot, preferred).unwrap_or_else(|| "—".to_string());
            HeaderField {
                label: "mux",
                value,
                placeholder: false,
                annotation: None,
            }
        }
        n => {
            // The preferred-by-provenance candidate is shown; the
            // ambiguity is surfaced via the annotation. The exact
            // count helps the renderer mirror the row glyph.
            let preferred = preferred_link(&candidates);
            let label = preferred
                .and_then(|link| link_target_label(snapshot, link))
                .unwrap_or_else(|| format!("— ({n} candidates)"));
            HeaderField {
                label: "mux",
                value: format!("{label}  ({n} candidates)"),
                placeholder: false,
                annotation: Some("⚠"),
            }
        }
    }
}

fn session_pr_field(
    snapshot: &GraphSnapshot,
    session: &NodeId,
    _home: Option<&Path>,
) -> HeaderField {
    // Sessions don't link to PRs directly today; the existing table
    // resolves session → worktree → branch → PR. v1 detail shows
    // the same: walk the resolved relationships once to find the
    // PR keyed off the session's worktree (cwd-matched).
    let session_node = match snapshot.nodes.iter().find(|n| n.id() == *session) {
        Some(GraphNode::AgentSession(node)) => node,
        _ => return placeholder("pr", "— (no PR)"),
    };
    let cwd = match session_node.cwd.as_deref() {
        Some(cwd) => cwd,
        None => return placeholder("pr", "— (no PR)"),
    };
    let Some(worktree_id) = snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::Worktree(wt) if wt.root == cwd => Some(NodeId::Worktree(wt.id.clone())),
        _ => None,
    }) else {
        return placeholder("pr", "— (no PR)");
    };
    let Some(branch_id) = preferred_target(snapshot, &worktree_id, RelationKind::CheckedOutBranch)
    else {
        return placeholder("pr", "— (no PR)");
    };
    let Some(pr_id) = preferred_target(snapshot, &branch_id, RelationKind::BranchHasForgePr) else {
        return placeholder("pr", "— (no PR)");
    };
    let Some(GraphNode::ForgePr(pr)) = snapshot.nodes.iter().find(|n| n.id() == pr_id) else {
        return placeholder("pr", "— (no PR)");
    };
    let state = pr.state.as_deref().unwrap_or("?");
    let draft_marker = if pr.is_draft { " · draft" } else { "" };
    HeaderField {
        label: "pr",
        value: format!(
            "{}/{}#{} ({state}){draft_marker}",
            pr.owner, pr.repo, pr.number
        ),
        placeholder: false,
        annotation: None,
    }
}

fn session_lineage_field(snapshot: &GraphSnapshot, session: &NodeId) -> HeaderField {
    let Some(parent_id) = preferred_target(snapshot, session, RelationKind::ParentSession) else {
        return placeholder("lineage", "— (no parent)");
    };
    let Some(GraphNode::AgentSession(parent)) = snapshot.nodes.iter().find(|n| n.id() == parent_id)
    else {
        return placeholder("lineage", "— (no parent)");
    };
    let short = node_short_id(&NodeId::AgentSession(parent.id.clone()));
    let short_tail: String = short.chars().rev().take(7).collect::<String>();
    let short_tail: String = short_tail.chars().rev().collect();
    HeaderField {
        label: "lineage",
        value: format!("{}:…{}", parent.harness_key, short_tail),
        placeholder: false,
        annotation: None,
    }
}

fn mux_session_fields(
    snapshot: &GraphSnapshot,
    mux: &MuxSessionNode,
    home: Option<&Path>,
) -> Vec<HeaderField> {
    let mut fields = vec![
        plain("mux", mux_display_label(mux)),
        plain("backend", mux.backend.clone()),
    ];
    if mux.native_id.chars().count() <= 36 {
        fields.push(plain("native_id", mux.native_id.clone()));
    }
    if let Some(cwd) = &mux.cwd {
        fields.push(HeaderField {
            label: "cwd",
            value: shorten_home(cwd, home),
            placeholder: false,
            annotation: None,
        });
    }
    let mux_id = NodeId::MuxSession(mux.id.clone());
    let attached_count = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .filter(|link| match &link.target {
            LinkEndpoint::Node { id } => id == &mux_id,
            _ => false,
        })
        .count();
    fields.push(plain("attached", format!("{attached_count}")));
    fields
}

fn forge_pr_fields(pr: &ForgePrNode) -> Vec<HeaderField> {
    let mut fields = vec![plain(
        "pr",
        format!(
            "{}/{}#{} ({})",
            pr.owner,
            pr.repo,
            pr.number,
            pr.state.as_deref().unwrap_or("?")
        ),
    )];
    if pr.is_draft {
        fields.push(plain("draft", "true".to_string()));
    }
    if let Some(url) = &pr.url {
        fields.push(plain("url", url.clone()));
    }
    fields
}

fn fork_fields(fork: &ForkNode) -> Vec<HeaderField> {
    let mut fields = vec![plain("provider", fork.provider.clone())];
    if let Some(name) = &fork.name {
        fields.push(plain("name", name.clone()));
    }
    if let Some(scope) = &fork.scope {
        fields.push(plain("scope", scope.clone()));
    }
    fields.push(plain("source_key", fork.provider_source_key.clone()));
    fields
}

fn repo_fields(repo: &RepoNode, home: Option<&Path>) -> Vec<HeaderField> {
    vec![HeaderField {
        label: "common_dir",
        value: shorten_home(&repo.common_dir, home),
        placeholder: false,
        annotation: None,
    }]
}

fn worktree_fields(worktree: &WorktreeNode, home: Option<&Path>) -> Vec<HeaderField> {
    let mut fields = vec![HeaderField {
        label: "root",
        value: shorten_home(&worktree.root, home),
        placeholder: false,
        annotation: None,
    }];
    if let Some(git_dir) = &worktree.git_dir {
        fields.push(HeaderField {
            label: "git_dir",
            value: shorten_home(git_dir, home),
            placeholder: false,
            annotation: None,
        });
    }
    if let Some(branch) = &worktree.current_branch {
        fields.push(plain("branch", branch.refname.clone()));
    }
    fields
}

fn workspace_fields(workspace: &WorkspaceNode, home: Option<&Path>) -> Vec<HeaderField> {
    let mut fields = vec![HeaderField {
        label: "root",
        value: shorten_home(&workspace.root, home),
        placeholder: false,
        annotation: None,
    }];
    if let Some(provider) = &workspace.provider {
        fields.push(plain("provider", provider.clone()));
    }
    if let Some(name) = &workspace.name {
        fields.push(plain("name", name.clone()));
    }
    fields
}

fn branch_fields(branch: &BranchNode) -> Vec<HeaderField> {
    let mut fields = vec![
        plain("refname", branch.refname.clone()),
        plain("repo", format!("{}", branch.id.repo)),
    ];
    if let Some(commit) = &branch.current_commit {
        fields.push(plain("commit", commit.clone()));
    }
    if let Some(upstream) = &branch.upstream {
        fields.push(plain("upstream", upstream.clone()));
    }
    fields
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn plain(label: &'static str, value: String) -> HeaderField {
    HeaderField {
        label,
        value,
        placeholder: false,
        annotation: None,
    }
}

fn placeholder(label: &'static str, value: &str) -> HeaderField {
    HeaderField {
        label,
        value: value.to_string(),
        placeholder: true,
        annotation: None,
    }
}

fn active_links_from<'a>(
    snapshot: &'a GraphSnapshot,
    source: &NodeId,
    relation: RelationKind,
) -> Vec<&'a GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            matches!(link.state, LinkState::Active)
                && link.source == *source
                && link.relation == relation
        })
        .collect()
}

fn preferred_link<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links.to_vec();
    ranked.sort_by(|left, right| {
        right
            .provenance
            .precedence()
            .cmp(&left.provenance.precedence())
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left.id.cmp(&right.id))
    });
    ranked.into_iter().next()
}

fn preferred_target(
    snapshot: &GraphSnapshot,
    source: &NodeId,
    relation: RelationKind,
) -> Option<NodeId> {
    snapshot
        .resolved_relationships
        .iter()
        .find(|rel| rel.source == *source && rel.relation == relation)
        .map(|rel| rel.target.clone())
}

fn link_target_label(snapshot: &GraphSnapshot, link: &GraphLink) -> Option<String> {
    let target = link.target_node_id()?;
    let node = snapshot.nodes.iter().find(|n| n.id() == *target)?;
    match node {
        GraphNode::MuxSession(mux) => Some(mux_display_label(mux)),
        GraphNode::AgentSession(session) => Some(format!(
            "{}:{}",
            session.harness_key, session.id.session_key
        )),
        GraphNode::Repo(repo) => Some(format!("repo:{}", repo.common_dir)),
        other => Some(format!("{}", other.id())),
    }
}

fn mux_display_label(mux: &MuxSessionNode) -> String {
    let native = if mux.native_id.chars().count() > 36 {
        let head: String = mux.native_id.chars().take(28).collect();
        let tail: String = mux
            .native_id
            .chars()
            .rev()
            .take(6)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("{head}…{tail}")
    } else {
        mux.native_id.clone()
    };
    format!("{}:{native}", mux.backend)
}

fn link_summaries(
    snapshot: &GraphSnapshot,
    id: &NodeId,
    _home: Option<&Path>,
) -> (Vec<LinkSummary>, Vec<LinkSummary>) {
    let mut outgoing = Vec::new();
    let mut incoming = Vec::new();
    for link in &snapshot.candidate_links {
        if !matches!(link.state, LinkState::Active) {
            continue;
        }
        if link.source == *id {
            outgoing.push(link_summary(link));
        }
        if let LinkEndpoint::Node { id: target } = &link.target
            && target == id
        {
            incoming.push(link_summary(link));
        }
    }
    (outgoing, incoming)
}

fn link_summary(link: &GraphLink) -> LinkSummary {
    LinkSummary {
        relation: link.relation.clone(),
        other: match &link.target {
            LinkEndpoint::Node { id } => LinkOther::Node(id.clone()),
            LinkEndpoint::Unresolved { evidence } => LinkOther::Unresolved {
                node_type: evidence.node_type.clone(),
                harness_key: evidence.harness_key.clone(),
                native_id: evidence.native_id.clone(),
                path: evidence.path.clone(),
            },
        },
        provenance: link.provenance,
        confidence: link.confidence,
        state: match &link.state {
            LinkState::Active => LinkStateLabel::Active,
            LinkState::Ignored { .. } => LinkStateLabel::Ignored,
            LinkState::Overridden { .. } => LinkStateLabel::Overridden,
        },
        link_id: link.id.clone(),
        adapter: link.source_metadata.adapter.clone(),
    }
}

fn resolved_summaries(snapshot: &GraphSnapshot, id: &NodeId) -> Vec<ResolvedSummary> {
    snapshot
        .resolved_relationships
        .iter()
        .filter(|rel| rel.source == *id || rel.target == *id)
        .cloned()
        .map(|rel: ResolvedRelationship| ResolvedSummary {
            relation: rel.relation,
            source: rel.source,
            target: rel.target,
            selected_link_id: rel.selected_link_id,
            competing_link_ids: rel.competing_link_ids,
        })
        .collect()
}

fn diagnostic_summaries(snapshot: &GraphSnapshot, id: &NodeId) -> Vec<DiagnosticSummary> {
    snapshot
        .diagnostics
        .iter()
        .filter_map(|diag| match diag {
            Diagnostic::UnresolvedEndpoint { link_id, relation } => {
                // Match diagnostics whose link is sourced at this node.
                if let Some(link) = snapshot.candidate_links.iter().find(|l| &l.id == link_id)
                    && link.source == *id
                {
                    Some(DiagnosticSummary::UnresolvedEndpoint {
                        link_id: link_id.clone(),
                        relation: relation.clone(),
                    })
                } else {
                    None
                }
            }
            Diagnostic::Config { path, message } => Some(DiagnosticSummary::Config {
                path: path.clone(),
                message: message.clone(),
            }),
            Diagnostic::Conflict {
                source,
                relation,
                selected_link_id,
                competing_link_ids,
            } if source == id => Some(DiagnosticSummary::Conflict {
                relation: relation.clone(),
                selected_link_id: selected_link_id.clone(),
                competing_link_ids: competing_link_ids.clone(),
            }),
            _ => None,
        })
        .collect()
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, ForgePrId, ForgePrNode, GraphSnapshot,
        LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RepoId, RepoNode,
        SourceMetadata, WorktreeId, WorktreeNode,
    };
    use crate::resolve::resolve_snapshot;
    use std::path::PathBuf;

    fn home() -> PathBuf {
        PathBuf::from("/home/op")
    }

    fn agent(harness: &str, key: &str, cwd: Option<&str>, title: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, "/state", key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: title.map(str::to_string),
            last_message_preview: None,
        })
    }

    fn build(snapshot: &GraphSnapshot, target: &NodeId, home: Option<&Path>) -> NodeDetail {
        build_node_detail(DetailInputs {
            snapshot,
            target,
            home,
        })
        .expect("detail exists")
    }

    #[test]
    fn unknown_node_returns_none() {
        let snapshot = GraphSnapshot::empty();
        let phantom = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "nope"));
        assert!(
            build_node_detail(DetailInputs {
                snapshot: &snapshot,
                target: &phantom,
                home: None,
            })
            .is_none()
        );
    }

    #[test]
    fn agent_session_with_no_mux_or_pr_shows_placeholders() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/src/x"), None));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

        let detail = build(&snapshot, &target, Some(home().as_path()));
        assert_eq!(detail.kind_label, "agent_session");
        assert_eq!(
            detail.title_line,
            format!("codex:…{}", &detail.short_id[detail.short_id.len() - 7..])
        );

        // No title row when unset.
        assert!(detail.header_fields.iter().all(|f| f.label != "title"));

        let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
        assert_eq!(labels, vec!["harness", "cwd", "mux", "pr", "lineage"]);

        let by_label = |label: &str| {
            detail
                .header_fields
                .iter()
                .find(|f| f.label == label)
                .unwrap()
                .clone()
        };
        assert_eq!(by_label("harness").value, "codex");
        assert_eq!(by_label("cwd").value, "~/src/x");
        assert!(by_label("mux").placeholder);
        assert_eq!(by_label("mux").value, "— (no attach)");
        assert!(by_label("pr").placeholder);
        assert!(by_label("lineage").placeholder);
    }

    #[test]
    fn agent_session_with_title_inserts_title_row() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "opencode",
            "abc",
            Some("/home/op/src/x"),
            Some("Phase 8 mockup"),
        ));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "abc"));
        let detail = build(&snapshot, &target, Some(home().as_path()));
        let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec!["harness", "cwd", "title", "mux", "pr", "lineage"]
        );
        let title = detail
            .header_fields
            .iter()
            .find(|f| f.label == "title")
            .unwrap();
        assert_eq!(title.value, "Phase 8 mockup");
        assert!(!title.placeholder);
    }

    #[test]
    fn ambiguous_mux_annotates_with_warning_and_count() {
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let editor = NodeId::MuxSession(MuxSessionId::new("editor"));
        let scratch = NodeId::MuxSession(MuxSessionId::new("scratch"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/src/x"), None));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("scratch"),
            backend: "tmux".into(),
            native_id: "scratch".into(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot.candidate_links.push(GraphLink {
            id: "mux-1".into(),
            source: session_id.clone(),
            target: LinkEndpoint::Node { id: editor },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        snapshot.candidate_links.push(GraphLink {
            id: "mux-2".into(),
            source: session_id.clone(),
            target: LinkEndpoint::Node { id: scratch },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);

        let detail = build(&snapshot, &session_id, Some(home().as_path()));
        let mux = detail
            .header_fields
            .iter()
            .find(|f| f.label == "mux")
            .unwrap();
        assert!(mux.value.contains("tmux:editor"), "value={}", mux.value);
        assert!(mux.value.contains("2 candidates"), "value={}", mux.value);
        assert_eq!(mux.annotation, Some("⚠"));
    }

    #[test]
    fn agent_session_pr_field_walks_worktree_branch_pr_chain() {
        // session at cwd /home/op/src/x → worktree → branch main →
        // PR octo/repo#7 (open).
        let cwd = "/home/op/src/x";
        let repo_id = RepoId::new(cwd);
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snapshot.nodes.push(GraphNode::Worktree(WorktreeNode {
            id: WorktreeId::new(repo_id.clone(), cwd.to_string()),
            root: cwd.to_string(),
            git_dir: None,
            current_branch: None,
        }));
        let branch_id = crate::model::BranchId::new(repo_id.clone(), "refs/heads/main");
        snapshot
            .nodes
            .push(GraphNode::Branch(crate::model::BranchNode {
                id: branch_id.clone(),
                refname: "refs/heads/main".to_string(),
                current_commit: None,
                upstream: None,
            }));
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        snapshot.nodes.push(GraphNode::ForgePr(ForgePrNode {
            id: pr_id.clone(),
            provider: "github".into(),
            host: "github.com".into(),
            owner: "octo".into(),
            repo: "repo".into(),
            number: 7,
            state: Some("open".into()),
            url: None,
            updated_epoch: None,
            is_draft: false,
        }));
        snapshot.nodes.push(agent("codex", "abc", Some(cwd), None));

        // Worktree → Branch (CheckedOutBranch)
        snapshot.candidate_links.push(GraphLink {
            id: "wt-branch".into(),
            source: NodeId::Worktree(WorktreeId::new(repo_id.clone(), cwd.to_string())),
            target: LinkEndpoint::Node {
                id: NodeId::Branch(branch_id.clone()),
            },
            relation: RelationKind::CheckedOutBranch,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        // Branch → PR (BranchHasForgePr)
        snapshot.candidate_links.push(GraphLink {
            id: "branch-pr".into(),
            source: NodeId::Branch(branch_id),
            target: LinkEndpoint::Node {
                id: NodeId::ForgePr(pr_id),
            },
            relation: RelationKind::BranchHasForgePr,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));

        let detail = build(&snapshot, &target, Some(home().as_path()));
        let pr = detail
            .header_fields
            .iter()
            .find(|f| f.label == "pr")
            .unwrap();
        assert_eq!(pr.value, "octo/repo#7 (open)");
        assert!(!pr.placeholder);
    }

    #[test]
    fn mux_session_detail_counts_attached_agents() {
        let mux_id = MuxSessionId::new("editor");
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: Some("/home/op/src/x".into()),
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot
            .nodes
            .push(agent("codex", "a", Some("/home/op/src/x"), None));
        snapshot
            .nodes
            .push(agent("codex", "b", Some("/home/op/src/x"), None));
        for (i, session_key) in ["a", "b"].iter().enumerate() {
            snapshot.candidate_links.push(GraphLink {
                id: format!("attached-{i}"),
                source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", *session_key)),
                target: LinkEndpoint::Node {
                    id: NodeId::MuxSession(mux_id.clone()),
                },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: crate::model::Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::MuxSession(mux_id);
        let detail = build(&snapshot, &target, Some(home().as_path()));
        assert_eq!(detail.kind_label, "mux_session");
        assert_eq!(detail.title_line, "tmux:editor");
        let attached = detail
            .header_fields
            .iter()
            .find(|f| f.label == "attached")
            .unwrap();
        assert_eq!(attached.value, "2");
        let cwd = detail
            .header_fields
            .iter()
            .find(|f| f.label == "cwd")
            .unwrap();
        assert_eq!(cwd.value, "~/src/x");
    }

    #[test]
    fn outgoing_and_incoming_link_summaries_populate_from_snapshot() {
        let mut snapshot = GraphSnapshot::empty();
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/x"), None));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot.candidate_links.push(GraphLink {
            id: "link-1".into(),
            source: session_id.clone(),
            target: LinkEndpoint::Node { id: mux_id.clone() },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        });
        let snapshot = resolve_snapshot(snapshot);

        let session_detail = build(&snapshot, &session_id, None);
        assert_eq!(session_detail.outgoing_links.len(), 1);
        assert!(session_detail.incoming_links.is_empty());

        let mux_detail = build(&snapshot, &mux_id, None);
        assert!(mux_detail.outgoing_links.is_empty());
        assert_eq!(mux_detail.incoming_links.len(), 1);
    }
}
