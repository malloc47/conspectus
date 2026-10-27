use std::collections::VecDeque;
use std::fs;
use std::time::{Duration, Instant};

use super::*;
use crate::content_search::{ContentEvent, ContentResult, Progress};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot};
use crate::tui::app::SnapshotHandle;
use crate::tui::{ContentStatus, RunConfig};
use crate::viewer::search::ContentKinds;

/// Events handed out one poll at a time, then silence or a hang-up.
struct Script {
    events: VecDeque<Poll>,
    hangs_up: bool,
}

impl Script {
    fn of(events: Vec<ContentEvent>) -> Self {
        Self {
            events: events
                .into_iter()
                .map(|e| Poll::Event(Box::new(e)))
                .collect(),
            hangs_up: false,
        }
    }
}

impl EventSource for Script {
    fn poll(&mut self) -> Poll {
        match self.events.pop_front() {
            Some(poll) => poll,
            None if self.hangs_up => Poll::Disconnected,
            None => Poll::Empty,
        }
    }
}

fn id(root: &str, key: &str) -> AgentSessionId {
    AgentSessionId::new("claude-code", root, key)
}

fn app_with(ids: &[AgentSessionId]) -> App {
    let snapshot = GraphSnapshot {
        nodes: ids
            .iter()
            .map(|id| GraphNode::AgentSession(AgentSessionNode::new(id.clone(), "claude-code")))
            .collect(),
        ..GraphSnapshot::empty()
    };
    let mut app = App::new(RunConfig::defaults());
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 0,
        initial_selection_hint: None,
    });
    app
}

fn search(app: &mut App, text: &str) -> u64 {
    app.update(Msg::StartContentSearch {
        text: text.into(),
        kinds: ContentKinds::default(),
    });
    app.take_scan_request().expect("a scan was requested");
    app.content_generation().expect("a search is active")
}

fn driver_running(source: Script, generation: u64) -> ContentDriver {
    ContentDriver {
        active: Some(Active {
            generation,
            handle: Box::new(source),
            last_flush: Instant::now(),
            batch: Vec::new(),
            pane_batch: Vec::new(),
            progress: Progress::default(),
            done: false,
        }),
        details: None,
        detail_reader: None,
        pane_backend: None,
    }
}

fn session_event(generation: u64, key: &str) -> ContentEvent {
    ContentEvent::Result {
        generation,
        result: ContentResult::Session {
            key: SessionKey {
                harness_key: "claude-code".into(),
                session_key: key.into(),
            },
            hits: crate::viewer::search::SessionHits {
                count: 1,
                best: crate::viewer::search::Hit {
                    turn_index: 0,
                    role: crate::viewer::model::TurnRole::User,
                    kind: crate::viewer::model::TurnKind::Message,
                    timestamp: None,
                    snippet: "a needle".into(),
                    highlight: 2..8,
                },
                latest: None,
            },
        },
    }
}

fn done_event(generation: u64) -> ContentEvent {
    ContentEvent::Done {
        generation,
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        matched: 1,
        cancelled: false,
    }
}

fn found(app: &App) -> usize {
    app.content().map_or(0, |content| content.hits().len())
}

#[test]
fn results_reach_the_app_no_more_often_than_the_flush_interval() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let generation = search(&mut app, "needle");
    let mut driver = driver_running(Script::of(vec![session_event(generation, "a")]), generation);

    driver.pump(&mut app);
    assert_eq!(found(&app), 0, "a result waits for the next flush");

    driver.active.as_mut().unwrap().last_flush = Instant::now() - FLUSH_EVERY * 2;
    driver.pump(&mut app);
    assert_eq!(
        found(&app),
        1,
        "once the interval has passed it is delivered"
    );
}

