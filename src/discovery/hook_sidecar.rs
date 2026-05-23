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
    LinkEndpoint, LinkState, Metadata, MuxSessionId, MuxSessionNode, NodeId, Provenance,
    RelationKind, SourceMetadata,
};

const ADAPTER_NAME: &str = "hook_sidecar";

pub fn apply_hook_sidecars(snapshot: &mut GraphSnapshot, root: &Path, _now_epoch: i64) {
    let mut records: Vec<HookRecord> = hook::HookStore::new(root)
        .read_records()
        .into_iter()
        .filter(|record| record.schema_version == hook::SCHEMA_VERSION)
        .collect();
    // Freshest first so the dedupe map keeps the winner per pane.
    records.sort_by(|a, b| b.observed_epoch.cmp(&a.observed_epoch));

    let mut winners: HashMap<(MuxSessionId, Option<String>), String> = HashMap::new();

    for record in records {
        let Some(mux) = find_mux(snapshot, &record) else {
            continue;
        };
        let pane_key = (
            mux.id.clone(),
            record.tmux.as_ref().and_then(|t| t.pane_id.clone()),
        );
        let session = ensure_session(snapshot, &record);
        let mut link = linked_to_mux(&session, &mux, &record);

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

        if !snapshot
            .candidate_links
            .iter()
            .any(|existing| existing.id == link.id)
        {
            snapshot.candidate_links.push(link);
        }
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
        link.source_metadata
            .fields
            .get("match_kind")
            .and_then(serde_json::Value::as_str)
            .or(link.source_metadata.evidence.as_deref()),
        Some("active_pane_command_session_match" | "exact_cwd_match" | "cwd_prefix_match")
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

fn ensure_session(snapshot: &mut GraphSnapshot, record: &HookRecord) -> AgentSessionNode {
    if let Some(session) = find_session(snapshot, record) {
        return session;
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
    };
    snapshot
        .nodes
        .push(GraphNode::AgentSession(session.clone()));
    session
}

fn inferred_state_scope(record: &HookRecord) -> String {
    record
        .transcript_path
        .as_deref()
        .and_then(|path| {
            Path::new(path)
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
        })
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "hook_sidecar".to_string())
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
    hook::default_root()
}

pub fn current_epoch() -> i64 {
    hook::current_epoch()
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

    fn cwd_link(session_key: &str, mux_native_id: &str) -> GraphLink {
        let source =
            NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", session_key));
        let target = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{mux_native_id}")));
        let mut fields = Metadata::new();
        fields.insert(
            "match_kind".to_string(),
            serde_json::Value::String("exact_cwd_match".to_string()),
        );
        GraphLink {
            id: format!("cwd:{session_key}:{mux_native_id}"),
            source,
            target: LinkEndpoint::Node { id: target },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "cross_link".to_string(),
                evidence: Some("exact_cwd_match".to_string()),
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
    fn fresh_sqlite_hook_record_links_session_to_mux() {
        let temp = tempdir().expect("tempdir");
        hook::HookStore::new(temp.path())
            .write_record(&hook::HookRecord {
                schema_version: hook::SCHEMA_VERSION,
                harness_key: "claude-code".to_string(),
                session_key: "current".to_string(),
                cwd: Some("/work".to_string()),
                pid: Some(123),
                ppid: Some(456),
                tmux: Some(hook::HookTmuxRecord {
                    session_name: Some("editor".to_string()),
                    native_id: None,
                    pane_id: Some("%1".to_string()),
                    socket_path: None,
                }),
                transcript_path: None,
                hook_event_name: Some("SessionStart".to_string()),
                observed_epoch: 1_700_000_000,
                harness_version: None,
            })
            .expect("write hook record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("current"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

        assert_eq!(snapshot.candidate_links.len(), 1);
        assert_eq!(
            snapshot.candidate_links[0]
                .source_metadata
                .evidence
                .as_deref(),
            Some("hook_session_match")
        );
    }

    #[test]
    fn fresh_hook_record_synthesizes_session_before_transcript_exists() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("record.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "cwd": "/work",
              "tmux": { "session_name": "editor" },
              "transcript_path": "/home/me/.claude/projects/-work/current.jsonl",
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

        let session_id = AgentSessionId::new("claude-code", "/home/me/.claude", "current");
        assert!(snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::AgentSession(session)
                if session.id == session_id
                    && session.cwd.as_deref() == Some("/work")
                    && session.last_active_epoch == Some(1_700_000_000)
        )));
        assert!(snapshot.candidate_links.iter().any(|link| {
            link.source == NodeId::AgentSession(session_id.clone())
                && link.source_metadata.evidence.as_deref() == Some("hook_session_path_match")
        }));
    }

    #[test]
    fn old_hook_record_still_links_when_no_fresher_record_supersedes() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("record.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("current"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        // Far beyond the former 15-minute TTL; with dedupe-by-pane the
        // record still links because no fresher observation supersedes it.
        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_000 + 86_400);

        let active: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, LinkState::Active)
            })
            .collect();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn fresher_hook_record_overrides_older_hook_for_same_pane() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("older.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "old",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write older record");
        fs::write(
            temp.path().join("newer.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000500
            }"#,
        )
        .expect("write newer record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("old"), session("current"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

        let current_id = AgentSessionId::new("claude-code", "/state", "current");
        let old_id = AgentSessionId::new("claude-code", "/state", "old");

        let winner = snapshot
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source == NodeId::AgentSession(current_id.clone())
            })
            .expect("winner link present");
        assert!(matches!(winner.state, LinkState::Active));

        let loser = snapshot
            .candidate_links
            .iter()
            .find(|link| {
                link.relation == RelationKind::LinkedToMux
                    && link.source == NodeId::AgentSession(old_id.clone())
            })
            .expect("loser link present");
        match &loser.state {
            LinkState::Overridden { by, reason } => {
                assert_eq!(by, &winner.id);
                assert_eq!(
                    reason.as_deref(),
                    Some("superseded by fresher hook sidecar record for same pane")
                );
            }
            other => panic!("expected Overridden, got {other:?}"),
        }
    }

    #[test]
    fn hook_records_for_different_panes_in_same_mux_both_remain_active() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("pane1.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "alpha",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000000
            }"#,
        )
        .expect("write pane 1 record");
        fs::write(
            temp.path().join("pane2.json"),
            r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "beta",
              "tmux": { "session_name": "editor", "pane_id": "%2" },
              "observed_epoch": 1700000500
            }"#,
        )
        .expect("write pane 2 record");
        let mut snapshot = GraphSnapshot {
            nodes: vec![session("alpha"), session("beta"), mux("editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

        let active_sources: Vec<_> = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, LinkState::Active)
            })
            .map(|link| link.source.clone())
            .collect();
        assert_eq!(active_sources.len(), 2);
        assert!(
            active_sources.contains(&NodeId::AgentSession(AgentSessionId::new(
                "claude-code",
                "/state",
                "alpha"
            )))
        );
        assert!(
            active_sources.contains(&NodeId::AgentSession(AgentSessionId::new(
                "claude-code",
                "/state",
                "beta"
            )))
        );
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

    #[test]
    fn fresh_hook_record_demotes_cwd_links_for_same_mux() {
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
            candidate_links: vec![cwd_link("old", "editor"), cwd_link("current", "editor")],
            ..GraphSnapshot::empty()
        };

        apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

        for id in ["cwd:old:editor", "cwd:current:editor"] {
            let link = snapshot
                .candidate_links
                .iter()
                .find(|link| link.id == id)
                .expect("cwd link");
            assert!(matches!(link.state, LinkState::Overridden { .. }));
        }
        assert!(snapshot.candidate_links.iter().any(|link| {
            matches!(link.state, LinkState::Active)
                && link.source
                    == NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", "current"))
                && link.source_metadata.evidence.as_deref() == Some("hook_session_match")
        }));
    }
}
