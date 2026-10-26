use std::sync::Arc;

use super::panes::*;
use super::source::{ContentResult, ContentSource, MuxRef, PaneHits, SourceEvent};
use super::test_support::{claude_lines, session, write_lines};
use super::{CancelToken, Progress, ScanOptions};
use crate::discovery::tmux::{FakeTmux, TmuxCaptureOutcome, TmuxOutcome, UnavailableReason};
use crate::viewer::parser::claude_code::ClaudeCodeParser;
use crate::viewer::search::{ContentKinds, ContentQuery};

const LISTING: &str = "work\t%1\t0\t0\tclaude\nwork\t%2\t0\t1\tzsh\nplay\t%3\t1\t0\tvim\n";

fn query(text: &str) -> ContentQuery {
    ContentQuery::new(text, ContentKinds::default())
}

fn mux(name: &str) -> String {
    MuxRef {
        backend: "tmux".into(),
        socket: None,
        session: name.into(),
    }
    .native_id()
}

struct Run {
    panes: Vec<PaneHits>,
    progress: Progress,
}

fn run(source: &PaneSource, text: &str) -> Run {
    let events = std::sync::Mutex::new(Vec::new());
    source.run(
        &query(text),
        &ScanOptions::default(),
        &CancelToken::new(),
        &|event| events.lock().unwrap().push(event),
    );
    let mut panes = Vec::new();
    let mut progress = Progress::default();
    for event in events.into_inner().unwrap() {
        match event {
            SourceEvent::Result(ContentResult::Pane(hits)) => panes.push(hits),
            SourceEvent::Result(_) => unreachable!("a pane source reports panes"),
            SourceEvent::Progress(p) => progress = p,
        }
    }
    panes.sort_by(|a, b| a.pane.pane_id.cmp(&b.pane.pane_id));
    Run { panes, progress }
}

fn fake() -> FakeTmux {
    FakeTmux::with_sessions("")
        .with_panes(None, TmuxOutcome::Sessions(LISTING.to_string()))
        .with_history(
            "%1",
            TmuxCaptureOutcome::captured(
                "$ claude\nthe retry budget is exhausted after three attempts\n\n",
            ),
        )
        .with_history(
            "%2",
            TmuxCaptureOutcome::captured(
                "\u{1b}[31merror\u{1b}[0m: Retry Budget gone\nsecond line\n",
            ),
        )
        .with_history("%3", TmuxCaptureOutcome::captured("nothing relevant\n"))
}

fn source(tmux: FakeTmux, attribution: Attribution) -> PaneSource {
    PaneSource::new(Arc::new(tmux), vec![None], attribution).at(1_000)
}

#[test]
fn a_listing_parses_into_panes() {
    let panes = parse_pane_list(LISTING, Some("sock"));
    assert_eq!(panes.len(), 3);
    let first = &panes[0].pane;
    assert_eq!(first.mux.session, "work");
    assert_eq!(first.mux.socket.as_deref(), Some("sock"));
    assert_eq!(first.pane_id, "%1");
    assert_eq!(first.window_index, "0");
    assert_eq!(first.command.as_deref(), Some("claude"));
    assert!(parse_pane_list("\n\tbroken\n", None).is_empty());
}

#[test]
fn scrollback_loses_escapes_and_trailing_blank_lines() {
    let back = scrollback_lines("\u{1b}[1mhello\u{1b}[0m  \nworld\n\n\n");
    assert_eq!(back.lines, ["hello", "world"]);
    assert!(!back.capped);
}

#[test]
fn scrollback_keeps_the_newest_lines_under_the_cap() {
    let text: String = (0..MAX_PANE_LINES + 10)
        .map(|i| format!("line {i}\n"))
        .collect();
    let back = scrollback_lines(&text);
    assert_eq!(back.lines.len(), MAX_PANE_LINES);
    assert_eq!(back.lines[0], "line 10");
    assert!(back.capped);
}

#[test]
fn matching_panes_report_line_snippet_and_highlight() {
    let found = run(&source(fake(), Attribution::new()), "retry budget");

    assert_eq!(found.panes.len(), 2);
    let first = &found.panes[0];
    assert_eq!(first.pane.mux.session, "work");
    assert_eq!(first.count, 1);
    assert_eq!(first.captured_at, 1_000);
    let hit = &first.hits[0];
    assert_eq!(hit.line, 2);
    assert_eq!(&hit.snippet[hit.highlight.clone()], "retry budget");
    let second = &found.panes[1];
    assert_eq!(second.hits[0].line, 1);
    assert_eq!(
        &second.hits[0].snippet[second.hits[0].highlight.clone()],
        "Retry Budget"
    );
    assert_eq!(
        found.progress,
        Progress {
            scanned: 3,
            total: 3,
            skipped: 0
        }
    );
}

