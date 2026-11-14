use std::sync::atomic::{AtomicBool, Ordering};

use super::super::blocked_tmux::BlockedTmux;
use super::*;
use crate::model::{AgentSessionId, GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode};
use crate::tui::app::SnapshotHandle;

fn session(key: &str) -> PreviewSource {
    PreviewSource::Transcript(AgentSessionId::new("codex", "/nonexistent/.codex", key))
}

fn app() -> App {
    App::new(RunConfig::defaults())
}

/// A backend for loads that never touch a pane.
fn idle_tmux() -> Arc<dyn MuxBackend> {
    Arc::new(crate::discovery::tmux::FakeTmux::with_sessions(""))
}

/// Drain until `done` says the app has what the test waits for.
fn drain_until(loads: &mut PreviewLoads, app: &mut App, done: impl Fn(&App) -> bool) {
    let begun = Instant::now();
    while !done(app) {
        assert!(
            begun.elapsed() < Duration::from_secs(5),
            "the load never landed"
        );
        loads.drain(app);
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A loader that runs until it is told to stop, then ends `Stopped`, as a
/// git load does between two commands.
fn runs_until_stopped() -> Loader {
    Arc::new(|_, _, stop| {
        let begun = Instant::now();
        while !stop() {
            if begun.elapsed() > Duration::from_secs(5) {
                return Ok(Some(PreviewContent::Failed("never stopped".to_string())));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(Stopped)
    })
}

#[test]
fn a_load_for_a_row_the_cursor_left_stops_and_the_row_loads_again_later() {
    let (a, b) = (session("a"), session("b"));
    let mut app = app();
    app.update(Msg::SetPreview {
        source: a.clone(),
        content: PreviewContent::Unavailable("old".to_string()),
    });
    let mut loads = PreviewLoads::with_loader(idle_tmux(), runs_until_stopped());
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&mut app, a.clone());
    assert!(app.previews().is_loading(&a));

    loads.keep_only(Some(&b));
    drain_until(&mut loads, &mut app, |app| !app.previews().is_loading(&a));

    let entry = app.previews().get(&a).expect("the old preview stays");
    assert_eq!(
        entry.content,
        PreviewContent::Unavailable("old".to_string())
    );
    assert!(
        entry.stale,
        "what is cached for the row is marked for reload"
    );
}

#[test]
fn a_row_never_loaded_has_nothing_cached_after_its_load_stops() {
    let (a, b) = (session("a"), session("b"));
    let mut app = app();
    let mut loads = PreviewLoads::with_loader(idle_tmux(), runs_until_stopped());
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&mut app, a.clone());

    loads.keep_only(Some(&b));
    drain_until(&mut loads, &mut app, |app| !app.previews().is_loading(&a));

    assert!(app.previews().get(&a).is_none());
}

#[test]
fn a_load_for_the_row_under_the_cursor_keeps_running() {
    let a = session("a");
    let mut app = app();
    let release = Arc::new(AtomicBool::new(false));
    let waits = Arc::clone(&release);
    let loader: Loader = Arc::new(move |_, _, stop| {
        while !waits.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        if stop() {
            return Err(Stopped);
        }
        Ok(Some(PreviewContent::Unavailable("loaded".to_string())))
    });
    let mut loads = PreviewLoads::with_loader(idle_tmux(), loader);
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&mut app, a.clone());

    loads.keep_only(Some(&a));
    release.store(true, Ordering::SeqCst);
    drain_until(&mut loads, &mut app, |app| app.previews().get(&a).is_some());

    assert_eq!(
        app.previews().get(&a).expect("loaded").content,
        PreviewContent::Unavailable("loaded".to_string())
    );
    assert!(!app.previews().is_loading(&a));
}

#[test]
fn a_loader_that_panics_still_ends_the_load() {
    let a = session("a");
    let mut app = app();
    let mut loads =
        PreviewLoads::with_loader(idle_tmux(), Arc::new(|_, _, _| panic!("a bad transcript")));
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&mut app, a.clone());

    drain_until(&mut loads, &mut app, |app| app.previews().get(&a).is_some());

    assert_eq!(
        app.previews().get(&a).expect("failed").content,
        PreviewContent::Failed("the preview reader crashed".to_string())
    );
    assert!(!app.previews().is_loading(&a));
}

