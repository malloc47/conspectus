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
        socket: None,
        backend_session_id: None,
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
            background_harness: false,
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
            background_harness: false,
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
            background_harness: false,
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
fn pi_hook_record_synthesizes_session_with_pi_state_scope() {
    let temp = tempdir().expect("tempdir");
    let sessions_root = temp.path().join("home/.pi/agent/sessions");
    let grouped_dir = sessions_root.join("--work--");
    fs::create_dir_all(&grouped_dir).expect("create grouped dir");
    let grouped = grouped_dir.join("2026-05-23T00-36-47-000Z_grouped-session.jsonl");
    let flat_root = temp.path().join("custom-sessions");
    fs::create_dir_all(&flat_root).expect("create flat dir");
    let flat = flat_root.join("2026-05-23T00-36-47-000Z_flat-session.jsonl");

    for (n, (session_key, transcript_path, pane)) in [
        ("grouped-session", &grouped, "%1"),
        ("flat-session", &flat, "%2"),
    ]
    .into_iter()
    .enumerate()
    {
        fs::write(transcript_path, "").expect("touch transcript");
        let record_payload = format!(
            r#"{{
                  "schema_version": 1,
                  "harness_key": "pi",
                  "session_key": "{session_key}",
                  "cwd": "/work",
                  "tmux": {{ "session_name": "editor", "pane_id": "{pane}" }},
                  "transcript_path": "{}",
                  "observed_epoch": 1700000000
                }}"#,
            transcript_path.display()
        );
        fs::write(temp.path().join(format!("record-{n}.json")), record_payload)
            .expect("write record");
    }
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux_with_command("editor", Some("pi"))],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_100);

    // The grouped layout's root is above the `--<cwd>--` directory; the
    // flat layout's root is the file's own directory.
    for (session_key, root) in [
        ("grouped-session", &sessions_root),
        ("flat-session", &flat_root),
    ] {
        let session_id = AgentSessionId::new("pi", root.to_string_lossy(), session_key);
        assert!(
            snapshot.nodes.iter().any(|node| matches!(
                node,
                GraphNode::AgentSession(session) if session.id == session_id
            )),
            "{session_key}: {:#?}",
            snapshot.nodes
        );
    }
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
fn pane_running_harness_recognizes_exact_harness_names_only() {
    let running = |command: &str| {
        let GraphNode::MuxSession(mux) = mux_with_command("m", Some(command)) else {
            unreachable!()
        };
        pane_running_harness(&mux)
    };
    assert_eq!(running("pi"), Some("pi"));
    assert_eq!(running("pi-rpc"), Some("pi"));
    assert_eq!(running("claude"), Some("claude-code"));
    assert_eq!(running("codex-rs"), Some("codex"));
    assert_eq!(running("pip"), None);
    assert_eq!(running("node"), None);
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

/// A probe of the default tmux server that listed `panes` of session
/// `editor`, each with the program it runs.
fn editor_panes(panes: &[(&str, &str)]) -> BTreeMap<String, crate::model::MuxProbe> {
    let mut probe =
        crate::model::MuxProbe::new(crate::model::MuxProbeStatus::Listed, 1_700_000_500);
    probe.panes = panes
        .iter()
        .map(|(pane, command)| {
            (
                pane.to_string(),
                crate::model::ProbedPane {
                    session: "editor".to_string(),
                    command: Some(command.to_string()),
                    active: *pane == "%1",
                },
            )
        })
        .collect();
    BTreeMap::from([("tmux".to_string(), probe)])
}

/// A live Codex record from pane `pane` of tmux session `editor`.
fn codex_pane_record(session_key: &str, pane: &str, observed_epoch: i64) -> hook::HookRecord {
    hook::HookRecord {
        harness_key: "codex".to_string(),
        ..pane_record(session_key, pane, observed_epoch)
    }
}

fn codex_session(key: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/codex", key),
            "codex".to_string(),
        )
        .with_cwd("/work".to_string()),
    )
}

/// The state of the hook link from `session` to a mux.
fn hook_link_state(snapshot: &GraphSnapshot, session: &NodeId) -> LinkState {
    hook_mux_links(snapshot)
        .into_iter()
        .find(|link| &link.source == session)
        .map(|link| link.state.clone())
        .expect("the session has a hook link")
}

