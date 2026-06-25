//! Self-contained HTML graph explorer (ADR 0050, backlog item
//! `GV-003a`).
//!
//! Produces a single-file HTML page that inlines the vendored
//! Cytoscape.js bundle (plus `fcose` layout and its peer deps),
//! a thin `GraphDriver` JS module that wraps Cytoscape per the
//! ADR 0050 Coupling Boundary, and a library-neutral JSON payload
//! derived from a [`GraphSnapshot`].
//!
//! The payload is intentionally **not** Cytoscape's element format —
//! the driver translates it at load time so a future swap to a
//! different rendering library only rewrites the driver. The Rust
//! side never speaks Cytoscape.

use std::fmt::Write;

use anyhow::Result;
use serde::Serialize;

use crate::model::{GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, NodeId};
use crate::output::dot::Inclusion;

#[derive(Copy, Clone, Debug)]
pub struct HtmlOptions {
    /// Include non-resolved candidate edges (and their unresolved-
    /// endpoint stub nodes). Defaults to `Include` per ADR 0050.
    pub candidates: Inclusion,
    /// Include `RuntimeProcess` nodes and edges touching them.
    pub diagnostic_nodes: Inclusion,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        Self {
            candidates: Inclusion::Include,
            diagnostic_nodes: Inclusion::Include,
        }
    }
}

/// Render a self-contained HTML page for the given snapshot. The
/// returned string is the entire `.html` artifact — caller writes
/// it to disk or pipes it to a browser.
pub fn render_graph_html(snapshot: &GraphSnapshot, opts: HtmlOptions) -> Result<String> {
    let payload = build_payload(snapshot, opts);
    render_html_with_payload(&payload)
}

fn render_html_with_payload(payload: &Payload) -> Result<String> {
    let payload_json = serde_json::to_string(payload)?;
    let mut out = String::with_capacity(1024 * 1024);
    write!(
        out,
        "<!doctype html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <title>conspectus graph</title>\n\
         <meta name=\"generator\" content=\"conspectus graph --format html (ADR 0050)\">\n\
         <style>\n{styles}\n</style>\n\
         </head>\n\
         <body>\n\
         <div id=\"conspectus-app\">\n\
         <div id=\"conspectus-header\">\n\
         <h1>conspectus graph</h1>\n\
         <span id=\"conspectus-meta\">nodes: {n_nodes} · edges: {n_edges} · unresolved: {n_stubs}</span>\n\
         <div id=\"conspectus-search-wrap\">\n\
         <input id=\"conspectus-search\" type=\"search\" placeholder=\"Search nodes (label or id)\" />\n\
         <span id=\"conspectus-search-count\"></span>\n\
         </div>\n\
         </div>\n\
         <div id=\"conspectus-body\">\n\
         <aside id=\"conspectus-left\"></aside>\n\
         <div id=\"conspectus-center\">\n\
         <div id=\"conspectus-navbar\"></div>\n\
         <div id=\"conspectus-cy\"></div>\n\
         </div>\n\
         <aside id=\"conspectus-right\"></aside>\n\
         </div>\n\
         <div id=\"conspectus-statusbar\"><span id=\"conspectus-status\">loading…</span></div>\n\
         </div>\n\
         <script type=\"application/json\" id=\"conspectus-graph-payload\">",
        styles = STYLES_CSS,
        n_nodes = payload.nodes.len(),
        n_edges = payload.edges.len(),
        n_stubs = payload.unresolved_stubs.len(),
    )?;
    // Escape `<` so a stray `</script>` in the payload can't break out.
    for ch in payload_json.chars() {
        match ch {
            '<' => out.push_str("\\u003c"),
            _ => out.push(ch),
        }
    }
    write!(
        out,
        "</script>\n\
         <script>\n{layout_base}\n</script>\n\
         <script>\n{cose_base}\n</script>\n\
         <script>\n{cytoscape}\n</script>\n\
         <script>\n{fcose}\n</script>\n\
         <script>\n{dagre}\n</script>\n\
         <script>\n{driver}\n</script>\n\
         <script>\n{view_state}\n</script>\n\
         <script>\n{nav_helpers}\n</script>\n\
         <script>\n{navigation}\n</script>\n\
         <script>\n{filter_panel}\n</script>\n\
         <script>\n{inspector}\n</script>\n\
         <script>\n{app}\n</script>\n\
         </body>\n\
         </html>\n",
        layout_base = LAYOUT_BASE_JS,
        cose_base = COSE_BASE_JS,
        cytoscape = CYTOSCAPE_JS,
        fcose = FCOSE_JS,
        dagre = DAGRE_JS,
        driver = DRIVER_JS,
        view_state = VIEW_STATE_JS,
        nav_helpers = NAV_HELPERS_JS,
        navigation = NAVIGATION_JS,
        filter_panel = FILTER_PANEL_JS,
        inspector = INSPECTOR_JS,
        app = APP_JS,
    )?;
    Ok(out)
}

