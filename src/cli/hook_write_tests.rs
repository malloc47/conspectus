use std::time::Duration;

use super::*;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::live_store::{
    LIVE_STATE_FILE, LIVE_STATE_LOCK, LiveEntry, LiveEvent, LiveEventKind, LiveFile,
};
use crate::model::live::LiveState;

/// A harness whose payloads map to one live event of `kind` (none when
/// `None`), and which records an event for attribution when
/// `attribution` is set.
struct FakeHarness {
    attribution: bool,
    kind: Option<LiveEventKind>,
}

impl FakeHarness {
    fn state_event() -> Self {
        Self {
            attribution: false,
            kind: Some(LiveEventKind::ToolFinished { agent: None }),
        }
    }

    fn session_start() -> Self {
        Self {
            attribution: true,
            kind: Some(LiveEventKind::SessionStarted {
                after_compaction: false,
            }),
        }
    }
}

impl HarnessAdapter for FakeHarness {
    fn harness_key(&self) -> &'static str {
        "fake"
    }

    fn discover(&self, _context: &DiscoveryContext) -> anyhow::Result<GraphFragment> {
        Ok(GraphFragment::empty())
    }

    fn is_attribution_event(&self, _payload: &serde_json::Value) -> bool {
        self.attribution
    }

    fn live_events_from_payload(
        &self,
        payload: &serde_json::Value,
        context: &LiveContext,
        observed_ms: i64,
    ) -> Vec<LiveEvent> {
        let Some(kind) = self.kind.clone() else {
            return Vec::new();
        };
        let session = payload["session_id"].as_str().unwrap_or("s-1");
        vec![LiveEvent::new("fake", session, observed_ms, kind).with_context(context.clone())]
    }
}

/// Stands in for tmux and the attribution path, and remembers what the
/// writer asked of them.
struct Recorder {
    /// What tmux answers.
    mux: Option<HookTmuxRecord>,
    mux_calls: usize,
    attributed: Vec<HookRecord>,
    /// The store as the attribution path found it.
    store_at_attribution: Vec<LiveFile>,
    store: LiveStore,
}

impl Recorder {
    fn new(store: &LiveStore) -> Self {
        Self {
            mux: Some(HookTmuxRecord {
                session_name: Some("work".to_string()),
                native_id: None,
                pane_id: Some("%3".to_string()),
                socket_path: Some("/tmp/tmux-1000/default".to_string()),
            }),
            mux_calls: 0,
            attributed: Vec::new(),
            store_at_attribution: Vec::new(),
            store: store.clone(),
        }
    }
}

impl WriterEffects for Recorder {
    fn mux_context(&mut self) -> Option<HookTmuxRecord> {
        self.mux_calls += 1;
        self.mux.clone()
    }

    fn attribute(&mut self, record: &HookRecord) {
        self.store_at_attribution.push(self.store.read());
        self.attributed.push(record.clone());
    }
}

fn in_tmux() -> WriterEnv {
    WriterEnv {
        observed_ms: 1_000,
        process: Some(HarnessProcess {
            pid: 4242,
            ppid: Some(4241),
            start: Some(77),
            background: false,
        }),
        tmux: Some(TmuxEnv {
            socket: Some("/tmp/tmux-1000/default".to_string()),
            pane: Some("%3".to_string()),
        }),
        harness_version: None,
    }
}

fn payload(session: &str) -> serde_json::Value {
    serde_json::json!({ "session_id": session, "cwd": "/work" })
}

fn entry(store: &LiveStore, session: &str) -> LiveEntry {
    store
        .read()
        .get("fake", session)
        .cloned()
        .expect("the session has an entry")
}

fn work_pane() -> LiveTmux {
    LiveTmux {
        socket: Some("/tmp/tmux-1000/default".to_string()),
        session: Some("work".to_string()),
        pane: Some("%3".to_string()),
    }
}

#[test]
fn a_state_event_asks_tmux_only_the_first_time_its_session_is_seen() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);
    let harness = FakeHarness::state_event();

    write_payload(
        &harness,
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    assert_eq!(recorder.mux_calls, 1);
    assert_eq!(entry(&store, "s-1").tmux, Some(work_pane()));

    // tmux would now name the session differently: the entry keeps the
    // name it learned, because tmux is not asked again.
    recorder.mux = None;
    write_payload(
        &harness,
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    assert_eq!(recorder.mux_calls, 1);
    assert_eq!(entry(&store, "s-1").tmux, Some(work_pane()));
    assert!(
        recorder.attributed.is_empty(),
        "a state event is not attributed"
    );
}

#[test]
fn a_state_event_records_the_process_and_its_start_time() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);

    write_payload(
        &FakeHarness::state_event(),
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    let observation = entry(&store, "s-1").observation;
    assert_eq!(observation.pid, Some(4242));
    assert_eq!(observation.pid_start, Some(77));
    assert_eq!(observation.state, LiveState::Working);
}

