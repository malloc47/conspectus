//! Graphviz DOT renderer for resolved [`GraphSnapshot`]s (ADR 0050,
//! backlog item `CSP-302`).
//!
//! The renderer is provider-neutral: shape and fill are keyed on
//! `NodeKind`, edge arrowhead on [`RelationKind`] category,
//! penwidth/color tint on [`Provenance`]. Candidate vs. resolved is
//! a styling difference, not a structural one: every resolved
//! relationship is also a candidate link (the
//! [`ResolvedRelationship::selected_link_id`]); the renderer marks
//! resolver-preferred candidates with a `★` suffix and dashes any
//! candidate that no resolved relationship selected.
//!
//! Emission is fully ordered so snapshot tests stay stable across
//! discovery reorderings (ADR 0050 decision 8).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use anyhow::Result;

use crate::model::{
    GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, NodeId, Provenance, RelationKind,
    ResolvedRelationship, UnresolvedEndpoint,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Inclusion {
    Include,
    Exclude,
}

#[derive(Copy, Clone, Debug)]
pub struct DotOptions {
    /// Include non-resolved candidate edges (and their unresolved-
    /// endpoint stub nodes). Defaults to `Include` per ADR 0050.
    pub candidates: Inclusion,
    /// Include `RuntimeProcess` nodes and edges touching them.
    /// Defaults to `Include` per ADR 0050.
    pub diagnostic_nodes: Inclusion,
}

impl Default for DotOptions {
    fn default() -> Self {
        Self {
            candidates: Inclusion::Include,
            diagnostic_nodes: Inclusion::Include,
        }
    }
}

pub fn render_graph_dot(snapshot: &GraphSnapshot, opts: DotOptions) -> Result<String> {
    /// One edge to emit, to a visible node or to an unresolved stub.
    struct EdgeRow<'a> {
        source: NodeId,
        target_label: String,
        link: &'a GraphLink,
        is_resolved: bool,
        unresolved: Option<&'a UnresolvedEndpoint>,
    }

    let mut out = String::new();
    writeln!(out, "digraph conspectus {{")?;
    writeln!(out, "  graph [rankdir=LR, fontname=\"Helvetica\"];")?;
    writeln!(
        out,
        "  node [fontname=\"Helvetica\", style=\"filled,rounded\"];"
    )?;
    writeln!(out, "  edge [fontname=\"Helvetica\", fontsize=10];")?;

    // ---------------------------------------------------------------
    // 1. Select visible nodes per filter flags.
    // ---------------------------------------------------------------
    let mut visible_nodes: BTreeMap<NodeId, &GraphNode> = BTreeMap::new();
    for node in &snapshot.nodes {
        if opts.diagnostic_nodes == Inclusion::Exclude
            && matches!(node, GraphNode::RuntimeProcess(_))
        {
            continue;
        }
        visible_nodes.insert(node.id(), node);
    }

    // ---------------------------------------------------------------
    // 2. Classify links: resolved-preferred vs. plain candidate.
    //    Both share the same edge representation; the resolved set
    //    just controls styling.
    // ---------------------------------------------------------------
    // ADR 0077: `selected_link_id` is `Option<String>` —
    // `filter_map` skips no-winner slots so the dot styling stays
    // restricted to actually-resolved edges.
    let selected_link_ids: BTreeSet<&str> = snapshot
        .resolved_relationships
        .iter()
        .filter_map(|r: &ResolvedRelationship| r.selected_link_id.as_deref())
        .collect();

    // ---------------------------------------------------------------
    // 3. Walk candidate links once, partitioning into:
    //      - edges between two visible nodes
    //      - edges to an unresolved-endpoint stub (synthesized node)
    //    Honor candidate/diagnostic filters.
    // ---------------------------------------------------------------
    let mut stub_nodes: BTreeMap<String, &UnresolvedEndpoint> = BTreeMap::new();
    let mut edges: Vec<EdgeRow<'_>> = Vec::new();

    for link in &snapshot.candidate_links {
        let is_resolved = selected_link_ids.contains(link.id.as_str());

        if !is_resolved && opts.candidates == Inclusion::Exclude {
            continue;
        }
        if !visible_nodes.contains_key(&link.source) {
            // Source node was filtered out (e.g. RuntimeProcess with
            // diagnostic-nodes=exclude). Skip the dangling edge.
            continue;
        }

        match &link.target {
            LinkEndpoint::Node { id } => {
                if !visible_nodes.contains_key(id) {
                    continue;
                }
                edges.push(EdgeRow {
                    source: link.source.clone(),
                    target_label: node_dot_id(id),
                    link,
                    is_resolved,
                    unresolved: None,
                });
            }
            LinkEndpoint::Unresolved { evidence } => {
                if opts.candidates == Inclusion::Exclude {
                    continue;
                }
                let stub_id = stub_dot_id(&link.id);
                stub_nodes.insert(stub_id.clone(), evidence);
                edges.push(EdgeRow {
                    source: link.source.clone(),
                    target_label: stub_id,
                    link,
                    is_resolved,
                    unresolved: Some(evidence),
                });
            }
        }
    }

    // ---------------------------------------------------------------
    // 4. Emit subgraph clusters in fixed kind order, with nodes
    //    sorted by NodeId.
    // ---------------------------------------------------------------
    for (cluster_idx, kind) in NODE_KIND_ORDER.iter().enumerate() {
        let members: Vec<&GraphNode> = visible_nodes
            .values()
            .copied()
            .filter(|n| node_kind_tag(n) == *kind)
            .collect();
        if members.is_empty() {
            continue;
        }
        writeln!(out, "  subgraph cluster_{cluster_idx}_{kind} {{")?;
        writeln!(
            out,
            "    label=\"{}\"; style=dashed; color=\"#90a4ae\"; fontcolor=\"#455a64\";",
            kind_display(kind)
        )?;
        for node in members {
            let id = node.id();
            let style = node_style(node);
            writeln!(
                out,
                "    {dot_id} [label={label}, shape={shape}, fillcolor=\"{fill}\"{extra}];",
                dot_id = node_dot_id(&id),
                label = dot_quote(&node_label(node)),
                shape = style.shape,
                fill = style.fill,
                extra = style.extra_attrs,
            )?;
        }
        writeln!(out, "  }}")?;
    }

    // ---------------------------------------------------------------
    // 5. Emit unresolved-endpoint stub nodes outside any cluster.
    // ---------------------------------------------------------------
    if !stub_nodes.is_empty() {
        writeln!(out, "  // Unresolved endpoints (dashed stubs)")?;
        for (stub_id, evidence) in &stub_nodes {
            writeln!(
                out,
                "  {stub} [label={label}, shape=circle, style=\"dashed\", fillcolor=\"#ffffff\", color=\"#90a4ae\", fontsize=9, width=0.3, height=0.3];",
                stub = stub_id,
                label = dot_quote(&unresolved_label(evidence)),
            )?;
        }
    }

    // ---------------------------------------------------------------
    // 6. Emit edges in deterministic order.
    // ---------------------------------------------------------------
    edges.sort_by(|a, b| {
        a.source
            .to_string()
            .cmp(&b.source.to_string())
            .then(
                a.link
                    .relation
                    .snake_case()
                    .cmp(b.link.relation.snake_case()),
            )
            .then(a.target_label.cmp(&b.target_label))
            .then(
                b.link
                    .provenance
                    .precedence()
                    .cmp(&a.link.provenance.precedence()),
            )
            .then(a.link.id.cmp(&b.link.id))
    });

    for edge in &edges {
        let style = edge_style(edge.link, edge.is_resolved, edge.unresolved.is_some());
        let label = edge_label(edge.link, edge.is_resolved);
        writeln!(
            out,
            "  {src} -> {tgt} [label={label}, color=\"{color}\", penwidth={pw:.1}, style=\"{line}\", arrowhead={arrow}{extras}];",
            src = node_dot_id(&edge.source),
            tgt = edge.target_label,
            label = dot_quote(&label),
            color = style.color,
            pw = style.penwidth,
            line = style.line,
            arrow = style.arrow,
            extras = style.extras,
        )?;
    }

    writeln!(out, "}}")?;
    Ok(out)
}