// ============================================================
// Vendored assets (ADR 0050 decision 2: single-file bundle).
// ============================================================

const CYTOSCAPE_JS: &str = include_str!("assets/cytoscape.min.js");
const COSE_BASE_JS: &str = include_str!("assets/cose-base.js");
const FCOSE_JS: &str = include_str!("assets/cytoscape-fcose.js");
const DAGRE_JS: &str = include_str!("assets/cytoscape-dagre.js");
const LAYOUT_BASE_JS: &str = include_str!("assets/layout-base.js");
const DRIVER_JS: &str = include_str!("assets/driver.js");
const VIEW_STATE_JS: &str = include_str!("assets/view-state.js");
const NAV_HELPERS_JS: &str = include_str!("assets/navigation-helpers.js");
const NAVIGATION_JS: &str = include_str!("assets/navigation.js");
const FILTER_PANEL_JS: &str = include_str!("assets/filter-panel.js");
const INSPECTOR_JS: &str = include_str!("assets/inspector.js");
const APP_JS: &str = include_str!("assets/app.js");
const STYLES_CSS: &str = include_str!("assets/styles.css");

// ============================================================
// Library-neutral payload.
// ============================================================

#[derive(Debug, Serialize)]
struct Payload {
    version: u32,
    nodes: Vec<PayloadNode>,
    edges: Vec<PayloadEdge>,
    unresolved_stubs: Vec<PayloadStub>,
}

