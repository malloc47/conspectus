//! The event loop, run against a scripted console: what it does with
//! input that arrives faster than frames draw (ADR 0236).
//!
//! Time here is the script's: events arrive at the times a test gives, a
//! key and a frame cost what the test says, and nothing sleeps.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use ratatui::crossterm::event::{Event, KeyCode};

use super::blocked_tmux::{BlockedTmux, Gate};
use super::console::scripted::{Scripted, key};
use super::*;
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, GraphSnapshot, NodeId};
use crate::tui::preview::{PreviewContent, PreviewSource};
use crate::tui::rows::RowId;

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// `count` sessions in a flat list, newest first, each titled
/// `session-NNN`.
fn sessions(count: usize) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for n in 0..count {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/nonexistent/.codex", format!("{n:03}")),
                "codex".to_string(),
            )
            .with_cwd(format!("/p/proj-{n:03}"))
            .with_title(format!("session-{n:03}"))
            .with_last_active_epoch(1_700_000_000 - n as i64),
        ));
    }
    crate::resolve::resolve_snapshot(snapshot)
}

/// What a run of the loop leaves behind.
struct Run {
    app: App,
    mode: Probe,
    console: Scripted,
}

impl Run {
    /// The screen the last frame drew.
    fn last_screen(&self) -> String {
        self.console.frames().last().expect("a frame").text()
    }

    /// The number the selected session is titled with.
    fn selected(&self) -> Option<String> {
        match self.app.selection()? {
            RowId::AgentSession(NodeId::AgentSession(id)) => Some(id.session_key.clone()),
            _ => None,
        }
    }

    /// Frames that began at `at`.
    fn frames_at(&self, at: Duration) -> usize {
        self.console
            .frames()
            .iter()
            .filter(|frame| frame.at == at)
            .count()
    }
}

/// A [`LoopMode`] with the scenario TUI's keys and no discovery, over a
/// scripted console. A key costs `key_cost` of the script's time, and a
/// preview load is recorded, stamped with the script's clock, and
/// answered at once with text naming the session, instead of run.
struct Probe {
    inner: StaticMode,
    key_cost: Duration,
    /// When the last key was handled.
    last_key: Duration,
    clock: Rc<Cell<Duration>>,
    /// Every load the loop started, and when.
    loads: Vec<(Duration, PreviewSource)>,
    /// Real workers for the loads, over a backend whose captures block,
    /// instead of answering each load at once.
    workers: Option<(PreviewLoads, Arc<Gate>)>,
}

impl LoopMode for Probe {
    type Terminal = Scripted;

    fn init(&mut self, _terminal: &mut Scripted, app: &mut App, config: &RunConfig) -> Result<()> {
        set_static_data(app, config, &self.inner.snapshot);
        Ok(())
    }

    fn drain(&mut self, app: &mut App, _config: &RunConfig) -> Result<()> {
        content::run_blocking(app, None);
        if let Some((loads, _)) = &mut self.workers {
            loads.drain(app);
        }
        Ok(())
    }

    fn dispatch(
        &mut self,
        terminal: &mut Scripted,
        app: &mut App,
        config: &RunConfig,
        action: Option<Action>,
    ) -> Result<()> {
        terminal.spend(self.key_cost);
        self.last_key = terminal.clock();
        self.inner.dispatch_action(app, config, action);
        Ok(())
    }

    fn tmux(&self) -> &dyn MuxBackend {
        self.inner.tmux.as_ref()
    }

    fn load_preview(&mut self, app: &mut App, source: PreviewSource) {
        self.loads.push((self.clock.get(), source.clone()));
        if let Some((loads, gate)) = &mut self.workers {
            // Hold the loop until the capture is under way, so what the
            // test sees next is the loop with a capture in flight.
            let begun = gate.started();
            loads.start(app, source);
            gate.wait_started(begun + 1);
            return;
        }
        if let PreviewSource::Transcript(session) = &source {
            let content = PreviewContent::Unavailable(format!("cached {}", session.session_key));
            app.update(Msg::SetPreview { source, content });
        }
    }
}

/// Run the loop over `snapshot` until `console` ends. The list is flat,
/// and the first row is selected.
fn run(snapshot: GraphSnapshot, console: Scripted, key_cost: Duration) -> Run {
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetGrouping(crate::tui::Grouping::Sessions(
        crate::tui::SessionsGrouping::None,
    )));
    run_in(app, snapshot, console, key_cost, None)
}

