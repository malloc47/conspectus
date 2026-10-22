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
        Duration::from_secs(3600)
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
        forge_fetch_lock: Arc::default(),
        discovery: Arc::new(|_| LocalDiscoveryConfig::empty()),
        shutdown: Arc::default(),
        history: None,
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
        background_harness: false,
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

/// A `gh` stand-in that counts calls and sleeps `delay` on each. With
/// `stops` set, each call flips that latch, as a shutdown arriving
/// mid-fetch would.
#[derive(Clone, Default)]
struct SlowGh {
    calls: Arc<std::sync::atomic::AtomicUsize>,
    delay: Duration,
    stops: Option<Arc<AtomicBool>>,
}

impl crate::discovery::forge::GhRunner for SlowGh {
    fn list_pull_requests(
        &self,
        _repo: &str,
        _filter: &crate::discovery::forge::PrListFilter,
        _fields: &str,
    ) -> Result<crate::discovery::forge::GhOutcome> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(stops) = &self.stops {
            stops.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        std::thread::sleep(self.delay);
        Ok(crate::discovery::forge::GhOutcome::PullRequests(
            r#"[{"number": 3, "headRefName": "main"}]"#.to_string(),
        ))
    }

    fn view_pull_request(
        &self,
        _repo: &str,
        _number: u64,
        _fields: &str,
    ) -> Result<crate::discovery::forge::GhOutcome> {
        Ok(crate::discovery::forge::GhOutcome::PullRequests(
            "{}".to_string(),
        ))
    }
}

/// A daemon context whose published snapshot holds one checkout of a
/// real git repo with a GitHub remote, and whose discovery runs no
/// providers but the forge, through `gh`.
fn forge_ctx(temp: &Path, gh: SlowGh) -> DispatchCtx {
    forge_ctx_with(temp, gh, Arc::default())
}

/// [`forge_ctx`] with a given shutdown latch.
fn forge_ctx_with(temp: &Path, gh: SlowGh, shutdown: Arc<AtomicBool>) -> DispatchCtx {
    use crate::model::{CheckoutId, CheckoutNode, RepoId};
    let root = temp.join("repo");
    std::fs::create_dir_all(&root).expect("repo dir");
    for args in [
        &["init", "--quiet", "--initial-branch", "main"][..],
        &["remote", "add", "origin", "git@github.com:octo/repo.git"][..],
    ] {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .status()
            .expect("git");
        assert!(status.success());
    }
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(
                RepoId::new(root.join(".git").display().to_string()),
                root.display().to_string(),
            ),
            root: root.display().to_string(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
        })],
        ..GraphSnapshot::empty()
    };
    let caches: Arc<DiscoveryCaches> = Arc::default();
    let factory_caches = Arc::clone(&caches);
    let factory_shutdown = Arc::clone(&shutdown);
    DispatchCtx {
        // Git discovery keeps finding the checkout, as it would in a
        // running daemon, so cycles don't evict it.
        scan_roots: Arc::new(crate::cwd::ScanRoots::explicit(vec![root])),
        intervals: Arc::new(ServerIntervals::default()),
        writer_lock: Arc::new(Mutex::new(())),
        state: Arc::new(Mutex::new(SchedulerState::default())),
        snapshot_bytes: Arc::new(Mutex::new(None)),
        snapshot_state: Arc::new(Mutex::new(Some(snapshot))),
        snapshot_path: Arc::new(temp.join("graph.bin")),
        discovery_caches: caches,
        forge_fetch_lock: Arc::default(),
        discovery: Arc::new(move |fetch| {
            LocalDiscoveryConfig::empty()
                .with_caches(Arc::clone(&factory_caches))
                .with_forge_runner(gh.clone())
                .with_forge_fetch(fetch)
                .with_forge_cancel(Arc::clone(&factory_shutdown))
        }),
        shutdown,
        history: None,
    }
}

fn refresh_request(class: &str) -> Request {
    Request {
        command: "refresh".to_string(),
        args: serde_json::json!({ "class": class }),
        id: "t".to_string(),
    }
}