#[test]
fn a_session_start_writes_live_state_before_its_attribution_and_asks_tmux_once() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);

    write_payload(
        &FakeHarness::session_start(),
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );

    assert_eq!(recorder.mux_calls, 1, "one answer serves both records");
    let [record] = recorder.attributed.as_slice() else {
        panic!("one attribution record: {:?}", recorder.attributed);
    };
    assert_eq!(record.harness_key, "fake");
    assert_eq!(record.session_key, "s-1");
    assert_eq!(record.pid, Some(4242));
    assert_eq!(record.ppid, Some(4241));
    assert_eq!(record.observed_epoch, 1);
    assert_eq!(
        record
            .tmux
            .as_ref()
            .and_then(|tmux| tmux.session_name.as_deref()),
        Some("work")
    );
    assert!(
        recorder.store_at_attribution[0]
            .get("fake", "s-1")
            .is_some(),
        "the live state was written first"
    );
    assert_eq!(entry(&store, "s-1").tmux, Some(work_pane()));
    assert_eq!(entry(&store, "s-1").observation.state, LiveState::Ready);
}

#[test]
fn the_pane_comes_from_the_environment_before_what_tmux_answers() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);
    let mut env = in_tmux();
    env.tmux = Some(TmuxEnv {
        socket: Some("/tmp/tmux-1000/other".to_string()),
        pane: Some("%9".to_string()),
    });

    write_payload(
        &FakeHarness::state_event(),
        &payload("s-1"),
        &env,
        Some(&store),
        &mut recorder,
    );
    assert_eq!(
        entry(&store, "s-1").tmux,
        Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/other".to_string()),
            session: Some("work".to_string()),
            pane: Some("%9".to_string()),
        })
    );
}

#[test]
fn outside_tmux_nothing_asks_tmux_for_a_state_event() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);
    let mut env = in_tmux();
    env.tmux = None;

    write_payload(
        &FakeHarness::state_event(),
        &payload("s-1"),
        &env,
        Some(&store),
        &mut recorder,
    );
    assert_eq!(recorder.mux_calls, 0);
    assert_eq!(entry(&store, "s-1").tmux, None);
}

#[test]
fn a_background_harness_records_no_pane_and_asks_tmux_nothing() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);
    let mut env = in_tmux();
    env.process = env.process.map(|process| HarnessProcess {
        background: true,
        ..process
    });

    for harness in [FakeHarness::state_event(), FakeHarness::session_start()] {
        write_payload(&harness, &payload("s-1"), &env, Some(&store), &mut recorder);
    }
    assert_eq!(recorder.mux_calls, 0);
    assert_eq!(entry(&store, "s-1").tmux, None);
    let [record] = recorder.attributed.as_slice() else {
        panic!("one attribution record: {:?}", recorder.attributed);
    };
    assert!(record.background_harness);
    assert_eq!(record.tmux, None);
}

#[test]
fn an_event_with_nothing_to_say_touches_nothing() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path().join("state"));
    let mut recorder = Recorder::new(&store);
    let harness = FakeHarness {
        attribution: false,
        kind: None,
    };

    write_payload(
        &harness,
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    assert_eq!(recorder.mux_calls, 0);
    assert!(recorder.attributed.is_empty());
    assert!(!store.dir().exists(), "the store was not created");
}

#[test]
fn a_busy_store_drops_the_event_and_the_session_start_is_still_attributed() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path()).with_lock_wait(Duration::from_millis(20));
    let mut recorder = Recorder::new(&store);
    let lock = fs::File::create(dir.path().join(LIVE_STATE_LOCK)).expect("lock file");
    lock.lock().expect("hold the lock");

    write_payload(
        &FakeHarness::session_start(),
        &payload("s-1"),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    drop(lock);
    assert!(
        !dir.path().join(LIVE_STATE_FILE).exists(),
        "the event was dropped"
    );
    assert_eq!(recorder.attributed.len(), 1);
}

#[test]
fn without_a_store_the_session_start_is_still_attributed() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);

    write_payload(
        &FakeHarness::session_start(),
        &payload("s-1"),
        &in_tmux(),
        None,
        &mut recorder,
    );
    assert_eq!(recorder.attributed.len(), 1);
    assert_eq!(recorder.mux_calls, 1);
}

#[test]
fn a_payload_the_adapter_cannot_build_a_record_from_is_not_attributed() {
    let dir = tempfile::tempdir().expect("state dir");
    let store = LiveStore::at(dir.path());
    let mut recorder = Recorder::new(&store);
    let harness = FakeHarness {
        attribution: true,
        kind: None,
    };

    write_payload(
        &harness,
        &serde_json::json!({"cwd": "/work"}),
        &in_tmux(),
        Some(&store),
        &mut recorder,
    );
    assert!(recorder.attributed.is_empty());
}

#[test]
fn the_tmux_pane_comes_from_tmux_and_tmux_pane() {
    assert_eq!(tmux_env(None, Some("%3".to_string())), None);
    assert_eq!(
        tmux_env(
            Some("/tmp/tmux-1000/default,4242,0".to_string()),
            Some("%3".to_string())
        ),
        Some(TmuxEnv {
            socket: Some("/tmp/tmux-1000/default".to_string()),
            pane: Some("%3".to_string()),
        })
    );
    assert_eq!(
        tmux_env(Some("garbled".to_string()), Some(String::new())),
        Some(TmuxEnv::default())
    );
}

#[test]
fn an_exported_pid_is_a_number_above_init() {
    assert_eq!(parse_exported_pid("4242"), Some(4242));
    assert_eq!(parse_exported_pid(" 4242\n"), Some(4242));
    for value in ["", "0", "1", "-5", "pid", "4242x"] {
        assert_eq!(parse_exported_pid(value), None, "{value:?}");
    }
}