// =========================================================================
// Node kind ordering and labels
// =========================================================================

const NODE_KIND_ORDER: &[&str] = &[
    "workspace",
    "repo",
    "checkout",
    "branch",
    "fork",
    "agent_session",
    "mux_session",
    "runtime_process",
    "forge_pr",
    "pin",
];

fn node_kind_tag(node: &GraphNode) -> &'static str {
    match node {
        GraphNode::Repo(_) => "repo",
        GraphNode::Checkout(_) => "checkout",
        GraphNode::Workspace(_) => "workspace",
        GraphNode::AgentSession(_) => "agent_session",
        GraphNode::MuxSession(_) => "mux_session",
        GraphNode::Pin(_) => "pin",
        GraphNode::RuntimeProcess(_) => "runtime_process",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

fn kind_display(tag: &str) -> &'static str {
    match tag {
        "repo" => "Repos",
        "checkout" => "Checkouts",
        "workspace" => "Workspaces",
        "agent_session" => "Agent Sessions",
        "mux_session" => "Mux Sessions",
        "pin" => "Pins",
        "runtime_process" => "Runtime Processes",
        "branch" => "Branches",
        "fork" => "Forks",
        "forge_pr" => "Forge PRs",
        _ => "Other",
    }
}

struct NodeStyle {
    shape: &'static str,
    fill: &'static str,
    extra_attrs: &'static str,
}

