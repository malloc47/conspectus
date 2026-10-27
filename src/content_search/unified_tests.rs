use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::panes::{Attribution, PaneSource};
use super::source::{
    ContentResult, ContentSource, SourceEvent, SourceKind, cmp_results, cmp_results_by_recency,
};
use super::test_support::{claude_lines, session, write_lines};
use super::unified::*;
use super::{CancelToken, Progress, ScanOptions};
use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome, TmuxOutcome};
use crate::viewer::parser::claude_code::ClaudeCodeParser;
use crate::viewer::search::{ContentKinds, ContentQuery};

fn query(text: &str) -> ContentQuery {
    ContentQuery::new(text, ContentKinds::default())
}

struct World {
    _dir: tempfile::TempDir,
    sources: Vec<Box<dyn ContentSource>>,
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("claude");
    write_lines(
        &root.join("projects/p1/s1.jsonl"),
        &claude_lines(&["a needle in a haystack", "found the needle"], true),
    );
    let sessions = vec![session("claude-code", "s1", &root, &ClaudeCodeParser)];
    let tmux = FakeTmux::with_sessions("")
        .with_panes(
            None,
            TmuxOutcome::Sessions("work\t%1\t0\t0\tzsh\n".to_string()),
        )
        .with_history("%1", TmuxCaptureOutcome::captured("$ grep needle\n"));
    let panes = PaneSource::new(Arc::new(tmux), vec![None], Attribution::new()).at(5);
    World {
        _dir: dir,
        sources: vec![Box::new(SessionSource::new(sessions)), Box::new(panes)],
    }
}

#[test]
fn both_sources_report_through_one_stream() {
    let world = world();
    let summary = search_blocking(
        world.sources,
        query("needle"),
        ScanOptions::default(),
        &CancelToken::new(),
    );

    assert!(!summary.cancelled);
    assert_eq!(summary.results.len(), 2);
    assert_eq!(
        summary.progress,
        Progress {
            scanned: 2,
            total: 2,
            skipped: 0
        }
    );
    // The session has two hits and ranks above the pane's one.
    assert_eq!(summary.results[0].kind(), SourceKind::Session);
    assert_eq!(summary.results[0].count(), 2);
    assert_eq!(summary.results[1].kind(), SourceKind::Pane);
}

#[test]
fn recency_orders_a_pane_captured_now_before_an_old_session() {
    let world = world();
    let mut summary = search_blocking(
        world.sources,
        query("needle"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    summary.sort_by_recency();

    // The session's messages are from 2026-06; the pane was captured at
    // epoch 5, so the session is newer here.
    assert_eq!(summary.results[0].kind(), SourceKind::Session);
    let a = summary.results[0].clone();
    let b = summary.results[1].clone();
    assert_eq!(cmp_results_by_recency(&a, &b), std::cmp::Ordering::Less);
    assert_eq!(cmp_results(&a, &b), std::cmp::Ordering::Less);
}

#[test]
fn events_carry_the_generation_and_done_comes_last() {
    let world = world();
    let handle = search(
        world.sources,
        query("needle"),
        ScanOptions {
            generation: 9,
            ..ScanOptions::default()
        },
    );
    let mut events = Vec::new();
    while let Some(event) = handle.recv() {
        let done = matches!(event, ContentEvent::Done { .. });
        events.push(event);
        if done {
            break;
        }
    }

    assert!(events.iter().all(|e| e.generation() == 9));
    assert!(matches!(
        events.last(),
        Some(ContentEvent::Done {
            matched: 2,
            cancelled: false,
            ..
        })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ContentEvent::Done { .. }))
            .count(),
        1
    );
    handle.join();
}

struct Panicking;

impl ContentSource for Panicking {
    fn kind(&self) -> SourceKind {
        SourceKind::Pane
    }

    fn run(
        &self,
        _: &ContentQuery,
        _: &ScanOptions,
        _: &CancelToken,
        _: &(dyn Fn(SourceEvent) + Sync),
    ) {
        panic!("source exploded");
    }
}

#[test]
fn a_panicking_source_costs_only_its_own_results() {
    let mut world = world();
    world.sources.push(Box::new(Panicking));

    let summary = search_blocking(
        world.sources,
        query("needle"),
        ScanOptions::default(),
        &CancelToken::new(),
    );

    assert_eq!(summary.results.len(), 2);
}

struct Waiting(Arc<AtomicBool>);

impl ContentSource for Waiting {
    fn kind(&self) -> SourceKind {
        SourceKind::Pane
    }

    fn run(
        &self,
        _: &ContentQuery,
        _: &ScanOptions,
        cancel: &CancelToken,
        _: &(dyn Fn(SourceEvent) + Sync),
    ) {
        while !cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(2));
        }
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn cancelling_reaches_every_source_and_is_reported() {
    let seen = Arc::new(AtomicBool::new(false));
    let handle = search(
        vec![Box::new(Waiting(seen.clone()))],
        query("x"),
        ScanOptions::default(),
    );
    handle.cancel();
    let mut cancelled = false;
    while let Some(event) = handle.recv() {
        if let ContentEvent::Done { cancelled: c, .. } = event {
            cancelled = c;
            break;
        }
    }

    assert!(cancelled);
    assert!(seen.load(Ordering::SeqCst));
}

#[test]
fn dropping_the_handle_cancels_the_search() {
    let seen = Arc::new(AtomicBool::new(false));
    let handle = search(
        vec![Box::new(Waiting(seen.clone()))],
        query("x"),
        ScanOptions::default(),
    );
    drop(handle);
    for _ in 0..500 {
        if seen.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the source never saw the cancellation");
}

#[test]
fn a_session_result_keeps_its_key() {
    let world = world();
    let summary = search_blocking(
        world.sources,
        query("needle"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    let ContentResult::Session { key, .. } = &summary.results[0] else {
        panic!("expected the session first");
    };
    assert_eq!(key.session_key, "s1");
}