fn run_in(
    mut app: App,
    snapshot: GraphSnapshot,
    mut console: Scripted,
    key_cost: Duration,
    workers: Option<(PreviewLoads, Arc<Gate>)>,
) -> Run {
    let mut mode = Probe {
        inner: StaticMode {
            tmux: Box::new(crate::discovery::tmux::FakeTmux::with_sessions("")),
            snapshot,
            fixture_path: None,
            recordings: None,
        },
        key_cost,
        last_key: Duration::ZERO,
        clock: console.clock_handle(),
        loads: Vec::new(),
        workers,
    };
    let config = app.config().clone();
    match drive_loop(&mut console, &mut app, &config, &mut mode) {
        Ok(()) => {}
        Err(err)
            if err
                .downcast_ref::<std::io::Error>()
                .is_some_and(|err| err.kind() == std::io::ErrorKind::UnexpectedEof) => {}
        Err(err) => panic!("the loop failed: {err:#}"),
    }
    Run { app, mode, console }
}

fn downs(count: usize) -> impl Iterator<Item = KeyCode> {
    std::iter::repeat_n(KeyCode::Down, count)
}

#[test]
fn a_burst_of_queued_keys_costs_one_frame() {
    let console = Scripted::new(100, 30)
        .keys_at(ms(10), downs(60))
        .ends_at(ms(300));
    let run = run(sessions(200), console, Duration::ZERO);

    assert_eq!(run.selected().as_deref(), Some("060"));
    assert_eq!(
        run.frames_at(ms(10)),
        1,
        "sixty keys that were already queued drew one frame"
    );
}

#[test]
fn keys_that_arrive_one_at_a_time_each_get_a_frame() {
    let mut console = Scripted::new(100, 30).ends_at(ms(500));
    for n in 0..5 {
        console = console.key_at(ms(100 + 50 * n), KeyCode::Down);
    }
    let run = run(sessions(50), console, Duration::ZERO);

    for n in 0..5 {
        assert_eq!(
            run.frames_at(ms(100 + 50 * n)),
            1,
            "key {n} arrived alone and drew its own frame"
        );
    }
}

#[test]
fn a_batch_leaves_the_state_the_same_keys_one_at_a_time_leave() {
    // Moves, a menu that opens inside the burst and takes the keys after
    // it, a key the menu leaves unhandled, and its close.
    let keys = [
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Char('?'),
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Esc,
        KeyCode::Down,
        KeyCode::Char('k'),
        KeyCode::Char('j'),
        KeyCode::Char('j'),
    ];
    let batched = run(
        sessions(40),
        Scripted::new(100, 30)
            .keys_at(ms(10), keys)
            .ends_at(ms(300)),
        Duration::ZERO,
    );
    let mut spread = Scripted::new(100, 30).ends_at(ms(3000));
    for (n, code) in keys.into_iter().enumerate() {
        spread = spread.key_at(ms(100 + 200 * n as u64), code);
    }
    let spread = run(sessions(40), spread, Duration::ZERO);

    assert_eq!(batched.selected(), spread.selected());
    assert_eq!(batched.selected().as_deref(), Some("005"));
    assert_eq!(batched.app.has_modal(), spread.app.has_modal());
    assert_eq!(batched.last_screen(), spread.last_screen());
}

#[test]
fn keys_after_one_that_opens_a_menu_go_to_the_menu() {
    let console = Scripted::new(100, 30)
        .keys_at(
            ms(10),
            [
                KeyCode::Down,
                KeyCode::Char('?'),
                KeyCode::Down,
                KeyCode::Down,
            ],
        )
        .ends_at(ms(300));
    let run = run(sessions(40), console, Duration::ZERO);

    assert!(run.app.has_modal(), "the help page is open");
    assert_eq!(
        run.selected().as_deref(),
        Some("001"),
        "the two Down keys after `?` belong to the help page"
    );
    assert_eq!(run.frames_at(ms(10)), 1);
}

#[test]
fn the_frame_after_a_batch_keeps_the_selected_row_in_view() {
    let console = Scripted::new(100, 30)
        .keys_at(ms(10), downs(150))
        .ends_at(ms(300));
    let run = run(sessions(300), console, Duration::ZERO);

    assert_eq!(run.selected().as_deref(), Some("150"));
    let frame = run
        .console
        .frames()
        .iter()
        .find(|frame| frame.at == ms(10))
        .expect("the batch's frame");
    assert!(
        frame.text().contains("session-150"),
        "the selected row is on screen:\n{}",
        frame.text()
    );
    assert!(
        !frame.text().contains("session-000"),
        "the list scrolled to it:\n{}",
        frame.text()
    );
}

