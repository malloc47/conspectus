//! Hook sidecar discovery.
//!
//! Harness hooks can write small JSON records outside project trees with the
//! current harness session and terminal context. This post-merge pass maps
//! those records onto already-discovered `AgentSession` and `MuxSession` nodes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::hook::{self, HookRecord, HookTmuxRecord};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot,
    LinkEndpoint, LinkState, MatchKind, Metadata, MuxSessionId, MuxSessionNode, NodeId, Provenance,
    RelationKind, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole, SourceMetadata,
};

const ADAPTER_NAME: &str = crate::discovery::providers::HOOK_SIDECAR;

pub fn apply_hook_sidecars(snapshot: &mut GraphSnapshot, root: &Path, now_epoch: i64) {
    let records: Vec<HookRecord> = hook::HookStore::new(root)
        .read_records()
        .into_iter()
        .filter(|record| record.schema_version == hook::SCHEMA_VERSION)
        .collect();
    apply_hook_records(snapshot, records, now_epoch);
}

/// A pane hook records are deduplicated by: the mux they matched and the
/// pane they name, if any.
type PaneKey = (MuxSessionId, Option<String>);

/// The Active hook link that speaks for a pane.
struct PaneWinner {
    link_id: String,
    observed_epoch: i64,
    /// Already in the graph before this pass, as when a daemon ingests
    /// one record into its live graph. A fresher record takes the pane
    /// from it; a record from this pass never yields to a later one.
    in_graph: bool,
}

/// Turn hook records into session ↔ mux links, one Active link per pane:
/// the freshest record wins and older ones for the same pane are
/// Overridden.
///
/// A discovery cycle runs this on a graph with no hook links, the pass
/// having been evicted first. A daemon's ingest runs it on its live
/// graph, whose Active hook links compete too: an ingested record that
/// wins its pane overrides the older link and drops its process
/// observation, as the next cycle would, so an ingest never leaves two
/// Active hook links for one pane.
pub fn apply_hook_records(
    snapshot: &mut GraphSnapshot,
    mut records: Vec<HookRecord>,
    now_epoch: i64,
) {
    // Freshest first so the dedupe map keeps the winner per pane.
    records.sort_by_key(|record| std::cmp::Reverse(record.observed_epoch));

    let mut winners = graph_pane_winners(snapshot);
    let mut superseded = false;
    let node_count_before = snapshot.nodes.len();
    let link_count_before = snapshot.candidate_links.len();

    for record in records {
        // A background harness serves every pane, so neither its tmux
        // context nor the pid and cwd fallbacks in `find_mux` say which
        // pane shows this session.
        if record.background_harness {
            continue;
        }
        let Some(mux) = find_mux(snapshot, &record) else {
            continue;
        };
        let pane_key = (
            mux.id.clone(),
            record.tmux.as_ref().and_then(|t| t.pane_id.clone()),
        );
        let Some(session) = ensure_session(snapshot, &record) else {
            continue;
        };
        let mut link = linked_to_mux(&session, &mux, &record);

        let role = hook_runtime_process_role(&record);
        if let Some(pid) = record.pid
            && !process_is_live(pid)
        {
            link.state = LinkState::Ignored {
                reason: Some(format!("hook record pid {pid} is no longer active")),
            };
        } else if role == RuntimeProcessRole::Background {
            link.state = LinkState::Ignored {
                reason: Some(
                    "hook record identifies a Claude background implementation process".to_string(),
                ),
            };
        } else if let Some(created_epoch) = mux.created_epoch
            && record.observed_epoch < created_epoch
        {
            link.state = LinkState::Ignored {
                reason: Some(format!(
                    "mux was created at {created_epoch}, after hook record observed at {}",
                    record.observed_epoch
                )),
            };
        } else if let Some(running) = pane_running_harness(&mux)
            && running != record.harness_key
        {
            link.state = LinkState::Ignored {
                reason: Some(format!(
                    "pane now running `{running}`; hook record is from `{}`",
                    record.harness_key
                )),
            };
        } else {
            match winners.get(&pane_key) {
                // The same observation the graph already links.
                Some(winner) if winner.link_id == link.id => {}
                Some(winner)
                    if !(winner.in_graph && record.observed_epoch >= winner.observed_epoch) =>
                {
                    link.state = LinkState::Overridden {
                        by: winner.link_id.clone(),
                        reason: Some(SAME_PANE_REASON.to_string()),
                    };
                }
                previous => {
                    if let Some(previous) = previous {
                        supersede_graph_link(snapshot, &previous.link_id, &link.id);
                        superseded = true;
                    }
                    winners.insert(
                        pane_key,
                        PaneWinner {
                            link_id: link.id.clone(),
                            observed_epoch: record.observed_epoch,
                            in_graph: false,
                        },
                    );
                    demote_weaker_mux_links(snapshot, &link);
                }
            }
        }

        let link_is_active = matches!(link.state, LinkState::Active);
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }

        if link_is_active {
            emit_runtime_process_observation(snapshot, &session, &mux, &record, role);
        }
    }

    // Stamp any nodes/links the hook-sidecar pass added with the
    // `hook_sidecar` provider; first-write-wins so prior entries (the
    // harness adapters' sessions, tmux's mux nodes, etc.) survive.
    if superseded
        || snapshot.nodes.len() != node_count_before
        || snapshot.candidate_links.len() != link_count_before
    {
        crate::discovery::stamp_snapshot_mutations(snapshot, ADAPTER_NAME, now_epoch);
    }
}

