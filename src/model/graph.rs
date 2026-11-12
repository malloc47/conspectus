//! The `GraphSnapshot` container and the `SnapshotIndex` lookups built over it.

use super::live::{EffectiveLive, LiveBinding, LiveCounts};
use super::*;

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct GraphSnapshot {
    pub nodes: Vec<GraphNode>,
    pub candidate_links: Vec<GraphLink>,
    pub resolved_relationships: Vec<ResolvedRelationship>,
    pub diagnostics: Vec<Diagnostic>,
    /// Operator-chosen display names per ADR 0029. Carried alongside
    /// the graph but kept out of the serialized snapshot so the JSON
    /// shape stays a pure view of discovered state. Empty for callers
    /// that don't load aliases.
    #[serde(skip, default)]
    pub aliases: crate::aliases::AliasOverlay,
    /// The pin slots (ADR 0141), from `pins.toml` and any legacy
    /// `[[pins.entries]]`, in slot order. The resolver turns each into
    /// a `PinNode` with its status and effective recipe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<PinSlot>,
    /// Per-node producing-provider metadata (ADR 0037).
    /// Empty for snapshots whose producers have not been
    /// instrumented yet; the loader falls back to the schema's
    /// `'unknown'` / `0` defaults for nodes without an entry. See
    /// [`NodeProvenance`].
    ///
    /// Serializes as a JSON array of `{node_id, provider,
    /// freshness_epoch}` entries (via `node_provenance_serde`)
    /// because JSON object keys must be strings and `NodeId` is a
    /// structured type. The in-memory shape stays a `BTreeMap` so
    /// loader lookups are O(log n) and the per-node iteration order
    /// is deterministic.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "node_provenance_serde"
    )]
    pub node_provenance: BTreeMap<NodeId, NodeProvenance>,
    /// Presence of nodes that aren't live (ADR 0140): history records
    /// projected as gone or unknown nodes. Live is the default and has
    /// no entry. Serializes as a list, like `node_provenance`.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "node_presence_serde"
    )]
    pub node_presence: BTreeMap<NodeId, NodePresence>,
    /// Live agent state (ADR 0205): the latest observation of what an
    /// agent is doing, with whether its process ran when the table was
    /// bound, keyed by its agent session, or by a mux for an agent seen
    /// in one of the mux's panes that no session matched. Discovery's
    /// `live_state` pass rebuilds it from the live-state store on every
    /// cycle. Sparse: no entry means no evidence, never idle. Read it
    /// through [`Self::live_state_of`] and [`Self::live_counts_of_mux`],
    /// which apply the read-time rules; a stored state is not shown as
    /// it is. Serializes as a list, like `node_presence`, and is left
    /// out when empty.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "node_live_serde"
    )]
    pub node_live: BTreeMap<NodeId, LiveBinding>,
    /// The latest probe of each mux backend and socket, keyed by
    /// [`probe_key`]. Kept across cycles that reuse a mux slice (it
    /// rides in `graph.bin`), so gone and unknown stay decidable
    /// between probes. Left out of JSON: it is discovery bookkeeping
    /// with wall-clock times, and `node_presence` carries its effect.
    #[serde(skip)]
    pub mux_probes: BTreeMap<String, MuxProbe>,
    /// The history records discovery read and wrote this cycle (ADR
    /// 0140), so views, pin slots, exports, and fixtures have recipes
    /// and record ids without the store.
    #[serde(default, skip_serializing_if = "HistorySnapshot::is_empty")]
    pub history: HistorySnapshot,
}

mod node_presence_serde {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{NodeId, NodePresence};

    #[derive(Serialize, Deserialize)]
    struct Entry {
        node_id: NodeId,
        #[serde(flatten)]
        presence: NodePresence,
    }

    pub fn serialize<S>(
        map: &BTreeMap<NodeId, NodePresence>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<Entry> = map
            .iter()
            .map(|(node_id, presence)| Entry {
                node_id: node_id.clone(),
                presence: *presence,
            })
            .collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<NodeId, NodePresence>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: Vec<Entry> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.node_id, entry.presence))
            .collect())
    }
}

/// Round-trip helper for `GraphSnapshot::node_live`: a list of entries,
/// each the node id beside the observation's own fields and the bound
/// liveness.
mod node_live_serde {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{LiveBinding, NodeId};

