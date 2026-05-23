//! Hook sidecar discovery.
//!
//! Harness hooks can write small JSON records outside project trees with the
//! current harness session and terminal context. This post-merge pass maps
//! those records onto already-discovered `AgentSession` and `MuxSession` nodes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::model::{
    AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint,
    LinkState, Metadata, MuxSessionNode, NodeId, Provenance, RelationKind, SourceMetadata,
};

const ADAPTER_NAME: &str = "hook_sidecar";
const SCHEMA_VERSION: u16 = 1;
const ACTIVE_TTL_SECONDS: i64 = 15 * 60;

pub fn apply_hook_sidecars(snapshot: &mut GraphSnapshot, root: &Path, now_epoch: i64) {
    for record in read_records(root) {
        if record.schema_version != SCHEMA_VERSION {
            continue;
        }
        if record.observed_epoch + ACTIVE_TTL_SECONDS < now_epoch {
            continue;
        }

        let Some(session) = find_session(snapshot, &record) else {
            continue;
        };
        let Some(mux) = find_mux(snapshot, &record) else {
            continue;
        };

        let link = linked_to_mux(&session, &mux, &record);
        demote_stale_launch_links(snapshot, &link);
        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
    }
}

fn demote_stale_launch_links(snapshot: &mut GraphSnapshot, current_link: &GraphLink) {
    let Some(target) = current_link.target_node_id().cloned() else {
        return;
    };

    for link in &mut snapshot.candidate_links {
        if link.relation != RelationKind::LinkedToMux
            || link.source == current_link.source
            || link.target_node_id() != Some(&target)
            || link
                .source_metadata
                .fields
                .get("match_kind")
                .and_then(serde_json::Value::as_str)
                != Some("active_pane_command_session_match")
        {
            continue;
        }

        link.state = LinkState::Overridden {
            by: current_link.id.clone(),
            reason: Some("fresh hook sidecar current-session evidence".to_string()),
        };
    }
}

fn read_records(root: &Path) -> Vec<HookSidecarRecord> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| read_record(&path).ok())
        .collect()
}

fn read_record(path: &Path) -> Result<HookSidecarRecord> {
    let body = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&body)?)
}

#[derive(Debug, Deserialize)]
struct HookSidecarRecord {
    schema_version: u16,
    harness_key: String,
    session_key: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    pid: Option<i64>,
    #[serde(default)]
    ppid: Option<i64>,
    #[serde(default)]
    tmux: Option<HookTmuxRecord>,
    #[serde(default)]
    transcript_path: Option<String>,
    #[serde(default)]
    hook_event_name: Option<String>,
    observed_epoch: i64,
    #[serde(default)]
    harness_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HookTmuxRecord {
    #[serde(default)]
    session_name: Option<String>,
    #[serde(default)]
    native_id: Option<String>,
    #[serde(default)]
    pane_id: Option<String>,
    #[serde(default)]
    socket_path: Option<String>,
}

fn find_session(snapshot: &GraphSnapshot, record: &HookSidecarRecord) -> Option<AgentSessionNode> {
    snapshot.nodes.iter().find_map(|node| {
        let GraphNode::AgentSession(session) = node else {
            return None;
        };
        (session.harness_key == record.harness_key && session.id.session_key == record.session_key)
            .then(|| session.clone())
    })
}

fn find_mux(snapshot: &GraphSnapshot, record: &HookSidecarRecord) -> Option<MuxSessionNode> {
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
    record: &HookSidecarRecord,
) -> GraphLink {
    let source = NodeId::AgentSession(session.id.clone());
    let target = NodeId::MuxSession(mux.id.clone());
    let match_kind = if record.transcript_path.is_some() {
        "hook_session_path_match"
    } else {
        "hook_session_match"
    };
    let mut fields = Metadata::new();
    fields.insert(
        "match_kind".to_string(),
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
    if let Some(path) = std::env::var_os("CONSPECTUS_HOOK_SIDECAR_STATE") {
        return Some(PathBuf::from(path));
    }

    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(path).join("conspectus/hooks"));
    }

    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("conspectus")
            .join("hooks")
    })
}

pub fn current_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::model::{AgentSessionId, MuxSessionId};

    fn session(key: &str) -> GraphNode {
        GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("claude-code", "/state", key),
            harness_key: "claude-code".to_string(),
            cwd: Some("/work".to_string()),
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
        })
    }

    fn mux(native_id: &str) -> GraphNode {
        GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new(format!("tmux:{native_id}")),
            backend: "tmux".to_string(),
            native_id: native_id.to_string(),
            cwd: Some("/work".to_string()),
            active_pane_command: Some("claude".to_string()),
            active_pane_pid: Some(123),
            active_pane_current_path: Some("/work".to_string()),
            active_pane_start_command: Some("claude --resume old".to_string()),
            activity_epoch: Some(1_700_000_000),
            created_epoch: None,
        })
    }

    fn launch_link(session_key: &str, mux_native_id: &str) -> GraphLink {
        let source =
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", session_key));
        let target = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{mux_native_id}")));
        let mut fields = Metadata::new();
        fields.insert(
            "match_kind".to_string(),
            serde_json::Value::String("active_pane_command_session_match".to_string()),
        );
        GraphLink {
            id: format!("launch:{session_key}:{mux_native_id}"),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "cross_link".to_string(),
                evidence: Some("active_pane_command_session_match".to_string()),
                fields,
            },
            state: LinkState::Active,
        }
    }

    #[test]
    fn fresh_hook_record_links_session_to_mux_by_tmux_session_name() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("record.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "cwd": "/work",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "transcript_path": "/home/me/.claude/projects/-work/current.jsonl",
              "hook_event_name": "SessionStart",
              "observed_epoch": 1700000000,
              "harness_version": "1.0.0"
            }"#,
        )
        .expect("write record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("current"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

        let links: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| link.relation == RelationKind::LinkedToMux)
            .collect();
        assert_eq!(links.len(), 1);
        assert_eq!(
            links[0].source_metadata.evidence.as_deref(),
            Some("hook_session_path_match")
        );
        assert_eq!(
            links[0].source,
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "current"))
        );
    }

    #[test]
    fn stale_hook_record_does_not_link_active_mux() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("record.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "tmux": { "session_name": "editor" },
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("current"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_001_000);

        assert!(snapshot.candidate_links.is_empty());
    }

    #[test]
    fn fresh_hook_record_demotes_stale_launch_argv_link_for_same_mux() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("record.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "tmux": { "session_name": "editor" },
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("old"), session("current"), mux("editor")],
            candidate_links: vec![launch_link("old", "editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

        let stale = snapshot
            .candidate_links
            .iter()
            .find(|link| link.id == "launch:old:editor")
            .expect("stale launch link");
        assert!(matches!(stale.state, LinkState::Overridden { .. }));
        assert!(snapshot.candidate_links.iter().any(|link| link.source
            == NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "current"))));
    }
}
