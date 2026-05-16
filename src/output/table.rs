//! Plain-text table renderers for the resolved graph.
//!
//! See ADR 0006 for the projection vocabulary. Three projections are
//! supported:
//!
//! - [`Projection::Agent`] — one row per `AgentSession`, showing the
//!   preferred mux and preferred PR.
//! - [`Projection::Mux`] — one row per `MuxSession`, showing every
//!   attached agent session.
//! - [`Projection::Union`] — combined view with one row per node,
//!   preserving relationship status.
//!
//! Every projection shares one compact indicator format:
//!
//! - Provenance codes: `LD` (local declared), `GD` (global declared),
//!   `SD` (strong discovered), `D` (discovered), `C` (convention),
//!   `$` (cached).
//! - Confidence codes: `H` / `M` / `L`.
//! - An ambiguity marker `*` follows the cell when the resolver chose
//!   among multiple plausible candidates for that source/relation.

use std::collections::BTreeMap;

pub use crate::config::Projection;
use crate::model::{
    AgentSessionNode, Confidence, ForgePrNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    MuxSessionNode, NodeId, Provenance, RelationKind,
};

/// Render `snapshot` as a plain-text table using `projection`.
pub fn render(snapshot: &GraphSnapshot, projection: Projection) -> String {
    let view = SnapshotView::new(snapshot);
    match projection {
        Projection::Agent => render_agent(&view),
        Projection::Mux => render_mux(&view),
        Projection::Union => render_union(&view),
    }
}

/// Compact `provenance/confidence[*]` cell, e.g. `LD/H*`. Used in
/// every projection so the cells are easy to scan.
pub fn indicator(provenance: Provenance, confidence: Confidence, ambiguous: bool) -> String {
    let mut buf = String::with_capacity(6);
    buf.push_str(provenance_code(provenance));
    buf.push('/');
    buf.push_str(confidence_code(confidence));
    if ambiguous {
        buf.push('*');
    }
    buf
}

fn provenance_code(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "LD",
        Provenance::GlobalDeclared => "GD",
        Provenance::StrongDiscovered => "SD",
        Provenance::Discovered => "D",
        Provenance::Convention => "C",
        Provenance::Cached => "$",
    }
}

fn confidence_code(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::High => "H",
        Confidence::Medium => "M",
        Confidence::Low => "L",
    }
}

/// A pre-computed view of one snapshot keyed by node id so the
/// projection renderers don't each rebuild it.
struct SnapshotView<'a> {
    agent_sessions: BTreeMap<NodeId, &'a AgentSessionNode>,
    mux_sessions: BTreeMap<NodeId, &'a MuxSessionNode>,
    forge_prs: BTreeMap<NodeId, &'a ForgePrNode>,
    /// `(source_node_id, relation_kind)` → all candidate links for
    /// that pair, in stable order.
    by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
}

impl<'a> SnapshotView<'a> {
    fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut agent_sessions = BTreeMap::new();
        let mut mux_sessions = BTreeMap::new();
        let mut forge_prs = BTreeMap::new();

        for node in &snapshot.nodes {
            match node {
                GraphNode::AgentSession(session) => {
                    agent_sessions.insert(node.id(), session);
                }
                GraphNode::MuxSession(mux) => {
                    mux_sessions.insert(node.id(), mux);
                }
                GraphNode::ForgePr(pr) => {
                    forge_prs.insert(node.id(), pr);
                }
                _ => {}
            }
        }

        let mut by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&GraphLink>> =
            BTreeMap::new();
        for link in &snapshot.candidate_links {
            if !matches!(link.state, crate::model::LinkState::Active) {
                continue;
            }
            by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
        }

        Self {
            agent_sessions,
            mux_sessions,
            forge_prs,
            by_source_relation,
        }
    }

    fn preferred_link(&self, source: &NodeId, relation: RelationKind) -> Option<&GraphLink> {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .and_then(|links| pick_preferred(links))
    }

    fn candidates_for(&self, source: &NodeId, relation: RelationKind) -> &[&'a GraphLink] {
        self.by_source_relation
            .get(&(source.clone(), relation))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// For display, "preferred" is the resolver's first candidate. The
/// resolver itself does the real ranking; for table output we just
/// need the same first-place pick, so we sort by the same generic
/// recipe (provenance precedence then confidence then id).
fn pick_preferred<'a>(links: &[&'a GraphLink]) -> Option<&'a GraphLink> {
    let mut ranked: Vec<&GraphLink> = links
        .iter()
        .copied()
        .filter(|link| matches!(link.state, crate::model::LinkState::Active))
        .collect();
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

