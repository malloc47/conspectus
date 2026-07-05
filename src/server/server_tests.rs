// Extracted from mod.rs H-HYG-011 rolling wave via #[path = "server_tests.rs"] mod tests;
use super::*;
use crate::hook::{HookRecord, HookTmuxRecord, SCHEMA_VERSION};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphNode, MuxSessionId, MuxSessionNode, RelationKind,
};
use std::time::Duration;

#[test]
fn writer_lock_recovers_from_poison() {
    // Simulates the per-class panic case: a thread panics
    // while holding the lock, leaving it poisoned. Subsequent
    // acquisitions must still succeed so the surviving class
    // threads can keep ticking; the in-process lock is only
    // a coordination primitive.
    let lock = Arc::new(Mutex::new(()));
    let panicker = {
        let lock = Arc::clone(&lock);
        thread::spawn(move || {
            let _guard = lock.lock().unwrap();
            panic!("simulated class-thread panic");
        })
    };
    let _ = panicker.join();
    // The same `Err -> into_inner` recovery the scheduler uses.
    let _guard = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
}

#[test]
fn class_intervals_use_server_interval_durations() {
    // Pin the mapping: a future refactor that swaps the
    // class -> interval wiring would silently change the
    // scheduler cadence. The test compares against the
    // ServerIntervals defaults.
    let intervals = ServerIntervals::default();
    assert_eq!(
        ProviderClass::Harness.ttl_duration(&intervals),
        Duration::from_secs(5)
    );
    assert_eq!(
        ProviderClass::Mux.ttl_duration(&intervals),
        Duration::from_secs(5)
    );
    assert_eq!(
        ProviderClass::Git.ttl_duration(&intervals),
        Duration::from_secs(30)
    );
    assert_eq!(
        ProviderClass::Forge.ttl_duration(&intervals),
        Duration::from_secs(300)
    );
}

#[test]
fn hook_ingest_updates_snapshot_state_and_published_bytes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let snapshot_bytes: SnapshotBytes = Arc::new(Mutex::new(None));
    let snapshot_state: SnapshotState = Arc::new(Mutex::new(Some(GraphSnapshot {
        nodes: vec![agent_session("current"), mux_session("editor")],
        ..GraphSnapshot::empty()
    })));
    let ctx = DispatchCtx {
        scan_roots: Arc::new(Vec::new()),
        intervals: Arc::new(ServerIntervals::default()),
        writer_lock: Arc::new(Mutex::new(())),
        state: Arc::new(Mutex::new(SchedulerState::default())),
        snapshot_bytes: Arc::clone(&snapshot_bytes),
        snapshot_state: Arc::clone(&snapshot_state),
        snapshot_path: Arc::new(temp.path().join("graph.bin")),
    };
    let record = HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: "current".to_string(),
        cwd: Some("/work".to_string()),
        pid: None,
        ppid: None,
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: 1_700_000_000,
        harness_version: None,
    };
    let request = Request {
        command: "hook_ingest".to_string(),
        args: serde_json::json!({ "record": record }),
        id: "test-hook".to_string(),
    };

    let response = handle_hook_ingest(&request, &ctx);

    assert_eq!(response.result, "ok");
    assert!(snapshot_bytes.lock().unwrap().is_some());
    let snapshot = snapshot_state.lock().unwrap().clone().expect("snapshot");
    assert!(snapshot.candidate_links.iter().any(|link| {
        link.relation == RelationKind::LinkedToMux
            && link.source_metadata.adapter == crate::discovery::providers::HOOK_SIDECAR
    }));
}

fn agent_session(session_key: &str) -> GraphNode {
    GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", session_key),
            "claude-code".to_string(),
        )
        .with_cwd("/work".to_string())
        .with_last_active_epoch(1_700_000_000),
    )
}

fn mux_session(native_id: &str) -> GraphNode {
    GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new(format!("tmux:{native_id}")),
            "tmux".to_string(),
            native_id.to_string(),
        )
        .with_cwd("/work".to_string())
        .with_active_pane_command("claude".to_string())
        .with_active_pane_current_path("/work".to_string())
        .with_client_attached(true),
    )
}