#[test]
fn a_record_from_a_split_pane_is_not_ignored_for_what_the_active_pane_runs() {
    // Codex is in the active pane %1; Claude Code in pane %2 beside it.
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("beta"), mux_with_command("editor", Some("codex"))],
        mux_probes: editor_panes(&[("%1", "codex"), ("%2", "claude")]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![pane_record("beta", "%2", 1_700_000_000)],
        1_700_000_600,
    );
    assert_eq!(
        hook_link_state(&snapshot, &session_node("beta")),
        LinkState::Active
    );
}

#[test]
fn a_record_whose_own_pane_now_runs_another_harness_is_still_ignored() {
    // Pane %2 is not the active one, and now runs Codex.
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("stale"), mux_with_command("editor", Some("claude"))],
        mux_probes: editor_panes(&[("%1", "claude"), ("%2", "codex")]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![pane_record("stale", "%2", 1_700_000_000)],
        1_700_000_600,
    );
    match hook_link_state(&snapshot, &session_node("stale")) {
        LinkState::Ignored { reason } => {
            let reason = reason.unwrap_or_default();
            assert!(
                reason.contains("`codex`") && reason.contains("`claude-code`"),
                "{reason}"
            );
        }
        other => panic!("expected Ignored, got {other:?}"),
    }
}

#[test]
fn a_record_from_a_pane_the_listing_does_not_show_is_not_gated_by_another_pane() {
    // The listing shows only the active pane; the record's pane is newer
    // than the probe, or in another session now.
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("beta"), mux_with_command("editor", Some("codex"))],
        mux_probes: editor_panes(&[("%1", "codex")]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![pane_record("beta", "%9", 1_700_000_000)],
        1_700_000_600,
    );
    assert_eq!(
        hook_link_state(&snapshot, &session_node("beta")),
        LinkState::Active
    );
}

#[test]
fn a_record_naming_no_pane_is_still_judged_by_the_active_pane() {
    let mut record = pane_record("stale", "%2", 1_700_000_000);
    record.tmux.as_mut().expect("tmux").pane_id = None;
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("stale"), mux_with_command("editor", Some("codex"))],
        mux_probes: editor_panes(&[("%1", "codex"), ("%2", "claude")]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(&mut snapshot, vec![record], 1_700_000_600);
    assert!(matches!(
        hook_link_state(&snapshot, &session_node("stale")),
        LinkState::Ignored { .. }
    ));
}

#[test]
fn agents_of_two_harnesses_in_split_panes_of_one_session_are_both_attributed() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("beta"),
            codex_session("alpha"),
            mux_with_command("editor", Some("codex")),
        ],
        mux_probes: editor_panes(&[("%1", "codex"), ("%2", "claude")]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![
            codex_pane_record("alpha", "%1", 1_700_000_000),
            pane_record("beta", "%2", 1_700_000_100),
        ],
        1_700_000_600,
    );
    let alpha = NodeId::AgentSession(AgentSessionId::new("codex", "/codex", "alpha"));
    assert_eq!(hook_link_state(&snapshot, &alpha), LinkState::Active);
    assert_eq!(
        hook_link_state(&snapshot, &session_node("beta")),
        LinkState::Active
    );
    assert_eq!(runtime_process_count(&snapshot), 2);

    // With the active pane switched to Claude Code, Codex stays too.
    let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] else {
        unreachable!()
    };
    mux.active_pane_command = Some("claude".to_string());
    snapshot.candidate_links.clear();
    snapshot
        .nodes
        .retain(|node| !matches!(node, GraphNode::RuntimeProcess(_)));
    apply_hook_records(
        &mut snapshot,
        vec![
            codex_pane_record("alpha", "%1", 1_700_000_000),
            pane_record("beta", "%2", 1_700_000_100),
        ],
        1_700_000_600,
    );
    assert_eq!(hook_link_state(&snapshot, &alpha), LinkState::Active);
    assert_eq!(
        hook_link_state(&snapshot, &session_node("beta")),
        LinkState::Active
    );
}

const DEFAULT_SOCKET: &str = "/tmp/tmux-1000/default";
const WORK_SOCKET: &str = "/tmp/tmux-1000/work";