const SAME_PANE_REASON: &str = "superseded by fresher hook sidecar record for same pane";

/// The graph's Active hook links, one per pane: the freshest. Any other
/// Active hook link for the same pane, which only a graph published
/// before the ingest rule could hold, is overridden by it.
fn graph_pane_winners(snapshot: &mut GraphSnapshot) -> HashMap<PaneKey, PaneWinner> {
    let mut active: Vec<(PaneKey, String, i64)> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source_metadata.adapter == ADAPTER_NAME
                && matches!(link.state, LinkState::Active)
        })
        .filter_map(|link| {
            let NodeId::MuxSession(mux) = link.target_node_id()? else {
                return None;
            };
            let fields = &link.source_metadata.fields;
            let observed_epoch = fields
                .get("observed_epoch")
                .and_then(serde_json::Value::as_i64)?;
            let pane = fields
                .get("tmux")
                .and_then(|tmux| tmux.get("pane_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            Some(((mux.clone(), pane), link.id.clone(), observed_epoch))
        })
        .collect();
    active.sort_by_key(|(_, _, observed_epoch)| std::cmp::Reverse(*observed_epoch));

    let mut winners: HashMap<PaneKey, PaneWinner> = HashMap::new();
    for (pane_key, link_id, observed_epoch) in active {
        match winners.get(&pane_key) {
            Some(winner) => {
                let winner_id = winner.link_id.clone();
                supersede_graph_link(snapshot, &link_id, &winner_id);
            }
            None => {
                winners.insert(
                    pane_key,
                    PaneWinner {
                        link_id,
                        observed_epoch,
                        in_graph: true,
                    },
                );
            }
        }
    }
    winners
}

/// Override the graph's hook link `link_id` by `winner_id`, a fresher
/// record's link for the same pane, and drop the process observation it
/// brought, which a cycle emits only for a pane's winner.
fn supersede_graph_link(snapshot: &mut GraphSnapshot, link_id: &str, winner_id: &str) {
    let Some(link) = snapshot
        .candidate_links
        .iter_mut()
        .find(|link| link.id == link_id)
    else {
        return;
    };
    link.state = LinkState::Overridden {
        by: winner_id.to_string(),
        reason: Some(SAME_PANE_REASON.to_string()),
    };
    let session = link.source.clone();
    let (Some(mux), Some(observed_epoch)) = (
        link.target_node_id().cloned(),
        link.source_metadata
            .fields
            .get("observed_epoch")
            .and_then(serde_json::Value::as_i64),
    ) else {
        return;
    };
    drop_process_observation(snapshot, &mux, &session, observed_epoch);
}

/// Remove the `RuntimeProcess` node a hook record observed in `mux` for
/// `session` at `observed_epoch`, with its two links.
fn drop_process_observation(
    snapshot: &mut GraphSnapshot,
    mux: &NodeId,
    session: &NodeId,
    observed_epoch: i64,
) {
    let key_prefix = format!("hook_sidecar:{mux}:pid:");
    let processes: BTreeSet<NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::ProcessIdentifiesSession
                && link.source_metadata.adapter == ADAPTER_NAME
                && link.target_node_id() == Some(session)
        })
        .map(|link| link.source.clone())
        .filter(|process| {
            matches!(
                snapshot.find_node(process),
                Some(GraphNode::RuntimeProcess(node))
                    if node.observation_key.starts_with(&key_prefix)
                        && node.observed_epoch == Some(observed_epoch)
            )
        })
        .collect();
    if processes.is_empty() {
        return;
    }
    snapshot
        .nodes
        .retain(|node| !processes.contains(&node.id()));
    snapshot
        .node_provenance
        .retain(|id, _| !processes.contains(id));
    snapshot.candidate_links.retain(|link| {
        link.source_metadata.adapter != ADAPTER_NAME
            || !(processes.contains(&link.source)
                || link
                    .target_node_id()
                    .is_some_and(|target| processes.contains(target)))
    });
}