#[test]
fn a_forge_fetch_does_not_need_the_writer_lock() {
    // ADR 0136: the network round runs under the fetch lock only, so a
    // cycle holding the writer lock can't stall it and it can't stall
    // the cycle.
    let temp = tempfile::tempdir().expect("tempdir");
    let gh = SlowGh::default();
    let ctx = forge_ctx(temp.path(), gh.clone());
    let _writer = ctx.writer_lock.lock().expect("writer lock");
    let (tx, rx) = std::sync::mpsc::channel();
    let fetch_ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_forges(&fetch_ctx, false));
    });
    let asked = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the fetch finished while the writer lock was held");
    assert_eq!(asked, 1);
    assert_eq!(gh.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_live_class_refresh_does_not_wait_for_a_forge_refresh() {
    let temp = tempfile::tempdir().expect("tempdir");
    let gh = SlowGh {
        delay: Duration::from_millis(800),
        ..SlowGh::default()
    };
    let ctx = forge_ctx(temp.path(), gh.clone());
    let forge_ctx = ctx.clone();
    let forge = std::thread::spawn(move || handle_refresh(&refresh_request("forge"), &forge_ctx));
    // Let the forge refresh get into its fetch.
    while gh.calls.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }

    let started = Instant::now();
    let mux = handle_refresh(&refresh_request("mux"), &ctx);
    let mux_took = started.elapsed();

    assert_eq!(mux.result, "ok");
    assert!(
        mux_took < Duration::from_millis(500),
        "a mux refresh waited {mux_took:?} on the forge fetch"
    );
    assert_eq!(forge.join().expect("forge refresh").result, "ok");
}

#[test]
fn routine_cycles_never_ask_a_forge() {
    let temp = tempfile::tempdir().expect("tempdir");
    let gh = SlowGh::default();
    let ctx = forge_ctx(temp.path(), gh.clone());
    for class in [
        ProviderClass::Mux,
        ProviderClass::Harness,
        ProviderClass::Git,
    ] {
        try_class_cycle(class, &ctx, crate::discovery::current_epoch()).expect("cycle");
        assert_eq!(
            handle_refresh(&refresh_request(class.name()), &ctx).result,
            "ok"
        );
    }
    run_full_rebuild(&ctx).expect("rebuild");
    assert_eq!(gh.calls.load(std::sync::atomic::Ordering::SeqCst), 0);

    // An explicit forge refresh does ask, and its cycle merges the PRs.
    assert_eq!(handle_refresh(&refresh_request("forge"), &ctx).result, "ok");
    assert_eq!(gh.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let published = load_snapshot_state(&ctx.snapshot_state).expect("published");
    assert!(
        published
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::ForgePr(pr) if pr.number == 3)),
        "the forge refresh's PR was merged"
    );
}

fn has_prs(ctx: &DispatchCtx) -> bool {
    load_snapshot_state(&ctx.snapshot_state).is_some_and(|snapshot| {
        snapshot
            .nodes
            .iter()
            .any(|node| matches!(node, GraphNode::ForgePr(_)))
    })
}

#[test]
fn a_shutdown_during_a_forge_poll_skips_the_merge() {
    // ADR 0137: the fetch is abandoned, nothing is cached, and the
    // scheduler doesn't take the writer lock to publish it.
    let temp = tempfile::tempdir().expect("tempdir");
    let shutdown: Arc<AtomicBool> = Arc::default();
    let gh = SlowGh {
        stops: Some(Arc::clone(&shutdown)),
        ..SlowGh::default()
    };
    let ctx = forge_ctx_with(temp.path(), gh, Arc::clone(&shutdown));

    assert!(!forge_tick(&ctx, &shutdown), "no merge cycle");
    assert!(!has_prs(&ctx));
    assert!(
        ctx.discovery_caches
            .forge
            .get("github.com/octo/repo")
            .is_none()
    );
    assert!(
        !ctx.state
            .lock()
            .expect("state")
            .snapshot()
            .contains_key("forge"),
        "no tick recorded"
    );

    // Without a shutdown the same poll merges its PRs.
    let temp = tempfile::tempdir().expect("tempdir");
    let ctx = forge_ctx(temp.path(), SlowGh::default());
    assert!(forge_tick(&ctx, &ctx.shutdown));
    assert!(has_prs(&ctx));
}

#[test]
fn a_refresh_whose_fetch_a_shutdown_cut_short_does_not_publish() {
    let temp = tempfile::tempdir().expect("tempdir");
    let shutdown: Arc<AtomicBool> = Arc::default();
    let gh = SlowGh {
        stops: Some(Arc::clone(&shutdown)),
        ..SlowGh::default()
    };
    let ctx = forge_ctx_with(temp.path(), gh, shutdown);

    let response = handle_refresh(&refresh_request("forge"), &ctx);

    assert_eq!(response.result, "error");
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some("shutting_down")
    );
    assert!(!has_prs(&ctx));
}