fn node_style(node: &GraphNode) -> NodeStyle {
    match node {
        GraphNode::Repo(_) => NodeStyle {
            shape: "folder",
            fill: "#c8e6c9",
            extra_attrs: "",
        },
        GraphNode::Checkout(_) => NodeStyle {
            shape: "note",
            fill: "#a5d6a7",
            extra_attrs: "",
        },
        GraphNode::Workspace(_) => NodeStyle {
            shape: "tab",
            fill: "#b3e5fc",
            extra_attrs: "",
        },
        GraphNode::Branch(_) => NodeStyle {
            shape: "cds",
            fill: "#fff9c4",
            extra_attrs: "",
        },
        GraphNode::AgentSession(_) => NodeStyle {
            shape: "box",
            fill: "#f8bbd0",
            extra_attrs: "",
        },
        GraphNode::MuxSession(_) => NodeStyle {
            shape: "box3d",
            fill: "#d1c4e9",
            extra_attrs: "",
        },
        GraphNode::Pin(_) => NodeStyle {
            shape: "box",
            fill: "#b2dfdb",
            extra_attrs: ", penwidth=2",
        },
        GraphNode::Fork(_) => NodeStyle {
            shape: "octagon",
            fill: "#ffe0b2",
            extra_attrs: "",
        },
        GraphNode::ForgePr(_) => NodeStyle {
            shape: "component",
            fill: "#ffccbc",
            extra_attrs: "",
        },
        GraphNode::RuntimeProcess(_) => NodeStyle {
            shape: "ellipse",
            fill: "#eceff1",
            extra_attrs: ", peripheries=2",
        },
    }
}

