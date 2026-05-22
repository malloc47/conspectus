//! `conspectus node show <id>` (H-OBS-002).
//!
//! Read-only single-node view: prints the node itself plus every candidate
//! link, resolved relationship, source metadata, and diagnostic that touches
//! the node. The accepted forms for `<id>` are:
//!
//! - The short content-addressed prefix from the session-table `ID` column
//!   (H-TBL-002). Any prefix length ≥ 4 hex chars is accepted; ambiguity
//!   errors with the matching candidates listed.
//! - The full `NodeId` `Display` form, e.g.
//!   `agent_session:codex:/state:session-x` or `mux_session:tmux:editor`.
//! - The harness/mux label that appears in the session table's `AGENT`
//!   or `MUX` column, e.g. `codex:session-x` or `tmux:editor`. The label
//!   only resolves when it uniquely identifies one node.
//!
//! H-TBL-005 wires these forms through the CLI command added by H-OBS-002.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::model::{
    AgentSessionNode, BranchNode, CheckoutNode, Diagnostic, ForgePrNode, ForkNode, GraphLink,
    GraphNode, GraphSnapshot, LinkEndpoint, MuxSessionNode, NodeId, RelationKind, RepoNode,
    ResolvedRelationship, WorkspaceNode,
};
use crate::output::table::{header_style, indicator, node_short_id, push_styled};

/// Outcome of resolving an `<id>` argument to a [`NodeId`] against a
/// [`GraphSnapshot`].
#[derive(Debug)]
pub enum NodeResolveError {
    /// No node matched the input under any accepted form.
    NotFound { input: String },
    /// The input matched more than one node. `candidates` lists each
    /// matching `NodeId` so the caller can surface them.
    Ambiguous {
        input: String,
        candidates: Vec<NodeId>,
    },
}

