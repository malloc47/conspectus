//! Selected-node detail view-models.
//!
//! Right-panel header data for a single node, in a non-string form
//! that the renderer can lay out at its own width. Mirrors the
//! content of `conspectus node show <id>` (and reuses
//! [`crate::output::table::node_short_id`] for stable internal row
//! identity) while exposing the locked mockup-review behavior:
//!
//! SQLite consumer surface (P10-010 / ADR 0043).
//! [`build_node_detail_from_conn`] is the production entry point —
//! it consumes from SQLite via [`crate::query::read_snapshot`]
//! per-call, then runs the typed-Rust view-model assembly below.
//! [`build_node_detail`] survives for fixture-heavy tests and
//! producer-side callers that still start from a typed snapshot.
//!
//! The trade-off vs. per-section SQL: the detail builder's typed
//! view-model assembly (header fields with kind dispatch, mux/pr/
//! lineage subqueries with ambiguity counts, link summaries) is
//! complex enough that rewriting each helper as SQL doubles the
//! line count for no observable behavior change. Routing through
//! `read_snapshot` keeps the assembly in one place and still
//! satisfies the consumer-side contract: the function takes a
//! `Connection`, returns a `NodeDetail`, and never persists a
//! `GraphSnapshot`. (Compare `output::node_show` and the projection
//! renderers, where the per-section SQL form was cheap because the
//! per-cell formatting is trivial — there the trade-off tipped the
//! other way.) When the TUI's refresh cadence makes the per-call
//! `read_snapshot` cost worth optimizing, a follow-up story can
//! split the helpers below into filtered queries.
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
    AgentSessionNode, BranchNode, CheckoutNode, Confidence, Diagnostic, ForgePrNode, ForkNode,
    GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionNode, NodeId,
    Provenance, RelationKind, RepoNode, ResolvedRelationship, RuntimeProcessNode,
    RuntimeProcessRole, WorkspaceNode,
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

/// SQLite-backed detail builder (P10-010 / ADR 0043). Materializes
/// a fresh typed snapshot from `conn` via
/// [`crate::query::read_snapshot`] and runs the typed-Rust assembly
/// below.
pub fn build_node_detail_from_conn(
    conn: &rusqlite::Connection,
    target: &NodeId,
    home: Option<&Path>,
) -> rusqlite::Result<Option<NodeDetail>> {
    let snapshot = crate::query::read_snapshot(conn)?;
    Ok(build_node_detail(DetailInputs {
        snapshot: &snapshot,
        target,
        home,
    }))
}

/// Build the detail view-model for the given node id. Returns
/// `None` when the node isn't in the snapshot (e.g. selection
/// pointed at a row that was just removed by a refresh).
///
/// This remains useful for fixture-heavy tests and producer-side
/// callers. Runtime TUI code uses [`build_node_detail_from_conn`].
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
    /// Compact identity label for callers that need a selected-node
    /// display name outside the field list.
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

impl NodeDetail {
    /// Group [`Self::header_fields`] into sections per ADR 0033.
    ///
    /// Sections are emitted in node-sensitive canonical order. Agent
    /// session details mirror the sessions view (Session, Mux, PR,
    /// Lineage); mux details lead with mux fields and put attached
    /// agent session rows under a later Session divider. A section
    /// is omitted entirely when every field it would contain is a
    /// no-annotation placeholder — operators see a shorter pane
    /// rather than rows of dashes.
    pub fn sections(&self) -> Vec<DetailSection> {
        group_fields_into_sections(self.kind_label, &self.header_fields)
    }
}

/// Section-group a flat header-field list under a given node kind.
/// Shared between [`NodeDetail::sections`] and the inline expansion
/// path so a linked entity's expanded view renders through the same
/// section structure as its standalone detail.
pub fn group_fields_into_sections(kind_label: &str, fields: &[HeaderField]) -> Vec<DetailSection> {
    let mut grouped: std::collections::BTreeMap<SectionKind, Vec<HeaderField>> =
        std::collections::BTreeMap::new();
    for field in fields {
        let kind = section_for(kind_label, field.label);
        grouped.entry(kind).or_default().push(field.clone());
    }
    let order = section_order(kind_label);
    order
        .iter()
        .filter_map(|kind| {
            let fields = grouped.remove(kind)?;
            let all_blank_placeholders = fields
                .iter()
                .all(|f| f.placeholder && f.annotation.is_none());
            if all_blank_placeholders {
                return None;
            }
            Some(DetailSection {
                kind: *kind,
                fields,
            })
        })
        .collect()
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
    /// Node referenced by this field, when the row is a compact
    /// linked-entity summary that can expand in place.
    pub target: Option<NodeId>,
    /// `kind_label` of [`Self::target`] when the field carries an
    /// expanded sub-detail. The renderer uses this to route
    /// [`Self::expanded_fields`] into the same section grouping the
    /// standalone detail of the target would produce, so the
    /// expansion matches the normal detail view byte-for-byte.
    pub expanded_kind_label: Option<&'static str>,
    /// One-level detail rows for [`Self::target`]. These are
    /// populated eagerly while the builder has the source snapshot;
    /// the renderer decides whether to show them.
    pub expanded_fields: Vec<HeaderField>,
}

/// Closed set of right-panel detail sections (ADR 0033). The
/// renderer walks sections in declaration order and emits a
/// right-anchored labeled divider before each one after the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SectionKind {
    Session,
    Process,
    Mux,
    Pr,
    Lineage,
}

