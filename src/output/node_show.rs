//! `conspectus node show <id>` (H-OBS-002).
//!
//! Read-only single-node view: prints the node itself plus every
//! candidate link, resolved relationship, source metadata, and
//! diagnostic that touches the node. The accepted forms for
//! `<id>` are:
//!
//! - The short content-addressed prefix from the session-table `ID`
//!   column (H-TBL-002). Any prefix length ≥ 4 hex chars is
//!   accepted; ambiguity errors with the matching candidates
//!   listed.
//! - The full `NodeId` `Display` form, e.g.
//!   `agent_session:codex:/state:session-x` or
//!   `mux_session:tmux:editor`.
//! - The harness/mux label that appears in the session table's
//!   `AGENT` or `MUX` column, e.g. `codex:session-x` or
//!   `tmux:editor`. The label only resolves when it uniquely
//!   identifies one node.
//! - A bare harness-native agent session key from the session
//!   table's `ID` column, when it uniquely identifies one node.
//!
//! In-memory renderer (P11-011b / ADR 0082). Iterates the
//! resolved [`GraphSnapshot`] directly — no SQLite materialization.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, CandidateScore, CheckoutNode, Diagnostic,
    ForgePrNode, ForkNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState,
    MuxSessionNode, NodeId, PinNode, ResolvedRelationship, RuntimeProcessNode, WorkspaceNode,
};
use crate::output::render::{self, header_style, node_short_id_from_display, push_styled};

/// Outcome of resolving an `<id>` argument to a [`NodeId`].
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

// -----------------------------------------------------------------------------
// Resolve
// -----------------------------------------------------------------------------