fn node_label(node: &GraphNode) -> String {
    match node {
        GraphNode::Repo(n) => {
            let trail = last_path_segment(&n.common_dir);
            format!("{}\\n{}", trail, short_id(&n.id.to_string()))
        }
        GraphNode::Checkout(n) => {
            let trail = last_path_segment(&n.root);
            format!("{}\\n{}", trail, short_id(&n.id.to_string()))
        }
        GraphNode::Workspace(n) => {
            let name = n.name.clone().unwrap_or_else(|| last_path_segment(&n.root));
            format!("{}\\n{}", name, short_id(&n.id.to_string()))
        }
        GraphNode::Branch(n) => {
            let short = n.refname.strip_prefix("refs/heads/").unwrap_or(&n.refname);
            format!("{}\\n{}", short, short_id(&n.id.to_string()))
        }
        GraphNode::AgentSession(n) => {
            let title = n
                .title
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("session");
            format!(
                "{}: {}\\n{}",
                n.harness_key,
                truncate(title, 32),
                short_id(&n.id.to_string()),
            )
        }
        GraphNode::MuxSession(n) => {
            format!(
                "{}:{}\\n{}",
                n.backend,
                truncate(&n.native_id, 20),
                short_id(&n.id.to_string()),
            )
        }
        GraphNode::Pin(n) => {
            format!(
                "pin:{}\\n{}",
                truncate(&n.display_name, 24),
                short_id(&n.id.to_string()),
            )
        }
        GraphNode::Fork(n) => {
            let name = n.name.as_deref().unwrap_or(&n.provider_source_key);
            format!(
                "{}: {}\\n{}",
                n.provider,
                truncate(name, 24),
                short_id(&n.id.to_string()),
            )
        }
        GraphNode::ForgePr(n) => {
            format!(
                "#{} {}/{}\\n{}",
                n.number,
                n.owner,
                truncate(&n.repo, 24),
                n.state.as_deref().unwrap_or(""),
            )
        }
        GraphNode::RuntimeProcess(n) => {
            let role = n
                .role
                .map_or_else(|| "process".to_string(), |r| format!("{r:?}"));
            let pid = n.pid.map(|p| p.to_string()).unwrap_or_default();
            let cmd = n.command.as_deref().unwrap_or("");
            format!("{role} {pid}\\n{}", truncate(cmd, 30))
        }
    }
}

fn unresolved_label(e: &UnresolvedEndpoint) -> String {
    let mut bits = vec![e.node_type.clone()];
    if let Some(h) = &e.harness_key {
        bits.push(h.clone());
    }
    if let Some(n) = &e.native_id {
        bits.push(truncate(n, 16));
    }
    format!("?\\n{}", bits.join(":"))
}

// =========================================================================
// Edge styling
// =========================================================================

struct EdgeStyle {
    color: &'static str,
    penwidth: f32,
    line: &'static str,
    arrow: &'static str,
    extras: String,
}

fn edge_style(link: &GraphLink, is_resolved: bool, is_unresolved_target: bool) -> EdgeStyle {
    // Ignored or overridden candidates render red regardless of
    // provenance — they're evidence the resolver chose to set aside.
    let (base_color, state_extras): (&str, String) = match &link.state {
        LinkState::Active => (provenance_color(link.provenance), String::new()),
        LinkState::Ignored { reason } => (
            "#e53935",
            format!(
                ", tooltip=\"ignored: {}\"",
                dot_quote_inner(reason.as_deref().unwrap_or(""))
            ),
        ),
        LinkState::Overridden { by, reason } => (
            "#e53935",
            format!(
                ", tooltip=\"overridden by {}: {}\"",
                dot_quote_inner(by),
                dot_quote_inner(reason.as_deref().unwrap_or(""))
            ),
        ),
    };

    let penwidth = match link.provenance {
        Provenance::LocalDeclared
        | Provenance::GlobalDeclared
        | Provenance::LocalPin
        | Provenance::GlobalPin => 2.5,
        Provenance::StrongDiscovered => 1.6,
        Provenance::Discovered => 1.0,
        Provenance::Convention => 0.8,
        Provenance::Cached => 0.6,
    };

    // Solid only for resolver-preferred candidates between concrete
    // nodes. Everything else — losing candidates, ignored/overridden
    // links, unresolved-endpoint stubs — is dashed.
    let line = if is_resolved && !is_unresolved_target {
        "solid"
    } else {
        "dashed"
    };

    EdgeStyle {
        color: base_color,
        penwidth,
        line,
        arrow: relation_arrowhead(&link.relation),
        extras: state_extras,
    }
}

fn edge_label(link: &GraphLink, is_resolved: bool) -> String {
    let star = if is_resolved { " ★" } else { "" };
    format!(
        "{}{}\\n[{} · {}]",
        link.relation.snake_case(),
        star,
        link.provenance.snake_case(),
        link.confidence.snake_case(),
    )
}

