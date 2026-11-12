use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::live_store::{LiveEntry, LiveTmux, PidStatus};
use crate::model::live::{LiveConfidence, LiveObservation, LiveSource, LiveState, Liveness};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, MuxSessionId, MuxSessionNode};

const NOW_MS: i64 = 1_790_769_600_000;
const STORE: &str = "/state/conspectus/live-state.json";
const PID: i64 = 4242;
const PID_START: u64 = 777;

/// A store file the test rewrites, counting the stats and reads the
/// overlay makes.
#[derive(Clone, Default)]
struct Files(Rc<RefCell<FilesState>>);

#[derive(Default)]
struct FilesState {
    /// The file's text; `None` when there is no file.
    text: Option<String>,
    /// Bumped by every rewrite, standing for the new inode.
    generation: u64,
    stats: usize,
    reads: usize,
}

impl Files {
    fn holding(text: &str) -> Self {
        let files = Self::default();
        files.write(text);
        files
    }

    /// Rename a new file with `text` into place.
    fn write(&self, text: &str) {
        let mut state = self.0.borrow_mut();
        state.text = Some(text.to_string());
        state.generation += 1;
    }

    fn remove(&self) {
        self.0.borrow_mut().text = None;
    }

    fn reads(&self) -> usize {
        self.0.borrow().reads
    }

    fn stats(&self) -> usize {
        self.0.borrow().stats
    }
}

impl StoreFiles for Files {
    fn stamp(&self, path: &Path) -> FileStamp {
        assert_eq!(path, Path::new(STORE));
        let mut state = self.0.borrow_mut();
        state.stats += 1;
        match &state.text {
            None => FileStamp::Missing,
            Some(text) => FileStamp::Present {
                modified: None,
                len: text.len() as u64,
                inode: state.generation,
            },
        }
    }

    fn read(&self, path: &Path) -> Option<String> {
        assert_eq!(path, Path::new(STORE));
        let mut state = self.0.borrow_mut();
        state.reads += 1;
        state.text.clone()
    }
}

/// A process table where only [`PID`] runs, counting the checks made.
#[derive(Clone, Default)]
struct Pids(Rc<RefCell<usize>>);

impl Pids {
    fn checks(&self) -> usize {
        *self.0.borrow()
    }
}

impl PidProbe for Pids {
    fn status(&self, pid: i64) -> PidStatus {
        *self.0.borrow_mut() += 1;
        if pid == PID {
            PidStatus::Running {
                start: Some(PID_START),
            }
        } else {
            PidStatus::Gone
        }
    }
}

fn overlay(files: &Files, pids: &Pids) -> LiveOverlay {
    LiveOverlay::over(STORE, Box::new(files.clone()), Box::new(pids.clone()))
}

fn session_id(key: &str) -> AgentSessionId {
    AgentSessionId::new("claude-code", "/home/me/.claude", key)
}

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(session_id(key))
}

fn mux(name: &str) -> NodeId {
    NodeId::MuxSession(MuxSessionId::new(format!("tmux:{name}")))
}

/// A graph with these sessions and the tmux mux `work`.
fn graph(sessions: &[&str]) -> GraphSnapshot {
    let mut nodes: Vec<GraphNode> = sessions
        .iter()
        .map(|key| GraphNode::AgentSession(AgentSessionNode::new(session_id(key), "claude-code")))
        .collect();
    nodes.push(GraphNode::MuxSession(MuxSessionNode::new(
        MuxSessionId::new("tmux:work"),
        "tmux",
        "work",
    )));
    GraphSnapshot {
        nodes,
        ..GraphSnapshot::empty()
    }
}

/// An entry for session `key` in `state`, observed `minutes_ago`, by the
/// running process, in a pane of `work`.
fn entry(key: &str, state: LiveState, minutes_ago: i64) -> LiveEntry {
    let observed = NOW_MS - minutes_ago * 60_000;
    let mut entry = LiveEntry::new("claude-code", key, observed);
    entry.observation =
        LiveObservation::new(state, LiveSource::Hook, observed).with_pid(PID, Some(PID_START));
    entry.tmux = Some(LiveTmux {
        socket: Some("/tmp/tmux-1000/default".into()),
        session: Some("work".into()),
        pane: Some("%1".into()),
    });
    entry
}

/// The store's text with `entries`, as a writer leaves it.
fn store(entries: impl IntoIterator<Item = LiveEntry>) -> String {
    let mut file = LiveFile::default();
    for (rev, mut entry) in (1..).zip(entries) {
        entry.rev = rev;
        file.rev = rev;
        file.entries.insert(entry.key(), entry);
    }
    file.render().expect("the store renders")
}

fn states(table: &BTreeMap<NodeId, LiveBinding>) -> Vec<(NodeId, LiveState)> {
    table
        .iter()
        .map(|(id, binding)| (id.clone(), binding.observation.state))
        .collect()
}