/// Probes of the tmux servers `servers` names, by socket name (`None`
/// is the default server), each with the socket path it read.
fn server_probes(
    servers: &[(Option<&str>, Option<&str>)],
) -> BTreeMap<String, crate::model::MuxProbe> {
    servers
        .iter()
        .map(|(socket, path)| {
            let mut probe =
                crate::model::MuxProbe::new(crate::model::MuxProbeStatus::Listed, 1_700_000_500);
            probe.socket_path = path.map(str::to_string);
            (crate::model::probe_key("tmux", *socket), probe)
        })
        .collect()
}

/// Session `editor` on the `-L work` server, in another directory and
/// with another active pane process than [`mux`]'s.
fn work_editor() -> GraphNode {
    let GraphNode::MuxSession(mut mux) = mux("work:editor") else {
        unreachable!()
    };
    mux.socket = Some("work".to_string());
    mux.cwd = Some("/elsewhere".to_string());
    mux.active_pane_current_path = Some("/elsewhere".to_string());
    mux.active_pane_pid = Some(456);
    GraphNode::MuxSession(mux)
}

/// [`pane_record`] from the server whose socket is at `socket`.
fn record_on(session_key: &str, socket: &str) -> hook::HookRecord {
    let mut record = pane_record(session_key, "%1", 1_700_000_000);
    record.tmux.as_mut().expect("tmux").socket_path = Some(socket.to_string());
    record
}

/// The mux each Active hook link ties `session` to.
fn hook_mux_targets(snapshot: &GraphSnapshot, session: &NodeId) -> Vec<NodeId> {
    hook_mux_links(snapshot)
        .into_iter()
        .filter(|link| &link.source == session && matches!(link.state, LinkState::Active))
        .filter_map(|link| link.target_node_id().cloned())
        .collect()
}

fn mux_node(native_id: &str) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(format!("tmux:{native_id}")))
}

#[test]
fn a_record_from_a_default_server_elsewhere_is_not_attributed_to_a_session_of_its_name() {
    // The record's server is a default server under another
    // `TMUX_TMPDIR`; its session shares the probed server's session's
    // name, its directory, and nothing else.
    let foreign = "/run/other/tmux-1000/default";
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("alpha"), mux("editor")],
        mux_probes: server_probes(&[(None, Some(DEFAULT_SOCKET))]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![record_on("alpha", foreign)],
        1_700_000_600,
    );
    assert!(hook_mux_links(&snapshot).is_empty());

    // The probed server's own record still matches by session name.
    apply_hook_records(
        &mut snapshot,
        vec![record_on("alpha", DEFAULT_SOCKET)],
        1_700_000_600,
    );
    assert_eq!(
        hook_mux_targets(&snapshot, &session_node("alpha")),
        [mux_node("editor")]
    );
}

#[test]
fn a_probe_that_read_no_socket_path_leaves_the_name_to_decide() {
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("alpha"), mux("editor")],
        mux_probes: server_probes(&[(None, None)]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![record_on("alpha", "/run/other/tmux-1000/default")],
        1_700_000_600,
    );
    assert_eq!(
        hook_mux_targets(&snapshot, &session_node("alpha")),
        [mux_node("editor")]
    );
}

#[test]
fn a_record_from_a_named_server_matches_its_mux_by_session_name_on_that_server() {
    // Session `editor` on the default server and on `-L work`; each
    // record names its own server's socket.
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            session("alpha"),
            session("beta"),
            mux("editor"),
            work_editor(),
        ],
        mux_probes: server_probes(&[
            (None, Some(DEFAULT_SOCKET)),
            (Some("work"), Some(WORK_SOCKET)),
        ]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(
        &mut snapshot,
        vec![
            record_on("alpha", WORK_SOCKET),
            record_on("beta", DEFAULT_SOCKET),
        ],
        1_700_000_600,
    );
    assert_eq!(
        hook_mux_targets(&snapshot, &session_node("alpha")),
        [mux_node("work:editor")]
    );
    assert_eq!(
        hook_mux_targets(&snapshot, &session_node("beta")),
        [mux_node("editor")]
    );
}

