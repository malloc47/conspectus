//! Resolve a pin slot against the graph (ADR 0141): which node it
//! points at, whether that node is live, gone, or unknown, and each
//! recipe field's effective value with the layer that supplied it.
//!
//! Precedence per field: overrides > live graph > history > snapshot >
//! defaults. The snapshot is only the slot's own copy from the last
//! operator gesture, so it comes after everything Conspectus can see.

use std::collections::BTreeMap;

use crate::discovery::harness::{adapter_in_command, launch_argv_for};
use crate::history::project::live_mux_for;
use crate::history::recorder::short_ref;
use crate::model::{
    ArgvSource, CheckoutId, Confidence, FieldSource, FieldValue, ForgePrId, ForgePrNode, Freshness,
    GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, MuxRecord, MuxSessionId, NodeId,
    NodePresence, NodeProvenance, PinId, PinKind, PinNode, PinSlot, Provenance, RecipeField,
    RelationKind, RepoId, SlotStatus, SlotTarget, UnresolvedEndpoint,
};

/// The adapter and provider key pin nodes and their links carry.
pub const PINS_ADAPTER: &str = "pins";

/// Build the `PinNode` for every slot, with its status and effective
/// recipe, and a `pin_targets` link to what it points at. Idempotent:
/// it drops what an earlier pass built first, so the daemon can
/// resolve an already-resolved snapshot again.
///
/// A PR slot whose PR left the open listing still needs a PR node to
/// point at, so the node is projected from the PR's history record
/// (ADR 0140).
pub fn apply_slots(snapshot: &mut GraphSnapshot) {
    snapshot
        .nodes
        .retain(|node| !matches!(node, GraphNode::Pin(_)));
    snapshot
        .node_provenance
        .retain(|id, _| !matches!(id, NodeId::Pin(_)));
    snapshot
        .candidate_links
        .retain(|link| link.source_metadata.adapter != PINS_ADAPTER);

    let slots = snapshot.pins.clone();
    let mut resolved: Vec<(PinSlot, ResolvedSlot)> = Vec::with_capacity(slots.len());
    for slot in slots {
        let resolution = resolve_slot(snapshot, &slot);
        project_pr(snapshot, &resolution);
        resolved.push((slot, resolution));
    }

    for (slot, resolution) in resolved {
        let pin_id = PinId::new(slot.id.clone());
        let id = NodeId::Pin(pin_id.clone());
        let node = PinNode {
            id: pin_id,
            kind: slot.kind,
            label: slot.label(),
            target: slot.target.clone(),
            target_node: resolution.target_node.clone(),
            status: resolution.status,
            overrides: slot.overrides.clone(),
            snapshot: slot.snapshot.clone(),
            effective: resolution.effective.clone(),
            order: slot.order,
            legacy: slot.legacy,
            pinned_at: slot.pinned_at.clone(),
        };
        snapshot.node_provenance.insert(
            id.clone(),
            NodeProvenance {
                provider: PINS_ADAPTER.to_string(),
                freshness_epoch: None,
            },
        );
        snapshot.nodes.push(GraphNode::Pin(node));
        if let Some(target) = &resolution.target_node {
            snapshot
                .candidate_links
                .push(target_link(snapshot, &slot, id, target));
        }
    }
    snapshot.canonicalize();
}

fn target_link(
    snapshot: &GraphSnapshot,
    slot: &PinSlot,
    source: NodeId,
    target: &NodeId,
) -> GraphLink {
    let endpoint = if snapshot.find_node(target).is_some() {
        LinkEndpoint::Node { id: target.clone() }
    } else {
        // The target is real but outside this scan: a checkout whose
        // root exists but isn't under the scan roots.
        LinkEndpoint::Unresolved {
            evidence: UnresolvedEndpoint {
                node_type: slot.kind.as_str().to_string(),
                harness_key: None,
                native_id: Some(target.to_string()),
                state_scope: None,
                path: None,
                metadata: Default::default(),
            },
        }
    };
    let mut link = GraphLink::new(
        format!("pin:{}:targets", slot.id),
        source,
        endpoint,
        RelationKind::PinTargets,
        Provenance::GlobalPin,
    );
    link.confidence = Confidence::High;
    link.freshness = Freshness::Fresh;
    link.source_metadata.adapter = PINS_ADAPTER.to_string();
    link
}