fn provenance_color(p: Provenance) -> &'static str {
    match p {
        Provenance::LocalDeclared | Provenance::GlobalDeclared => "#1565c0",
        Provenance::LocalPin | Provenance::GlobalPin => "#6a1b9a",
        Provenance::StrongDiscovered => "#212121",
        Provenance::Discovered => "#424242",
        Provenance::Convention => "#757575",
        Provenance::Cached => "#9e9e9e",
    }
}

fn relation_arrowhead(rel: &RelationKind) -> &'static str {
    match rel {
        // Containment: diamond head (UML composition).
        RelationKind::WorkspaceContainsRepo
        | RelationKind::BelongsToRepo
        | RelationKind::CheckedOutBranch
        | RelationKind::MuxContainsProcess => "diamond",
        // Linkage between peer entities: vee.
        RelationKind::LinkedToMux
        | RelationKind::AssociatedWith
        | RelationKind::AssociatedBranch
        | RelationKind::ReferencedCheckout
        | RelationKind::RootedIn
        | RelationKind::RootedAtPath
        | RelationKind::BranchHasForgePr
        | RelationKind::PinTargetsMux
        | RelationKind::PinRealizedBySession => "vee",
        // Lineage / parent-child: normal arrow.
        RelationKind::ParentFork
        | RelationKind::ParentSession
        | RelationKind::ChildSession
        | RelationKind::ForksWorkspace
        | RelationKind::ForksRepo
        | RelationKind::CreatedCheckout
        | RelationKind::CreatedBranch => "normal",
        // Runtime-process attribution: dot heads to read as "observes".
        RelationKind::ProcessIdentifiesSession | RelationKind::ProcessCandidatesSession => "dot",
    }
}

// =========================================================================
// DOT id / string helpers
// =========================================================================

fn node_dot_id(id: &NodeId) -> String {
    format!("\"n_{}\"", sanitize_id(&id.to_string()))
}

fn stub_dot_id(link_id: &str) -> String {
    format!("\"u_{}\"", sanitize_id(link_id))
}

fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Quote a string as a DOT label, escaping the few characters that
/// the DOT grammar treats specially inside double quotes. Newlines in
/// labels are encoded as `\n` escape sequences by the caller (we
/// build labels with the literal two-character sequence `\` + `n`),
/// so this function only needs to escape `"` and `\` that are not
/// already part of an escape sequence.
fn dot_quote(label: &str) -> String {
    format!("\"{}\"", dot_quote_inner(label))
}

fn dot_quote_inner(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => {
                // Preserve `\n` / `\l` / `\r` escape sequences; escape
                // a lone backslash.
                match chars.peek() {
                    Some('n' | 'l' | 'r' | 't') => {
                        out.push('\\');
                        out.push(chars.next().unwrap());
                    }
                    _ => out.push_str("\\\\"),
                }
            }
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

fn last_path_segment(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .find(|seg| !seg.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn short_id(full: &str) -> String {
    // Use the suffix after the last `:` for compactness; ids are
    // structured prefix:scope:native.
    full.rsplit_once(':')
        .map_or_else(|| full.to_string(), |(_, tail)| tail.to_string())
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_renders_header_and_braces() {
        let out = render_graph_dot(&GraphSnapshot::empty(), DotOptions::default()).unwrap();
        assert!(out.starts_with("digraph conspectus {"));
        assert!(out.trim_end().ends_with('}'));
    }

    #[test]
    fn dot_quote_escapes_quotes_and_preserves_escape_sequences() {
        assert_eq!(dot_quote("hello"), "\"hello\"");
        assert_eq!(dot_quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(dot_quote("first\\nsecond"), "\"first\\nsecond\"");
        assert_eq!(dot_quote("trailing\\"), "\"trailing\\\\\"");
    }

    #[test]
    fn sanitize_id_replaces_unsafe_chars() {
        // `repo:/a/b` has two non-alnum chars between `o` and `a`
        // (`:` and `/`) and one between `a` and `b` (`/`).
        assert_eq!(sanitize_id("repo:/a/b"), "repo__a_b");
    }
}