impl SectionKind {
    /// Divider label shown to the operator (Title Case per the
    /// agent-deck-style mockup in the styling overhaul plan).
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Process => "Process",
            Self::Mux => "Mux",
            Self::Pr => "PR",
            Self::Lineage => "Lineage",
        }
    }
}

/// One section of the right-panel detail. Built lazily from
/// [`NodeDetail::sections`] so the existing `header_fields` slice
/// stays the source of truth for tests; the section view-model is
/// a renderer-facing grouping that respects ADR 0033's suppression
/// rule (omit when every field is a no-annotation placeholder).
#[derive(Clone, Debug, PartialEq)]
pub struct DetailSection {
    pub kind: SectionKind,
    pub fields: Vec<HeaderField>,
}

/// Map a `(kind_label, field_label)` pair to its owning section.
/// Defaults to [`SectionKind::Session`] so any field added without
/// an explicit routing still lands somewhere visible — the
/// suppression rule then hides empty sections.
fn section_for(kind_label: &str, field_label: &str) -> SectionKind {
    use SectionKind::*;
    match (kind_label, field_label) {
        ("agent_session", "mux") => Mux,
        ("agent_session", "process") => Process,
        ("agent_session", "pr") => Pr,
        ("agent_session", "lineage") => Lineage,
        ("mux_session", "name" | "backend" | "cwd" | "attached") => Mux,
        ("mux_session", "session" | "id" | "harness" | "alias" | "title") => Session,
        ("mux_session", "process") => Process,
        ("runtime_process", "mux") => Mux,
        ("runtime_process", "session") => Session,
        ("runtime_process", _) => Process,
        ("forge_pr", _) => Pr,
        ("fork", _) => Lineage,
        _ => Session,
    }
}

fn section_order(kind_label: &str) -> &'static [SectionKind] {
    use SectionKind::*;
    match kind_label {
        "mux_session" => &[Mux, Process, Session, Pr, Lineage],
        "runtime_process" => &[Process, Mux, Session, Pr, Lineage],
        _ => &[Session, Process, Mux, Pr, Lineage],
    }
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
        GraphNode::Checkout(_) => "checkout",
        GraphNode::Workspace(_) => "workspace",
        GraphNode::AgentSession(_) => "agent_session",
        GraphNode::MuxSession(_) => "mux_session",
        GraphNode::RuntimeProcess(_) => "runtime_process",
        GraphNode::Branch(_) => "branch",
        GraphNode::Fork(_) => "fork",
        GraphNode::ForgePr(_) => "forge_pr",
    }
}

fn title_line(node: &GraphNode) -> String {
    match node {
        GraphNode::AgentSession(session) => agent_session_display_id(session),
        GraphNode::MuxSession(mux) => mux_display_label(mux),
        GraphNode::RuntimeProcess(process) => format!("process:{}", process.observation_key),
        GraphNode::ForgePr(pr) => format!("forge_pr:{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(fork) => match &fork.name {
            Some(name) => format!("fork:{name}"),
            None => format!("fork:{}", fork.provider_source_key),
        },
        GraphNode::Repo(repo) => format!("repo:{}", repo.common_dir),
        GraphNode::Checkout(worktree) => format!("checkout:{}", worktree.root),
        GraphNode::Workspace(workspace) => format!("workspace:{}", workspace.root),
        GraphNode::Branch(branch) => format!("branch:{}", branch.refname),
    }
}

fn header_fields(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
) -> Vec<HeaderField> {
    header_fields_inner(snapshot, node, home, true)
}

fn header_fields_inner(
    snapshot: &GraphSnapshot,
    node: &GraphNode,
    home: Option<&Path>,
    include_linked_details: bool,
) -> Vec<HeaderField> {
    match node {
        GraphNode::AgentSession(session) => {
            agent_session_fields(snapshot, session, home, include_linked_details)
        }
        GraphNode::MuxSession(mux) => {
            mux_session_fields(snapshot, mux, home, include_linked_details)
        }
        GraphNode::RuntimeProcess(process) => runtime_process_fields(snapshot, process, home),
        GraphNode::ForgePr(pr) => forge_pr_fields(pr),
        GraphNode::Fork(fork) => fork_fields(fork),
        GraphNode::Repo(repo) => repo_fields(repo, home),
        GraphNode::Checkout(worktree) => worktree_fields(worktree, home),
        GraphNode::Workspace(workspace) => workspace_fields(workspace, home),
        GraphNode::Branch(branch) => branch_fields(branch),
    }
}

fn agent_session_fields(
    snapshot: &GraphSnapshot,
    session: &AgentSessionNode,
    home: Option<&Path>,
    include_linked_details: bool,
) -> Vec<HeaderField> {
    let mut fields = Vec::new();
    let session_id = NodeId::AgentSession(session.id.clone());
    fields.push(plain("id", session.id.session_key.clone()));
    fields.push(plain("harness", session.harness_key.clone()));
    let cwd_value = match &session.cwd {
        Some(cwd) => plain("cwd", shorten_home(cwd, home)),
        None => placeholder("cwd", "— (unknown)"),
    };
    fields.push(cwd_value);
    let session_id_for_alias = NodeId::AgentSession(session.id.clone());
    let alias = snapshot
        .aliases
        .get(&session_id_for_alias)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if let Some(alias) = &alias {
        fields.push(plain("alias", alias.clone()));
    }
    // ADR 0029: the alias hides the harness-native title in default
    // renders. Auditing both surfaces lives behind
    // `conspectus alias list` (H-RENAME-008).
    if alias.is_none()
        && let Some(title) = session
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    {
        fields.push(plain("title", title.to_string()));
    }

    fields.extend(pin_diagnostic_fields(snapshot, session));

    fields.push(session_mux_field(snapshot, &session_id));
    fields.extend(session_process_fields(snapshot, &session_id));
    fields.push(session_pr_field(snapshot, &session_id, home));
    fields.push(session_lineage_field(snapshot, &session_id));
    if include_linked_details {
        attach_linked_details(snapshot, &mut fields, home);
    }

    fields
}