/// Add a PR node from history for a PR slot whose PR is no longer
/// listed.
fn project_pr(snapshot: &mut GraphSnapshot, resolution: &ResolvedSlot) {
    let (Some(NodeId::ForgePr(id)), Some(record_id)) =
        (&resolution.target_node, &resolution.record_id)
    else {
        return;
    };
    if snapshot.find_node(&NodeId::ForgePr(id.clone())).is_some() {
        return;
    }
    let Some(record) = snapshot.history.pr(record_id) else {
        return;
    };
    let r = &record.recipe;
    let node = ForgePrNode {
        id: id.clone(),
        provider: r.provider.clone(),
        host: r.host.clone(),
        owner: r.owner.clone(),
        repo: r.repo.clone(),
        number: r.number,
        state: r.state.clone(),
        title: r.title.clone(),
        url: r.url.clone(),
        updated_epoch: None,
        is_draft: r.draft,
        head_ref: r.head_ref.clone(),
    };
    let last_seen = record.meta.last_seen;
    snapshot.node_provenance.insert(
        NodeId::ForgePr(id.clone()),
        NodeProvenance {
            provider: crate::history::HISTORY_ADAPTER.to_string(),
            freshness_epoch: Some(last_seen),
        },
    );
    snapshot.nodes.push(GraphNode::ForgePr(node));
}

/// What a slot resolves to.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSlot {
    /// The node the slot points at, live or gone. `None` for a planned
    /// slot, or one whose target is in neither graph nor history.
    pub target_node: Option<NodeId>,
    /// The history record behind the target, when there is one.
    pub record_id: Option<String>,
    pub status: SlotStatus,
    pub effective: Vec<RecipeField>,
}

impl ResolvedSlot {
    pub fn field(&self, name: &str) -> Option<&FieldValue> {
        self.effective
            .iter()
            .find(|field| field.name == name)
            .map(|field| &field.value)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.field(name).and_then(FieldValue::as_text)
    }

    pub fn list(&self, name: &str) -> Option<&[String]> {
        self.field(name).and_then(FieldValue::as_list)
    }
}

/// Resolve one slot.
pub fn resolve_slot(snapshot: &GraphSnapshot, slot: &PinSlot) -> ResolvedSlot {
    let target = resolve_target(snapshot, slot);
    let layers = Layers::new(snapshot, slot, &target);
    let mut effective = Vec::new();
    for name in slot.kind.recipe_fields() {
        if let Some((value, source)) = layers.value(name) {
            effective.push(RecipeField {
                name: (*name).to_string(),
                value,
                source,
            });
        }
    }
    if let Some((value, source)) = layers.value("label") {
        effective.insert(
            0,
            RecipeField {
                name: "label".to_string(),
                value,
                source,
            },
        );
    }
    ResolvedSlot {
        target_node: target.node,
        record_id: target.record_id,
        status: target.status,
        effective,
    }
}

#[derive(Debug)]
struct Target {
    node: Option<NodeId>,
    record_id: Option<String>,
    status: SlotStatus,
}

fn resolve_target(snapshot: &GraphSnapshot, slot: &PinSlot) -> Target {
    let missing = Target {
        node: None,
        record_id: None,
        status: SlotStatus::Missing,
    };
    let Ok(target) = slot.parsed_target() else {
        return missing;
    };
    match target {
        SlotTarget::Planned => planned_target(snapshot, slot),
        SlotTarget::History(record_id) => history_target(snapshot, slot.kind, &record_id),
        SlotTarget::Node(id) => node_target(snapshot, &id),
    }
}