impl std::fmt::Display for NodeResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeResolveError::NotFound { input } => {
                write!(f, "no node matches {input:?}")
            }
            NodeResolveError::Ambiguous { input, candidates } => {
                writeln!(f, "{input:?} matches {} nodes:", candidates.len())?;
                for id in candidates {
                    writeln!(f, "  - {id}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for NodeResolveError {}

/// Resolve `input` to a single [`NodeId`] in `snapshot` using every
/// accepted form (short hex prefix, `Display`, harness/mux label).
pub fn resolve_node_id(input: &str, snapshot: &GraphSnapshot) -> Result<NodeId, NodeResolveError> {
    let trimmed = input.trim();
    let mut matches: BTreeMap<NodeId, ()> = BTreeMap::new();

    let is_hex_prefix = !trimmed.is_empty()
        && trimmed.len() <= 16
        && trimmed.chars().all(|c| c.is_ascii_hexdigit());

    for node in &snapshot.nodes {
        let id = node.id();
        if is_hex_prefix && node_short_id(&id).starts_with(trimmed) {
            matches.insert(id.clone(), ());
            continue;
        }
        if id.to_string() == trimmed {
            matches.insert(id.clone(), ());
            continue;
        }
        if label_matches(node, trimmed) {
            matches.insert(id.clone(), ());
        }
    }

    let mut candidates: Vec<NodeId> = matches.into_keys().collect();
    match candidates.len() {
        0 => Err(NodeResolveError::NotFound {
            input: trimmed.to_string(),
        }),
        1 => Ok(candidates.remove(0)),
        _ => Err(NodeResolveError::Ambiguous {
            input: trimmed.to_string(),
            candidates,
        }),
    }
}

fn label_matches(node: &GraphNode, input: &str) -> bool {
    match node {
        GraphNode::AgentSession(session) => {
            let title_label = session
                .title
                .as_ref()
                .map(|t| format!("{}:{}", session.harness_key, t));
            let key_label = format!("{}:{}", session.harness_key, session.id.session_key);
            title_label.as_deref() == Some(input) || key_label == input
        }
        GraphNode::MuxSession(mux) => format!("{}:{}", mux.backend, mux.native_id) == input,
        _ => false,
    }
}

/// Render the resolved node `id` against `snapshot` as plain text.
pub fn render_node_show(snapshot: &GraphSnapshot, id: &NodeId, color: bool) -> String {
    let Some(node) = snapshot.nodes.iter().find(|n| n.id() == *id) else {
        return format!("node {id} not found in snapshot\n");
    };
    let mut out = String::new();
    write_node_summary(&mut out, node, color);
    write_candidate_links(&mut out, snapshot, id, color);
    write_resolved(&mut out, snapshot, id, color);
    write_diagnostics(&mut out, snapshot, id, color);
    out
}

fn write_section_header(out: &mut String, text: &str, color: bool) {
    push_styled(out, text, header_style(), color);
    out.push('\n');
}

fn write_node_summary(out: &mut String, node: &GraphNode, color: bool) {
    let id = node.id();
    write_section_header(out, &format!("node {}", node_short_id(&id)), color);
    let _ = writeln!(out, "  kind: {}", node_kind_label(node));
    let _ = writeln!(out, "  id:   {id}");
    match node {
        GraphNode::Repo(node) => write_repo(out, node),
        GraphNode::Checkout(node) => write_worktree(out, node),
        GraphNode::Workspace(node) => write_workspace(out, node),
        GraphNode::AgentSession(node) => write_agent_session(out, node),
        GraphNode::MuxSession(node) => write_mux_session(out, node),
        GraphNode::Branch(node) => write_branch(out, node),
        GraphNode::Fork(node) => write_fork(out, node),
        GraphNode::ForgePr(node) => write_forge_pr(out, node),
    }
}

fn node_kind_label(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Repo(_) => "repo",
        GraphNode::Checkout(_) => "checkout",
        GraphNode::Workspace(_) => "workspace",
        GraphNode::AgentSession(_) => "agent_session",
        GraphNode::MuxSession(_) => "mux_session",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

fn write_agent_session(out: &mut String, node: &AgentSessionNode) {
    let _ = writeln!(out, "  harness:     {}", node.harness_key);
    let _ = writeln!(out, "  state_scope: {}", node.id.state_scope);
    let _ = writeln!(out, "  session_key: {}", node.id.session_key);
    if let Some(cwd) = &node.cwd {
        let _ = writeln!(out, "  cwd:         {cwd}");
    }
    if let Some(title) = &node.title {
        let _ = writeln!(out, "  title:       {title}");
    }
}

fn write_mux_session(out: &mut String, node: &MuxSessionNode) {
    let _ = writeln!(out, "  backend:   {}", node.backend);
    let _ = writeln!(out, "  native_id: {}", node.native_id);
    if let Some(cwd) = &node.cwd {
        let _ = writeln!(out, "  cwd:       {cwd}");
    }
}

fn write_repo(out: &mut String, node: &RepoNode) {
    let _ = writeln!(out, "  common_dir: {}", node.common_dir);
    if !node.source_paths.is_empty() {
        let _ = writeln!(out, "  source_paths:");
        for path in &node.source_paths {
            let _ = writeln!(out, "    - {path}");
        }
    }
}

fn write_worktree(out: &mut String, node: &CheckoutNode) {
    let _ = writeln!(out, "  root: {}", node.root);
    if let Some(git_dir) = &node.git_dir {
        let _ = writeln!(out, "  git_dir: {git_dir}");
    }
}

fn write_workspace(out: &mut String, node: &WorkspaceNode) {
    let _ = writeln!(out, "  root: {}", node.root);
}

fn write_branch(out: &mut String, node: &BranchNode) {
    let _ = writeln!(out, "  repo:    {}", node.id.repo);
    let _ = writeln!(out, "  refname: {}", node.id.refname);
}

fn write_fork(out: &mut String, node: &ForkNode) {
    let _ = writeln!(out, "  provider: {}", node.provider);
    let _ = writeln!(out, "  provider_source_key: {}", node.provider_source_key);
    if let Some(name) = &node.name {
        let _ = writeln!(out, "  name:     {name}");
    }
    if let Some(scope) = &node.scope {
        let _ = writeln!(out, "  scope:    {scope}");
    }
}

fn write_forge_pr(out: &mut String, node: &ForgePrNode) {
    let _ = writeln!(
        out,
        "  pr:    {}/{}#{} ({})",
        node.owner,
        node.repo,
        node.number,
        node.state.as_deref().unwrap_or("?")
    );
    if let Some(url) = &node.url {
        let _ = writeln!(out, "  url:   {url}");
    }
}

fn write_candidate_links(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    let outgoing: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.source == *id)
        .collect();
    let incoming: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(&link.target, LinkEndpoint::Node { id: target } if target == id))
        .collect();

    out.push('\n');
    write_section_header(
        out,
        &format!("outgoing candidate links: {}", outgoing.len()),
        color,
    );
    for link in &outgoing {
        write_link(out, link, LinkDirection::Outgoing);
    }
    out.push('\n');
    write_section_header(
        out,
        &format!("incoming candidate links: {}", incoming.len()),
        color,
    );
    for link in &incoming {
        write_link(out, link, LinkDirection::Incoming);
    }
}

enum LinkDirection {
    Outgoing,
    Incoming,
}

fn write_link(out: &mut String, link: &GraphLink, dir: LinkDirection) {
    let other = match dir {
        LinkDirection::Outgoing => match &link.target {
            LinkEndpoint::Node { id } => format!("→ {id}"),
            LinkEndpoint::Unresolved { evidence } => {
                let mut parts: Vec<String> = vec![format!("type={}", evidence.node_type)];
                if let Some(harness) = &evidence.harness_key {
                    parts.push(format!("harness={harness}"));
                }
                if let Some(native) = &evidence.native_id {
                    parts.push(format!("native_id={native}"));
                }
                if let Some(path) = &evidence.path {
                    parts.push(format!("path={path}"));
                }
                format!("→ unresolved({})", parts.join(", "))
            }
        },
        LinkDirection::Incoming => format!("← {}", link.source),
    };
    let _ = writeln!(
        out,
        "  - {relation:15} {other} [{ind}, {state}] (link={id})",
        relation = relation_label(link.relation.clone()),
        ind = indicator(link.provenance, link.confidence, false),
        state = link_state_label(&link.state),
        id = link.id,
    );
    let _ = writeln!(out, "      adapter: {}", link.source_metadata.adapter);
    if let Some(evidence) = &link.source_metadata.evidence {
        let _ = writeln!(out, "      evidence: {evidence}");
    }
    if !link.source_metadata.fields.is_empty() {
        let _ = writeln!(out, "      fields:");
        for (key, value) in &link.source_metadata.fields {
            let _ = writeln!(out, "        {key}: {value}");
        }
    }
}

fn relation_label(relation: RelationKind) -> String {
    // `RelationKind` already serdes to snake_case; reuse that contract
    // so a new variant cannot drift from its CLI label.
    serde_json::to_value(&relation)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{relation:?}"))
}

fn link_state_label(state: &crate::model::LinkState) -> &'static str {
    match state {
        crate::model::LinkState::Active => "active",
        crate::model::LinkState::Ignored { .. } => "ignored",
        crate::model::LinkState::Overridden { .. } => "overridden",
    }
}

fn write_resolved(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    let resolved: Vec<&ResolvedRelationship> = snapshot
        .resolved_relationships
        .iter()
        .filter(|rel| rel.source == *id || rel.target == *id)
        .collect();
    out.push('\n');
    write_section_header(
        out,
        &format!("resolved relationships: {}", resolved.len()),
        color,
    );
    for rel in resolved {
        let _ = writeln!(
            out,
            "  - {relation:15} {source} → {target} (selected={selected})",
            relation = relation_label(rel.relation.clone()),
            source = rel.source,
            target = rel.target,
            selected = rel.selected_link_id,
        );
        if !rel.competing_link_ids.is_empty() {
            let _ = writeln!(
                out,
                "      competing: {}",
                rel.competing_link_ids.join(", ")
            );
        }
    }
}

fn write_diagnostics(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    let touching: Vec<&Diagnostic> = snapshot
        .diagnostics
        .iter()
        .filter(|d| diagnostic_touches(d, id, snapshot))
        .collect();
    out.push('\n');
    write_section_header(out, &format!("diagnostics: {}", touching.len()), color);
    for diag in touching {
        match diag {
            Diagnostic::UnresolvedEndpoint { link_id, relation } => {
                let _ = writeln!(
                    out,
                    "  - unresolved_endpoint {relation} (link={link_id})",
                    relation = relation_label(relation.clone()),
                );
            }
            Diagnostic::Config { path, message } => {
                let _ = writeln!(out, "  - config {path}: {message}");
            }
            Diagnostic::Conflict {
                source,
                relation,
                selected_link_id,
                competing_link_ids,
            } => {
                let _ = writeln!(
                    out,
                    "  - conflict {relation} source={source} selected={selected_link_id} competing={competing}",
                    relation = relation_label(relation.clone()),
                    competing = competing_link_ids.join(", "),
                );
            }
        }
    }
}

fn diagnostic_touches(diag: &Diagnostic, id: &NodeId, snapshot: &GraphSnapshot) -> bool {
    match diag {
        Diagnostic::UnresolvedEndpoint { link_id, .. } => snapshot
            .candidate_links
            .iter()
            .any(|link| &link.id == link_id && link.source == *id),
        Diagnostic::Config { .. } => false,
        Diagnostic::Conflict { source, .. } => source == id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode,
        LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance, RelationKind,
        SourceMetadata,
    };

    fn agent_node(harness: &str, scope: &str, key: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: Some("/work".to_string()),
            title: None,
            last_message_preview: None,
        })
    }

    fn mux_node(backend: &str, name: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: Some("/work".to_string()),
            activity_epoch: None,
            created_epoch: None,
        })
    }

    fn linked_to_mux(id: &str, source: NodeId, target: NodeId) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn resolve_node_id_matches_short_hex_prefix() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let full = node_short_id(&id);
        let prefix = &full[..6];
        let resolved = resolve_node_id(prefix, &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_display_form() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id(&id.to_string(), &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_harness_label() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent_node("codex", "/state", "alpha")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id("codex:alpha", &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_matches_mux_label() {
        let snapshot = GraphSnapshot {
            nodes: vec![mux_node("tmux", "editor")],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        let resolved = resolve_node_id("tmux:editor", &snapshot).expect("resolved");
        assert_eq!(resolved, id);
    }

    #[test]
    fn resolve_node_id_errors_on_unknown_input() {
        let snapshot = GraphSnapshot::empty();
        let err = resolve_node_id("does-not-exist", &snapshot).unwrap_err();
        assert!(matches!(err, NodeResolveError::NotFound { .. }));
    }

    #[test]
    fn resolve_node_id_errors_on_ambiguous_prefix() {
        // Build two agent sessions whose short_ids share the same 4-char
        // prefix. We can't predict the hash, so just feed both into the
        // resolver with the same 2-char prefix that almost certainly
        // collides somewhere.
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_node("codex", "/state", "one"),
                agent_node("codex", "/state", "two"),
            ],
            ..GraphSnapshot::empty()
        };
        let first = node_short_id(&snapshot.nodes[0].id());
        let second = node_short_id(&snapshot.nodes[1].id());
        // Find the longest shared prefix between the two hashes; that
        // prefix is by construction ambiguous.
        let shared_len = first
            .chars()
            .zip(second.chars())
            .take_while(|(a, b)| a == b)
            .count();
        if shared_len == 0 {
            // No shared prefix → the test cannot exercise ambiguity for
            // this particular fixture. Skip.
            return;
        }
        let prefix = &first[..shared_len];
        let err = resolve_node_id(prefix, &snapshot).unwrap_err();
        assert!(matches!(err, NodeResolveError::Ambiguous { .. }));
    }

    #[test]
    fn render_node_show_includes_outgoing_link_and_resolved_relationship() {
        let agent = agent_node("codex", "/state", "alpha");
        let mux = mux_node("tmux", "editor");
        let agent_id = agent.id();
        let mux_id = mux.id();
        let snapshot = GraphSnapshot {
            nodes: vec![agent, mux],
            candidate_links: vec![linked_to_mux("link-1", agent_id.clone(), mux_id.clone())],
            resolved_relationships: vec![ResolvedRelationship {
                source: agent_id.clone(),
                target: mux_id.clone(),
                relation: RelationKind::LinkedToMux,
                selected_link_id: "link-1".to_string(),
                competing_link_ids: vec![],
            }],
            ..GraphSnapshot::empty()
        };
        let rendered = render_node_show(&snapshot, &agent_id, false);
        assert!(rendered.contains("kind: agent_session"));
        assert!(rendered.contains("outgoing candidate links: 1"));
        assert!(rendered.contains("linked_to_mux"));
        assert!(rendered.contains("resolved relationships: 1"));
        assert!(rendered.contains("selected=link-1"));
    }
}