fn pin_diagnostic_fields(snapshot: &GraphSnapshot, session: &AgentSessionNode) -> Vec<HeaderField> {
    let pin_id = snapshot.pins.iter().find_map(|pin| match &pin.binding {
        Some(crate::model::PinBinding::Bound { session: bound, .. }) if bound == &session.id => {
            Some(pin.id.as_str())
        }
        _ => None,
    });
    let Some(pin_id) = pin_id else {
        return Vec::new();
    };
    crate::tui::actions::pin_diagnostics_for_id(snapshot, pin_id)
        .into_iter()
        .map(|diagnostic| match diagnostic {
            crate::tui::actions::PinDiagnosticView::Ambiguous {
                pin_id,
                chosen,
                competing,
            } => {
                let competitors = competing
                    .iter()
                    .map(|id| format!("{}:{}", id.harness_key, id.session_key))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut field = plain(
                    "pin",
                    format!(
                        "{pin_id} ambiguous: chosen {}:{}; competing {competitors}",
                        chosen.harness_key, chosen.session_key
                    ),
                );
                field.annotation = Some("b to bind");
                field
            }
            crate::tui::actions::PinDiagnosticView::Drift {
                pin_id,
                declared_cwd,
                observed_cwd,
            } => {
                let mut field = plain(
                    "pin",
                    format!("{pin_id} cwd drift: declared {declared_cwd}; observed {observed_cwd}"),
                );
                field.annotation = Some("advisory");
                field
            }
            crate::tui::actions::PinDiagnosticView::StaleMux { pin_id, mux } => {
                let mut field = plain("pin", format!("{pin_id} stale mux {}", mux.native_id));
                field.annotation = Some("Enter relaunch");
                field
            }
            crate::tui::actions::PinDiagnosticView::Unbound {
                pin_id,
                expected_mux_native_id,
                last_session,
            } => {
                let (value, annotation) = match last_session {
                    Some(last) => (
                        format!(
                            "{pin_id} unbound: expected {expected_mux_native_id} · last session {}",
                            last.session_id
                        ),
                        "Enter resume",
                    ),
                    None => (
                        format!("{pin_id} unbound: expected {expected_mux_native_id}"),
                        "Enter launch",
                    ),
                };
                let mut field = plain("pin", value);
                field.annotation = Some(annotation);
                field
            }
        })
        .collect()
}