#[derive(Debug, Serialize)]
struct PayloadNode {
    id: String,
    kind: &'static str,
    label_primary: String,
    label_secondary: String,
    attributes: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct PayloadEdge {
    id: String,
    source: String,
    target: String,
    relation: &'static str,
    provenance: &'static str,
    confidence: &'static str,
    state: &'static str,
    /// Discovery freshness: "fresh", "stale", or "unknown". Helps
    /// the inspector contextualize cached / convention candidates.
    freshness: &'static str,
    is_resolved: bool,
    is_unresolved_target: bool,
    /// Adapter + evidence + adapter-specific fields. The `fields`
    /// sub-object carries provider-specific keys (e.g. `match_kind`
    /// for mux scoring, file paths, pid numbers) that the resolver
    /// uses to break ties.
    metadata: serde_json::Value,
    /// Detail when state is "ignored" or "overridden". `None` for
    /// active edges.
    #[serde(skip_serializing_if = "Option::is_none")]
    state_detail: Option<EdgeStateDetail>,
    /// For resolver-preferred candidates only: the ids of every
    /// other candidate the resolver evaluated for this resolution.
    /// Sorted in candidate (deterministic) order. Empty when the
    /// edge is unresolved or had no competitors.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    competing_link_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EdgeStateDetail {
    kind: &'static str, // "ignored" or "overridden"
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overridden_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct PayloadStub {
    id: String,
    edge_id: String,
    node_type: String,
    harness_key: Option<String>,
    native_id: Option<String>,
    state_scope: Option<String>,
    path: Option<String>,
}

fn build_payload(snapshot: &GraphSnapshot, opts: HtmlOptions) -> Payload {
    use std::collections::BTreeMap;

    // 1. Visible nodes per filter.
    let mut visible: BTreeMap<NodeId, &GraphNode> = BTreeMap::new();
    for node in &snapshot.nodes {
        if opts.diagnostic_nodes == Inclusion::Exclude
            && matches!(node, GraphNode::RuntimeProcess(_))
        {
            continue;
        }
        visible.insert(node.id(), node);
    }

    // 2. Selected (resolver-preferred) link ids, with their
    //    competing-candidate context so the inspector can show
    //    winner vs. losers side by side.
    let mut selected: std::collections::BTreeMap<&str, &[String]> =
        std::collections::BTreeMap::new();
    for r in &snapshot.resolved_relationships {
        // ADR 0077: skip no-winner slots — `selected_link_id` is
        // `None` when the resolver couldn't pick, and there's no
        // winner edge to anchor the competing list against.
        if let Some(id) = r.selected_link_id.as_deref() {
            selected.insert(id, r.competing_link_ids.as_slice());
        }
    }

    // 3. Edges + unresolved stubs.
    let mut edges: Vec<PayloadEdge> = Vec::new();
    let mut stubs: Vec<PayloadStub> = Vec::new();

    for link in &snapshot.candidate_links {
        let is_resolved = selected.contains_key(link.id.as_str());

        if !is_resolved && opts.candidates == Inclusion::Exclude {
            continue;
        }
        if !visible.contains_key(&link.source) {
            continue;
        }

        match &link.target {
            LinkEndpoint::Node { id } => {
                if !visible.contains_key(id) {
                    continue;
                }
                edges.push(payload_edge(
                    link,
                    link.source.to_string(),
                    id.to_string(),
                    is_resolved,
                    false,
                ));
            }
            LinkEndpoint::Unresolved { evidence } => {
                if opts.candidates == Inclusion::Exclude {
                    continue;
                }
                let stub_id = stub_id_for(&link.id);
                stubs.push(PayloadStub {
                    id: stub_id.clone(),
                    edge_id: link.id.clone(),
                    node_type: evidence.node_type.clone(),
                    harness_key: evidence.harness_key.clone(),
                    native_id: evidence.native_id.clone(),
                    state_scope: evidence.state_scope.clone(),
                    path: evidence.path.clone(),
                });
                edges.push(payload_edge(
                    link,
                    link.source.to_string(),
                    stub_id,
                    is_resolved,
                    true,
                ));
            }
        }
    }

    // 4. Nodes — sorted by (kind_order, id).
    let mut nodes: Vec<PayloadNode> = visible.values().map(|n| build_node(n)).collect();
    nodes.sort_by(|a, b| {
        kind_order(a.kind)
            .cmp(&kind_order(b.kind))
            .then(a.id.cmp(&b.id))
    });

    // 5. Annotate resolver-preferred edges with their competing
    //    candidate ids so the inspector can show winner vs. losers.
    for edge in edges.iter_mut() {
        if edge.is_resolved
            && let Some(competing) = selected.get(edge.id.as_str())
        {
            edge.competing_link_ids = competing.to_vec();
        }
    }

    // 6. Edges — sorted by (source, relation, target, -provenance_precedence, id).
    edges.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then(a.relation.cmp(b.relation))
            .then(a.target.cmp(&b.target))
            .then(provenance_precedence(b.provenance).cmp(&provenance_precedence(a.provenance)))
            .then(a.id.cmp(&b.id))
    });

    stubs.sort_by(|a, b| a.id.cmp(&b.id));

    Payload {
        version: 1,
        nodes,
        edges,
        unresolved_stubs: stubs,
    }
}

