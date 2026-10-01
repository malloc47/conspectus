use std::fs;

use tempfile::tempdir;

use super::*;
use crate::model::{AgentSessionId, MuxSessionId};

fn session(key: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", key),
            "claude-code".to_string(),
        )
        .with_cwd("/work".to_string()),
    )
}

fn mux(native_id: &str) -> GraphNode {
    mux_with_command(native_id, Some("claude"))
}

fn mux_with_command(native_id: &str, command: Option<&str>) -> GraphNode {
    mux_with_command_created(native_id, command, None)
}

fn mux_with_command_created(
    native_id: &str,
    command: Option<&str>,
    created_epoch: Option<i64>,
) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native_id}")),
        backend: "tmux".to_string(),
        native_id: native_id.to_string(),
        cwd: Some("/work".to_string()),
        active_pane_command: command.map(str::to_string),
        active_pane_pid: Some(123),
        active_pane_current_path: Some("/work".to_string()),
        active_pane_start_command: Some("claude --resume old".to_string()),
        client_attached: None,
        activity_epoch: Some(1_700_000_000),
        created_epoch,
        last_attached_epoch: None,
    })
}

fn launch_link(session_key: &str, mux_native_id: &str) -> GraphLink {
    let source = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", session_key));
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
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn cwd_link(session_key: &str, mux_native_id: &str) -> GraphLink {
    let source = NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", session_key));
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
            freshness_epoch: None,
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
fn fresh_spooled_hook_record_links_session_to_mux() {
    let temp = tempdir().expect("tempdir");
    let pid = i64::from(std::process::id());
    hook::HookStore::new(temp.path())
        .write_record(&hook::HookRecord {
            schema_version: hook::SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: "current".to_string(),
            cwd: Some("/work".to_string()),
            pid: Some(pid),
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

    let mux_links: Vec<_> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::LinkedToMux)
        .collect();
    assert_eq!(mux_links.len(), 1);
    assert_eq!(
        mux_links[0].source_metadata.evidence.as_deref(),
        Some("hook_session_match")
    );
    assert!(snapshot.nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::RuntimeProcess(process)
                if process.pid == Some(pid)
                    && process.parent_pid == Some(456)
                    && process.harness_key.as_deref() == Some("claude-code")
                    && process.role == Some(RuntimeProcessRole::HumanAgent)
        )
    }));
    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|link| link.relation == RelationKind::ProcessIdentifiesSession)
    );
}

#[test]
fn hook_record_with_stale_pid_is_ignored_and_does_not_emit_process_links() {
    let temp = tempdir().expect("tempdir");
    hook::HookStore::new(temp.path())
        .write_record(&hook::HookRecord {
            schema_version: hook::SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: "current".to_string(),
            cwd: Some("/work".to_string()),
            pid: Some(i64::MAX),
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

    let mux_link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("ignored mux link");
    assert!(matches!(mux_link.state, LinkState::Ignored { .. }));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::RuntimeProcess(_))),
        "stale hook pid must not create runtime process nodes"
    );
    assert!(
        !snapshot.candidate_links.iter().any(|link| {
            matches!(
                link.relation,
                RelationKind::MuxContainsProcess | RelationKind::ProcessIdentifiesSession
            )
        }),
        "stale hook pid must not create process relationship evidence"
    );
}