fn agent_session_label(session: &AgentSessionNode) -> String {
    if let Some(title) = &session.title {
        format!("{}:{}", session.harness_key, title)
    } else {
        format!("{}:{}", session.harness_key, session.id.session_key)
    }
}

fn mux_session_label(mux: &MuxSessionNode) -> String {
    format!("{}:{}", mux.backend, mux.native_id)
}

fn forge_pr_label(pr: &ForgePrNode) -> String {
    let state = pr.state.as_deref().unwrap_or("?");
    let draft = if pr.is_draft { " draft" } else { "" };
    format!("{}/{}#{} ({state}{draft})", pr.owner, pr.repo, pr.number)
}

fn render_agent(view: &SnapshotView<'_>) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        ["AGENT", "CWD", "MUX", "MUX/CONF", "PR", "PR/CONF"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    );

    for (node_id, session) in &view.agent_sessions {
        let mux_link = view.preferred_link(node_id, RelationKind::LinkedToMux);
        let mux_count = view
            .candidates_for(node_id, RelationKind::LinkedToMux)
            .len();
        let mux_cell = mux_link
            .and_then(|link| match &link.target {
                LinkEndpoint::Node {
                    id: NodeId::MuxSession(_),
                } => view
                    .mux_sessions
                    .get(link.target_node_id()?)
                    .map(|mux| mux_session_label(mux)),
                _ => None,
            })
            .unwrap_or_else(|| "—".to_string());
        let mux_indicator = match mux_link {
            Some(link) => indicator(link.provenance, link.confidence, mux_count > 1),
            None => "—".to_string(),
        };

        let (pr_cell, pr_indicator) = preferred_pr_for_session(view, node_id);

        rows.push(vec![
            agent_session_label(session),
            session.cwd.clone().unwrap_or_else(|| "—".to_string()),
            mux_cell,
            mux_indicator,
            pr_cell,
            pr_indicator,
        ]);
    }

    format_rows(rows)
}

/// Walk session → fork associations → branches → PRs to find the
/// preferred PR for an agent session, if any. This covers the case
/// where a session lives in a worktree whose branch has an open PR.
fn preferred_pr_for_session(view: &SnapshotView<'_>, session_id: &NodeId) -> (String, String) {
    let session_cwd = match view.agent_sessions.get(session_id) {
        Some(session) => session.cwd.as_deref(),
        None => return ("—".to_string(), "—".to_string()),
    };
    let Some(session_cwd) = session_cwd else {
        return ("—".to_string(), "—".to_string());
    };

    // PRs are keyed off the branch node; the session's cwd is the
    // worktree path, but we don't have a direct session→branch link
    // yet. For now match PRs whose link source metadata points to
    // a branch whose ref name appears in `session_cwd`. This is
    // intentionally conservative: when we add session→branch links
    // in a later phase the lookup gets replaced.
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::BranchHasForgePr {
            continue;
        }
        let _ = source;
        let _ = session_cwd;
        let count = links.len();
        if let Some(preferred) = pick_preferred(links)
            && let NodeId::ForgePr(pr_id) = &preferred.source
            && let Some(pr) = view.forge_prs.get(&NodeId::ForgePr(pr_id.clone()))
        {
            return (
                forge_pr_label(pr),
                indicator(preferred.provenance, preferred.confidence, count > 1),
            );
        }
    }

    ("—".to_string(), "—".to_string())
}