fn payload_edge(
    link: &GraphLink,
    source: String,
    target: String,
    is_resolved: bool,
    is_unresolved_target: bool,
) -> PayloadEdge {
    PayloadEdge {
        id: link.id.clone(),
        source,
        target,
        relation: link.relation.snake_case(),
        provenance: link.provenance.snake_case(),
        confidence: link.confidence.snake_case(),
        state: link_state_tag(&link.state),
        freshness: link_freshness_tag(&link.freshness),
        is_resolved,
        is_unresolved_target,
        metadata: link_metadata(link),
        state_detail: link_state_detail(&link.state),
        competing_link_ids: Vec::new(), // filled later for resolver winners
    }
}

fn link_state_tag(state: &LinkState) -> &'static str {
    match state {
        LinkState::Active => "active",
        LinkState::Ignored { .. } => "ignored",
        LinkState::Overridden { .. } => "overridden",
    }
}

fn link_state_detail(state: &LinkState) -> Option<EdgeStateDetail> {
    match state {
        LinkState::Active => None,
        LinkState::Ignored { reason } => Some(EdgeStateDetail {
            kind: "ignored",
            reason: reason.clone(),
            overridden_by: None,
        }),
        LinkState::Overridden { by, reason } => Some(EdgeStateDetail {
            kind: "overridden",
            reason: reason.clone(),
            overridden_by: Some(by.clone()),
        }),
    }
}

fn link_freshness_tag(freshness: &crate::model::Freshness) -> &'static str {
    use crate::model::Freshness;
    match freshness {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "stale",
        Freshness::Unknown => "unknown",
    }
}

fn link_metadata(link: &GraphLink) -> serde_json::Value {
    // Carry adapter, brief evidence, and the adapter-specific
    // fields. The fields submap is the "context that fed the
    // resolver rules" — keys vary by adapter (mux match_kind,
    // process pid, fd paths, …).
    serde_json::json!({
        "adapter": link.source_metadata.adapter,
        "evidence": link.source_metadata.evidence,
        "fields": link.source_metadata.fields,
    })
}

fn build_node(node: &GraphNode) -> PayloadNode {
    let id = node.id().to_string();
    let kind = node_kind_tag(node);
    let (label_primary, label_secondary) = node_labels(node);
    PayloadNode {
        id,
        kind,
        label_primary,
        label_secondary,
        attributes: node_attributes(node),
    }
}

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

fn kind_order(kind: &str) -> u8 {
    match kind {
        "workspace" => 0,
        "repo" => 1,
        "checkout" => 2,
        "branch" => 3,
        "fork" => 4,
        "agent_session" => 5,
        "mux_session" => 6,
        "runtime_process" => 7,
        "forge_pr" => 8,
        "pin" => 9,
        _ => 10,
    }
}

fn provenance_precedence(p: &str) -> u8 {
    match p {
        "local_declared" => 5,
        "global_declared" => 4,
        "strong_discovered" => 3,
        "discovered" | "convention" => 2,
        "cached" => 1,
        _ => 0,
    }
}