/// An app whose snapshot has the muxes `names` (`tmux:<name>`).
fn mux_app(names: &[&str]) -> App {
    let mut snapshot = GraphSnapshot::empty();
    for name in names {
        snapshot
            .nodes
            .push(GraphNode::MuxSession(MuxSessionNode::new(
                MuxSessionId::new(format!("tmux:{name}")),
                "tmux".to_string(),
                (*name).to_string(),
            )));
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let mut app = app();
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

fn pane(name: &str) -> (MuxSessionId, PreviewSource) {
    let mux = MuxSessionId::new(format!("tmux:{name}"));
    (mux.clone(), PreviewSource::Pane(mux))
}

/// The text of the capture stored for `mux`.
fn captured(app: &App, mux: &MuxSessionId) -> Option<String> {
    match &app.mux_preview(mux)?.content {
        PreviewContent::Text(capture) => Some(capture.text.clone()),
        other => panic!("not a capture: {other:?}"),
    }
}

#[test]
fn a_pane_capture_runs_off_the_calling_thread_and_lands_through_drain() {
    let (tmux, gate) = BlockedTmux::closed();
    let mut app = mux_app(&["editor"]);
    let (mux, source) = pane("editor");
    let mut loads = PreviewLoads::new(tmux);
    app.update(Msg::PreviewLoadStarted(source.clone()));

    // `start` returns while the capture blocks: nothing waited on tmux.
    loads.start(&mut app, source);
    gate.wait_started(1);
    assert_eq!(gate.finished(), 0);
    loads.drain(&mut app);
    assert!(app.mux_preview(&mux).is_none(), "nothing has landed");
    assert!(app.previews().is_loading(&PreviewSource::Pane(mux.clone())));

    gate.open();
    drain_until(&mut loads, &mut app, |app| app.mux_preview(&mux).is_some());
    assert_eq!(captured(&app, &mux).as_deref(), Some("editor capture 1"));
    assert!(!app.previews().is_loading(&PreviewSource::Pane(mux)));
}

#[test]
fn a_pane_has_at_most_one_capture_in_flight() {
    let (tmux, gate) = BlockedTmux::closed();
    let mut app = mux_app(&["editor", "build"]);
    let (editor, source) = pane("editor");
    let (_, other) = pane("build");
    let mut loads = PreviewLoads::new(tmux);

    loads.start(&mut app, source.clone());
    loads.start(&mut app, source.clone());
    loads.start(&mut app, source.clone());
    gate.wait_started(1);
    // A different pane is its own capture.
    loads.start(&mut app, other);
    gate.wait_started(2);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(gate.started(), 2, "one capture each, not one per request");

    gate.open();
    drain_until(&mut loads, &mut app, |app| {
        app.mux_preview(&editor).is_some()
    });
    // Landed, so the pane can be captured again.
    loads.start(&mut app, source);
    gate.wait_started(3);
}

#[test]
fn a_capture_that_lands_after_the_cursor_left_is_stored_and_changes_no_other_preview() {
    let (tmux, gate) = BlockedTmux::closed();
    let mut app = mux_app(&["editor", "build"]);
    let (editor, source) = pane("editor");
    let (build, other) = pane("build");
    let mut loads = PreviewLoads::new(tmux);
    loads.start(&mut app, source);
    gate.wait_started(1);

    // The cursor moves to another pane while the capture is in flight.
    loads.keep_only(Some(&other));
    gate.open();
    drain_until(&mut loads, &mut app, |app| {
        app.mux_preview(&editor).is_some()
    });

    assert_eq!(captured(&app, &editor).as_deref(), Some("editor capture 1"));
    assert!(
        app.mux_preview(&build).is_none(),
        "the other pane's preview is not the capture that landed"
    );
}

#[test]
fn a_pane_captured_before_keeps_its_last_capture_until_the_new_one_lands() {
    let (tmux, gate) = BlockedTmux::closed();
    let mut app = mux_app(&["editor"]);
    let (mux, source) = pane("editor");
    let mut loads = PreviewLoads::new(tmux);
    gate.open();
    loads.start(&mut app, source.clone());
    drain_until(&mut loads, &mut app, |app| app.mux_preview(&mux).is_some());
    assert_eq!(captured(&app, &mux).as_deref(), Some("editor capture 1"));

    gate.close();
    app.update(Msg::PreviewLoadStarted(source.clone()));
    loads.start(&mut app, source);
    gate.wait_started(2);
    loads.drain(&mut app);
    assert_eq!(
        captured(&app, &mux).as_deref(),
        Some("editor capture 1"),
        "the old capture stays on screen while the new one is in flight"
    );

    gate.open();
    drain_until(&mut loads, &mut app, |app| {
        captured(app, &mux).as_deref() == Some("editor capture 2")
    });
}

#[test]
fn a_mux_with_no_pane_to_capture_says_so_without_asking_tmux() {
    let (tmux, gate) = BlockedTmux::closed();
    let mut app = mux_app(&["editor"]);
    let (gone, source) = pane("gone");
    let mut loads = PreviewLoads::new(tmux);

    loads.start(&mut app, source);

    assert_eq!(
        app.mux_preview(&gone).expect("answered at once").content,
        PreviewContent::NoTarget
    );
    assert_eq!(gate.started(), 0);
}