#[test]
fn an_overlay_that_is_off_reads_the_snapshots_own_table() {
    let mut snapshot = graph(&["s1"]);
    snapshot.node_live.insert(
        session("s1"),
        LiveBinding::new(
            LiveObservation::new(LiveState::Ready, LiveSource::Hook, NOW_MS),
            Liveness::Unknown,
        ),
    );
    let mut off = LiveOverlay::off();

    assert!(!off.is_on());
    assert!(!off.poll(Instant::now(), Some(&snapshot)));
    off.rebind(&snapshot, Instant::now());
    assert_eq!(off.table(&snapshot), &snapshot.node_live);
}

#[test]
fn the_rows_read_the_overlays_table_in_place_of_the_snapshots() {
    let mut snapshot = graph(&["s1"]);
    snapshot.node_live.insert(
        session("s1"),
        LiveBinding::new(
            LiveObservation::new(LiveState::Ready, LiveSource::Hook, NOW_MS),
            Liveness::Unknown,
        ),
    );
    let files = Files::holding(&store([entry("s1", LiveState::Waiting, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);

    overlay.rebind(&snapshot, Instant::now());

    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Waiting)]
    );
    assert_eq!(
        overlay.table(&snapshot)[&session("s1")].liveness,
        Liveness::Alive,
        "the process was checked when the store was bound"
    );
}