#[test]
fn a_record_from_a_named_server_falls_back_to_a_directory_on_that_server_only() {
    // The record's session is not listed (renamed since, say); the cwd
    // fallback finds the `-L work` mux in its directory, not the default
    // server's mux in the same directory.
    let mut record = record_on("alpha", WORK_SOCKET);
    record.tmux.as_mut().expect("tmux").session_name = Some("renamed".to_string());
    record.cwd = Some("/work".to_string());
    let GraphNode::MuxSession(mut work) = work_editor() else {
        unreachable!()
    };
    work.cwd = Some("/work".to_string());
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("alpha"), mux("editor"), GraphNode::MuxSession(work)],
        mux_probes: server_probes(&[
            (None, Some(DEFAULT_SOCKET)),
            (Some("work"), Some(WORK_SOCKET)),
        ]),
        ..GraphSnapshot::empty()
    };
    apply_hook_records(&mut snapshot, vec![record], 1_700_000_600);
    assert_eq!(
        hook_mux_targets(&snapshot, &session_node("alpha")),
        [mux_node("work:editor")]
    );
}

/// A live Claude Code record from pane `pane` of tmux session `editor`:
/// its pid is this test process, so the liveness gate passes.
fn pane_record(session_key: &str, pane: &str, observed_epoch: i64) -> hook::HookRecord {
    hook::HookRecord {
        schema_version: hook::SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: session_key.to_string(),
        cwd: Some("/work".to_string()),
        pid: Some(i64::from(std::process::id())),
        ppid: None,
        tmux: Some(hook::HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some(pane.to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch,
        harness_version: None,
        background_harness: false,
    }
}

fn session_node(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

/// The sessions with an Active hook link to a mux.
fn active_hook_sources(snapshot: &GraphSnapshot) -> Vec<NodeId> {
    let mut sources: Vec<NodeId> = hook_mux_links(snapshot)
        .into_iter()
        .filter(|link| matches!(link.state, LinkState::Active))
        .map(|link| link.source.clone())
        .collect();
    sources.sort();
    sources
}

/// The sessions the graph's hook process observations identify.
fn observed_process_sessions(snapshot: &GraphSnapshot) -> Vec<NodeId> {
    let mut sessions: Vec<NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::ProcessIdentifiesSession)
        .filter_map(|link| link.target_node_id().cloned())
        .collect();
    sessions.sort();
    sessions
}

fn runtime_process_count(snapshot: &GraphSnapshot) -> usize {
    snapshot
        .nodes
        .iter()
        .filter(|node| matches!(node, GraphNode::RuntimeProcess(_)))
        .count()
}

#[test]
fn hook_records_for_different_panes_in_same_mux_both_remain_active() {
    let temp = tempdir().expect("tempdir");
    let store = hook::HookStore::new(temp.path());
    store
        .write_record(&pane_record("alpha", "%1", 1_700_000_000))
        .expect("write pane 1 record");
    store
        .write_record(&pane_record("beta", "%2", 1_700_000_500))
        .expect("write pane 2 record");
    let mut snapshot = GraphSnapshot {
        nodes: vec![session("alpha"), session("beta"), mux("editor")],
        ..GraphSnapshot::empty()
    };

    apply_hook_sidecars(&mut snapshot, temp.path(), 1_700_000_600);

    assert_eq!(
        active_hook_sources(&snapshot),
        [session_node("alpha"), session_node("beta")]
    );
}

/// The daemon's ingest applies one record to a graph that already holds
/// the previous cycle's hook links.
fn ingest(snapshot: &mut GraphSnapshot, record: hook::HookRecord) {
    apply_hook_records(snapshot, vec![record], 1_700_001_000);
}

fn editor_graph(sessions: &[&str]) -> GraphSnapshot {
    let mut nodes: Vec<GraphNode> = sessions.iter().map(|key| session(key)).collect();
    nodes.push(mux("editor"));
    GraphSnapshot {
        nodes,
        ..GraphSnapshot::empty()
    }
}

#[test]
fn an_ingest_overrides_the_graphs_older_hook_link_for_the_same_pane() {
    let mut snapshot = editor_graph(&["alpha", "beta"]);
    ingest(&mut snapshot, pane_record("alpha", "%1", 1_700_000_000));
    ingest(&mut snapshot, pane_record("beta", "%1", 1_700_000_500));

    assert_eq!(active_hook_sources(&snapshot), [session_node("beta")]);
    let winner = hook_mux_links(&snapshot)
        .into_iter()
        .find(|link| link.source == session_node("beta"))
        .expect("winner")
        .id
        .clone();
    let loser = hook_mux_links(&snapshot)
        .into_iter()
        .find(|link| link.source == session_node("alpha"))
        .expect("the older link stays for diagnostics");
    assert_eq!(
        loser.state,
        LinkState::Overridden {
            by: winner,
            reason: Some("superseded by fresher hook sidecar record for same pane".to_string()),
        }
    );
    assert_eq!(runtime_process_count(&snapshot), 1);
    assert_eq!(
        observed_process_sessions(&snapshot),
        [session_node("beta")],
        "only the pane's winner keeps a process observation"
    );
}

#[test]
fn an_ingest_matches_what_the_next_cycle_replays() {
    let temp = tempdir().expect("tempdir");
    let store = hook::HookStore::new(temp.path());
    let mut live = editor_graph(&["alpha", "beta", "gamma"]);
    for record in [
        pane_record("alpha", "%1", 1_700_000_000),
        pane_record("beta", "%2", 1_700_000_100),
        pane_record("gamma", "%1", 1_700_000_200),
    ] {
        store.write_record(&record).expect("spool");
        ingest(&mut live, record);
    }
    let mut cycled = editor_graph(&["alpha", "beta", "gamma"]);
    apply_hook_sidecars(&mut cycled, temp.path(), 1_700_001_000);

    let expected = [session_node("beta"), session_node("gamma")];
    assert_eq!(active_hook_sources(&live), expected);
    assert_eq!(active_hook_sources(&cycled), expected);
    assert_eq!(observed_process_sessions(&live), expected);
    assert_eq!(observed_process_sessions(&cycled), expected);
}

#[test]
fn an_ingest_older_than_the_graphs_link_for_its_pane_is_overridden() {
    let mut snapshot = editor_graph(&["alpha", "beta"]);
    ingest(&mut snapshot, pane_record("beta", "%1", 1_700_000_500));
    ingest(&mut snapshot, pane_record("alpha", "%1", 1_700_000_000));

    assert_eq!(active_hook_sources(&snapshot), [session_node("beta")]);
    let late = hook_mux_links(&snapshot)
        .into_iter()
        .find(|link| link.source == session_node("alpha"))
        .expect("late arrival");
    assert!(matches!(late.state, LinkState::Overridden { .. }));
    assert_eq!(observed_process_sessions(&snapshot), [session_node("beta")]);
}

#[test]
fn an_ingest_of_the_panes_session_again_keeps_one_link_and_one_process() {
    let mut snapshot = editor_graph(&["alpha"]);
    ingest(&mut snapshot, pane_record("alpha", "%1", 1_700_000_000));
    ingest(&mut snapshot, pane_record("alpha", "%1", 1_700_000_000));
    ingest(&mut snapshot, pane_record("alpha", "%1", 1_700_000_300));

    assert_eq!(active_hook_sources(&snapshot), [session_node("alpha")]);
    assert_eq!(
        hook_mux_links(&snapshot).len(),
        2,
        "the first stays Overridden"
    );
    assert_eq!(runtime_process_count(&snapshot), 1);
}

#[test]
fn a_graph_holding_two_active_hook_links_for_one_pane_keeps_the_fresher() {
    let mut snapshot = editor_graph(&["alpha", "beta", "gamma"]);
    // What an ingest could leave behind before it superseded links:
    // both pane-1 links Active, each with its process observation.
    let mut first = snapshot.clone();
    ingest(&mut first, pane_record("alpha", "%1", 1_700_000_000));
    let mut second = snapshot.clone();
    ingest(&mut second, pane_record("beta", "%1", 1_700_000_500));
    snapshot.nodes = first.nodes;
    for node in second.nodes {
        if snapshot.find_node(&node.id()).is_none() {
            snapshot.nodes.push(node);
        }
    }
    snapshot.candidate_links = first.candidate_links;
    snapshot.candidate_links.extend(second.candidate_links);
    assert_eq!(active_hook_sources(&snapshot).len(), 2);

    ingest(&mut snapshot, pane_record("gamma", "%2", 1_700_000_600));

    assert_eq!(
        active_hook_sources(&snapshot),
        [session_node("beta"), session_node("gamma")]
    );
    assert_eq!(
        observed_process_sessions(&snapshot),
        [session_node("beta"), session_node("gamma")]
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

/// Proves the opencode end-to-end path: a SQLite
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
        socket: None,
        backend_session_id: None,
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
            background_harness: false,
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

fn opencode_pane(native_id: &str, pane_pid: i64) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("tmux:{native_id}")),
        backend: "tmux".to_string(),
        native_id: native_id.to_string(),
        cwd: Some("/work/proj".to_string()),
        active_pane_command: Some("opencode".to_string()),
        active_pane_pid: Some(pane_pid),
        active_pane_current_path: Some("/work/proj".to_string()),
        active_pane_start_command: Some("opencode".to_string()),
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
        socket: None,
        backend_session_id: None,
    })
}