#[test]
fn the_end_of_a_scan_is_delivered_at_once() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let generation = search(&mut app, "needle");
    let mut driver = driver_running(
        Script::of(vec![session_event(generation, "a"), done_event(generation)]),
        generation,
    );

    driver.pump(&mut app);

    assert_eq!(found(&app), 1);
    assert!(matches!(
        app.content().unwrap().status,
        ContentStatus::Done(_)
    ));
    assert!(driver.active.is_none(), "a finished scan is let go");
}

#[test]
fn events_of_an_older_scan_are_ignored() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let generation = search(&mut app, "needle");
    let mut driver = driver_running(
        Script::of(vec![session_event(generation - 1, "a")]),
        generation,
    );
    driver.active.as_mut().unwrap().last_flush = Instant::now() - FLUSH_EVERY * 2;

    driver.pump(&mut app);

    assert_eq!(found(&app), 0);
}

#[test]
fn a_scan_the_app_no_longer_wants_is_dropped() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let generation = search(&mut app, "needle");
    let mut driver = driver_running(Script::of(vec![session_event(generation, "a")]), generation);

    app.update(Msg::ClearContentSearch);
    driver.pump(&mut app);

    assert!(driver.active.is_none());
}

#[test]
fn a_scan_that_vanishes_without_saying_so_still_ends() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let generation = search(&mut app, "needle");
    let mut source = Script::of(vec![session_event(generation, "a")]);
    source.hangs_up = true;
    let mut driver = driver_running(source, generation);

    driver.pump(&mut app);

    assert!(
        matches!(app.content().unwrap().status, ContentStatus::Done(_)),
        "the search does not stay on Searching forever"
    );
    assert_eq!(found(&app), 1, "what it found is kept");
    assert!(driver.active.is_none());
}

#[test]
fn a_new_request_replaces_a_finished_scan() {
    let ids = [id("/s", "a")];
    let mut app = app_with(&ids);
    let mut driver = ContentDriver::default();
    app.update(Msg::StartContentSearch {
        text: "needle".into(),
        kinds: ContentKinds::default(),
    });

    driver.pump(&mut app);

    // A scan this small can finish inside the first pump, so "started"
    // is either still running or already delivered.
    assert!(
        driver.active.is_some()
            || app
                .content()
                .is_some_and(crate::tui::app::ContentSearch::status_is_done),
        "the requested scan started"
    );
    assert!(app.take_scan_request().is_none(), "the request was taken");
}

fn detail_app(root: &std::path::Path) -> (App, u64) {
    let ids = [id(&root.display().to_string(), "a")];
    let mut app = app_with(&ids);
    // Per-message results are read for the sessions open in the Search
    // view.
    app.update(Msg::SwitchView(crate::tui::View::Search));
    let generation = search(&mut app, "needle");
    app.update(Msg::SetContentGroup(
        crate::viewer::search::Grouping::Message,
    ));
    app.update(Msg::ContentHits {
        generation,
        batch: vec![match session_event(generation, "a") {
            ContentEvent::Result {
                result: ContentResult::Session { key, hits },
                ..
            } => (key, hits),
            _ => unreachable!(),
        }],
        progress: Progress {
            scanned: 1,
            total: 1,
            skipped: 0,
        },
        done: true,
    });
    (app, generation)
}

/// A reader that reports the thread it runs on, then waits until the
/// test lets it go, so the test decides when a read is in flight.
struct Gate {
    reader: DetailReader,
    started: std::sync::mpsc::Receiver<std::thread::ThreadId>,
    release: std::sync::mpsc::Sender<()>,
}

fn gated_reader() -> Gate {
    let (started_tx, started) = std::sync::mpsc::channel();
    let (release, release_rx) = std::sync::mpsc::channel::<()>();
    let started_tx = std::sync::Mutex::new(started_tx);
    let release_rx = std::sync::Mutex::new(release_rx);
    let reader: DetailReader = Arc::new(move |_session, _query| {
        let _ = started_tx.lock().unwrap().send(std::thread::current().id());
        let _ = release_rx.lock().unwrap().recv();
        Vec::new()
    });
    Gate {
        reader,
        started,
        release,
    }
}