#[test]
fn an_unchanged_store_is_stat_ed_every_tick_and_read_once() {
    let files = Files::holding(&store([entry("s1", LiveState::Working, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();

    assert!(
        overlay.poll(start, Some(&snapshot)),
        "the first tick binds it"
    );
    let (reads, checks) = (files.reads(), pids.checks());
    for tick in 1..=100 {
        let now = start + Duration::from_millis(100 * tick);
        assert!(!overlay.poll(now, Some(&snapshot)));
    }

    assert_eq!(files.stats(), 101, "one stat a tick");
    assert_eq!(files.reads(), reads, "an unchanged store is not read again");
    assert_eq!(pids.checks(), checks, "nor are its processes checked again");
    assert_eq!((reads, checks), (1, 1));
}

#[test]
fn a_rewrite_within_a_second_of_the_last_read_waits_for_the_second() {
    let files = Files::holding(&store([entry("s1", LiveState::Working, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();
    overlay.poll(start, Some(&snapshot));

    files.write(&store([entry("s1", LiveState::Waiting, 0)]));
    for millis in [100, 500, 999] {
        assert!(
            !overlay.poll(start + Duration::from_millis(millis), Some(&snapshot)),
            "{millis} ms after the last read"
        );
    }
    assert_eq!(files.reads(), 1);
    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Working)]
    );

    assert!(overlay.poll(start + REREAD_INTERVAL, Some(&snapshot)));
    assert_eq!(files.reads(), 2);
    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Waiting)]
    );
}

#[test]
fn a_rewrite_after_the_second_is_read_on_the_next_tick() {
    let files = Files::holding(&store([entry("s1", LiveState::Working, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();
    overlay.poll(start, Some(&snapshot));

    files.write(&store([entry("s1", LiveState::Ready, 0)]));
    assert!(overlay.poll(start + Duration::from_secs(5), Some(&snapshot)));
    assert_eq!(files.reads(), 2);
    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Ready)]
    );
}

#[test]
fn a_rewrite_that_binds_the_same_is_no_change() {
    let entries = || {
        let mut elsewhere = entry("gone", LiveState::Ready, 0);
        elsewhere.tmux = None;
        [entry("s1", LiveState::Working, 0), elsewhere]
    };
    let files = Files::holding(&store(entries()));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();
    overlay.poll(start, Some(&snapshot));

    // Only the entry that binds nowhere changed.
    let mut rewritten = entries();
    rewritten[1].observation.state = LiveState::Error;
    files.write(&store(rewritten));

    assert!(!overlay.poll(start + Duration::from_secs(2), Some(&snapshot)));
    assert_eq!(files.reads(), 2, "the rewrite was read");
}

#[test]
fn a_missing_store_is_an_empty_table_and_nothing_is_read() {
    let files = Files::default();
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let mut snapshot = graph(&["s1"]);
    // The snapshot's own table gives way to the overlay's, empty or not.
    snapshot.node_live.insert(
        session("s1"),
        LiveBinding::new(
            LiveObservation::new(LiveState::Working, LiveSource::Hook, NOW_MS),
            Liveness::Unknown,
        ),
    );

    overlay.rebind(&snapshot, Instant::now());
    assert!(!overlay.poll(Instant::now(), Some(&snapshot)));

    assert!(overlay.table(&snapshot).is_empty());
    assert_eq!(files.reads(), 0, "a missing file is not opened");
    assert_eq!(pids.checks(), 0);
}

#[test]
fn a_store_that_goes_missing_empties_the_table() {
    let files = Files::holding(&store([entry("s1", LiveState::Working, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();
    overlay.poll(start, Some(&snapshot));

    files.remove();

    assert!(overlay.poll(start + REREAD_INTERVAL, Some(&snapshot)));
    assert!(overlay.table(&snapshot).is_empty());
}

#[test]
fn a_corrupt_or_newer_store_is_an_empty_table() {
    let files = Files::holding("{ not json");
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();

    overlay.poll(start, Some(&snapshot));
    assert!(overlay.table(&snapshot).is_empty(), "corrupt");

    files.write(&store([entry("s1", LiveState::Working, 0)]));
    assert!(overlay.poll(start + REREAD_INTERVAL, Some(&snapshot)));
    assert_eq!(
        overlay.table(&snapshot).len(),
        1,
        "the repaired store binds"
    );

    files.write(r#"{ "schema_version": 999, "entries": {} }"#);
    assert!(overlay.poll(start + 2 * REREAD_INTERVAL, Some(&snapshot)));
    assert!(overlay.table(&snapshot).is_empty(), "a newer schema");
}

#[test]
fn a_store_read_before_the_first_snapshot_binds_when_it_arrives() {
    let files = Files::holding(&store([entry("s1", LiveState::Waiting, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let start = Instant::now();

    assert!(!overlay.poll(start, None), "nothing to bind to yet");
    assert_eq!(files.reads(), 1);

    let snapshot = graph(&["s1"]);
    overlay.rebind(&snapshot, start + Duration::from_millis(100));
    assert_eq!(files.reads(), 1, "the store read is reused");
    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Waiting)]
    );
}

#[test]
fn the_first_snapshot_reads_a_store_never_read() {
    let files = Files::holding(&store([entry("s1", LiveState::Ready, 0)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);

    overlay.rebind(&snapshot, Instant::now());

    assert_eq!(files.reads(), 1);
    assert_eq!(
        states(overlay.table(&snapshot)),
        vec![(session("s1"), LiveState::Ready)]
    );
}

#[test]
fn a_new_snapshot_rebinds_the_store_to_its_own_nodes() {
    let mut old = entry("old", LiveState::Ready, 0);
    old.tmux = None;
    let files = Files::holding(&store([entry("new", LiveState::Starting, 0), old]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let start = Instant::now();

    // No session `new` yet: its entry binds to the mux it runs in.
    let before = graph(&["old"]);
    overlay.rebind(&before, start);
    assert_eq!(
        states(overlay.table(&before)),
        vec![
            (session("old"), LiveState::Ready),
            (mux("work"), LiveState::Starting),
        ]
    );

    // The next snapshot has `new` and no longer has `old`.
    let after = graph(&["new"]);
    overlay.rebind(&after, start + Duration::from_millis(10));
    assert_eq!(
        states(overlay.table(&after)),
        vec![(session("new"), LiveState::Starting)],
        "nothing stays keyed to the nodes of the snapshot replaced"
    );
    assert_eq!(files.reads(), 1, "the swap binds what was read");
}

#[test]
fn time_alone_moves_a_state_and_is_checked_at_most_once_a_second() {
    // Working, with the last evidence just inside the quiet window.
    let files = Files::holding(&store([entry("s1", LiveState::Working, 9)]));
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&["s1"]);
    let start = Instant::now();
    overlay.rebind(&snapshot, start);
    let (reads, checks) = (files.reads(), pids.checks());

    assert!(overlay.aged(&snapshot, start, NOW_MS), "the first check");
    assert!(
        !overlay.aged(
            &snapshot,
            start + Duration::from_millis(500),
            NOW_MS + 120_000
        ),
        "a second check within the second is skipped, whatever the time"
    );
    assert!(
        !overlay.aged(&snapshot, start + AGE_CHECK_INTERVAL, NOW_MS + 1_000),
        "still fresh"
    );
    assert!(
        overlay.aged(&snapshot, start + 2 * AGE_CHECK_INTERVAL, NOW_MS + 120_000),
        "past the quiet window"
    );
    let cells = cells_at(overlay.table(&snapshot), NOW_MS + 120_000);
    assert_eq!(cells[&session("s1")].state, LiveState::Working);
    assert_eq!(cells[&session("s1")].confidence, LiveConfidence::Quiet);

    assert_eq!(
        (files.reads(), pids.checks()),
        (reads, checks),
        "time is read off the bound table alone"
    );
}

#[test]
fn an_empty_table_never_ages() {
    let files = Files::default();
    let pids = Pids::default();
    let mut overlay = overlay(&files, &pids);
    let snapshot = graph(&[]);
    let start = Instant::now();
    overlay.rebind(&snapshot, start);

    for second in 0..5 {
        assert!(!overlay.aged(
            &snapshot,
            start + Duration::from_secs(second),
            NOW_MS + 3_600_000 * i64::try_from(second).unwrap()
        ));
    }
}