#[test]
fn a_stream_that_never_pauses_still_draws_at_least_every_100_ms() {
    // A key every millisecond, each costing two to handle and each frame
    // five: the queue only grows, so the loop never finds it empty.
    let mut console = Scripted::new(100, 30).frame_cost(ms(5)).ends_at(ms(4000));
    for n in 0..1000 {
        console = console.key_at(ms(n), KeyCode::Down);
    }
    let run = run(sessions(1200), console, ms(2));

    assert_eq!(run.selected().as_deref(), Some("1000"));
    let frames = run.console.frames();
    let streaming: Vec<_> = frames
        .iter()
        .filter(|frame| frame.at <= run.mode.last_key)
        .collect();
    for pair in streaming.windows(2) {
        let gap = pair[1].at - pair[0].at;
        assert!(
            gap <= ms(100),
            "{gap:?} between frames at {:?} and {:?} while keys kept arriving",
            pair[0].at,
            pair[1].at
        );
    }
    assert!(
        streaming.len() < 100,
        "{} frames for 1000 keys",
        streaming.len()
    );
}

#[test]
fn a_paste_inside_a_batch_types_into_the_focused_field() {
    let events = |at: Duration| -> Vec<(Duration, Event)> {
        vec![
            (at, key(KeyCode::Char('/'))),
            (at, key(KeyCode::Char('a'))),
            (at, Event::Paste("bc\nde".to_string())),
            (at, key(KeyCode::Char('f'))),
        ]
    };
    let mut batched = Scripted::new(100, 30).ends_at(ms(400));
    for (at, event) in events(ms(10)) {
        batched = batched.event_at(at, event);
    }
    let mut apart = Scripted::new(100, 30).ends_at(ms(4000));
    for (n, (_, event)) in events(Duration::ZERO).into_iter().enumerate() {
        apart = apart.event_at(ms(100 + 500 * n as u64), event);
    }
    let batched = run(sessions(10), batched, Duration::ZERO);
    let apart = run(sessions(10), apart, Duration::ZERO);

    let query = |run: &Run| {
        run.app
            .search_overlay()
            .map(|overlay| overlay.query().to_string())
    };
    // The paste is one line: the break in it becomes a space.
    assert_eq!(query(&apart).as_deref(), Some("abc def"));
    assert_eq!(query(&batched), query(&apart));
    assert_eq!(batched.last_screen(), apart.last_screen());
}

#[test]
fn a_quit_inside_a_batch_ends_the_loop_with_the_rest_unread() {
    let console = Scripted::new(100, 30)
        .keys_at(
            ms(10),
            [
                KeyCode::Down,
                KeyCode::Char('q'),
                KeyCode::Down,
                KeyCode::Down,
            ],
        )
        .ends_at(ms(300));
    let run = run(sessions(40), console, Duration::ZERO);

    assert!(run.app.should_quit());
    assert_eq!(run.selected().as_deref(), Some("001"));
    assert_eq!(run.console.unread(), 2);
    assert_eq!(
        run.frames_at(ms(10)),
        0,
        "no frame is drawn once the app has quit"
    );
}

/// When each load started and for which session.
fn loads(run: &Run) -> Vec<(Duration, String)> {
    run.mode
        .loads
        .iter()
        .map(|(at, source)| match source {
            PreviewSource::Transcript(session) => (*at, session.session_key.clone()),
            other => panic!("a load for {other:?}"),
        })
        .collect()
}

#[test]
fn a_burst_of_moves_starts_one_load_for_the_row_it_rests_on() {
    let console = Scripted::new(100, 30)
        .keys_at(ms(200), downs(20))
        .ends_at(ms(600));
    let run = run(sessions(50), console, Duration::ZERO);

    // The first row loads when the data lands, with no key behind it;
    // the twenty rows the burst passes load nothing.
    assert_eq!(
        loads(&run),
        [(ms(0), "000".to_string()), (ms(300), "020".to_string())]
    );
}

#[test]
fn moves_that_each_arrive_alone_still_load_only_where_the_cursor_rests() {
    // A key every 20 ms, each in a pass of its own: faster than a load
    // may start, so none starts until the keys stop.
    let mut console = Scripted::new(100, 30).ends_at(ms(900));
    for n in 0..30 {
        console = console.key_at(ms(200 + 20 * n), KeyCode::Down);
    }
    let run = run(sessions(50), console, Duration::ZERO);

    let loads = loads(&run);
    assert_eq!(loads.len(), 2, "{loads:?}");
    assert_eq!(loads[1].1, "030");
    let rested = loads[1].0 - ms(200 + 20 * 29);
    assert!(
        rested >= previews::REST && rested <= ms(150),
        "the row loaded {rested:?} after the last key"
    );
}

#[test]
fn one_move_then_a_pause_loads_the_new_row() {
    let console = Scripted::new(100, 30)
        .key_at(ms(300), KeyCode::Down)
        .ends_at(ms(800));
    let run = run(sessions(50), console, Duration::ZERO);

    assert_eq!(
        loads(&run),
        [(ms(0), "000".to_string()), (ms(400), "001".to_string())]
    );
}