pub fn resolve_node_id(input: &str, snapshot: &GraphSnapshot) -> Result<NodeId, NodeResolveError> {
    let trimmed = input.trim();
    let is_hex_prefix = !trimmed.is_empty()
        && trimmed.len() <= 16
        && trimmed.chars().all(|c| c.is_ascii_hexdigit());

    let mut matches: BTreeMap<NodeId, ()> = BTreeMap::new();

    for node in &snapshot.nodes {
        let id = node.id();
        let display = id.to_string();
        let matches_input = (is_hex_prefix
            && node_short_id_from_display(&display).starts_with(trimmed))
            || display == trimmed;
        if matches_input {
            matches.insert(id, ());
        }
    }

    // Label matches: `<harness>:<session_key>` or
    // `<harness>:<title>` for agents, bare `<session_key>` for
    // agents when unique, `<backend>:<native_id>` for muxes.
    for node in &snapshot.nodes {
        match node {
            GraphNode::AgentSession(agent) => {
                let key_label = format!("{}:{}", agent.harness_key, agent.id.session_key);
                let title_label = agent
                    .title
                    .as_deref()
                    .map(|t| format!("{}:{}", agent.harness_key, t));
                if agent.id.session_key == trimmed
                    || key_label == trimmed
                    || title_label.as_deref() == Some(trimmed)
                {
                    matches.insert(NodeId::AgentSession(agent.id.clone()), ());
                }
            }
            GraphNode::MuxSession(mux) => {
                if format!("{}:{}", mux.backend, mux.native_id) == trimmed {
                    matches.insert(NodeId::MuxSession(mux.id.clone()), ());
                }
            }
            GraphNode::Pin(pin) => {
                if pin.id.id == trimmed || pin.display_name == trimmed {
                    matches.insert(NodeId::Pin(pin.id.clone()), ());
                }
            }
            _ => {}
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

// -----------------------------------------------------------------------------
// Render entry point
// -----------------------------------------------------------------------------

pub fn render_node_show(snapshot: &GraphSnapshot, id: &NodeId, color: bool) -> String {
    let mut out = String::new();
    if !write_node_summary(&mut out, snapshot, id, color) {
        return format!("node {id} not found in snapshot\n");
    }
    write_candidate_links(&mut out, snapshot, id, color);
    write_resolved(&mut out, snapshot, id, color);
    write_diagnostics(&mut out, snapshot, id, color);
    out
}

// -----------------------------------------------------------------------------
// Section helpers
// -----------------------------------------------------------------------------

fn write_section_header(out: &mut String, text: &str, color: bool) {
    push_styled(out, text, header_style(), color);
    out.push('\n');
}

fn node_kind_label(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::Pin(_) => "pin",
        NodeId::RuntimeProcess(_) => "runtime_process",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}

/// External display label for a node referenced from a link or
/// resolved relationship. Mirrors the pre-P11-011b
/// `node_reference_label_from_display` behavior.
fn node_reference_label(snapshot: &GraphSnapshot, id: &NodeId) -> String {
    let display = id.to_string();
    let Some(node) = snapshot.nodes.iter().find(|n| &n.id() == id) else {
        return display;
    };
    match (id, node) {
        (NodeId::AgentSession(_), GraphNode::AgentSession(agent)) => {
            format!("{}:{}", agent.harness_key, agent.id.session_key)
        }
        (NodeId::MuxSession(_), GraphNode::MuxSession(mux)) => {
            format!("{}:{}", mux.backend, mux.native_id)
        }
        (NodeId::Pin(_), GraphNode::Pin(pin)) => format!("pin:{}", pin.display_name),
        (NodeId::RuntimeProcess(_), GraphNode::RuntimeProcess(proc)) => {
            match (proc.pid, proc.command.as_deref()) {
                (Some(pid), Some(command)) => format!("pid {pid}: {command}"),
                (Some(pid), None) => format!("pid {pid}"),
                (None, Some(command)) => command.to_string(),
                (None, None) => proc.observation_key.clone(),
            }
        }
        (NodeId::ForgePr(_), GraphNode::ForgePr(pr)) => {
            format!("{}/{}#{}", pr.owner, pr.repo, pr.number)
        }
        (NodeId::Fork(_), GraphNode::Fork(fork)) => fork
            .name
            .clone()
            .unwrap_or_else(|| fork.provider_source_key.clone()),
        (NodeId::Checkout(_), GraphNode::Checkout(checkout)) => {
            format!("checkout:{}", checkout.root)
        }
        (NodeId::Workspace(_), GraphNode::Workspace(workspace)) => {
            format!("workspace:{}", workspace.root)
        }
        (NodeId::Repo(_), GraphNode::Repo(repo)) => {
            format!("repo:{}", repo.common_dir)
        }
        (NodeId::Branch(branch_id), _) => format!("branch:{}", branch_id.refname),
        _ => display,
    }
}

fn find_node<'a>(snapshot: &'a GraphSnapshot, id: &NodeId) -> Option<&'a GraphNode> {
    snapshot.nodes.iter().find(|n| &n.id() == id)
}

/// Returns `false` when the node doesn't exist; callers emit
/// "not found" output.
fn write_node_summary(
    out: &mut String,
    snapshot: &GraphSnapshot,
    id: &NodeId,
    color: bool,
) -> bool {
    let display = id.to_string();
    let id_short = node_short_id_from_display(&display);
    write_section_header(out, &format!("node {id_short}"), color);
    let _ = writeln!(out, "  kind: {}", node_kind_label(id));
    let _ = writeln!(out, "  id:   {display}");

    match (id, find_node(snapshot, id)) {
        (NodeId::Repo(_), Some(GraphNode::Repo(repo))) => {
            write_repo_summary(out, repo);
            true
        }
        (NodeId::Checkout(_), Some(GraphNode::Checkout(checkout))) => {
            write_checkout_summary(out, checkout);
            true
        }
        (NodeId::Workspace(_), Some(GraphNode::Workspace(workspace))) => {
            write_workspace_summary(out, snapshot, workspace);
            true
        }
        (NodeId::AgentSession(_), Some(GraphNode::AgentSession(agent))) => {
            write_agent_summary(out, snapshot, agent);
            true
        }
        (NodeId::MuxSession(_), Some(GraphNode::MuxSession(mux))) => {
            write_mux_summary(out, mux);
            true
        }
        (NodeId::Pin(_), Some(GraphNode::Pin(pin))) => {
            write_pin_summary(out, pin);
            true
        }
        (NodeId::RuntimeProcess(_), Some(GraphNode::RuntimeProcess(proc))) => {
            write_runtime_process_summary(out, proc);
            true
        }
        // Branches are summarized from their typed BranchId alone —
        // node_branches carries no extra columns the header cares
        // about. Detect presence by snapshot lookup.
        (NodeId::Branch(branch_id), Some(GraphNode::Branch(_))) => {
            write_branch_summary(out, branch_id);
            true
        }
        (NodeId::Fork(_), Some(GraphNode::Fork(fork))) => {
            write_fork_summary(out, fork);
            true
        }
        (NodeId::ForgePr(_), Some(GraphNode::ForgePr(pr))) => {
            write_forge_pr_summary(out, pr);
            true
        }
        _ => false,
    }
}

fn write_repo_summary(out: &mut String, repo: &crate::model::RepoNode) {
    let _ = writeln!(out, "  common_dir: {}", repo.common_dir);
    if !repo.source_paths.is_empty() {
        let _ = writeln!(out, "  source_paths:");
        for path in &repo.source_paths {
            let _ = writeln!(out, "    - {path}");
        }
    }
}

fn write_checkout_summary(out: &mut String, checkout: &CheckoutNode) {
    let _ = writeln!(out, "  root: {}", checkout.root);
    if let Some(git_dir) = &checkout.git_dir {
        let _ = writeln!(out, "  git_dir: {git_dir}");
    }
}

fn write_workspace_summary(out: &mut String, snapshot: &GraphSnapshot, workspace: &WorkspaceNode) {
    let _ = writeln!(out, "  root:     {}", workspace.root);
    if let Some(provider) = &workspace.provider {
        let _ = writeln!(out, "  provider: {provider}");
    }
    if let Some(name) = &workspace.name {
        let _ = writeln!(out, "  name:     {name}");
    }
    for member in workspace_member_displays(snapshot, &workspace.id) {
        let _ = writeln!(out, "  member:   {member}");
    }
}

/// Resolver-chosen `workspace_contains_repo` members rendered as
/// the basename of each selected link's `logical_path` source
/// field. Mirrors the TUI's `member` HeaderField labeling.
fn workspace_member_displays(
    snapshot: &GraphSnapshot,
    workspace_id: &crate::model::WorkspaceId,
) -> Vec<String> {
    let link_by_id: BTreeMap<&str, &GraphLink> = snapshot
        .candidate_links
        .iter()
        .map(|link| (link.id.as_str(), link))
        .collect();
    let mut entries: Vec<(NodeId, &GraphLink)> = Vec::new();
    for resolved in &snapshot.resolved_relationships {
        if !matches!(
            resolved.relation,
            crate::model::RelationKind::WorkspaceContainsRepo
        ) {
            continue;
        }
        let NodeId::Workspace(source) = &resolved.source else {
            continue;
        };
        if source != workspace_id {
            continue;
        }
        let Some(link_id) = resolved.selected_link_id.as_deref() else {
            continue;
        };
        if let Some(link) = link_by_id.get(link_id) {
            entries.push((resolved.target.clone(), *link));
        }
    }
    entries.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));

    let mut out: Vec<String> = Vec::new();
    for (target, link) in entries {
        let display = link
            .source_metadata
            .fields
            .get("logical_path")
            .and_then(|v| v.as_str())
            .and_then(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| match &target {
                NodeId::Repo(repo_id) => repo_id.common_dir.clone(),
                _ => target.to_string(),
            });
        out.push(display);
    }
    out
}