#[test]
fn a_line_a_hosted_session_already_says_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("claude");
    write_lines(
        &root.join("projects/p1/s1.jsonl"),
        &claude_lines(
            &[
                "why did it fail",
                "The retry budget is exhausted after three attempts.",
            ],
            true,
        ),
    );
    let mut attribution = Attribution::new();
    attribution.add(
        mux("work"),
        session("claude-code", "s1", &root, &ClaudeCodeParser),
    );

    let found = run(&source(fake(), attribution), "retry budget");

    // Pane %1 mirrors the session; pane %2 (same mux) is a shell whose
    // line the session never said.
    assert_eq!(found.panes.len(), 1, "{:?}", found.panes);
    assert_eq!(found.panes[0].pane.pane_id, "%2");
    assert_eq!(found.panes[0].deduplicated, 0);
}

#[test]
fn the_dropped_count_is_reported_when_other_lines_survive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("claude");
    write_lines(
        &root.join("projects/p1/s1.jsonl"),
        &claude_lines(
            &["q", "The retry budget is exhausted after three attempts."],
            true,
        ),
    );
    let mut attribution = Attribution::new();
    attribution.add(
        mux("work"),
        session("claude-code", "s1", &root, &ClaudeCodeParser),
    );
    let tmux = fake().with_history(
        "%1",
        TmuxCaptureOutcome::captured(
            "the retry budget is exhausted after three attempts\n$ grep -r retry budget .\n",
        ),
    );

    let found = run(&source(tmux, attribution), "retry budget");

    let pane = &found.panes[0];
    assert_eq!(pane.pane.pane_id, "%1");
    assert_eq!(pane.count, 1);
    assert_eq!(pane.deduplicated, 1);
    assert_eq!(pane.hits[0].line, 2);
}

#[test]
fn text_only_a_tool_result_says_still_counts_as_the_sessions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("claude");
    let lines = vec![
        r#"{"type":"user","uuid":"u0","message":{"role":"user","content":"run it"}}"#.to_string(),
        r#"{"type":"assistant","uuid":"u1","parentUuid":"u0","message":{"role":"assistant","content":[{"type":"tool_use","id":"t","name":"Bash","input":{"command":"cargo test"}}]}}"#.to_string(),
        r#"{"type":"user","uuid":"u2","parentUuid":"u1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":"panic: unreachable zebra state in worker"}]}}"#.to_string(),
    ];
    write_lines(&root.join("projects/p1/s1.jsonl"), &lines);
    let mut attribution = Attribution::new();
    attribution.add(
        mux("work"),
        session("claude-code", "s1", &root, &ClaudeCodeParser),
    );
    let tmux = FakeTmux::with_sessions("")
        .with_panes(
            None,
            TmuxOutcome::Sessions("work\t%1\t0\t0\tclaude\n".into()),
        )
        .with_history(
            "%1",
            TmuxCaptureOutcome::captured("panic: unreachable zebra state in worker\n"),
        );

    // Tool output is not searched by default, but it is on screen in the
    // pane: the pane must not surface what the session search withheld.
    let found = run(&source(tmux, attribution), "zebra");

    assert!(found.panes.is_empty(), "{:?}", found.panes);
}

#[test]
fn an_unreadable_pane_counts_as_skipped() {
    let tmux = fake().with_history("%3", TmuxCaptureOutcome::NoTarget);
    let found = run(&source(tmux, Attribution::new()), "retry budget");

    assert_eq!(found.progress.skipped, 1);
    assert_eq!(found.progress.scanned, 3);
}

#[test]
fn a_failed_listing_is_skipped_and_a_missing_server_is_not() {
    let failing = FakeTmux::with_sessions("");
    let found = run(&source(failing, Attribution::new()), "x");
    assert_eq!(found.progress.skipped, 1);

    let none = FakeTmux::with_sessions("")
        .with_panes(None, TmuxOutcome::Unavailable(UnavailableReason::NoServer));
    let found = run(&source(none, Attribution::new()), "x");
    assert_eq!(found.progress, Progress::default());
}

#[test]
fn a_cancelled_search_captures_nothing() {
    let tmux = fake();
    let cancel = CancelToken::new();
    cancel.cancel();
    let source = source(tmux.clone(), Attribution::new());
    source.run(&query("retry"), &ScanOptions::default(), &cancel, &|_| {});

    assert!(tmux.pane_calls().captured.is_empty());
}

#[test]
fn an_empty_query_searches_nothing() {
    let tmux = fake();
    let source = source(tmux.clone(), Attribution::new());
    source.run(
        &query("   "),
        &ScanOptions::default(),
        &CancelToken::new(),
        &|_| {},
    );

    assert!(tmux.pane_calls().listed.is_empty());
}