/// Maps the mux's active pane command back to the harness key when the
/// command is exactly one of a registered harness's executable names.
/// Conservative: anything else (e.g. `node`, `shell`) returns `None` so
/// hook records aren't filtered on weak evidence.
fn pane_running_harness(mux: &MuxSessionNode) -> Option<&'static str> {
    use crate::discovery::harness::{HarnessAdapter, adapters_for_program};
    adapters_for_program(mux.active_pane_command.as_deref()?)
        .next()
        .map(HarnessAdapter::harness_key)
}

fn demote_weaker_mux_links(snapshot: &mut GraphSnapshot, current_link: &GraphLink) {
    let Some(target) = current_link.target_node_id().cloned() else {
        return;
    };

    for link in &mut snapshot.candidate_links {
        if link.relation != RelationKind::LinkedToMux
            || link.target_node_id() != Some(&target)
            || !is_weaker_mux_evidence(link)
        {
            continue;
        }

        link.state = LinkState::Overridden {
            by: current_link.id.clone(),
            reason: Some("fresh hook sidecar current-session evidence".to_string()),
        };
    }
}

fn is_weaker_mux_evidence(link: &GraphLink) -> bool {
    matches!(
        link.source_metadata.match_kind(),
        Some(
            MatchKind::ActivePaneCommandSessionMatch
                | MatchKind::ExactCwdMatch
                | MatchKind::CwdPrefixMatch
        )
    )
}

fn find_session(snapshot: &GraphSnapshot, record: &HookRecord) -> Option<AgentSessionNode> {
    snapshot.nodes.iter().find_map(|node| {
        let GraphNode::AgentSession(session) = node else {
            return None;
        };
        (session.harness_key == record.harness_key && session.id.session_key == record.session_key)
            .then(|| session.clone())
    })
}

fn ensure_session(snapshot: &mut GraphSnapshot, record: &HookRecord) -> Option<AgentSessionNode> {
    if let Some(session) = find_session(snapshot, record) {
        return Some(session);
    }

    // Strict gating: if the hook claims a transcript path, require the file
    // to exist before synthesizing a placeholder node. Otherwise an old hook
    // record for a deleted or never-flushed session would leave an empty row
    // in the TUI indistinguishable from a real session with no preview yet.
    if let Some(path) = record.transcript_path.as_deref()
        && !Path::new(path).is_file()
    {
        return None;
    }

    let session = AgentSessionNode {
        id: AgentSessionId::new(
            &record.harness_key,
            inferred_state_scope(record),
            &record.session_key,
        ),
        harness_key: record.harness_key.clone(),
        cwd: record.cwd.clone(),
        title: None,
        last_message_preview: None,
        last_active_epoch: Some(record.observed_epoch),
        session_kind: None,
    };
    snapshot
        .nodes
        .push(GraphNode::AgentSession(session.clone()));
    Some(session)
}