    #[derive(Serialize, Deserialize)]
    struct Entry {
        node_id: NodeId,
        #[serde(flatten)]
        binding: LiveBinding,
    }

    pub fn serialize<S>(
        map: &BTreeMap<NodeId, LiveBinding>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<Entry> = map
            .iter()
            .map(|(node_id, binding)| Entry {
                node_id: node_id.clone(),
                binding: *binding,
            })
            .collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<NodeId, LiveBinding>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: Vec<Entry> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.node_id, entry.binding))
            .collect())
    }
}

/// Round-trip helper for the `GraphSnapshot::node_provenance`
/// sidecar. The map serializes as an ordered list of
/// `NodeProvenanceEntry` records so the JSON payload remains
/// hand-inspectable and tooling-friendly; the in-memory shape stays
/// `BTreeMap<NodeId, NodeProvenance>` for fast lookups.
mod node_provenance_serde {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{NodeId, NodeProvenance};

    #[derive(Serialize, Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
    struct Entry {
        node_id: NodeId,
        #[serde(flatten)]
        provenance: NodeProvenance,
    }

    pub fn serialize<S>(
        map: &BTreeMap<NodeId, NodeProvenance>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entries: Vec<Entry> = map
            .iter()
            .map(|(node_id, provenance)| Entry {
                node_id: node_id.clone(),
                provenance: provenance.clone(),
            })
            .collect();
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<NodeId, NodeProvenance>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: Vec<Entry> = Vec::deserialize(deserializer)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.node_id, entry.provenance))
            .collect())
    }
}

/// Indexed view over a [`GraphSnapshot`] (ADR 0035 Stage 1).
/// Built once per snapshot publish so consumers don't linear-scan
/// `snapshot.nodes` / `snapshot.candidate_links` per render. Holds
/// the `id → &GraphNode` map, links by source and relation, by
/// relation, and by id, and per-session mux candidate counts.
///
/// Cheap to build: one pass over the nodes and two over the
/// candidate links.
#[derive(Debug)]
pub struct SnapshotIndex<'a> {
    /// Borrow of the source snapshot so links + resolved_relationships
    /// stay accessible through the same handle during migration.
    pub snapshot: &'a GraphSnapshot,
    id_to_node: BTreeMap<NodeId, &'a GraphNode>,
    /// Per-agent count of distinct active
    /// `LinkedToMux` mux targets, read through
    /// `index.mux_candidate_count(agent_node_id)`. Deduped by target
    /// mux id.
    agent_mux_candidate_counts: std::collections::HashMap<String, usize>,
    /// `(source, relation) → Vec<&GraphLink>`
    /// map. Consumers that today linear-scan `candidate_links`
    /// looking for a specific source-relation combination fold
    /// the filter through this map instead. Only holds
    /// `LinkState::Active` links because every existing hot
    /// consumer filters to active first.
    links_by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>>,
    /// `relation → Vec<&GraphLink>` map.
    /// Consumers that scan every link with a specific relation
    /// kind (cross_link's parent-subagent override pass, the
    /// resolver's per-relation aggregation) fold their filter
    /// through this map. Only holds `LinkState::Active` links.
    links_by_relation: BTreeMap<RelationKind, Vec<&'a GraphLink>>,
    /// `link_id → &GraphLink` map. Consumers
    /// that today `.find(|link| link.id == some_id)` collapse
    /// to O(log n) lookup. Includes links in every state so
    /// consumers can inspect superseded / dead links too.
    links_by_id: BTreeMap<String, &'a GraphLink>,
}

impl<'a> SnapshotIndex<'a> {
    /// A node's presence (ADR 0140); `None` means live.
    pub fn presence(&self, id: &NodeId) -> Option<NodePresence> {
        self.snapshot.presence(id)
    }

    /// Whether `id` exists now rather than only in history.
    pub fn is_live(&self, id: &NodeId) -> bool {
        self.snapshot.is_live(id)
    }