fn presence_status(presence: Option<NodePresence>) -> SlotStatus {
    match presence {
        None => SlotStatus::Live,
        Some(NodePresence::Gone { .. }) => SlotStatus::Gone,
        Some(NodePresence::Unknown { .. }) => SlotStatus::Unknown,
    }
}

/// A planned mux slot that names a mux which is already running (a
/// legacy pin, or a slot whose first rebuild hasn't been retargeted
/// yet) shows that mux's state.
fn planned_target(snapshot: &GraphSnapshot, slot: &PinSlot) -> Target {
    let planned = Target {
        node: None,
        record_id: None,
        status: SlotStatus::Planned,
    };
    if slot.kind != PinKind::Mux {
        return planned;
    }
    let Some(name) = slot
        .overrides
        .get("name")
        .or_else(|| slot.snapshot.get("name"))
        .and_then(FieldValue::as_text)
    else {
        return planned;
    };
    let socket = slot
        .overrides
        .get("socket")
        .or_else(|| slot.snapshot.get("socket"))
        .and_then(FieldValue::as_text);
    let found = snapshot.live_nodes().find_map(|node| match node {
        GraphNode::MuxSession(mux)
            if mux.session_name() == name
                && crate::model::effective_socket(mux.socket.as_deref())
                    == crate::model::effective_socket(socket) =>
        {
            Some(NodeId::MuxSession(mux.id.clone()))
        }
        _ => None,
    });
    match found {
        Some(node) => Target {
            node: Some(node),
            record_id: None,
            status: SlotStatus::Live,
        },
        None => planned,
    }
}

fn history_target(snapshot: &GraphSnapshot, kind: PinKind, record_id: &str) -> Target {
    let missing = || Target {
        node: None,
        record_id: None,
        status: SlotStatus::Missing,
    };
    let history = &snapshot.history;
    let presence = history.presence_of(record_id);
    let found = |node: NodeId, status: SlotStatus| Target {
        node: Some(node),
        record_id: Some(record_id.to_string()),
        status,
    };
    match kind {
        PinKind::Mux => {
            let Some(record) = history.mux(record_id) else {
                return missing();
            };
            if let Some(node) = live_mux_for(snapshot, record) {
                return found(node, SlotStatus::Live);
            }
            let node = NodeId::MuxSession(MuxSessionId::new(record.recipe.native_id()));
            found(
                node,
                presence.map_or(SlotStatus::Unknown, |p| presence_status(Some(p))),
            )
        }
        PinKind::Checkout => {
            let Some(record) = history.checkout(record_id) else {
                return missing();
            };
            let repo = RepoId::new(record.recipe.common_dir().to_string());
            let node = NodeId::Checkout(CheckoutId::new(repo, record.recipe.root.clone()));
            found(node, presence_status(presence))
        }
        PinKind::Repo => {
            let Some(record) = history.repo(record_id) else {
                return missing();
            };
            let node = NodeId::Repo(RepoId::new(record.recipe.common_dir.clone()));
            found(node, presence_status(presence))
        }
        PinKind::Pr => {
            let Some(record) = history.pr(record_id) else {
                return missing();
            };
            let r = &record.recipe;
            let node = NodeId::ForgePr(ForgePrId::new(
                r.provider.clone(),
                r.host.clone(),
                r.owner.clone(),
                r.repo.clone(),
                r.number,
            ));
            // A PR never goes away; it only leaves the open listing.
            found(node, SlotStatus::Live)
        }
        _ => missing(),
    }
}

fn node_target(snapshot: &GraphSnapshot, id: &NodeId) -> Target {
    if snapshot.find_node(id).is_some() {
        return Target {
            node: Some(id.clone()),
            record_id: None,
            status: presence_status(snapshot.presence(id)),
        };
    }
    // A PR that left the listing lives on as its history record.
    if let NodeId::ForgePr(_) = id
        && let Some((_, meta)) = snapshot.history.by_key(&id.to_string())
    {
        return Target {
            node: Some(id.clone()),
            record_id: Some(meta.id.clone()),
            status: SlotStatus::Live,
        };
    }
    Target {
        node: None,
        record_id: None,
        status: SlotStatus::Missing,
    }
}