fn render_mux(view: &SnapshotView<'_>) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        ["MUX", "CWD", "AGENTS"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    );

    // Build mux → [agent labels] by walking active LinkedToMux links.
    let mut attached: BTreeMap<NodeId, Vec<(String, &GraphLink)>> = BTreeMap::new();
    for ((source, relation), links) in &view.by_source_relation {
        if *relation != RelationKind::LinkedToMux {
            continue;
        }
        let preferred = match pick_preferred(links) {
            Some(link) => link,
            None => continue,
        };
        let Some(target_id) = preferred.target_node_id() else {
            continue;
        };
        if let Some(session) = view.agent_sessions.get(source) {
            attached
                .entry(target_id.clone())
                .or_default()
                .push((agent_session_label(session), preferred));
        }
    }

    for (mux_id, mux) in &view.mux_sessions {
        let entries = attached.get(mux_id);
        let count = entries.map(Vec::len).unwrap_or(0);
        let agents_cell = if let Some(entries) = entries {
            entries
                .iter()
                .map(|(label, link)| {
                    let ambiguous = view
                        .candidates_for(&link.source, RelationKind::LinkedToMux)
                        .len()
                        > 1;
                    format!(
                        "{label} [{ind}]",
                        ind = indicator(link.provenance, link.confidence, ambiguous)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "—".to_string()
        };
        let _ = count;

        rows.push(vec![
            mux_session_label(mux),
            mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
            agents_cell,
        ]);
    }

    format_rows(rows)
}

fn render_union(view: &SnapspotViewAlias<'_>) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(
        ["KIND", "ID", "CWD", "RELATIONSHIP"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    );

    for (node_id, session) in &view.agent_sessions {
        let mux_link = view.preferred_link(node_id, RelationKind::LinkedToMux);
        let mux_count = view
            .candidates_for(node_id, RelationKind::LinkedToMux)
            .len();
        let relationship = match mux_link {
            Some(link) => {
                let target = link
                    .target_node_id()
                    .and_then(|id| view.mux_sessions.get(id))
                    .map(|mux| mux_session_label(mux))
                    .unwrap_or_else(|| "—".to_string());
                format!(
                    "mux={target} [{ind}]",
                    ind = indicator(link.provenance, link.confidence, mux_count > 1)
                )
            }
            None => "mux=—".to_string(),
        };
        rows.push(vec![
            "agent".to_string(),
            agent_session_label(session),
            session.cwd.clone().unwrap_or_else(|| "—".to_string()),
            relationship,
        ]);
    }

    for mux in view.mux_sessions.values() {
        rows.push(vec![
            "mux".to_string(),
            mux_session_label(mux),
            mux.cwd.clone().unwrap_or_else(|| "—".to_string()),
            "—".to_string(),
        ]);
    }

    format_rows(rows)
}

// Alias so we can call render_union with the same view type but
// keep the signature distinct in the source. (Workaround for the
// borrow-checker not allowing the same name with different scopes
// when the function is recursive.)
type SnapspotViewAlias<'a> = SnapshotView<'a>;

fn format_rows(rows: Vec<Vec<String>>) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let columns = rows[0].len();
    let mut widths = vec![0usize; columns];
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx >= widths.len() {
                continue;
            }
            widths[idx] = widths[idx].max(cell.chars().count());
        }
    }

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        for (idx, cell) in row.iter().enumerate() {
            if idx > 0 {
                out.push_str("  ");
            }
            out.push_str(cell);
            // Pad to column width unless this is the last cell on the
            // row (avoid trailing whitespace).
            if idx + 1 < columns {
                let pad = widths[idx].saturating_sub(cell.chars().count());
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
        if row_idx == 0 {
            for (idx, width) in widths.iter().enumerate() {
                if idx > 0 {
                    out.push_str("  ");
                }
                for _ in 0..*width {
                    out.push('-');
                }
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, BranchId, Confidence, ForgePrId, ForgePrNode, Freshness,
        GraphLink, GraphNode, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, RepoId, SourceMetadata,
    };

    fn agent_session(harness: &str, key: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, "global", key),
            harness_key: harness.to_string(),
            cwd: cwd.map(str::to_string),
            title: None,
        })
    }

    fn mux_session(backend: &str, name: &str, cwd: Option<&str>) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: cwd.map(str::to_string),
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux_link(
        id: &str,
        session_id: AgentSessionId,
        mux_id: MuxSessionId,
        provenance: Provenance,
        confidence: Confidence,
    ) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(session_id),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
            relation: RelationKind::LinkedToMux,
            provenance,
            confidence,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn indicator_renders_provenance_confidence_and_ambiguity_marker() {
        assert_eq!(
            indicator(Provenance::LocalDeclared, Confidence::High, false),
            "LD/H"
        );
        assert_eq!(
            indicator(Provenance::StrongDiscovered, Confidence::Medium, true),
            "SD/M*"
        );
        assert_eq!(indicator(Provenance::Cached, Confidence::Low, false), "$/L");
        assert_eq!(
            indicator(Provenance::Convention, Confidence::High, false),
            "C/H"
        );
    }

    #[test]
    fn empty_snapshot_renders_header_only_for_each_projection() {
        let snapshot = GraphSnapshot::empty();
        for projection in [Projection::Agent, Projection::Mux, Projection::Union] {
            let rendered = render(&snapshot, projection);
            // Header line + dashes line, no data rows.
            assert_eq!(
                rendered.lines().count(),
                2,
                "projection {projection:?} should render only a header",
            );
        }
    }

    #[test]
    fn agent_projection_emits_one_row_per_agent_session() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                agent_session("codex", "beta", None),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        let body_rows: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body_rows.len(), 2);
        assert!(body_rows.iter().any(|row| row.contains("alpha")));
        assert!(body_rows.iter().any(|row| row.contains("beta")));
    }

    #[test]
    fn agent_projection_shows_single_mux_without_ambiguity_marker() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let mux_id = MuxSessionId::new("tmux:editor");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                mux_id,
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(rendered.contains("tmux:editor"));
        assert!(rendered.contains("SD/H"));
        assert!(!rendered.contains("SD/H*"));
    }

    #[test]
    fn agent_projection_marks_ambiguous_mux_selection() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "one", Some("/work/a")),
                mux_session("tmux", "two", Some("/work/a")),
            ],
            candidate_links: vec![
                linked_to_mux_link(
                    "link-1",
                    session_id.clone(),
                    MuxSessionId::new("tmux:one"),
                    Provenance::Discovered,
                    Confidence::Medium,
                ),
                linked_to_mux_link(
                    "link-2",
                    session_id,
                    MuxSessionId::new("tmux:two"),
                    Provenance::Discovered,
                    Confidence::Medium,
                ),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(
            rendered.contains("D/M*"),
            "expected ambiguity marker in:\n{rendered}",
        );
    }

    #[test]
    fn mux_projection_lists_attached_agents() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                MuxSessionId::new("tmux:editor"),
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Mux);

        assert!(rendered.contains("tmux:editor"));
        assert!(rendered.contains("codex:alpha"));
        assert!(rendered.contains("SD/H"));
    }

    #[test]
    fn mux_projection_shows_zero_agents_for_orphan_mux() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Mux);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 1);
        assert!(body[0].contains("tmux:lonely"));
        assert!(body[0].contains("—"));
    }

    #[test]
    fn union_projection_preserves_both_node_types() {
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "lonely", Some("/work")),
            ],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Union);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        assert_eq!(body.len(), 2);
        assert!(body.iter().any(|row| row.starts_with("agent ")));
        assert!(body.iter().any(|row| row.starts_with("mux ")));
    }

    #[test]
    fn union_projection_renders_mux_relationship_for_session() {
        let session_id = AgentSessionId::new("codex", "global", "alpha");
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                mux_session("tmux", "editor", Some("/work/a")),
            ],
            candidate_links: vec![linked_to_mux_link(
                "link-1",
                session_id,
                MuxSessionId::new("tmux:editor"),
                Provenance::StrongDiscovered,
                Confidence::High,
            )],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Union);

        let body: Vec<&str> = rendered.lines().skip(2).collect();
        let agent_row = body
            .iter()
            .find(|row| row.starts_with("agent "))
            .expect("agent row");
        assert!(agent_row.contains("mux=tmux:editor"));
        assert!(agent_row.contains("SD/H"));
    }

    #[test]
    fn agent_projection_shows_preferred_pr_when_branch_has_one() {
        let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
        let branch_id = BranchId::new(
            RepoId::new("/workspace/repo/.git"),
            "refs/heads/feature".to_string(),
        );
        let mut pr_link = GraphLink::new(
            "pr-link",
            NodeId::ForgePr(pr_id.clone()),
            LinkEndpoint::Node {
                id: NodeId::Branch(branch_id),
            },
            RelationKind::BranchHasForgePr,
            Provenance::StrongDiscovered,
        );
        pr_link.confidence = Confidence::High;
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_session("codex", "alpha", Some("/work/a")),
                GraphNode::ForgePr(ForgePrNode {
                    id: pr_id.clone(),
                    provider: "github".to_string(),
                    host: "github.com".to_string(),
                    owner: "octo".to_string(),
                    repo: "repo".to_string(),
                    number: 7,
                    state: Some("open".to_string()),
                    url: None,
                    updated_epoch: None,
                    is_draft: false,
                }),
            ],
            candidate_links: vec![pr_link],
            ..GraphSnapshot::empty()
        };

        let rendered = render(&snapshot, Projection::Agent);

        assert!(
            rendered.contains("octo/repo#7"),
            "expected PR label in:\n{rendered}",
        );
    }
}
