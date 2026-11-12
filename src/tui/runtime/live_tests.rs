//! The live loop keeps live state current between snapshots: a write to
//! the store reaches the rows within about a second without a discovery,
//! whether snapshots come from a daemon or from local discovery, and
//! every new snapshot is bound again.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::live_store::{
    LiveEvent, LiveEventKind, LiveStore, LiveTmux, ProcProbe, WaitKind, now_ms,
};
use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState,
    MuxSessionNode, NodeId, Provenance, RelationKind,
};
use crate::tui::live_overlay::{LiveOverlay, REREAD_INTERVAL, SystemFiles};
use crate::tui::rows::RowKind;

/// How long a test waits for the loop before it fails: the re-read
/// interval and room for a slow machine.
const PATIENCE: Duration = Duration::from_secs(5);

const HARNESS: &str = "claude-code";
const SESSION: &str = "0b8e8f8a-1c2d-4e5f-8a9b-0c1d2e3f4a5b";
const NEWCOMER: &str = "7c6d5e4f-3a2b-4c1d-9e8f-7a6b5c4d3e2f";

struct NeverAsked;

impl handoff::IncarnationProbe for NeverAsked {
    fn still_there(&self, _target: &crate::tui::app::MuxIncarnationTarget) -> bool {
        unreachable!("no hand-off happens here")
    }
}

/// The live loop with an overlay over the store in `dir`, and a discovery
/// that counts its calls.
struct Rig {
    mode: LiveMode,
    app: App,
    discoveries: Arc<AtomicUsize>,
    store: LiveStore,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a scratch state directory");
        let store = LiveStore::at(dir.path().join("conspectus"));
        let (result_tx, result_rx) = mpsc::channel();
        let (preview_tx, preview_rx) = mpsc::channel();
        let (confirmed_tx, confirmed_rx) = mpsc::channel();
        let discoveries = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&discoveries);
        let discover = move |_: &mut RunConfig, _: DaemonNudge| {
            counted.fetch_add(1, Ordering::SeqCst);
            Discovered {
                snapshot: Ok(GraphSnapshot::empty()),
                from_daemon: false,
                warning: None,
            }
        };
        let mode = LiveMode {
            tmux: Box::new(crate::discovery::tmux::FakeTmux::with_sessions("")),
            result_tx,
            result_rx,
            preview_tx,
            preview_rx,
            confirmed_tx,
            confirmed_rx,
            probe: Arc::new(NeverAsked),
            discover: Arc::new(discover),
            workers: WorkerLedger::default(),
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(3600),
            forge: ForgeWorker::new(),
            visibility: None,
            content: content::ContentDriver::default(),
        };
        let mut app = App::new(RunConfig::defaults());
        app.set_live_overlay(LiveOverlay::over(
            store.path(),
            Box::new(SystemFiles),
            Box::new(ProcProbe::system()),
        ));
        Self {
            mode,
            app,
            discoveries,
            store,
            _dir: dir,
        }
    }

    /// A refresh lands `snapshot`, from a daemon or a local discovery.
    fn lands(&mut self, snapshot: GraphSnapshot, from_daemon: bool) {
        let generation = self.mode.workers.start();
        self.mode
            .result_tx
            .send(DiscoveryResult {
                generation,
                snapshot: Ok(snapshot),
                warning: None,
                from_daemon,
            })
            .unwrap();
        self.drain();
    }

    fn drain(&mut self) {
        let config = self.app.config().clone();
        self.mode.drain(&mut self.app, &config).unwrap();
    }

    /// Drain until the row of `session` shows `state`, and return how
    /// long that took.
    fn drain_until_state(&mut self, session: &str, state: Option<LiveState>) -> Duration {
        let start = Instant::now();
        loop {
            self.drain();
            if row_state(&self.app, session) == Some(state) {
                return start.elapsed();
            }
            assert!(
                start.elapsed() < PATIENCE,
                "timed out waiting for {session} to show {state:?}; it shows {:?}",
                row_state(&self.app, session)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn write(&self, key: &str, kind: LiveEventKind) {
        let event = LiveEvent::new(HARNESS, key, now_ms(), kind);
        self.store
            .apply_event(&event)
            .expect("the store takes the event");
    }
}

fn session_id(key: &str) -> AgentSessionId {
    AgentSessionId::new(HARNESS, "/home/me/.claude", key)
}

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(session_id(key))
}

fn mux_id() -> MuxSessionId {
    MuxSessionId::new("tmux:work")
}

/// The tmux mux `work` hosting these sessions.
fn world(sessions: &[&str]) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            mux_id(),
            "tmux",
            "work",
        )));
    for key in sessions {
        let mut node = AgentSessionNode::new(session_id(key), HARNESS);
        node.cwd = Some("/home/me/src/project".into());
        snapshot.nodes.push(GraphNode::AgentSession(node));
        let mut link = GraphLink::new(
            format!("hosted-{key}"),
            session(key),
            LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id()),
            },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );
        link.state = LinkState::Active;
        snapshot.candidate_links.push(link);
    }
    crate::resolve::resolve_snapshot(snapshot)
}