    /// Build the index by walking `snapshot.nodes` +
    /// `snapshot.candidate_links` once.
    pub fn new(snapshot: &'a GraphSnapshot) -> Self {
        let mut id_to_node = BTreeMap::new();
        for node in &snapshot.nodes {
            id_to_node.insert(node.id(), node);
        }

        // Precompute per-agent mux candidate
        // counts once so row builders don't re-scan per render.
        let mut per_agent: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
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
            let LinkEndpoint::Node { id: target_id } = &link.target else {
                continue;
            };
            per_agent
                .entry(link.source.to_string())
                .or_default()
                .insert(target_id.to_string());
        }
        let agent_mux_candidate_counts = per_agent.into_iter().map(|(k, v)| (k, v.len())).collect();

        // candidate_links indices. One pass
        // populates both `links_by_source_relation` (wave 3),
        // `links_by_relation` (wave 6), and `links_by_id`
        // (wave 6).
        let mut links_by_source_relation: BTreeMap<(NodeId, RelationKind), Vec<&'a GraphLink>> =
            BTreeMap::new();
        let mut links_by_relation: BTreeMap<RelationKind, Vec<&'a GraphLink>> = BTreeMap::new();
        let mut links_by_id: BTreeMap<String, &'a GraphLink> = BTreeMap::new();
        for link in &snapshot.candidate_links {
            // Wave 6: `links_by_id` covers every state so
            // consumers can inspect superseded links too.
            links_by_id.insert(link.id.clone(), link);
            if !matches!(link.state, LinkState::Active) {
                continue;
            }
            links_by_source_relation
                .entry((link.source.clone(), link.relation.clone()))
                .or_default()
                .push(link);
            links_by_relation
                .entry(link.relation.clone())
                .or_default()
                .push(link);
        }

        Self {
            snapshot,
            id_to_node,
            agent_mux_candidate_counts,
            links_by_source_relation,
            links_by_relation,
            links_by_id,
        }
    }

    /// Look up a node by id. `None` for ids that aren't in
    /// this snapshot.
    pub fn node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.id_to_node.get(id).copied()
    }

    /// Number of indexed nodes. Consistent with `snapshot.nodes.len()`
    /// unless the snapshot has duplicate ids (which is invalid; a
    /// wave-2 consistency test asserts this).
    pub fn node_count(&self) -> usize {
        self.id_to_node.len()
    }

    /// Per-agent `LinkedToMux` candidate count
    /// keyed by `NodeId::AgentSession(...).to_string()`.
    /// Retires `tui::rows::collect_agent_mux_candidate_counts`.
    pub fn agent_mux_candidate_counts(&self) -> &std::collections::HashMap<String, usize> {
        &self.agent_mux_candidate_counts
    }

    /// Active `(source, relation)` links.
    /// Returns an empty slice for unknown `(source, relation)`
    /// combinations. Consumers that today do
    /// `snapshot.candidate_links.iter().filter(|link| link.source
    /// == id && link.relation == RelationKind::Foo)` collapse to
    /// `index.links_for(&id, RelationKind::Foo)`.
    pub fn links_for(&self, source: &NodeId, relation: RelationKind) -> &[&GraphLink] {
        static EMPTY: &[&GraphLink] = &[];
        self.links_by_source_relation
            .get(&(source.clone(), relation))
            .map_or(EMPTY, Vec::as_slice)
    }

    /// Active links matching `relation`.
    /// Consumers that scan every link with a specific relation
    /// (`cross_link.rs` subagent override pass) collapse to
    /// `index.links_with_relation(RelationKind::LinkedToMux)`.
    /// Returns an empty slice for unknown relations.
    pub fn links_with_relation(&self, relation: RelationKind) -> &[&GraphLink] {
        static EMPTY: &[&GraphLink] = &[];
        self.links_by_relation
            .get(&relation)
            .map_or(EMPTY, Vec::as_slice)
    }

    /// Link by `link.id`. Includes every state
    /// so consumers can inspect superseded / dead links too.
    /// Consumers that today do
    /// `snapshot.candidate_links.iter().find(|l| l.id == id)`
    /// collapse to `index.link(id)`.
    pub fn link(&self, link_id: &str) -> Option<&GraphLink> {
        self.links_by_id.get(link_id).copied()
    }
}