fn inferred_state_scope(record: &HookRecord) -> String {
    let Some(path) = record.transcript_path.as_deref().map(Path::new) else {
        return "hook_sidecar".to_string();
    };
    crate::discovery::harness::registered_adapters()
        .find(|adapter| adapter.harness_key() == record.harness_key)
        .and_then(|adapter| adapter.state_scope_for_transcript(path))
        .or_else(|| {
            path.parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .map(|root| root.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "hook_sidecar".to_string())
}

fn find_mux(snapshot: &GraphSnapshot, record: &HookRecord) -> Option<MuxSessionNode> {
    // A gone mux can't host what a hook just reported (ADR 0140).
    let muxes: Vec<_> = snapshot
        .live_nodes()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => Some(mux),
            _ => None,
        })
        .collect();

    let native_id = record
        .tmux
        .as_ref()
        .and_then(|tmux| tmux.native_id.as_deref().or(tmux.session_name.as_deref()));
    if let Some(native_id) = native_id
        && let Some(mux) = muxes.iter().find(|mux| mux.native_id == native_id)
    {
        return Some((*mux).clone());
    }

    if let Some(pid) = record.pid.or(record.ppid)
        && let Some(mux) = muxes.iter().find(|mux| {
            mux.active_pane_pid
                .is_some_and(|active_pid| active_pid == pid)
        })
    {
        return Some((*mux).clone());
    }

    if let Some(cwd) = record.cwd.as_deref()
        && let Some(mux) = muxes.iter().find(|mux| {
            mux.cwd.as_deref() == Some(cwd) || mux.active_pane_current_path.as_deref() == Some(cwd)
        })
    {
        return Some((*mux).clone());
    }

    None
}

fn linked_to_mux(
    session: &AgentSessionNode,
    mux: &MuxSessionNode,
    record: &HookRecord,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let target = NodeId::MuxSession(mux.id.clone());
    let match_kind = if record.transcript_path.is_some() {
        MatchKind::HookSessionPathMatch
    } else {
        MatchKind::HookSessionMatch
    };
    let mut fields = Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(match_kind.to_string()),
    );
    fields.insert(
        "observed_epoch".to_string(),
        serde_json::Value::Number(record.observed_epoch.into()),
    );
    if let Some(path) = &record.transcript_path {
        fields.insert(
            "transcript_path".to_string(),
            serde_json::Value::String(path.clone()),
        );
    }
    if let Some(event) = &record.hook_event_name {
        fields.insert(
            "hook_event_name".to_string(),
            serde_json::Value::String(event.clone()),
        );
    }
    if let Some(version) = &record.harness_version {
        fields.insert(
            "harness_version".to_string(),
            serde_json::Value::String(version.clone()),
        );
    }
    if let Some(tmux) = &record.tmux {
        let tmux_fields = tmux_metadata(tmux);
        if !tmux_fields.is_empty() {
            fields.insert(
                "tmux".to_string(),
                serde_json::Value::Object(tmux_fields.into_iter().collect()),
            );
        }
    }

    GraphLink {
        id: format!(
            "hook_sidecar:{source}:linked_to_mux:{target}:{}",
            record.observed_epoch
        ),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(match_kind.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn emit_runtime_process_observation(
    snapshot: &mut GraphSnapshot,
    session: &AgentSessionNode,
    mux: &MuxSessionNode,
    record: &HookRecord,
    role: RuntimeProcessRole,
) {
    let Some(pid) = record.pid else {
        return;
    };
    let observation_key = format!(
        "hook_sidecar:{}:pid:{}:{}",
        NodeId::MuxSession(mux.id.clone()),
        pid,
        record.observed_epoch
    );
    let process_id = RuntimeProcessId::new(&observation_key);
    let process_node_id = NodeId::RuntimeProcess(process_id.clone());
    if snapshot.find_node(&process_node_id).is_none() {
        snapshot
            .nodes
            .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: process_id,
                observation_key,
                pid: Some(pid),
                parent_pid: record.ppid,
                root_pane_pid: mux.active_pane_pid,
                command: None,
                cwd: record.cwd.clone(),
                harness_key: Some(record.harness_key.clone()),
                role: Some(role),
                depth: None,
                observed_epoch: Some(record.observed_epoch),
            }));
    }

    let mux_id = NodeId::MuxSession(mux.id.clone());
    let session_id = NodeId::AgentSession(session.id.clone());
    let containment = hook_process_link(
        format!("hook_sidecar:{mux_id}:mux_contains_process:{process_node_id}"),
        mux_id,
        LinkEndpoint::Node {
            id: process_node_id.clone(),
        },
        RelationKind::MuxContainsProcess,
        record,
    );
    let identifies = hook_process_link(
        format!("hook_sidecar:{process_node_id}:process_identifies_session:{session_id}"),
        process_node_id,
        LinkEndpoint::Node { id: session_id },
        RelationKind::ProcessIdentifiesSession,
        record,
    );
    for link in [containment, identifies] {
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
    }
}