fn write_agent_summary(out: &mut String, snapshot: &GraphSnapshot, agent: &AgentSessionNode) {
    let _ = writeln!(out, "  harness:     {}", agent.harness_key);
    let _ = writeln!(out, "  state_scope: {}", agent.id.state_scope);
    let _ = writeln!(out, "  session_key: {}", agent.id.session_key);
    if let Some(cwd) = &agent.cwd {
        let _ = writeln!(out, "  cwd:         {cwd}");
    }
    let alias = snapshot
        .aliases
        .get(&NodeId::AgentSession(agent.id.clone()))
        .map(|s| s.to_string());
    if let Some(alias) = &alias {
        let _ = writeln!(out, "  alias:       {alias}");
    }
    if alias.is_none()
        && let Some(title) = &agent.title
    {
        let _ = writeln!(out, "  title:       {title}");
    }
    for workspace_label in agent_workspace_labels(snapshot, &agent.id) {
        let _ = writeln!(out, "  workspace:   {workspace_label}");
    }
}

/// Resolved `associated_with` workspace targets for a session,
/// labeled by `WorkspaceNode.name` falling back to the basename
/// of `root`. Multi-workspace sessions emit one row per
/// association.
fn agent_workspace_labels(snapshot: &GraphSnapshot, agent_id: &AgentSessionId) -> Vec<String> {
    let workspace_lookup: BTreeMap<NodeId, &WorkspaceNode> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Workspace(workspace) => {
                Some((NodeId::Workspace(workspace.id.clone()), workspace))
            }
            _ => None,
        })
        .collect();
    let mut entries: Vec<(String, &WorkspaceNode)> = Vec::new();
    for resolved in &snapshot.resolved_relationships {
        if !matches!(
            resolved.relation,
            crate::model::RelationKind::AssociatedWith
        ) {
            continue;
        }
        let NodeId::AgentSession(source_id) = &resolved.source else {
            continue;
        };
        if source_id != agent_id {
            continue;
        }
        let NodeId::Workspace(_) = &resolved.target else {
            continue;
        };
        if let Some(workspace) = workspace_lookup.get(&resolved.target) {
            entries.push((workspace.root.clone(), workspace));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
        .into_iter()
        .map(|(root, workspace)| {
            workspace.name.clone().unwrap_or_else(|| {
                std::path::Path::new(&root)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or(root)
            })
        })
        .collect()
}

fn write_mux_summary(out: &mut String, mux: &MuxSessionNode) {
    let _ = writeln!(out, "  backend:   {}", mux.backend);
    let _ = writeln!(out, "  native_id: {}", mux.native_id);
    if let Some(cwd) = &mux.cwd {
        let _ = writeln!(out, "  cwd:       {cwd}");
    }
}

fn write_pin_summary(out: &mut String, pin: &PinNode) {
    let _ = writeln!(out, "  name:     {}", pin.display_name);
    let _ = writeln!(out, "  harness:  {}", pin.harness);
    let _ = writeln!(out, "  cwd:      {}", pin.cwd);
    let _ = writeln!(out, "  mux:      {}", pin.mux.native_id());
    let _ = writeln!(out, "  store:    {}", pin.store_path);
    let _ = writeln!(out, "  source:   {}", pin.provenance.snake_case());
    if let Some(argv) = &pin.launch_argv {
        let _ = writeln!(out, "  launch:   {}", argv.join(" "));
    }
    if let Some(reason) = &pin.reason {
        let _ = writeln!(out, "  reason:   {reason}");
    }
    let binding = match &pin.binding {
        Some(crate::model::PinBinding::Bound { mux, session }) => {
            format!(
                "bound to {} via {}:{}",
                mux.native_id, session.harness_key, session.session_key
            )
        }
        Some(crate::model::PinBinding::StaleMux { mux }) => {
            format!("stale mux {}", mux.native_id)
        }
        Some(crate::model::PinBinding::Unbound) => "unbound".to_string(),
        None => "unresolved".to_string(),
    };
    let _ = writeln!(out, "  binding:  {binding}");
}

fn write_runtime_process_summary(out: &mut String, proc: &RuntimeProcessNode) {
    let _ = writeln!(out, "  observation_key: {}", proc.observation_key);
    if let Some(pid) = proc.pid {
        let _ = writeln!(out, "  pid:             {pid}");
    }
    if let Some(parent_pid) = proc.parent_pid {
        let _ = writeln!(out, "  parent_pid:      {parent_pid}");
    }
    if let Some(root_pane_pid) = proc.root_pane_pid {
        let _ = writeln!(out, "  root_pane_pid:   {root_pane_pid}");
    }
    if let Some(command) = &proc.command {
        let _ = writeln!(out, "  command:         {command}");
    }
    if let Some(cwd) = &proc.cwd {
        let _ = writeln!(out, "  cwd:             {cwd}");
    }
    if let Some(harness_key) = &proc.harness_key {
        let _ = writeln!(out, "  harness:         {harness_key}");
    }
    if let Some(role) = &proc.role {
        let role_label = match role {
            crate::model::RuntimeProcessRole::HumanAgent => "human_agent",
            crate::model::RuntimeProcessRole::Subagent => "subagent",
            crate::model::RuntimeProcessRole::Background => "background",
            crate::model::RuntimeProcessRole::Shell => "shell",
            crate::model::RuntimeProcessRole::Unknown => "unknown",
        };
        let _ = writeln!(out, "  role:            {role_label}");
    }
    if let Some(depth) = proc.depth {
        let _ = writeln!(out, "  depth:           {depth}");
    }
    if let Some(observed_epoch) = proc.observed_epoch {
        let _ = writeln!(out, "  observed_epoch:  {observed_epoch}");
    }
}

fn write_branch_summary(out: &mut String, id: &BranchId) {
    let _ = writeln!(out, "  repo:    {}", id.repo);
    let _ = writeln!(out, "  refname: {}", id.refname);
}

fn write_fork_summary(out: &mut String, fork: &ForkNode) {
    let _ = writeln!(out, "  provider: {}", fork.provider);
    let _ = writeln!(out, "  provider_source_key: {}", fork.provider_source_key);
    if let Some(name) = &fork.name {
        let _ = writeln!(out, "  name:     {name}");
    }
    if let Some(scope) = &fork.scope {
        let _ = writeln!(out, "  scope:    {scope}");
    }
}

fn write_forge_pr_summary(out: &mut String, pr: &ForgePrNode) {
    let _ = writeln!(
        out,
        "  pr:    {}/{}#{} ({})",
        pr.owner,
        pr.repo,
        pr.number,
        pr.state.as_deref().unwrap_or("?")
    );
    if let Some(url) = &pr.url {
        let _ = writeln!(out, "  url:   {url}");
    }
}

// -----------------------------------------------------------------------------
// Candidate links / resolved relationships / diagnostics
// -----------------------------------------------------------------------------

fn write_candidate_links(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    let mut outgoing: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| &link.source == id)
        .collect();
    outgoing.sort_by(|a, b| a.id.cmp(&b.id));
    let mut incoming: Vec<&GraphLink> = snapshot
        .candidate_links
        .iter()
        .filter(|link| match &link.target {
            LinkEndpoint::Node { id: target_id } => target_id == id,
            _ => false,
        })
        .collect();
    incoming.sort_by(|a, b| a.id.cmp(&b.id));

    out.push('\n');
    write_section_header(
        out,
        &format!("outgoing candidate links: {}", outgoing.len()),
        color,
    );
    for link in &outgoing {
        write_link(out, snapshot, link, LinkDirection::Outgoing);
    }
    out.push('\n');
    write_section_header(
        out,
        &format!("incoming candidate links: {}", incoming.len()),
        color,
    );
    for link in &incoming {
        write_link(out, snapshot, link, LinkDirection::Incoming);
    }
}