impl GraphSnapshot {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Linear-scan lookup of a node by id. For repeated lookups over
    /// one snapshot, build a [`SnapshotIndex`] instead.
    pub fn find_node(&self, id: &NodeId) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.has_id(id))
    }

    /// A node's presence (ADR 0140); `None` means live.
    pub fn presence(&self, id: &NodeId) -> Option<NodePresence> {
        self.node_presence.get(id).copied()
    }

    /// The live state of `id` at `now_ms` (ADR 0205): its `node_live`
    /// binding read through [`LiveBinding::effective`], with the
    /// liveness found when the table was bound, or `None` when the table
    /// has no entry for it, which means no evidence, not idle. `id` is an
    /// agent session, or a mux for an agent seen in one of its panes that
    /// no session matched.
    pub fn live_state_of(&self, id: &NodeId, now_ms: i64) -> Option<EffectiveLive> {
        self.node_live
            .get(id)
            .map(|binding| binding.effective(now_ms))
    }

    /// How many of the agents in `mux` are in each live state at
    /// `now_ms` (ADR 0205). A mux has no state of its own; these counts
    /// stand for it. Each of these counts once, by its effective state
    /// as [`Self::live_state_of`] reads it:
    ///
    /// - every agent session with a `node_live` entry whose preferred
    ///   mux ([`Self::preferred_mux_for`]) is `mux`;
    /// - the entry keyed on `mux` itself, an agent seen in one of its
    ///   panes that no session matched.
    ///
    /// A session with no entry is not counted (no evidence is not idle),
    /// and neither is one whose strongest mux candidate is another mux.
    pub fn live_counts_of_mux(&self, mux: &MuxSessionId, now_ms: i64) -> LiveCounts {
        self.node_live
            .iter()
            .filter(|(id, _)| self.live_counts_host(id).as_ref() == Some(mux))
            .map(|(_, binding)| binding.effective(now_ms).state)
            .collect()
    }

    /// [`Self::live_counts_of_mux`] for every mux at once, over `table`
    /// in place of `node_live`: a table bound to this snapshot later
    /// than the snapshot was, as the TUI binds one between snapshots.
    /// A mux nothing counts toward has no entry.
    pub(crate) fn live_counts_by_mux(
        &self,
        table: &BTreeMap<NodeId, LiveBinding>,
        now_ms: i64,
    ) -> BTreeMap<MuxSessionId, LiveCounts> {
        let mut counts: BTreeMap<MuxSessionId, LiveCounts> = BTreeMap::new();
        for (id, binding) in table {
            if let Some(mux) = self.live_counts_host(id) {
                counts
                    .entry(mux)
                    .or_default()
                    .add(binding.effective(now_ms).state);
            }
        }
        counts
    }

    /// The mux whose counts the live entry keyed on `id` joins: a
    /// session's preferred mux, or the mux itself for an agent seen in
    /// one of its panes.
    fn live_counts_host(&self, id: &NodeId) -> Option<MuxSessionId> {
        match id {
            NodeId::AgentSession(_) => self.preferred_mux_for(id),
            NodeId::MuxSession(mux) => Some(mux.clone()),
            _ => None,
        }
    }

    /// The resolver-preferred live mux for `session`: the strongest
    /// active `linked_to_mux` candidate.
    pub fn preferred_mux_for(&self, session: &NodeId) -> Option<MuxSessionId> {
        let mut candidates: Vec<&GraphLink> = self
            .candidate_links
            .iter()
            .filter(|link| {
                link.source == *session
                    && link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, LinkState::Active)
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .provenance
                .precedence()
                .cmp(&left.provenance.precedence())
                .then_with(|| right.confidence.cmp(&left.confidence))
                .then_with(|| left.id.cmp(&right.id))
        });
        match candidates.into_iter().next()?.target_node_id()? {
            NodeId::MuxSession(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// The mux that hosts `session` now, or failing that the one that
    /// hosted it last (ADR 0140). `None` when no mux is known for it.
    pub fn mux_host_of(&self, session: &NodeId) -> Option<MuxHost> {
        if let Some(mux) = self.preferred_mux_for(session) {
            let id = NodeId::MuxSession(mux);
            if let Some(GraphNode::MuxSession(node)) = self.find_node(&id) {
                return Some(MuxHost {
                    name: node.session_name().to_string(),
                    live: self.is_live(&id),
                });
            }
        }
        let NodeId::AgentSession(agent) = session else {
            return None;
        };
        // Among the muxes whose history lists the session, the one that
        // saw it most recently.
        self.history
            .muxes
            .iter()
            .filter_map(|record| {
                let hosted = record.recipe.hosted.iter().find(|h| {
                    h.harness == agent.harness_key && h.session_id == agent.session_key
                })?;
                Some((hosted.last_seen, record))
            })
            .max_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| rb.meta.id.cmp(&ra.meta.id)))
            .map(|(_, record)| MuxHost {
                name: record.recipe.name.clone(),
                live: self.history.presence_of(&record.meta.id).is_none(),
            })
    }

    /// Whether a new mux named `name` would collide with one Conspectus
    /// knows: a mux node, live or gone, with that session name, or a
    /// mux in history recorded under it. The socket is ignored, so a
    /// name taken on any server counts.
    pub(crate) fn mux_name_taken(&self, name: &str) -> bool {
        self.nodes
            .iter()
            .any(|node| matches!(node, GraphNode::MuxSession(mux) if mux.session_name() == name))
            || self
                .history
                .muxes
                .iter()
                .any(|record| record.recipe.name == name)
    }

    /// Whether `id` exists now rather than only in history. Code that
    /// acts on a node, counts it, or links to it checks this first.
    pub fn is_live(&self, id: &NodeId) -> bool {
        !self.node_presence.contains_key(id)
    }

    /// The nodes that exist now, without history's gone and unknown
    /// ones.
    pub fn live_nodes(&self) -> impl Iterator<Item = &GraphNode> {
        self.nodes
            .iter()
            .filter(|node| self.node_presence.is_empty() || self.is_live(&node.id()))
    }

    /// Sort every list into a stable order, so equal graphs compare and
    /// serialize alike. The node-keyed tables (`node_provenance`,
    /// `node_presence`, `node_live`) and `mux_probes` are maps kept in
    /// key order already.
    pub fn canonicalize(&mut self) {
        self.nodes.sort_by_key(GraphNode::id);
        self.candidate_links.sort_by_key(stable_json_key);
        self.resolved_relationships.sort();
        self.diagnostics.sort();
        self.pins
            .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
    }

    /// Remove every node, candidate link, and provenance entry
    /// belonging to `provider`. Used by the warm-start path
    /// to evict a stale provider's slice before
    /// re-running it, and by the eventual `conspectus serve` tick
    /// to swap a single provider's contribution without rebuilding
    /// the rest of the graph.
    ///
    /// Semantics:
    ///
    /// * Nodes are dropped iff their `node_provenance` entry's
    ///   provider equals `provider`. Nodes without a provenance
    ///   entry are kept (they pre-date instrumentation; eviction
    ///   stays conservative).
    /// * Candidate links are dropped iff their
    ///   `source_metadata.adapter` equals `provider`. Declared
    ///   links, cross-link inferences, and other providers' links
    ///   survive untouched.
    /// * The matching `node_provenance` entries are removed, and so
    ///   are the `node_presence` and `node_live` entries of the
    ///   dropped nodes: a live state stays only while its node does.
    /// * `resolved_relationships` are cleared because the resolver
    ///   runs against `candidate_links` and must re-derive after
    ///   the candidate set changes.
    /// * `aliases`, `pins`, and `diagnostics` are independent of
    ///   provider identity and survive intact.
    pub fn evict_provider(&mut self, provider: &str) {
        let mut evicted_ids: BTreeSet<NodeId> = BTreeSet::new();
        for (id, prov) in &self.node_provenance {
            if prov.provider == provider {
                evicted_ids.insert(id.clone());
            }
        }
        self.nodes.retain(|node| !evicted_ids.contains(&node.id()));
        self.candidate_links
            .retain(|link| link.source_metadata.adapter != provider);
        self.node_provenance
            .retain(|_, prov| prov.provider != provider);
        self.node_presence.retain(|id, _| !evicted_ids.contains(id));
        self.node_live.retain(|id, _| !evicted_ids.contains(id));
        // A mux provider's probes go with its nodes: keeping a probe
        // whose sessions were evicted would read every one as gone.
        self.mux_probes.retain(|key, _| {
            key.split_once(':')
                .map_or(key.as_str(), |(backend, _)| backend)
                != provider
        });
        self.resolved_relationships.clear();
    }
}

fn stable_json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("graph model values serialize")
}

/// The mux that hosts or last hosted a session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxHost {
    /// The bare mux session name.
    pub name: String,
    /// Whether that mux exists now. A session whose mux is gone shows
    /// it as a `was in` chip.
    pub live: bool,
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