fn process_is_live(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    let proc_root = Path::new("/proc");
    if !proc_root.is_dir() {
        return true;
    }
    proc_root.join(pid.to_string()).exists()
}

fn hook_runtime_process_role(record: &HookRecord) -> RuntimeProcessRole {
    if record.harness_key == "claude-code"
        && record
            .pid
            .and_then(read_claude_session_kind_for_pid)
            .as_deref()
            == Some("bg")
    {
        return RuntimeProcessRole::Background;
    }
    RuntimeProcessRole::HumanAgent
}

fn read_claude_session_kind_for_pid(pid: i64) -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home)
        .join(".claude")
        .join("sessions")
        .join(format!("{pid}.json"));
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn hook_process_link(
    id: String,
    source: NodeId,
    target: LinkEndpoint,
    relation: RelationKind,
    record: &HookRecord,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert(
        crate::model::source_field::MATCH_KIND.to_string(),
        serde_json::Value::String(MatchKind::HookProcessObservation.to_string()),
    );
    fields.insert(
        "observed_epoch".to_string(),
        serde_json::Value::Number(record.observed_epoch.into()),
    );
    if let Some(pid) = record.pid {
        fields.insert(
            "process_pid".to_string(),
            serde_json::Value::Number(pid.into()),
        );
    }
    if let Some(ppid) = record.ppid {
        fields.insert(
            "process_parent_pid".to_string(),
            serde_json::Value::Number(ppid.into()),
        );
    }

    GraphLink {
        id,
        source,
        target,
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: ADAPTER_NAME.to_string(),
            evidence: Some(MatchKind::HookProcessObservation.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn tmux_metadata(tmux: &HookTmuxRecord) -> BTreeMap<String, serde_json::Value> {
    let mut fields = BTreeMap::new();
    if let Some(value) = &tmux.session_name {
        fields.insert(
            "session_name".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &tmux.native_id {
        fields.insert(
            "native_id".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &tmux.pane_id {
        fields.insert(
            "pane_id".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    if let Some(value) = &tmux.socket_path {
        fields.insert(
            "socket_path".to_string(),
            serde_json::Value::String(value.clone()),
        );
    }
    fields
}

pub fn default_sidecar_root() -> Option<PathBuf> {
    hook::default_root()
}

#[cfg(test)]
#[path = "hook_sidecar_tests.rs"]
mod tests;
