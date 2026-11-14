use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::model::AgentSessionId;

fn session(key: &str) -> PreviewSource {
    PreviewSource::Transcript(AgentSessionId::new("codex", "/nonexistent/.codex", key))
}

fn app() -> App {
    App::new(RunConfig::defaults())
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
    let mut loads = PreviewLoads::with_loader(runs_until_stopped());
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&app, a.clone());
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
    let mut loads = PreviewLoads::with_loader(runs_until_stopped());
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&app, a.clone());

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
    let mut loads = PreviewLoads::with_loader(loader);
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&app, a.clone());

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
    let mut loads = PreviewLoads::with_loader(Arc::new(|_, _, _| panic!("a bad transcript")));
    app.update(Msg::PreviewLoadStarted(a.clone()));
    loads.start(&app, a.clone());

    drain_until(&mut loads, &mut app, |app| app.previews().get(&a).is_some());

    assert_eq!(
        app.previews().get(&a).expect("failed").content,
        PreviewContent::Failed("the preview reader crashed".to_string())
    );
    assert!(!app.previews().is_loading(&a));
}