#[test]
fn hook_record_with_unknown_pid_stays_active() {
    // When the hook writer can't resolve the
    // harness's pid (non-Linux, or no ancestor matches the
    // harness binary), `record.pid` is persisted as `None` so
    // the discovery liveness check is skipped rather than
    // failing against a stillborn writer pid. The record must
    // remain Active and contribute its `LinkedToMux` candidate.
    let temp = tempdir().expect("tempdir");
    hook::HookStore::new(temp.path())
        .write_record(&hook::HookRecord {
            schema_version: hook::SCHEMA_VERSION,
            harness_key: "claude-code".to_string(),
            session_key: "current".to_string(),
            cwd: Some("/work".to_string()),
            pid: None,
            ppid: None,
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

    let mux_link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("mux link emitted");
    assert!(
        matches!(mux_link.state, LinkState::Active),
        "hook record without a pid must stay active, got {:?}",
        mux_link.state,
    );
}

#[test]
fn hook_record_with_missing_transcript_does_not_synthesize_phantom_session() {
    let temp = tempdir().expect("tempdir");
    // Hook claims a transcript_path that does not exist on disk —
    // the session was deleted, or never flushed. Strict gating in
    // ensure_session should refuse to synthesize a placeholder node.
    fs::write(
        temp.path().join("record.json"),
        r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "ghost",
              "cwd": "/work",
              "tmux": { "session_name": "editor" },
              "transcript_path": "/nonexistent/.claude/projects/-work/ghost.jsonl",
              "observed_epoch": 1700000000
            }"#,
    )
    .expect("write record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux("editor")],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

    assert!(
        !snapshot.nodes.iter().any(|node| matches!(
            node,
            GraphNode::AgentSession(session) if session.id.session_key == "ghost"
        )),
        "no phantom session should be synthesized when transcript is missing"
    );
    assert!(
        snapshot.candidate_links.is_empty(),
        "no link should be emitted for a suppressed phantom"
    );
}

#[test]
fn hook_record_synthesizes_session_when_transcript_exists_but_adapter_missed_it() {
    let temp = tempdir().expect("tempdir");
    // Real transcript on disk that the harness adapter happened to
    // miss this pass (e.g. a different state-root scan). The hook
    // synthesizer should still fill in the gap.
    let claude_root = temp.path().join("home/.claude");
    let project_dir = claude_root.join("projects/-work");
    fs::create_dir_all(&project_dir).expect("create transcript dir");
    let transcript_path = project_dir.join("current.jsonl");
    fs::write(&transcript_path, "").expect("touch transcript");

    let record_payload = format!(
        r#"{{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "current",
              "cwd": "/work",
              "tmux": {{ "session_name": "editor" }},
              "transcript_path": "{}",
              "observed_epoch": 1700000000
            }}"#,
        transcript_path.display()
    );
    fs::write(temp.path().join("record.json"), record_payload).expect("write record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux("editor")],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

    let expected_state_scope = claude_root.to_string_lossy().into_owned();
    let session_id = AgentSessionId::new("claude-code", expected_state_scope, "current");
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
fn codex_hook_record_synthesizes_session_with_codex_state_scope() {
    let temp = tempdir().expect("tempdir");
    let codex_root = temp.path().join("home/.codex");
    let session_dir = codex_root.join("sessions/2026/05/23");
    fs::create_dir_all(&session_dir).expect("create transcript dir");
    let transcript_path =
        session_dir.join("rollout-2026-05-23T00-36-47-019e531f-19ee-7823-816f-4526ef89d70b.jsonl");
    fs::write(&transcript_path, "").expect("touch transcript");

    let record_payload = format!(
        r#"{{
              "schema_version": 1,
              "harness_key": "codex",
              "session_key": "019e531f-19ee-7823-816f-4526ef89d70b",
              "cwd": "/work",
              "tmux": {{ "session_name": "editor" }},
              "transcript_path": "{}",
              "observed_epoch": 1700000000
            }}"#,
        transcript_path.display()
    );
    fs::write(temp.path().join("record.json"), record_payload).expect("write record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux_with_command("editor", Some("codex"))],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

    let expected_state_scope = codex_root.to_string_lossy().into_owned();
    let session_id = AgentSessionId::new(
        "codex",
        expected_state_scope,
        "019e531f-19ee-7823-816f-4526ef89d70b",
    );
    assert!(snapshot.nodes.iter().any(|node| matches!(
        node,
        GraphNode::AgentSession(session)
            if session.id == session_id
                && session.cwd.as_deref() == Some("/work")
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

    assert_eq!(
        snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, LinkState::Active)
            })
            .count(),
        1
    );
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
fn hook_record_is_ignored_when_pane_runs_a_different_harness() {
    let temp = tempdir().expect("tempdir");
    // Stale claude hook left over from when claude was running in this pane.
    fs::write(
        temp.path().join("stale_claude.json"),
        r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "stale",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000000
            }"#,
    )
    .expect("write stale claude record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("stale"),
            // Pane is now running codex.
            mux_with_command("editor", Some("codex")),
        ],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("link present for diagnostics");
    match &link.state {
        LinkState::Ignored { reason } => {
            let reason = reason.as_deref().unwrap_or("");
            assert!(
                reason.contains("codex") && reason.contains("claude-code"),
                "expected reason to name both harnesses, got: {reason}"
            );
        }
        other => panic!("expected Ignored, got {other:?}"),
    }
}

#[test]
fn hook_record_is_ignored_when_mux_was_created_after_observation() {
    let temp = tempdir().expect("tempdir");
    fs::write(
        temp.path().join("stale_reused_name.json"),
        r#"{
              "schema_version": 1,
              "harness_key": "claude-code",
              "session_key": "stale",
              "tmux": { "session_name": "editor", "pane_id": "%1" },
              "observed_epoch": 1700000000
            }"#,
    )
    .expect("write stale record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("stale"),
            mux_with_command_created("editor", Some("claude"), Some(1_700_000_100)),
        ],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("link present for diagnostics");
    match &link.state {
        LinkState::Ignored { reason } => {
            let reason = reason.as_deref().unwrap_or("");
            assert!(
                reason.contains("created at 1700000100")
                    && reason.contains("observed at 1700000000"),
                "expected reason to describe stale mux creation, got: {reason}"
            );
        }
        other => panic!("expected Ignored, got {other:?}"),
    }
}