fn session_mux_field(snapshot: &GraphSnapshot, session: &NodeId) -> HeaderField {
    let candidates = active_links_from(snapshot, session, RelationKind::LinkedToMux);
    match candidates.len() {
        0 => placeholder("mux", "— (no attach)"),
        1 => {
            let preferred = candidates[0];
            let value = link_target_label(snapshot, preferred).unwrap_or_else(|| "—".to_string());
            linked("mux", value, preferred.target_node_id().cloned())
        }
        n => {
            // The preferred-by-provenance candidate is shown; the
            // ambiguity is surfaced via the annotation. The exact
            // count helps the renderer mirror the row glyph.
            let preferred = preferred_link(&candidates);
            let label = preferred
                .and_then(|link| link_target_label(snapshot, link))
                .unwrap_or_else(|| format!("— ({n} candidates)"));
            let mut field = linked(
                "mux",
                format!("{label}  ({n} candidates)"),
                preferred.and_then(|link| link.target_node_id().cloned()),
            );
            field.annotation = Some("⚠");
            field
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
    // PR keyed off the session's checkout (cwd-matched).
    let session_node = match snapshot.nodes.iter().find(|n| n.id() == *session) {
        Some(GraphNode::AgentSession(node)) => node,
        _ => return placeholder("pr", "— (no PR)"),
    };
    let cwd = match session_node.cwd.as_deref() {
        Some(cwd) => cwd,
        None => return placeholder("pr", "— (no PR)"),
    };
    let Some(worktree_id) = snapshot.nodes.iter().find_map(|node| match node {
        GraphNode::Checkout(wt) if wt.root == cwd => Some(NodeId::Checkout(wt.id.clone())),
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
    plain(
        "pr",
        format!(
            "{}/{}#{} ({state}){draft_marker}",
            pr.owner, pr.repo, pr.number
        ),
    )
}

fn session_lineage_field(snapshot: &GraphSnapshot, session: &NodeId) -> HeaderField {
    let Some(parent_id) = preferred_target(snapshot, session, RelationKind::ParentSession) else {
        return placeholder("lineage", "— (no parent)");
    };
    let Some(GraphNode::AgentSession(parent)) = snapshot.nodes.iter().find(|n| n.id() == parent_id)
    else {
        return placeholder("lineage", "— (no parent)");
    };
    plain("lineage", agent_session_display_id(parent))
}

fn mux_session_fields(
    snapshot: &GraphSnapshot,
    mux: &MuxSessionNode,
    home: Option<&Path>,
    include_linked_details: bool,
) -> Vec<HeaderField> {
    let mut fields = vec![
        plain("name", mux.native_id.clone()),
        plain("backend", mux.backend.clone()),
    ];
    if let Some(cwd) = &mux.cwd {
        fields.push(plain("cwd", shorten_home(cwd, home)));
    }
    let mux_id = NodeId::MuxSession(mux.id.clone());
    let attached_sessions = attached_sessions_for_mux(snapshot, &mux_id);
    let attached_count = attached_sessions.len();
    fields.push(plain("attached", format!("{attached_count}")));
    for process in processes_for_mux(snapshot, &mux_id) {
        fields.push(linked(
            "process",
            process_link_label(snapshot, &process).unwrap_or_else(|| format!("{}", process)),
            Some(process),
        ));
    }
    for session in attached_sessions {
        fields.push(linked(
            "session",
            agent_session_link_label(snapshot, session).unwrap_or_else(|| format!("{}", session)),
            Some(session.clone()),
        ));
    }
    if include_linked_details {
        attach_linked_details(snapshot, &mut fields, home);
    }
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

fn runtime_process_fields(
    snapshot: &GraphSnapshot,
    process: &RuntimeProcessNode,
    home: Option<&Path>,
) -> Vec<HeaderField> {
    let mut fields = vec![plain("observation", process.observation_key.clone())];
    if let Some(pid) = process.pid {
        fields.push(plain("pid", pid.to_string()));
    }
    if let Some(parent_pid) = process.parent_pid {
        fields.push(plain("parent", parent_pid.to_string()));
    }
    if let Some(root_pane_pid) = process.root_pane_pid {
        fields.push(plain("pane_pid", root_pane_pid.to_string()));
    }
    if let Some(command) = &process.command {
        fields.push(plain("command", command.clone()));
    }
    if let Some(cwd) = &process.cwd {
        fields.push(plain("cwd", shorten_home(cwd, home)));
    }
    if let Some(harness_key) = &process.harness_key {
        fields.push(plain("harness", harness_key.clone()));
    }
    if let Some(role) = process.role {
        fields.push(plain(
            "role",
            match role {
                RuntimeProcessRole::HumanAgent => "human_agent",
                RuntimeProcessRole::Subagent => "subagent",
                RuntimeProcessRole::Background => "background",
                RuntimeProcessRole::Shell => "shell",
                RuntimeProcessRole::Unknown => "unknown",
            }
            .to_string(),
        ));
    }
    if let Some(depth) = process.depth {
        fields.push(plain("depth", depth.to_string()));
    }
    if let Some(observed_epoch) = process.observed_epoch {
        fields.push(plain("observed", observed_epoch.to_string()));
    }
    let process_id = NodeId::RuntimeProcess(process.id.clone());
    for mux in muxes_for_process(snapshot, &process_id) {
        fields.push(linked(
            "mux",
            link_target_label_by_id(snapshot, &mux).unwrap_or_else(|| format!("{}", mux)),
            Some(mux),
        ));
    }
    for (session, relation) in sessions_for_process(snapshot, &process_id) {
        let mut field = linked(
            "session",
            agent_session_link_label(snapshot, &session).unwrap_or_else(|| format!("{}", session)),
            Some(session),
        );
        if relation == RelationKind::ProcessCandidatesSession {
            field.annotation = Some("⚠");
        }
        fields.push(field);
    }
    fields
}

fn repo_fields(repo: &RepoNode, home: Option<&Path>) -> Vec<HeaderField> {
    vec![plain("common_dir", shorten_home(&repo.common_dir, home))]
}

fn worktree_fields(worktree: &CheckoutNode, home: Option<&Path>) -> Vec<HeaderField> {
    let mut fields = vec![plain("root", shorten_home(&worktree.root, home))];
    if let Some(git_dir) = &worktree.git_dir {
        fields.push(plain("git_dir", shorten_home(git_dir, home)));
    }
    if let Some(branch) = &worktree.current_branch {
        fields.push(plain("branch", branch.refname.clone()));
    }
    fields
}

fn workspace_fields(workspace: &WorkspaceNode, home: Option<&Path>) -> Vec<HeaderField> {
    let mut fields = vec![plain("root", shorten_home(&workspace.root, home))];
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
        target: None,
        expanded_kind_label: None,
        expanded_fields: Vec::new(),
    }
}

fn placeholder(label: &'static str, value: &str) -> HeaderField {
    HeaderField {
        label,
        value: value.to_string(),
        placeholder: true,
        annotation: None,
        target: None,
        expanded_kind_label: None,
        expanded_fields: Vec::new(),
    }
}

fn linked(label: &'static str, value: String, target: Option<NodeId>) -> HeaderField {
    HeaderField {
        label,
        value,
        placeholder: false,
        annotation: None,
        target,
        expanded_kind_label: None,
        expanded_fields: Vec::new(),
    }
}

fn attach_linked_details(
    snapshot: &GraphSnapshot,
    fields: &mut [HeaderField],
    home: Option<&Path>,
) {
    for field in fields {
        let Some(target) = field.target.as_ref() else {
            continue;
        };
        let Some(node) = snapshot.nodes.iter().find(|node| node.id() == *target) else {
            continue;
        };
        let sub_kind_label = kind_label(node);
        let mut expanded = header_fields_inner(snapshot, node, home, false);
        for nested in &mut expanded {
            nested.target = None;
            nested.expanded_kind_label = None;
            nested.expanded_fields.clear();
        }
        field.expanded_kind_label = Some(sub_kind_label);
        field.expanded_fields = expanded;
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
    link_target_label_by_id(snapshot, target)
}

fn link_target_label_by_id(snapshot: &GraphSnapshot, target: &NodeId) -> Option<String> {
    let node = snapshot.nodes.iter().find(|n| n.id() == *target)?;
    match node {
        GraphNode::MuxSession(mux) => Some(mux_display_label(mux)),
        GraphNode::AgentSession(session) => Some(format!(
            "{}:{}",
            session.harness_key, session.id.session_key
        )),
        GraphNode::RuntimeProcess(process) => Some(runtime_process_display_label(process)),
        GraphNode::Repo(repo) => Some(format!("repo:{}", repo.common_dir)),
        other => Some(node_reference_label(other)),
    }
}

fn attached_sessions_for_mux<'a>(snapshot: &'a GraphSnapshot, mux_id: &NodeId) -> Vec<&'a NodeId> {
    let mut sessions: Vec<&NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .filter(|link| match &link.target {
            LinkEndpoint::Node { id } => id == mux_id,
            _ => false,
        })
        .map(|link| &link.source)
        .collect();
    sessions.sort();
    sessions.dedup();
    sessions
}

fn session_process_fields(snapshot: &GraphSnapshot, session_id: &NodeId) -> Vec<HeaderField> {
    let mut fields = Vec::new();
    for (process, relation) in processes_for_session(snapshot, session_id) {
        let mut field = linked(
            "process",
            process_link_label(snapshot, &process).unwrap_or_else(|| format!("{}", process)),
            Some(process),
        );
        if relation == RelationKind::ProcessCandidatesSession {
            field.annotation = Some("⚠");
        }
        fields.push(field);
    }
    fields
}

fn processes_for_mux(snapshot: &GraphSnapshot, mux_id: &NodeId) -> Vec<NodeId> {
    let mut processes: Vec<NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| link.relation == RelationKind::MuxContainsProcess)
        .filter(|link| link.source == *mux_id)
        .filter_map(|link| link.target_node_id().cloned())
        .collect();
    processes.sort();
    processes.dedup();
    processes
}

fn processes_for_session(
    snapshot: &GraphSnapshot,
    session_id: &NodeId,
) -> Vec<(NodeId, RelationKind)> {
    let mut processes: Vec<(NodeId, RelationKind)> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| {
            matches!(
                link.relation,
                RelationKind::ProcessIdentifiesSession | RelationKind::ProcessCandidatesSession
            )
        })
        .filter(|link| link.target_node_id() == Some(session_id))
        .map(|link| (link.source.clone(), link.relation.clone()))
        .collect();
    processes.sort();
    processes.dedup();
    processes
}

fn muxes_for_process(snapshot: &GraphSnapshot, process_id: &NodeId) -> Vec<NodeId> {
    let mut muxes: Vec<NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| link.relation == RelationKind::MuxContainsProcess)
        .filter(|link| link.target_node_id() == Some(process_id))
        .map(|link| link.source.clone())
        .collect();
    muxes.sort();
    muxes.dedup();
    muxes
}