/// The values each layer can supply for one slot.
struct Layers<'a> {
    snapshot: &'a GraphSnapshot,
    slot: &'a PinSlot,
    live: BTreeMap<&'static str, FieldValue>,
    history: BTreeMap<&'static str, FieldValue>,
    history_argv_exact: bool,
}

impl<'a> Layers<'a> {
    fn new(snapshot: &'a GraphSnapshot, slot: &'a PinSlot, target: &Target) -> Self {
        let mut layers = Self {
            snapshot,
            slot,
            live: BTreeMap::new(),
            history: BTreeMap::new(),
            history_argv_exact: false,
        };
        if let Some(node) = &target.node
            && target.status == SlotStatus::Live
            && let Some(graph_node) = snapshot.find_node(node)
        {
            layers.fill_live(graph_node);
        }
        if let Some(record_id) = &target.record_id {
            layers.fill_history(record_id);
        }
        layers
    }

    fn value(&self, name: &str) -> Option<(FieldValue, FieldSource)> {
        if let Some(value) = self.slot.overrides.get(name) {
            return Some((value.clone(), FieldSource::Override));
        }
        // The recorded argv is the exact tokens Conspectus launched
        // with; the live one is tmux's re-quoted rendering of them.
        if name == "argv"
            && self.history_argv_exact
            && let Some(value) = self.history.get("argv")
        {
            return Some((value.clone(), FieldSource::History));
        }
        if let Some(value) = self.live.get(name) {
            return Some((value.clone(), FieldSource::Live));
        }
        if let Some(value) = self.history.get(name) {
            return Some((value.clone(), FieldSource::History));
        }
        if let Some(value) = self.slot.snapshot.get(name) {
            return Some((value.clone(), FieldSource::Snapshot));
        }
        self.default(name)
            .map(|value| (value, FieldSource::Default))
    }

    fn default(&self, name: &str) -> Option<FieldValue> {
        match (self.slot.kind, name) {
            (_, "label") => Some(FieldValue::Text(self.slot.id.clone())),
            (PinKind::Mux, "name") => Some(FieldValue::Text(self.slot.id.clone())),
            (PinKind::Mux, "argv") => {
                let harness = self.value("harness")?.0;
                let argv: Vec<String> = launch_argv_for(harness.as_text()?)
                    .into_iter()
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect();
                (!argv.is_empty()).then_some(FieldValue::List(argv))
            }
            _ => None,
        }
    }