#[test]
fn matching_turns_are_read_on_another_thread_and_delivered() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, generation) = detail_app(dir.path());
    let gate = gated_reader();
    let mut driver = ContentDriver {
        detail_reader: Some(gate.reader.clone()),
        ..ContentDriver::default()
    };

    driver.pump(&mut app);

    // The reader is held, so the read is certainly in flight.
    let active = driver.details.as_ref().expect("a read started");
    assert_eq!(active.generation, generation);
    let reader_thread = gate
        .started
        .recv_timeout(Duration::from_secs(10))
        .expect("the reader ran");
    assert_ne!(
        reader_thread,
        std::thread::current().id(),
        "the read is not on the UI thread"
    );

    // Let it finish; once its thread has ended the channel is complete.
    gate.release.send(()).unwrap();
    let worker = driver.details.as_mut().unwrap().thread.take().unwrap();
    worker.join().unwrap();
    driver.details.as_mut().unwrap().last_flush = Instant::now() - FLUSH_EVERY * 2;
    driver.pump(&mut app);
    assert!(driver.details.is_none(), "the read ended");
}

#[test]
fn a_new_search_stops_the_turn_reads_of_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = detail_app(dir.path());
    let gate = gated_reader();
    let mut driver = ContentDriver {
        detail_reader: Some(gate.reader.clone()),
        ..ContentDriver::default()
    };
    driver.pump(&mut app);
    gate.started
        .recv_timeout(Duration::from_secs(10))
        .expect("the reader ran");
    let cancel = driver
        .details
        .as_ref()
        .expect("a read started")
        .cancel
        .clone();

    app.update(Msg::ClearContentSearch);
    driver.pump(&mut app);

    assert!(cancel.is_cancelled(), "the old read was told to stop");
    // The Search view still shows the cleared search, so its turns are
    // read again, as a remembered search's.
    assert!(
        driver
            .details
            .as_ref()
            .is_none_or(|details| details.serial.is_some()),
        "the read of the running search is not kept"
    );
    gate.release.send(()).unwrap();
}

fn claude_transcript(root: &std::path::Path, key: &str, body: &str) {
    let dir = root.join("projects/-p");
    fs::create_dir_all(&dir).unwrap();
    let record = format!(
        r#"{{"cwd":"/p","sessionId":"{key}","type":"user","uuid":"u1","parentUuid":null,"timestamp":"2026-10-01T10:00:00.000Z","message":{{"role":"user","content":[{{"type":"text","text":"{body}"}}]}}}}"#
    );
    fs::write(dir.join(format!("{key}.jsonl")), record + "\n").unwrap();
}

#[test]
fn a_blocking_run_finishes_the_search_before_it_returns() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("state");
    claude_transcript(&root, "a", "the needle is here");
    claude_transcript(&root, "b", "nothing to see");
    let mut app = app_with(&[
        id(&root.display().to_string(), "a"),
        id(&root.display().to_string(), "b"),
    ]);
    search(&mut app, "needle");
    // `search` took the request; ask again so the blocking run sees one.
    app.update(Msg::RescanContent);

    run_blocking(&mut app, None);

    let content = app.content().unwrap();
    assert!(content.status_is_done());
    assert_eq!(content.hits().len(), 1);
    assert!(content.hits().get("claude-code", "a").is_some());
}

#[test]
fn a_blocking_run_rebases_the_recorded_state_paths() {
    let dir = tempfile::tempdir().unwrap();
    let written = dir.path().join("world");
    let root = written.join("state");
    claude_transcript(&root, "a", "the needle is here");
    // The scenario recorded its state under `/recorded`.
    let mut app = app_with(&[id("/recorded/state", "a")]);
    search(&mut app, "needle");
    app.update(Msg::RescanContent);

    run_blocking(&mut app, Some((Path::new("/recorded"), &written)));

    assert_eq!(app.content().unwrap().hits().len(), 1);
}