/// What the row of `key` carries: `None` when there is no such row,
/// `Some(None)` when the row has no live state.
fn row_state(app: &App, key: &str) -> Option<Option<LiveState>> {
    app.tree().rows.iter().find_map(|row| match &row.kind {
        RowKind::AgentSession(row) if row.primary_node == session(key) => {
            Some(row.live.map(|cell| cell.state))
        }
        _ => None,
    })
}

fn in_work_pane(key: &str) -> LiveEvent {
    LiveEvent::new(
        HARNESS,
        key,
        now_ms(),
        LiveEventKind::SessionStarted {
            after_compaction: false,
        },
    )
    .with_context(crate::live_store::LiveContext {
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: Some("work".into()),
            pane: Some("%7".into()),
        }),
        ..Default::default()
    })
}

/// A write to the store reaches the session's row on its own, with the
/// snapshot from `from_daemon`.
fn a_store_write_reaches_the_rows_without_a_discovery(from_daemon: bool) {
    let mut rig = Rig::new();
    let mut snapshot = world(&[SESSION]);
    // What the snapshot's discovery bound: the overlay supersedes it.
    snapshot.node_live.insert(
        session(SESSION),
        LiveBinding::new(
            LiveObservation::new(LiveState::Ready, LiveSource::Hook, now_ms()),
            Liveness::Unknown,
        ),
    );
    rig.lands(snapshot, from_daemon);
    assert_eq!(
        row_state(&rig.app, SESSION),
        Some(None),
        "the store is empty, and the overlay's table stands in for the snapshot's"
    );

    rig.write(SESSION, LiveEventKind::TurnStarted);
    let took = rig.drain_until_state(SESSION, Some(LiveState::Working));
    assert!(
        took <= REREAD_INTERVAL + Duration::from_millis(500),
        "reached the row in {took:?}"
    );

    rig.write(
        SESSION,
        LiveEventKind::WaitStarted {
            agent: None,
            kind: WaitKind::Permission,
        },
    );
    rig.drain_until_state(SESSION, Some(LiveState::Waiting));

    assert_eq!(
        rig.discoveries.load(Ordering::SeqCst),
        0,
        "no discovery ran"
    );
    assert!(rig.mode.workers.idle(), "no refresh was started");
}

#[test]
fn a_store_write_reaches_the_rows_without_a_daemon() {
    a_store_write_reaches_the_rows_without_a_discovery(false);
}

#[test]
fn a_store_write_reaches_the_rows_beside_a_daemon() {
    a_store_write_reaches_the_rows_without_a_discovery(true);
}

#[test]
fn a_removed_or_corrupt_store_leaves_the_rows_without_live_state_and_says_nothing() {
    let mut rig = Rig::new();
    rig.write(SESSION, LiveEventKind::TurnStarted);
    rig.lands(world(&[SESSION]), false);
    assert_eq!(row_state(&rig.app, SESSION), Some(Some(LiveState::Working)));

    std::fs::remove_file(rig.store.path()).unwrap();
    rig.drain_until_state(SESSION, None);

    rig.write(SESSION, LiveEventKind::TurnStarted);
    rig.drain_until_state(SESSION, Some(LiveState::Working));
    std::fs::write(rig.store.path(), "{ not a store").unwrap();
    rig.drain_until_state(SESSION, None);

    assert!(rig.app.messages().is_empty(), "nothing is logged");
    assert!(rig.app.toasts().is_empty(), "nothing is toasted");
    assert_eq!(rig.app.status_message(), None);
}

#[test]
fn a_new_snapshot_binds_the_store_to_its_own_nodes() {
    let mut rig = Rig::new();
    rig.store
        .apply_event(&in_work_pane(NEWCOMER))
        .expect("the store takes the event");

    // No session node yet: the agent shows on the mux it runs in.
    rig.lands(world(&[SESSION]), true);
    assert_eq!(row_state(&rig.app, NEWCOMER), None, "no row for it yet");
    let snapshot = rig.app.snapshot_handle().unwrap().snapshot().clone();
    assert_eq!(
        rig.app
            .live_table(&snapshot)
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![NodeId::MuxSession(mux_id())]
    );

    // The next snapshot has its session, and the store has not changed.
    rig.lands(world(&[SESSION, NEWCOMER]), true);
    assert_eq!(row_state(&rig.app, NEWCOMER), Some(Some(LiveState::Ready)));
    let snapshot = rig.app.snapshot_handle().unwrap().snapshot().clone();
    assert_eq!(
        rig.app
            .live_table(&snapshot)
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![session(NEWCOMER)],
        "the entry moved to its session"
    );
    assert_eq!(rig.discoveries.load(Ordering::SeqCst), 0);
}