    fn fill_live(&mut self, node: &GraphNode) {
        let text = |value: &str| FieldValue::Text(value.to_string());
        let live = &mut self.live;
        match node {
            GraphNode::MuxSession(mux) => {
                live.insert("name", text(mux.session_name()));
                if let Some(socket) = crate::model::effective_socket(mux.socket.as_deref()) {
                    live.insert("socket", text(socket));
                }
                if let Some(cwd) = &mux.cwd {
                    live.insert("cwd", text(cwd));
                }
                if let Some(start) = &mux.active_pane_start_command {
                    let argv = crate::argv::split_command_line(start);
                    if let Some(adapter) = adapter_in_command(argv.iter().map(String::as_str)) {
                        live.insert("harness", text(adapter.harness_key()));
                    }
                    if !argv.is_empty() {
                        live.insert("argv", FieldValue::List(argv));
                    }
                }
            }
            GraphNode::AgentSession(session) => {
                live.insert("harness", text(&session.harness_key));
                live.insert("session_id", text(&session.id.session_key));
                if let Some(title) = &session.title {
                    live.insert("title", text(title));
                }
            }
            GraphNode::Checkout(checkout) => {
                live.insert(
                    "repo",
                    text(&NodeId::Repo(checkout.id.repo.clone()).to_string()),
                );
                live.insert("root", text(&checkout.root));
                if let Some(branch) = &checkout.current_branch {
                    live.insert("branch", text(short_ref(&branch.refname)));
                }
                if let Some(head) = &checkout.head_commit {
                    live.insert("head", text(head));
                }
            }
            GraphNode::Repo(repo) => {
                live.insert("common_dir", text(&repo.common_dir));
                if let Some(url) = repo.remotes.first().and_then(|r| r.split_once('=')) {
                    live.insert("remote", text(url.1));
                }
            }
            GraphNode::ForgePr(pr) => {
                live.insert("repo", text(&format!("{}/{}", pr.owner, pr.repo)));
                live.insert("number", text(&pr.number.to_string()));
                if let Some(title) = &pr.title {
                    live.insert("title", text(title));
                    live.insert("label", text(&format!("#{} {title}", pr.number)));
                }
                if let Some(state) = &pr.state {
                    live.insert("state", text(state));
                }
                if let Some(url) = &pr.url {
                    live.insert("url", text(url));
                }
            }
            GraphNode::Branch(branch) => {
                live.insert(
                    "repo",
                    text(&NodeId::Repo(branch.id.repo.clone()).to_string()),
                );
                live.insert("refname", text(&branch.refname));
            }
            GraphNode::Workspace(workspace) => {
                live.insert("root", text(&workspace.root));
            }
            GraphNode::Fork(fork) => {
                live.insert("provider", text(&fork.provider));
                if let Some(name) = &fork.name {
                    live.insert("name", text(name));
                }
            }
            GraphNode::RuntimeProcess(_) | GraphNode::Pin(_) => {}
        }
    }

    fn fill_history(&mut self, record_id: &str) {
        let text = |value: &str| FieldValue::Text(value.to_string());
        let history = &self.snapshot.history;
        let out = &mut self.history;
        if let Some(MuxRecord { recipe, .. }) = history.mux(record_id) {
            out.insert("name", text(&recipe.name));
            if let Some(socket) = crate::model::effective_socket(recipe.socket.as_deref()) {
                out.insert("socket", text(socket));
            }
            if let Some(cwd) = &recipe.cwd {
                out.insert("cwd", text(cwd));
            }
            if let Some(harness) = &recipe.harness {
                out.insert("harness", text(harness));
            }
            if let Some(argv) = &recipe.argv {
                out.insert("argv", FieldValue::List(argv.clone()));
                self.history_argv_exact = recipe.argv_source == Some(ArgvSource::Conspectus);
            }
        } else if let Some(record) = history.checkout(record_id) {
            let recipe = &record.recipe;
            out.insert("repo", text(&recipe.repo));
            out.insert("root", text(&recipe.root));
            if let Some(branch) = &recipe.branch {
                out.insert("branch", text(branch));
            }
            if let Some(head) = &recipe.head {
                out.insert("head", text(head));
            }
        } else if let Some(record) = history.repo(record_id) {
            let recipe = &record.recipe;
            out.insert("common_dir", text(&recipe.common_dir));
            if let Some(remote) = recipe.remotes.first() {
                out.insert("remote", text(&remote.url));
            }
            if let Some(root) = &recipe.primary_root {
                out.insert("path", text(root));
            }
        } else if let Some(record) = history.pr(record_id) {
            let recipe = &record.recipe;
            out.insert("repo", text(&format!("{}/{}", recipe.owner, recipe.repo)));
            out.insert("number", text(&recipe.number.to_string()));
            if let Some(title) = &recipe.title {
                out.insert("title", text(title));
                out.insert("label", text(&format!("#{} {title}", recipe.number)));
            }
            if let Some(state) = &recipe.state {
                out.insert("state", text(state));
            }
            if let Some(url) = &recipe.url {
                out.insert("url", text(url));
            }
        }
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