#[test]
fn a_row_passed_shows_its_cached_preview_or_the_placeholder() {
    // The cursor rests on 000 and then 001, so both are cached; then it
    // steps down to 002 and back up to 001 inside the rest.
    let console = Scripted::new(120, 30)
        .key_at(ms(200), KeyCode::Down)
        .key_at(ms(500), KeyCode::Down)
        .key_at(ms(510), KeyCode::Up)
        .ends_at(ms(900));
    let run = run(sessions(10), console, Duration::ZERO);

    let at = |t: u64| {
        run.console
            .frames()
            .iter()
            .find(|frame| frame.at == ms(t))
            .unwrap_or_else(|| panic!("no frame at {t} ms"))
            .text()
    };
    // On 002 no preview was ever loaded: the placeholder, not 001's.
    assert!(at(500).contains("loading transcript"), "{}", at(500));
    assert!(!at(500).contains("cached 001"), "{}", at(500));
    // Back on 001 before the rest: its cached preview, with no new load.
    assert!(at(510).contains("cached 001"), "{}", at(510));
    assert_eq!(
        loads(&run)
            .into_iter()
            .map(|(_, key)| key)
            .collect::<Vec<_>>(),
        ["000", "001"],
        "the row passed loads nothing, and 001 is not loaded again"
    );
}

#[test]
fn a_refresh_loads_the_selection_without_waiting_for_the_rest() {
    let console = Scripted::new(100, 30)
        .keys_at(ms(300), [KeyCode::Down, KeyCode::Down, KeyCode::Char('r')])
        .ends_at(ms(800));
    let run = run(sessions(50), console, Duration::ZERO);

    assert_eq!(
        loads(&run),
        [(ms(0), "000".to_string()), (ms(300), "002".to_string())]
    );
}

/// Muxes `tmux:<name>` for each of `names`.
fn muxes(names: &[&str]) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for name in names {
        snapshot
            .nodes
            .push(GraphNode::MuxSession(crate::model::MuxSessionNode::new(
                crate::model::MuxSessionId::new(format!("tmux:{name}")),
                "tmux".to_string(),
                (*name).to_string(),
            )));
    }
    crate::resolve::resolve_snapshot(snapshot)
}

#[test]
fn keys_are_handled_and_frames_drawn_while_a_pane_capture_is_in_flight() {
    // Every capture waits at a gate the test holds closed, as a tmux
    // server that has stopped answering would hold it.
    let (tmux, gate) = BlockedTmux::closed();
    let mut config = RunConfig::defaults();
    config.default_view = crate::tui::View::Mux;
    let console = Scripted::new(100, 30)
        .key_at(ms(300), KeyCode::Down)
        .key_at(ms(320), KeyCode::Down)
        .key_at(ms(340), KeyCode::Up)
        .ends_at(ms(1000));
    let mut run = run_in(
        App::new(config),
        muxes(&["alpha", "beta", "gamma"]),
        console,
        Duration::ZERO,
        Some((PreviewLoads::new(tmux), Arc::clone(&gate))),
    );

    // The first pane's capture began when the data landed and the cursor
    // rests on beta at the end, so a second began there; neither has
    // finished, and the loop drew and took keys all the while.
    assert_eq!(gate.started(), 2);
    assert_eq!(gate.finished(), 0);
    assert!(
        run.console
            .frames()
            .iter()
            .filter(|f| f.at >= ms(300))
            .count()
            >= 8,
        "frames kept coming while a capture was in flight"
    );
    let selected = run.app.selection().cloned();
    assert!(
        matches!(&selected, Some(RowId::MuxSession(NodeId::MuxSession(id))) if id.native_id == "tmux:beta"),
        "the keys moved the cursor: {selected:?}"
    );
    assert!(
        run.last_screen().contains("loading mux preview"),
        "a pane never captured shows the placeholder:\n{}",
        run.last_screen()
    );

    // The captures finish after the cursor has left the first pane's row:
    // each is stored, and the screen shows the pane the cursor is on.
    gate.open();
    let (loads, _) = run.mode.workers.as_mut().expect("workers");
    let begun = Instant::now();
    while gate.finished() < 2
        || run
            .app
            .mux_preview(&crate::model::MuxSessionId::new("tmux:alpha"))
            .is_none()
    {
        assert!(
            begun.elapsed() < Duration::from_secs(5),
            "the captures never landed"
        );
        loads.drain(&mut run.app);
        std::thread::sleep(Duration::from_millis(1));
    }
    loads.drain(&mut run.app);
    run.console.draw(&mut run.app).expect("draw");
    let screen = run.last_screen();
    assert!(screen.contains("beta capture"), "{screen}");
    assert!(!screen.contains("alpha capture"), "{screen}");
}