fn sessions_for_process(
    snapshot: &GraphSnapshot,
    process_id: &NodeId,
) -> Vec<(NodeId, RelationKind)> {
    let mut sessions: Vec<(NodeId, RelationKind)> = snapshot
        .candidate_links
        .iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .filter(|link| {
            matches!(
                link.relation,
                RelationKind::ProcessIdentifiesSession | RelationKind::ProcessCandidatesSession
            )
        })
        .filter(|link| link.source == *process_id)
        .filter_map(|link| {
            link.target_node_id()
                .cloned()
                .map(|target| (target, link.relation.clone()))
        })
        .collect();
    sessions.sort();
    sessions.dedup();
    sessions
}

fn agent_session_link_label(snapshot: &GraphSnapshot, session_id: &NodeId) -> Option<String> {
    let GraphNode::AgentSession(session) = snapshot.nodes.iter().find(|n| n.id() == *session_id)?
    else {
        return None;
    };
    Some(agent_session_display_id(session))
}

fn agent_session_display_id(session: &AgentSessionNode) -> String {
    format!("{}:{}", session.harness_key, session.id.session_key)
}

fn mux_display_label(mux: &MuxSessionNode) -> String {
    format!("{}:{}", mux.backend, mux.native_id)
}

fn process_link_label(snapshot: &GraphSnapshot, process_id: &NodeId) -> Option<String> {
    let GraphNode::RuntimeProcess(process) =
        snapshot.nodes.iter().find(|n| n.id() == *process_id)?
    else {
        return None;
    };
    Some(runtime_process_display_label(process))
}

fn runtime_process_display_label(process: &RuntimeProcessNode) -> String {
    match (process.pid, process.command.as_deref()) {
        (Some(pid), Some(command)) => format!("pid {pid}: {command}"),
        (Some(pid), None) => format!("pid {pid}"),
        (None, Some(command)) => command.to_string(),
        (None, None) => process.observation_key.clone(),
    }
}