#[test]
fn hook_record_links_normally_when_pane_command_is_unknown() {
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
        nodes: vec![
            session("current"),
            // Operator dropped to a shell in the pane; not a known harness command.
            mux_with_command("editor", Some("zsh")),
        ],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

    let link = snapshot
        .candidate_links
        .iter()
        .find(|link| link.relation == RelationKind::LinkedToMux)
        .expect("link present");
    assert!(
        matches!(link.state, LinkState::Active),
        "unknown pane command should not filter; got {:?}",
        link.state
    );
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
            link.relation == RelationKind::LinkedToMux && matches!(link.state, LinkState::Active)
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

/// Proves the opencode end-to-end path for H-MUXPROC-014: a SQLite
/// hook record written by the `@conspectus/opencode-hook` plugin
/// produces a fresh `LinkedToMux` candidate sourced from an opencode
/// session, and demotes a stale `active_pane_command_session_match`
/// candidate pointing at the launch-argv session for the same mux.
/// The harness-key check in `pane_running_harness` and the demotion
/// rule in `demote_weaker_mux_links` are both already
/// harness-agnostic; this test pins that behavior under the opencode
/// harness key explicitly so future refactors cannot regress it.
#[test]
fn opencode_hook_record_demotes_stale_launch_argv_for_same_mux() {
    let temp = tempdir().expect("tempdir");

    let opencode_session = |key: &str| -> GraphNode {
        GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("opencode", "/oc-state", key),
                "opencode".to_string(),
            )
            .with_cwd("/work/proj".to_string()),
        )
    };

    let opencode_mux = GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new("tmux:editor".to_string()),
        backend: "tmux".to_string(),
        native_id: "editor".to_string(),
        cwd: Some("/work/proj".to_string()),
        active_pane_command: Some("opencode".to_string()),
        active_pane_pid: Some(4242),
        active_pane_current_path: Some("/work/proj".to_string()),
        active_pane_start_command: Some(
            "opencode /work/proj --session launch-argv-session".to_string(),
        ),
        client_attached: None,
        activity_epoch: Some(1_700_000_000),
        created_epoch: None,
        last_attached_epoch: None,
    });

    let stale_argv_link = {
        let mut fields = Metadata::new();
        fields.insert(
            "match_kind".to_string(),
            serde_json::Value::String("active_pane_command_session_match".to_string()),
        );
        GraphLink {
            id: "cross_link:cmd:launch-argv-session:editor".to_string(),
            source: NodeId::AgentSession(AgentSessionId::new(
                "opencode",
                "/oc-state",
                "launch-argv-session",
            )),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(MuxSessionId::new("tmux:editor".to_string())),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: Freshness::Fresh,
            source_metadata: SourceMetadata {
                adapter: "cross_link".to_string(),
                evidence: Some("active_pane_command_session_match".to_string()),
                fields,
                freshness_epoch: None,
            },
            state: LinkState::Active,
        }
    };

    hook::HookStore::new(temp.path())
        .write_record(&hook::HookRecord {
            schema_version: hook::SCHEMA_VERSION,
            harness_key: "opencode".to_string(),
            session_key: "current-after-resume".to_string(),
            cwd: Some("/work/proj".to_string()),
            pid: Some(i64::from(std::process::id())),
            ppid: Some(4241),
            tmux: Some(hook::HookTmuxRecord {
                session_name: Some("editor".to_string()),
                native_id: None,
                pane_id: Some("%1".to_string()),
                socket_path: None,
            }),
            transcript_path: None,
            hook_event_name: Some("session.updated".to_string()),
            observed_epoch: 1_700_000_050,
            harness_version: Some("0.1.0".to_string()),
        })
        .expect("write hook record");

    let mut snapshot = GraphSnapshot {
        nodes: vec![
            opencode_session("launch-argv-session"),
            opencode_session("current-after-resume"),
            opencode_mux,
        ],
        candidate_links: vec![stale_argv_link],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

    let stale = snapshot
        .candidate_links
        .iter()
        .find(|link| link.id == "cross_link:cmd:launch-argv-session:editor")
        .expect("stale argv link");
    match &stale.state {
        LinkState::Overridden { reason, .. } => {
            assert!(
                reason.as_ref().is_some_and(|r| r.contains("hook sidecar")),
                "expected hook-sidecar override reason, got {reason:?}"
            );
        }
        other => panic!("expected stale argv link to be Overridden, got {other:?}"),
    }

    let fresh = snapshot
        .candidate_links
        .iter()
        .find(|link| {
            link.source
                == NodeId::AgentSession(AgentSessionId::new(
                    "opencode",
                    "/oc-state",
                    "current-after-resume",
                ))
        })
        .expect("fresh hook link");
    assert!(matches!(fresh.state, LinkState::Active));
    assert_eq!(fresh.source_metadata.adapter, "hook_sidecar");
    assert_eq!(
        fresh.source_metadata.evidence.as_deref(),
        Some("hook_session_match")
    );
}
