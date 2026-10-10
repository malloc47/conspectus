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
        scan_roots: Arc::new(crate::cwd::ScanRoots::default()),
        intervals: Arc::new(ServerIntervals::default()),
        writer_lock: Arc::new(Mutex::new(())),
        state: Arc::new(Mutex::new(SchedulerState::default())),
        snapshot_bytes: Arc::clone(&snapshot_bytes),
        snapshot_state: Arc::clone(&snapshot_state),
        snapshot_path: Arc::new(temp.path().join("graph.bin")),
        discovery_caches: Arc::default(),
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

#[test]
fn min_cycle_gap_clamps_short_and_long_intervals_into_the_floor_ceiling_band() {
    // Short intervals collapse to the 250ms floor so a
    // hypothetical sub-second class stays responsive to a
    // watcher wake without a giant throttle.
    assert_eq!(
        min_cycle_gap(Duration::from_millis(500)),
        Duration::from_millis(250)
    );
    // Standard harness/mux interval (5s) picks up the quartered
    // value (1.25s) unchanged — inside the band.
    assert_eq!(
        min_cycle_gap(Duration::from_secs(5)),
        Duration::from_millis(1250)
    );
    // Long intervals (git = 30s, forge = 5m) saturate at the
    // 2s ceiling so a rare wake still runs promptly rather
    // than sitting on a multi-second throttle.
    assert_eq!(
        min_cycle_gap(Duration::from_secs(30)),
        Duration::from_secs(2)
    );
    assert_eq!(
        min_cycle_gap(Duration::from_secs(300)),
        Duration::from_secs(2)
    );
}

#[test]
fn throttle_since_returns_immediately_once_the_gap_has_elapsed() {
    // The cycle already ran long enough ago that no throttle
    // is needed; the helper must not sleep. A generous window
    // (100ms) keeps the assertion stable on a loaded CI box.
    let shutdown = AtomicBool::new(false);
    let last_end = Instant::now() - Duration::from_secs(5);
    let before = Instant::now();
    let broke = throttle_since(last_end, Duration::from_millis(500), &shutdown);
    let waited = before.elapsed();
    assert!(!broke, "no shutdown → returns false");
    assert!(
        waited < Duration::from_millis(100),
        "expected no sleep, waited {waited:?}"
    );
}

#[test]
fn throttle_since_sleeps_the_remaining_gap_when_the_last_cycle_was_recent() {
    // A very-recent cycle should force the caller to wait
    // roughly `min_gap` before the next watcher wait.
    let shutdown = AtomicBool::new(false);
    let last_end = Instant::now();
    let min_gap = Duration::from_millis(300);
    let before = Instant::now();
    let broke = throttle_since(last_end, min_gap, &shutdown);
    let waited = before.elapsed();
    assert!(!broke);
    assert!(
        waited >= min_gap - Duration::from_millis(50),
        "expected ≥{min_gap:?} sleep, waited {waited:?}"
    );
    assert!(
        waited < min_gap + Duration::from_millis(400),
        "sleep should be bounded by min_gap + one poll window, waited {waited:?}"
    );
}

#[test]
fn throttle_since_returns_early_when_shutdown_flips_mid_wait() {
    // A Ctrl-C mid-throttle should surface as `true` (break
    // the caller's loop) within the 200ms poll granularity —
    // not sit on the full remaining gap.
    let shutdown = Arc::new(AtomicBool::new(false));
    let last_end = Instant::now();
    let min_gap = Duration::from_secs(3);
    let flipper = {
        let shutdown = Arc::clone(&shutdown);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            shutdown.store(true, Ordering::Relaxed);
        })
    };
    let before = Instant::now();
    let broke = throttle_since(last_end, min_gap, &shutdown);
    let waited = before.elapsed();
    flipper.join().expect("flipper thread joins");
    assert!(broke, "shutdown mid-throttle must return true");
    assert!(
        waited < Duration::from_millis(600),
        "expected early return well before {min_gap:?}, waited {waited:?}"
    );
}