fn opencode_session(key: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "/oc-state", key),
            "opencode".to_string(),
        )
        .with_cwd("/work/proj".to_string()),
    )
}

/// A record as the hook writer produces it: `tmux_pane` is the writer's
/// pane context, and `background_harness` marks a record from opencode
/// 2.x's shared service, whose context came from the first client.
fn opencode_record(
    session_key: &str,
    tmux_pane: Option<(&str, &str)>,
    observed_epoch: i64,
    background_harness: bool,
) -> hook::HookRecord {
    hook::HookRecord {
        schema_version: hook::SCHEMA_VERSION,
        harness_key: "opencode".to_string(),
        session_key: session_key.to_string(),
        cwd: Some("/work/proj".to_string()),
        pid: Some(i64::from(std::process::id())),
        ppid: None,
        tmux: tmux_pane.map(|(session_name, pane_id)| hook::HookTmuxRecord {
            session_name: Some(session_name.to_string()),
            native_id: None,
            pane_id: Some(pane_id.to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("session.created".to_string()),
        observed_epoch,
        harness_version: Some("0.2.0".to_string()),
        background_harness,
    }
}

fn two_opencode_panes() -> GraphSnapshot {
    GraphSnapshot {
        nodes: vec![
            opencode_pane("alpha", 100),
            opencode_pane("beta", 200),
            opencode_session("ses_alpha00001"),
            opencode_session("ses_beta000001"),
        ],
        ..GraphSnapshot::empty()
    }
}

fn hook_mux_links(snapshot: &GraphSnapshot) -> Vec<&GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::LinkedToMux
                && link.source_metadata.adapter == ADAPTER_NAME
        })
        .collect()
}