enum LinkDirection {
    Outgoing,
    Incoming,
}

fn write_link(out: &mut String, snapshot: &GraphSnapshot, link: &GraphLink, dir: LinkDirection) {
    let other = match dir {
        LinkDirection::Outgoing => match &link.target {
            LinkEndpoint::Node { id } => {
                format!("→ {}", node_reference_label(snapshot, id))
            }
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
        LinkDirection::Incoming => {
            format!("← {}", node_reference_label(snapshot, &link.source))
        }
    };
    let state_label = match &link.state {
        LinkState::Active => "active",
        LinkState::Ignored { .. } => "ignored",
        LinkState::Overridden { .. } => "overridden",
    };
    let _ = writeln!(
        out,
        "  - {relation:15} {other} [{ind}, {state}] (link={link_id})",
        relation = link.relation.snake_case(),
        ind = render::indicator_from_tags(
            link.provenance.snake_case(),
            link.confidence.snake_case(),
            false
        ),
        state = state_label,
        link_id = link.id,
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

fn write_resolved(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    let mut entries: Vec<&ResolvedRelationship> = snapshot
        .resolved_relationships
        .iter()
        .filter(|r| &r.source == id || &r.target == id)
        .collect();
    entries.sort_by(|a, b| {
        a.relation
            .snake_case()
            .cmp(b.relation.snake_case())
            .then_with(|| a.source.to_string().cmp(&b.source.to_string()))
            .then_with(|| a.target.to_string().cmp(&b.target.to_string()))
    });

    out.push('\n');
    write_section_header(
        out,
        &format!("resolved relationships: {}", entries.len()),
        color,
    );
    for rel in &entries {
        let selected = rel.selected_link_id.as_deref().unwrap_or("");
        let _ = writeln!(
            out,
            "  - {relation:15} {source} → {target} (selected={selected})",
            relation = rel.relation.snake_case(),
            source = node_reference_label(snapshot, &rel.source),
            target = node_reference_label(snapshot, &rel.target),
        );
        if !rel.competing_link_ids.is_empty() {
            let _ = writeln!(
                out,
                "      competing: {}",
                rel.competing_link_ids.join(", ")
            );
        }
        if let Some(explanation) = &rel.explanation {
            if let Some(axis) = &explanation.decisive_axis {
                let _ = writeln!(out, "      decisive axis: {axis}");
            }
            if let Some(selected) = &explanation.selected {
                write_candidate_score(out, "selected score", selected);
            }
            for candidate in &explanation.competing {
                write_candidate_score(out, "competing score", candidate);
            }
        }
    }
}

fn write_candidate_score(out: &mut String, label: &str, score: &CandidateScore) {
    let _ = writeln!(out, "      {label}: {}", score.link_id);
    for axis in &score.axes {
        let _ = writeln!(out, "        {}: {}", axis.name, axis.value);
    }
}

fn write_diagnostics(out: &mut String, snapshot: &GraphSnapshot, id: &NodeId, color: bool) {
    // UnresolvedEndpoint: the diagnostic touches the node when
    // the referenced candidate link has source = this node.
    let mut unresolved: Vec<(&String, &crate::model::RelationKind)> = Vec::new();
    let mut conflicts: Vec<(&crate::model::RelationKind, &String, &Vec<String>)> = Vec::new();
    for diagnostic in &snapshot.diagnostics {
        match diagnostic {
            Diagnostic::UnresolvedEndpoint { link_id, relation } => {
                let touches = snapshot
                    .candidate_links
                    .iter()
                    .any(|link| &link.id == link_id && &link.source == id);
                if touches {
                    unresolved.push((link_id, relation));
                }
            }
            Diagnostic::Conflict {
                source,
                relation,
                selected_link_id,
                competing_link_ids,
            } => {
                if source == id {
                    conflicts.push((relation, selected_link_id, competing_link_ids));
                }
            }
            _ => {}
        }
    }
    unresolved.sort_by(|a, b| a.0.cmp(b.0));
    conflicts.sort_by(|a, b| a.1.cmp(b.1));

    let total = unresolved.len() + conflicts.len();
    out.push('\n');
    write_section_header(out, &format!("diagnostics: {total}"), color);
    for (link_id, relation) in &unresolved {
        let _ = writeln!(
            out,
            "  - unresolved_endpoint {} (link={link_id})",
            relation.snake_case()
        );
    }
    for (relation, selected, competing) in &conflicts {
        let _ = writeln!(
            out,
            "  - conflict {} source={id} selected={selected} competing={}",
            relation.snake_case(),
            competing.join(", "),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode,
        LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, Provenance, RelationKind,
        RepoId, RepoNode, SourceMetadata, WorkspaceId, WorkspaceNode,
    };
    use crate::resolve::resolve_snapshot;

    fn agent_node(harness: &str, scope: &str, key: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(harness, scope, key),
            harness_key: harness.to_string(),
            cwd: Some("/work".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        })
    }

    fn mux_node(backend: &str, name: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("{backend}:{name}")),
            backend: backend.to_string(),
            native_id: name.to_string(),
            cwd: Some("/work".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
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
        let prefix = &node_short_id_from_display(&id.to_string())[..6];
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
        let snapshot = GraphSnapshot {
            nodes: vec![
                agent_node("codex", "/state", "one"),
                agent_node("codex", "/state", "two"),
            ],
            ..GraphSnapshot::empty()
        };
        let first = node_short_id_from_display(&snapshot.nodes[0].id().to_string());
        let second = node_short_id_from_display(&snapshot.nodes[1].id().to_string());
        let shared_len = first
            .chars()
            .zip(second.chars())
            .take_while(|(a, b)| a == b)
            .count();
        if shared_len == 0 {
            return;
        }
        let prefix = &first[..shared_len];
        let err = resolve_node_id(prefix, &snapshot).unwrap_err();
        assert!(matches!(err, NodeResolveError::Ambiguous { .. }));
    }

    #[test]
    fn node_show_renders_alias_in_place_of_title() {
        let mut snapshot = GraphSnapshot {
            nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("opencode", "/state", "alpha"),
                harness_key: "opencode".to_string(),
                cwd: Some("/work".to_string()),
                title: Some("harness title that should be hidden".to_string()),
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            })],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();
        snapshot
            .aliases
            .insert(id.clone(), "ingest-refactor".to_string());

        let output = render_node_show(&snapshot, &id, false);
        assert!(
            output.contains("alias:       ingest-refactor"),
            "alias row missing in:\n{output}",
        );
        assert!(
            !output.contains("title:"),
            "title row should be hidden when alias is set:\n{output}",
        );
    }

    #[test]
    fn node_show_falls_back_to_title_when_no_alias() {
        let snapshot = GraphSnapshot {
            nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new("opencode", "/state", "alpha"),
                harness_key: "opencode".to_string(),
                cwd: Some("/work".to_string()),
                title: Some("Phase 8 mockup".to_string()),
                last_message_preview: None,
                last_active_epoch: None,
                session_kind: None,
            })],
            ..GraphSnapshot::empty()
        };
        let id = snapshot.nodes[0].id();

        let output = render_node_show(&snapshot, &id, false);
        assert!(
            output.contains("title:       Phase 8 mockup"),
            "title row should appear when alias is unset:\n{output}",
        );
        assert!(!output.contains("alias:"));
    }

    fn workspace_node(root: &str, provider: Option<&str>, name: Option<&str>) -> GraphNode {
        GraphNode::Workspace(WorkspaceNode {
            id: WorkspaceId::new(root),
            root: root.to_string(),
            provider: provider.map(str::to_string),
            name: name.map(str::to_string),
        })
    }

    fn repo_node(common_dir: &str) -> GraphNode {
        GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
    }

    fn workspace_contains_repo_link(
        link_id: &str,
        workspace_root: &str,
        repo_common_dir: &str,
        logical_path: &str,
    ) -> GraphLink {
        let mut fields = Metadata::new();
        fields.insert(
            "logical_path".to_string(),
            serde_json::Value::String(logical_path.to_string()),
        );
        GraphLink {
            id: link_id.to_string(),
            source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(RepoId::new(repo_common_dir)),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "test".to_string(),
                evidence: None,
                fields,
                freshness_epoch: None,
            },
            state: LinkState::Active,
        }
    }

    fn session_workspace_link(
        link_id: &str,
        session_id: NodeId,
        workspace_root: &str,
    ) -> GraphLink {
        GraphLink {
            id: link_id.to_string(),
            source: session_id,
            target: LinkEndpoint::Node {
                id: NodeId::Workspace(WorkspaceId::new(workspace_root)),
            },
            relation: RelationKind::AssociatedWith,
            provenance: Provenance::Discovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata::default(),
            state: LinkState::Active,
        }
    }

    #[test]
    fn workspace_node_show_includes_provider_name_and_member_rows() {
        let workspace_root = "/work/atelier";
        let repo_a = "/work/atelier/atelier/.git";
        let repo_b = "/work/atelier/conspectus/.git";
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
                repo_node(repo_a),
                repo_node(repo_b),
            ],
            candidate_links: vec![
                workspace_contains_repo_link(
                    "ws-atelier",
                    workspace_root,
                    repo_a,
                    "/work/atelier/atelier",
                ),
                workspace_contains_repo_link(
                    "ws-conspectus",
                    workspace_root,
                    repo_b,
                    "/work/atelier/conspectus",
                ),
            ],
            ..GraphSnapshot::empty()
        });
        let target = NodeId::Workspace(WorkspaceId::new(workspace_root));
        let rendered = render_node_show(&snapshot, &target, false);

        assert!(rendered.contains("kind: workspace"), "got:\n{rendered}");
        assert!(rendered.contains("provider: atelier"), "got:\n{rendered}");
        assert!(
            rendered.contains("name:     atelier-ws"),
            "got:\n{rendered}"
        );
        assert!(rendered.contains("member:   atelier"), "got:\n{rendered}");
        assert!(
            rendered.contains("member:   conspectus"),
            "got:\n{rendered}",
        );
    }

    #[test]
    fn workspace_node_show_omits_member_rows_when_no_members() {
        let workspace_root = "/work/empty";
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![workspace_node(workspace_root, None, None)],
            ..GraphSnapshot::empty()
        });
        let target = NodeId::Workspace(WorkspaceId::new(workspace_root));
        let rendered = render_node_show(&snapshot, &target, false);

        assert!(rendered.contains("kind: workspace"));
        assert!(
            !rendered.contains("member:"),
            "no member rows expected when the workspace has no resolved repos:\n{rendered}",
        );
        assert!(
            !rendered.contains("provider:"),
            "no provider row expected when the workspace has no provider:\n{rendered}",
        );
    }

    #[test]
    fn agent_session_node_show_includes_workspace_row() {
        let workspace_root = "/work/atelier";
        let session = agent_node("codex", "/state", "alpha");
        let session_id = session.id();
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                session,
                workspace_node(workspace_root, Some("atelier"), Some("atelier-ws")),
            ],
            candidate_links: vec![session_workspace_link(
                "assoc-1",
                session_id.clone(),
                workspace_root,
            )],
            ..GraphSnapshot::empty()
        });
        let rendered = render_node_show(&snapshot, &session_id, false);

        assert!(rendered.contains("kind: agent_session"));
        assert!(
            rendered.contains("workspace:   atelier-ws"),
            "workspace row missing from agent_session output:\n{rendered}",
        );
    }

    #[test]
    fn agent_session_node_show_emits_one_workspace_row_per_association() {
        let session = agent_node("codex", "/state", "alpha");
        let session_id = session.id();
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![
                session,
                workspace_node("/work/a", None, Some("alpha-ws")),
                workspace_node("/work/b", None, Some("beta-ws")),
            ],
            candidate_links: vec![
                session_workspace_link("assoc-a", session_id.clone(), "/work/a"),
                session_workspace_link("assoc-b", session_id.clone(), "/work/b"),
            ],
            ..GraphSnapshot::empty()
        });
        let rendered = render_node_show(&snapshot, &session_id, false);

        assert!(rendered.contains("workspace:   alpha-ws"));
        assert!(rendered.contains("workspace:   beta-ws"));
    }

    #[test]
    fn agent_session_node_show_omits_workspace_row_when_unassociated() {
        let session = agent_node("codex", "/state", "alpha");
        let session_id = session.id();
        let snapshot = resolve_snapshot(GraphSnapshot {
            nodes: vec![session],
            ..GraphSnapshot::empty()
        });
        let rendered = render_node_show(&snapshot, &session_id, false);
        assert!(
            !rendered.contains("workspace:"),
            "no workspace row expected without an association:\n{rendered}",
        );
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
            resolved_relationships: vec![crate::model::ResolvedRelationship {
                source: agent_id.clone(),
                target: mux_id.clone(),
                relation: RelationKind::LinkedToMux,
                selected_link_id: Some("link-1".to_string()),
                competing_link_ids: vec![],
                explanation: None,
            }],
            ..GraphSnapshot::empty()
        };
        let rendered = render_node_show(&snapshot, &agent_id, false);
        assert!(rendered.contains("kind: agent_session"));
        assert!(rendered.contains("outgoing candidate links: 1"));
        assert!(rendered.contains("linked_to_mux"));
        assert!(
            rendered.contains("linked_to_mux   → tmux:editor"),
            "candidate link should use mux-native label:\n{rendered}",
        );
        assert!(rendered.contains("resolved relationships: 1"));
        assert!(
            rendered.contains("linked_to_mux   codex:alpha → tmux:editor"),
            "resolved relationship should use external labels:\n{rendered}",
        );
        assert!(rendered.contains("selected=link-1"));
    }
}
