//! Hook sidecar discovery.
//!
//! Harness hooks can write small JSON records outside project trees with the
//! current harness session and terminal context. This post-merge pass maps
//! those records onto already-discovered `AgentSession` and `MuxSession` nodes.

use std::collections::{BTreeMap, HashMap};
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

pub fn apply_hook_records(
    snapshot: &mut GraphSnapshot,
    mut records: Vec<HookRecord>,
    now_epoch: i64,
) {
    // Freshest first so the dedupe map keeps the winner per pane.
    records.sort_by(|a, b| b.observed_epoch.cmp(&a.observed_epoch));

    let mut winners: HashMap<(MuxSessionId, Option<String>), String> = HashMap::new();
    let node_count_before = snapshot.nodes.len();
    let link_count_before = snapshot.candidate_links.len();

    for record in records {
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
                None => {
                    winners.insert(pane_key, link.id.clone());
                    demote_weaker_mux_links(snapshot, &link);
                }
                Some(winner_id) => {
                    link.state = LinkState::Overridden {
                        by: winner_id.clone(),
                        reason: Some(
                            "superseded by fresher hook sidecar record for same pane".to_string(),
                        ),
                    };
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
    if snapshot.nodes.len() != node_count_before
        || snapshot.candidate_links.len() != link_count_before
    {
        crate::discovery::stamp_snapshot_mutations(snapshot, ADAPTER_NAME, now_epoch);
    }
}

/// Maps the mux's active pane command back to the harness key when the
/// command unambiguously identifies one. Conservative: ambiguous commands
/// (e.g. `node`, `shell`) return `None` so hook records aren't filtered on
/// weak evidence.
fn pane_running_harness(mux: &MuxSessionNode) -> Option<&'static str> {
    match mux.active_pane_command.as_deref()? {
        "claude" => Some("claude-code"),
        "codex" | "codex-rs" => Some("codex"),
        "opencode" => Some("opencode"),
        "aider" => Some("aider"),
        _ => None,
    }
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
    if record.harness_key == "codex"
        && let Some(path) = record.transcript_path.as_deref()
        && let Some(scope) = path_ancestor_named(path, ".codex")
    {
        return scope;
    }

    record
        .transcript_path
        .as_deref()
        .and_then(|path| {
            Path::new(path)
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
        })
        .map_or_else(
            || "hook_sidecar".to_string(),
            |path| path.to_string_lossy().to_string(),
        )
}

fn path_ancestor_named(path: &str, name: &str) -> Option<String> {
    Path::new(path).ancestors().find_map(|ancestor| {
        (ancestor.file_name().and_then(|value| value.to_str()) == Some(name))
            .then(|| ancestor.to_string_lossy().to_string())
    })
}

fn find_mux(snapshot: &GraphSnapshot, record: &HookRecord) -> Option<MuxSessionNode> {
    let muxes: Vec<_> = snapshot
        .nodes
        .iter()
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

// Re-export the canonical `current_epoch` so callers in this
// module's namespace keep a short path.
pub use crate::discovery::current_epoch;

#[cfg(test)]
#[path = "hook_sidecar_tests.rs"]
mod tests;