#[test]
fn in_pane_opencode_record_still_links_its_pane() {
    let mut snapshot = two_opencode_panes();

    apply_hook_records(
        &mut snapshot,
        vec![opencode_record(
            "ses_beta000001",
            Some(("beta", "%1")),
            1_700_000_000,
            false,
        )],
        1_700_000_010,
    );

    let links = hook_mux_links(&snapshot);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].state, LinkState::Active);
    assert_eq!(
        links[0].target_node_id(),
        Some(&NodeId::MuxSession(MuxSessionId::new("tmux:beta")))
    );
}

#[test]
fn background_harness_record_with_inherited_tmux_links_no_mux() {
    let mut snapshot = two_opencode_panes();

    apply_hook_records(
        &mut snapshot,
        vec![opencode_record(
            "ses_beta000001",
            Some(("alpha", "%0")),
            1_700_000_000,
            true,
        )],
        1_700_000_010,
    );

    assert!(hook_mux_links(&snapshot).is_empty());
}

#[test]
fn background_harness_record_does_not_fall_back_to_pid_or_cwd() {
    let mut snapshot = two_opencode_panes();
    let mut record = opencode_record("ses_beta000001", None, 1_700_000_000, true);
    record.pid = Some(100);

    apply_hook_records(&mut snapshot, vec![record], 1_700_000_010);

    assert!(hook_mux_links(&snapshot).is_empty());
}