fn node_reference_label(node: &GraphNode) -> String {
    match node {
        GraphNode::AgentSession(session) => agent_session_display_id(session),
        GraphNode::MuxSession(mux) => mux_display_label(mux),
        GraphNode::RuntimeProcess(process) => runtime_process_display_label(process),
        GraphNode::ForgePr(pr) => format!("{}/{}#{}", pr.owner, pr.repo, pr.number),
        GraphNode::Fork(fork) => fork
            .name
            .as_deref()
            .unwrap_or(&fork.provider_source_key)
            .to_string(),
        GraphNode::Repo(repo) => format!("repo:{}", repo.common_dir),
        GraphNode::Checkout(checkout) => format!("checkout:{}", checkout.root),
        GraphNode::Workspace(workspace) => format!("workspace:{}", workspace.root),
        GraphNode::Branch(branch) => format!("branch:{}", branch.refname),
    }
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
        AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, Confidence, Diagnostic,
        ForgePrId, ForgePrNode, GraphSnapshot, LinkEndpoint, LinkState, MuxSessionId,
        MuxSessionNode, PinBinding, PinCandidate, PinMuxRef, Provenance, RepoId, RepoNode,
        RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SourceMetadata,
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
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn runtime_process(observation_key: &str, pid: i64, command: &str) -> GraphNode {
        GraphNode::RuntimeProcess(RuntimeProcessNode {
            id: RuntimeProcessId::new(observation_key),
            observation_key: observation_key.to_string(),
            pid: Some(pid),
            parent_pid: None,
            root_pane_pid: Some(pid),
            command: Some(command.to_string()),
            cwd: Some("/home/op/src/x".to_string()),
            harness_key: Some("codex".to_string()),
            role: Some(RuntimeProcessRole::HumanAgent),
            depth: Some(0),
            observed_epoch: Some(1_700_000_500),
        })
    }

    fn process_link(id: &str, source: NodeId, target: NodeId, relation: RelationKind) -> GraphLink {
        GraphLink {
            id: id.to_string(),
            source,
            target: LinkEndpoint::Node { id: target },
            relation,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
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

    /// Parity guard for the bridge entry point: the SQLite-backed
    /// builder should produce the same `NodeDetail` as the in-memory
    /// one for the same input. Catches drift if a future story
    /// refactors only one path.
    #[test]
    fn from_conn_matches_snapshot_path_for_agent_session() {
        let snapshot = GraphSnapshot {
            nodes: vec![agent(
                "codex",
                "alpha",
                Some("/home/op/work"),
                Some("title"),
            )],
            ..GraphSnapshot::empty()
        };
        let target = snapshot.nodes[0].id();
        let direct = build_node_detail(DetailInputs {
            snapshot: &snapshot,
            target: &target,
            home: Some(home().as_path()),
        });
        let conn = crate::query::materialize_snapshot(&snapshot).expect("materialize");
        let via_conn = build_node_detail_from_conn(&conn, &target, Some(home().as_path()))
            .expect("from_conn ok");
        assert_eq!(direct, via_conn);
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
        assert_eq!(detail.title_line, "codex:abc");

        // No title row when unset.
        assert!(detail.header_fields.iter().all(|f| f.label != "title"));

        let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
        assert_eq!(labels, vec!["id", "harness", "cwd", "mux", "pr", "lineage"]);

        let by_label = |label: &str| {
            detail
                .header_fields
                .iter()
                .find(|f| f.label == label)
                .unwrap()
                .clone()
        };
        assert_eq!(by_label("id").value, "abc");
        assert_eq!(by_label("harness").value, "codex");
        assert_eq!(by_label("cwd").value, "~/src/x");
        assert!(by_label("mux").placeholder);
        assert_eq!(by_label("mux").value, "— (no attach)");
        assert!(by_label("pr").placeholder);
        assert!(by_label("lineage").placeholder);

        // ADR 0033 / Phase 6: sparse sessions collapse to just the
        // Session section. Mux / PR / Lineage sections are all
        // placeholder-only here and should be suppressed.
        let sections = detail.sections();
        assert_eq!(
            sections.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![SectionKind::Session],
            "sparse session should hide placeholder-only sections",
        );
        let session_field_labels: Vec<&str> = sections[0].fields.iter().map(|f| f.label).collect();
        assert_eq!(session_field_labels, vec!["id", "harness", "cwd"]);
    }

    #[test]
    fn agent_session_bound_to_ambiguous_pin_shows_bind_hint() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("codex", "alpha", Some("/home/op/src/x"), None));
        let chosen = AgentSessionId::new("codex", "/state", "alpha");
        let competing = AgentSessionId::new("codex", "/state", "beta");
        snapshot.pins.push(PinCandidate {
            id: "ingest".to_string(),
            display_name: "ingest".to_string(),
            harness: "codex".to_string(),
            cwd: "/home/op/src/x".to_string(),
            mux: PinMuxRef {
                backend: "tmux".to_string(),
                name: "ingest".to_string(),
                socket_name: None,
            },
            launch_argv: None,
            reason: None,
            provenance: Provenance::LocalPin,
            store_path: "/home/op/src/x/.conspectus.toml".to_string(),
            binding: Some(PinBinding::Bound {
                mux: MuxSessionId::new("tmux:ingest"),
                session: chosen.clone(),
            }),
        });
        snapshot.diagnostics.push(Diagnostic::PinAmbiguous {
            pin_id: "ingest".to_string(),
            chosen: chosen.clone(),
            competing: vec![competing],
        });

        let detail = build(
            &snapshot,
            &NodeId::AgentSession(chosen),
            Some(home().as_path()),
        );
        let pin = detail
            .header_fields
            .iter()
            .find(|field| field.label == "pin")
            .expect("pin diagnostic field");

        assert!(pin.value.contains("ingest ambiguous"));
        assert!(pin.value.contains("chosen codex:alpha"));
        assert!(pin.value.contains("competing codex:beta"));
        assert_eq!(pin.annotation, Some("b to bind"));
    }

    #[test]
    fn sections_emit_mux_section_when_ambiguous_mux_carries_warning() {
        // A session with ≥2 mux candidates has a `⚠` annotation on
        // its mux field; even though the value reads as a
        // placeholder-ish "— (N candidates)" it is *not* a blank
        // placeholder per ADR 0033's suppression rule and the Mux
        // section stays visible.
        use crate::model::{Freshness, GraphLink, RelationKind};
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/src/x"), None));
        let mux_a = MuxSessionId::new("editor");
        let mux_b = MuxSessionId::new("scratch");
        for native in ["editor", "scratch"] {
            snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new(native),
                backend: "tmux".to_string(),
                native_id: native.to_string(),
                cwd: None,
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                client_attached: None,
                activity_epoch: None,
                created_epoch: None,
            }));
        }
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        for (idx, mux) in [mux_a, mux_b].into_iter().enumerate() {
            snapshot.candidate_links.push(GraphLink {
                id: format!("link-{idx}"),
                source: session_id.clone(),
                target: LinkEndpoint::Node {
                    id: NodeId::MuxSession(mux),
                },
                relation: RelationKind::LinkedToMux,
                provenance: Provenance::Discovered,
                confidence: Confidence::Medium,
                freshness: Freshness::Fresh,
                source_metadata: SourceMetadata::default(),
                state: LinkState::Active,
            });
        }
        let snapshot = resolve_snapshot(snapshot);
        let detail = build(&snapshot, &session_id, Some(home().as_path()));
        let kinds: Vec<SectionKind> = detail.sections().iter().map(|s| s.kind).collect();
        assert!(
            kinds.contains(&SectionKind::Mux),
            "ambiguous-mux session keeps the Mux section: got {kinds:?}",
        );
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
            vec!["id", "harness", "cwd", "title", "mux", "pr", "lineage"]
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
    fn agent_session_detail_uses_full_native_session_id() {
        let long_id = "019eced4-4fb0-70d1-b8f4-72de7c469e62";
        let mut snapshot = GraphSnapshot::empty();
        snapshot
            .nodes
            .push(agent("codex", long_id, Some("/home/op/src/x"), None));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::AgentSession(AgentSessionId::new("codex", "/state", long_id));
        let detail = build(&snapshot, &target, Some(home().as_path()));
        let session = detail
            .header_fields
            .iter()
            .find(|f| f.label == "id")
            .expect("id field");

        assert_eq!(session.value, long_id);
        assert!(
            !session.value.contains('…'),
            "right pane session id should not be truncated: {}",
            session.value
        );
    }

    #[test]
    fn mux_session_detail_uses_full_native_session_name() {
        let long_native_id =
            "agentdeck_worktrunk-multi-repo_d459b661-extra-long-copyable-session-name";
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(long_native_id),
            backend: "tmux".into(),
            native_id: long_native_id.into(),
            cwd: Some("/home/op/src/worktrunk".into()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        let snapshot = resolve_snapshot(snapshot);
        let target = NodeId::MuxSession(MuxSessionId::new(long_native_id));
        let detail = build(&snapshot, &target, Some(home().as_path()));
        let mux = detail
            .header_fields
            .iter()
            .find(|f| f.label == "name")
            .expect("name field");

        assert_eq!(mux.value, long_native_id);
        assert!(
            !mux.value.contains('…'),
            "right pane mux name should not be truncated: {}",
            mux.value
        );
    }

    #[test]
    fn alias_replaces_title_in_detail_header_per_adr_0029() {
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(agent(
            "opencode",
            "abc",
            Some("/home/op/src/x"),
            Some("harness title that should be hidden"),
        ));
        let session_id = NodeId::AgentSession(AgentSessionId::new("opencode", "/state", "abc"));
        snapshot
            .aliases
            .insert(session_id.clone(), "ingest-refactor".to_string());
        let snapshot = resolve_snapshot(snapshot);
        let detail = build(&snapshot, &session_id, Some(home().as_path()));

        let labels: Vec<&str> = detail.header_fields.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            vec!["id", "harness", "cwd", "alias", "mux", "pr", "lineage"],
            "alias row replaces title row when both would be present"
        );
        let alias = detail
            .header_fields
            .iter()
            .find(|f| f.label == "alias")
            .expect("alias header present");
        assert_eq!(alias.value, "ingest-refactor");
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
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("scratch"),
            backend: "tmux".into(),
            native_id: "scratch".into(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id.clone(), cwd.to_string()),
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
            source: NodeId::Checkout(CheckoutId::new(repo_id.clone(), cwd.to_string())),
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
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
        let sections = detail.sections();
        assert_eq!(
            sections.iter().map(|s| s.kind).collect::<Vec<_>>(),
            vec![SectionKind::Mux, SectionKind::Session],
            "mux details should lead with mux fields, then attached sessions"
        );
        let mux_field_labels: Vec<&str> = sections[0].fields.iter().map(|f| f.label).collect();
        assert_eq!(mux_field_labels, vec!["name", "backend", "cwd", "attached"]);
        let session_field_labels: Vec<&str> = sections[1].fields.iter().map(|f| f.label).collect();
        assert_eq!(session_field_labels, vec!["session", "session"]);
        let session_values: Vec<&str> = sections[1]
            .fields
            .iter()
            .map(|f| f.value.as_str())
            .collect();
        assert_eq!(session_values, vec!["codex:a", "codex:b"]);
        let first_session = sections[1].fields.first().unwrap();
        assert_eq!(
            first_session
                .expanded_fields
                .iter()
                .map(|field| field.label)
                .collect::<Vec<_>>(),
            vec!["id", "harness", "cwd", "mux", "pr", "lineage"],
            "attached session row should carry one-level details for expansion"
        );
        let cwd = detail
            .header_fields
            .iter()
            .find(|f| f.label == "cwd")
            .unwrap();
        assert_eq!(cwd.value, "~/src/x");
    }

    #[test]
    fn process_links_surface_on_agent_and_mux_details() {
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let process_id = NodeId::RuntimeProcess(RuntimeProcessId::new("tmux:editor:pid:4242"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: Some("/home/op/src/x".into()),
            active_pane_command: None,
            active_pane_pid: Some(4242),
            active_pane_current_path: Some("/home/op/src/x".into()),
            active_pane_start_command: Some("codex".into()),
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/src/x"), None));
        snapshot
            .nodes
            .push(runtime_process("tmux:editor:pid:4242", 4242, "codex"));
        snapshot.candidate_links.push(process_link(
            "mux-process",
            mux_id.clone(),
            process_id.clone(),
            RelationKind::MuxContainsProcess,
        ));
        snapshot.candidate_links.push(process_link(
            "process-session",
            process_id.clone(),
            session_id.clone(),
            RelationKind::ProcessIdentifiesSession,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let session_detail = build(&snapshot, &session_id, Some(home().as_path()));
        let session_process = session_detail
            .header_fields
            .iter()
            .find(|field| field.label == "process")
            .expect("session process field");
        assert_eq!(session_process.value, "pid 4242: codex");
        assert_eq!(session_process.target, Some(process_id.clone()));

        let mux_detail = build(&snapshot, &mux_id, Some(home().as_path()));
        let mux_process = mux_detail
            .sections()
            .into_iter()
            .find(|section| section.kind == SectionKind::Process)
            .and_then(|section| section.fields.into_iter().next())
            .expect("mux process field");
        assert_eq!(mux_process.value, "pid 4242: codex");
        assert_eq!(mux_process.target, Some(process_id));
    }

    #[test]
    fn runtime_process_detail_links_back_to_mux_and_session() {
        let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let process_id = NodeId::RuntimeProcess(RuntimeProcessId::new("tmux:editor:pid:4242"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("editor"),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: None,
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot.nodes.push(agent("codex", "abc", None, None));
        snapshot
            .nodes
            .push(runtime_process("tmux:editor:pid:4242", 4242, "codex"));
        snapshot.candidate_links.push(process_link(
            "mux-process",
            mux_id.clone(),
            process_id.clone(),
            RelationKind::MuxContainsProcess,
        ));
        snapshot.candidate_links.push(process_link(
            "process-session",
            process_id.clone(),
            session_id.clone(),
            RelationKind::ProcessCandidatesSession,
        ));
        let snapshot = resolve_snapshot(snapshot);

        let detail = build(&snapshot, &process_id, Some(home().as_path()));
        let sections = detail.sections();
        assert_eq!(
            sections
                .iter()
                .map(|section| section.kind)
                .collect::<Vec<_>>(),
            vec![SectionKind::Process, SectionKind::Mux, SectionKind::Session]
        );
        let mux = detail
            .header_fields
            .iter()
            .find(|field| field.label == "mux")
            .expect("mux field");
        assert_eq!(mux.value, "tmux:editor");
        assert_eq!(mux.target, Some(mux_id));
        let session = detail
            .header_fields
            .iter()
            .find(|field| field.label == "session")
            .expect("session field");
        assert_eq!(session.value, "codex:abc");
        assert_eq!(session.target, Some(session_id));
        assert_eq!(session.annotation, Some("⚠"));
    }

    #[test]
    fn agent_session_mux_row_carries_mux_detail_for_expansion() {
        let mux_id = MuxSessionId::new("editor");
        let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
        let mut snapshot = GraphSnapshot::empty();
        snapshot.nodes.push(GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            backend: "tmux".into(),
            native_id: "editor".into(),
            cwd: Some("/home/op/src/x".into()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        }));
        snapshot
            .nodes
            .push(agent("codex", "abc", Some("/home/op/src/x"), None));
        snapshot.candidate_links.push(GraphLink {
            id: "attached".into(),
            source: session_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id),
            },
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
            .find(|field| field.label == "mux")
            .expect("mux field");

        assert_eq!(
            mux.target,
            Some(NodeId::MuxSession(MuxSessionId::new("editor")))
        );
        assert_eq!(
            mux.expanded_fields
                .iter()
                .map(|field| field.label)
                .collect::<Vec<_>>(),
            vec!["name", "backend", "cwd", "attached", "session"]
        );
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
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