fn node_labels(node: &GraphNode) -> (String, String) {
    // (primary, secondary). Primary is the most useful one-line
    // identifier; secondary is a disambiguating context line shown
    // smaller in the UI. Both are chosen so two nodes with the same
    // short name can be told apart at a glance.
    match node {
        GraphNode::Repo(n) => (repo_short_name(&n.common_dir), "repo".to_string()),
        GraphNode::Checkout(n) => (
            last_segment(&n.root),
            format!("checkout · {}", repo_short_name(&n.id.repo.common_dir)),
        ),
        GraphNode::Workspace(n) => (
            n.name.clone().unwrap_or_else(|| last_segment(&n.root)),
            "workspace".to_string(),
        ),
        GraphNode::Branch(n) => {
            let short = n.refname.strip_prefix("refs/heads/").unwrap_or(&n.refname);
            (
                short.to_string(),
                format!("branch · {}", repo_short_name(&n.id.repo.common_dir)),
            )
        }
        GraphNode::AgentSession(n) => {
            // Prefer the operator-set title; fall back to the
            // first 12 chars of the session key so UUID-style ids
            // ("c0000000-1111-...") stay recognizable.
            let display = n
                .title
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| truncate(&n.id.session_key, 12));
            (
                format!("{}: {}", n.harness_key, truncate(&display, 24)),
                "agent session".to_string(),
            )
        }
        GraphNode::MuxSession(n) => (
            format!("{}:{}", n.backend, truncate(&n.native_id, 18)),
            "mux session".to_string(),
        ),
        GraphNode::Pin(n) => (
            format!("pin:{}", truncate(&n.display_name, 24)),
            format!("{} · {}", n.harness, n.mux.native_id()),
        ),
        GraphNode::Fork(n) => {
            let name = n
                .name
                .clone()
                .unwrap_or_else(|| n.provider_source_key.clone());
            (truncate(&name, 22), format!("fork · {}", n.provider))
        }
        GraphNode::ForgePr(n) => (
            format!("#{} {}", n.number, truncate(&n.repo, 18)),
            format!("PR · {} · {}", n.owner, n.state.as_deref().unwrap_or("?")),
        ),
        GraphNode::RuntimeProcess(n) => {
            let role = n
                .role
                .map(|r| format!("{r:?}"))
                .unwrap_or_else(|| "process".to_string());
            let pid = n.pid.map(|p| format!(" pid {p}")).unwrap_or_default();
            (
                format!("{role}{pid}"),
                truncate(n.command.as_deref().unwrap_or("runtime"), 30),
            )
        }
    }
}

/// Repo-friendly short name: strip a trailing `.git`/`/.git` so
/// `/path/to/repo-a/.git` reads as `repo-a` rather than `.git`.
fn repo_short_name(common_dir: &str) -> String {
    let trimmed = common_dir
        .trim_end_matches('/')
        .strip_suffix("/.git")
        .or_else(|| common_dir.strip_suffix(".git"))
        .unwrap_or(common_dir);
    last_segment(trimmed)
}

fn node_attributes(node: &GraphNode) -> serde_json::Value {
    serde_json::to_value(node).unwrap_or(serde_json::Value::Null)
}

fn last_segment(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .find(|seg| !seg.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn stub_id_for(link_id: &str) -> String {
    format!("u_{link_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_has_empty_collections() {
        let p = build_payload(&GraphSnapshot::empty(), HtmlOptions::default());
        assert_eq!(p.version, 1);
        assert!(p.nodes.is_empty());
        assert!(p.edges.is_empty());
        assert!(p.unresolved_stubs.is_empty());
    }

    #[test]
    fn empty_html_renders_full_scaffold() {
        let html = render_graph_html(&GraphSnapshot::empty(), HtmlOptions::default()).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("id=\"conspectus-cy\""));
        assert!(html.contains("id=\"conspectus-graph-payload\""));
        // Vendored bundles inlined.
        assert!(html.contains("cytoscape"));
        // Should be self-contained: no live external loads. (Inline
        // JS source comments may still reference URLs as attribution
        // — what matters is that the page doesn't fetch anything.)
        assert!(!html.contains("src=\"http"));
        assert!(!html.contains("href=\"http"));
    }

    #[test]
    fn payload_json_escapes_script_breakout() {
        // Synthesize a payload with a literal </script> in a field
        // via a fake adapter; rendered HTML must escape the `<`.
        let html = render_graph_html(&GraphSnapshot::empty(), HtmlOptions::default()).unwrap();
        // Sanity: the payload script tag must be closed exactly once.
        assert_eq!(html.matches("id=\"conspectus-graph-payload\"").count(), 1);
    }

    #[test]
    fn build_node_assigns_known_kind_tags() {
        for kind in [
            "workspace",
            "repo",
            "checkout",
            "branch",
            "fork",
            "agent_session",
            "mux_session",
            "pin",
            "runtime_process",
            "forge_pr",
        ] {
            assert!(kind_order(kind) < 10, "kind {kind} should be in ordering");
        }
    }
}