/// Before background records were set aside, the shared service's records
/// for both sessions named the first client's pane, and the freshest one
/// won that pane at hook rank.
#[test]
fn shared_service_records_do_not_force_two_panes_sessions_onto_one_mux() {
    let mut snapshot = two_opencode_panes();

    apply_hook_records(
        &mut snapshot,
        vec![
            opencode_record("ses_alpha00001", Some(("alpha", "%0")), 1_700_000_000, true),
            opencode_record("ses_beta000001", Some(("alpha", "%0")), 1_700_000_005, true),
        ],
        1_700_000_010,
    );
    let snapshot = crate::resolve::resolve_snapshot(snapshot);

    let beta_session = NodeId::AgentSession(AgentSessionId::new(
        "opencode",
        "/oc-state",
        "ses_beta000001",
    ));
    let alpha_mux = NodeId::MuxSession(MuxSessionId::new("tmux:alpha"));
    assert!(
        !snapshot.resolved_relationships.iter().any(|relationship| {
            relationship.relation == RelationKind::LinkedToMux
                && relationship.source == beta_session
                && relationship.target == alpha_mux
                && relationship.selected_link_id.is_some()
        }),
        "beta's session must not resolve to alpha's pane"
    );
}

/// The opencode 2.x plugin's TUI entrypoint writes from each client's
/// own pane (ADR 0126), so two clients on one repo bind separately.
#[test]
fn two_opencode_tui_clients_bind_their_own_sessions() {
    let mut snapshot = two_opencode_panes();

    apply_hook_records(
        &mut snapshot,
        vec![
            opencode_record(
                "ses_alpha00001",
                Some(("alpha", "%0")),
                1_700_000_000,
                false,
            ),
            opencode_record("ses_beta000001", Some(("beta", "%1")), 1_700_000_005, false),
        ],
        1_700_000_010,
    );
    let snapshot = crate::resolve::resolve_snapshot(snapshot);

    for (session, mux) in [
        ("ses_alpha00001", "tmux:alpha"),
        ("ses_beta000001", "tmux:beta"),
    ] {
        let source = NodeId::AgentSession(AgentSessionId::new("opencode", "/oc-state", session));
        let resolved = snapshot
            .resolved_relationships
            .iter()
            .find(|relationship| {
                relationship.relation == RelationKind::LinkedToMux
                    && relationship.source == source
                    && relationship.selected_link_id.is_some()
            })
            .unwrap_or_else(|| panic!("{session} resolves to a mux"));
        assert_eq!(
            resolved.target,
            NodeId::MuxSession(MuxSessionId::new(mux)),
            "{session}"
        );
    }
}

/// A client outside tmux writes no tmux context; its pid is still its
/// own, so the record only binds a pane whose process it is.
#[test]
fn opencode_tui_record_without_tmux_binds_only_through_its_own_pid() {
    let mut snapshot = two_opencode_panes();
    let mut record = opencode_record("ses_beta000001", None, 1_700_000_000, false);
    record.pid = Some(200);

    apply_hook_records(&mut snapshot, vec![record], 1_700_000_010);

    let links = hook_mux_links(&snapshot);
    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target_node_id(),
        Some(&NodeId::MuxSession(MuxSessionId::new("tmux:beta")))
    );
}

/// The same session open in two clients yields an active candidate for
/// each pane; the resolver, not the sidecar, decides what that means.
#[test]
fn one_session_open_in_two_opencode_clients_links_both_panes() {
    let mut snapshot = two_opencode_panes();

    apply_hook_records(
        &mut snapshot,
        vec![
            opencode_record(
                "ses_beta000001",
                Some(("alpha", "%0")),
                1_700_000_000,
                false,
            ),
            opencode_record("ses_beta000001", Some(("beta", "%1")), 1_700_000_005, false),
        ],
        1_700_000_010,
    );

    let mut targets: Vec<_> = hook_mux_links(&snapshot)
        .into_iter()
        .filter(|link| link.state == LinkState::Active)
        .filter_map(|link| link.target_node_id().map(ToString::to_string))
        .collect();
    targets.sort();
    assert_eq!(
        targets,
        vec!["mux_session:tmux:alpha", "mux_session:tmux:beta"]
    );
    let _ = crate::resolve::resolve_snapshot(snapshot);
}

#[test]
fn no_opencode_hook_records_leaves_attribution_to_other_evidence() {
    let mut snapshot = two_opencode_panes();
    let before = snapshot.candidate_links.len();

    apply_hook_records(&mut snapshot, Vec::new(), 1_700_000_010);

    assert_eq!(snapshot.candidate_links.len(), before);
    assert!(hook_mux_links(&snapshot).is_empty());
}
