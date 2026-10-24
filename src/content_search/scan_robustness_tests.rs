//! How a scan behaves when parsers misbehave, when it is cancelled, and
//! what the prefilter does and does not skip. These tests steer the scan
//! with stub parsers, not timing.

use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

use super::scan::{Polled, keep_newest};
use super::test_support::{StubParser, corpus, synthesized_corpus, with_parser, write_lines};
use super::{
    CancelToken, MAX_HITS_PER_SESSION, ScanEvent, ScanOptions, ScanSession, scan, scan_blocking,
    session_detail,
};
use crate::viewer::model::{TurnKind, TurnRole};
use crate::viewer::parser::claude_code::ClaudeCodeParser;
use crate::viewer::search::{ContentKinds, ContentQuery, Hit};

fn query(text: &str) -> ContentQuery {
    ContentQuery::new(text, ContentKinds::default())
}

fn claude_sessions(corpus: &super::test_support::Corpus) -> Vec<ScanSession> {
    corpus
        .sessions
        .iter()
        .filter(|s| s.key.harness_key == "claude-code")
        .cloned()
        .collect()
}

fn drain(handle: &super::ScanHandle) -> Vec<ScanEvent> {
    let mut events = Vec::new();
    while let Some(event) = handle.recv() {
        events.push(event);
    }
    events
}

// --- a parser that panics costs one session, not the scan ---

#[test]
fn a_parser_that_panics_skips_that_session_and_the_scan_goes_on() {
    let corpus = corpus();
    let stub = StubParser::wrapping(&ClaudeCodeParser)
        .panicking_on("claude-plain")
        .leak();
    let sessions = with_parser(&claude_sessions(&corpus), stub);

    let summary = scan_blocking(
        &sessions,
        &query("needle in a haystack"),
        ScanOptions::default(),
        &CancelToken::new(),
    );

    let found: Vec<_> = summary
        .results
        .iter()
        .map(|(key, _)| key.session_key.as_str())
        .collect();
    assert_eq!(found, ["claude-escaped"]);
    assert_eq!(summary.progress.skipped, 1, "the panic counts as skipped");
    assert_eq!(summary.progress.scanned, sessions.len());
}

#[test]
fn a_scan_that_dies_outside_a_session_still_ends_with_done() {
    let corpus = corpus();
    let stub = StubParser::wrapping(&ClaudeCodeParser)
        .panicking_in_index()
        .leak();
    let sessions = with_parser(&claude_sessions(&corpus), stub);
    let total = sessions.len();

    let handle = scan(
        sessions,
        query("needle"),
        ScanOptions {
            generation: 3,
            ..ScanOptions::default()
        },
    );
    let events = drain(&handle);

    let Some(ScanEvent::Done {
        generation,
        progress,
        matched,
        cancelled,
    }) = events.last()
    else {
        panic!("the scan ends with Done: {events:?}");
    };
    assert_eq!(*generation, 3);
    assert_eq!((progress.total, progress.skipped), (total, total));
    assert_eq!(*matched, 0);
    assert!(!cancelled);
    assert!(
        matches!(handle.poll(), Polled::Ended),
        "once drained, the handle says the scan has ended"
    );
}

#[test]
fn polling_tells_a_quiet_scan_from_an_ended_one() {
    let corpus = corpus();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let release_rx = Mutex::new(release_rx);
    let stub = StubParser::wrapping(&ClaudeCodeParser)
        .on_read(move || {
            let _ = entered_tx.lock().expect("lock").send(());
            let _ = release_rx.lock().expect("lock").recv();
        })
        .leak();
    let sessions = with_parser(&claude_sessions(&corpus)[..1], stub);

    let handle = scan(sessions, query("needle"), ScanOptions::default());
    entered_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the read started");
    // The read is held, so the scan is running but has nothing new.
    let mut saw_empty = false;
    for _ in 0..10 {
        match handle.poll() {
            Polled::Empty => {
                saw_empty = true;
                break;
            }
            Polled::Event(_) => {}
            Polled::Ended => panic!("a running scan is not ended"),
        }
    }
    assert!(saw_empty, "a running scan with nothing to say polls Empty");

    drop(release_tx);
    drain(&handle);
    assert!(matches!(handle.poll(), Polled::Ended));
}

// --- cancellation, without timing ---

/// A stub whose reads announce themselves and then wait for release.
struct Gate {
    stub: &'static StubParser,
    entered: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
}

fn gated() -> Gate {
    let (entered_tx, entered) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let entered_tx = Mutex::new(entered_tx);
    let release_rx = Mutex::new(release_rx);
    let stub = StubParser::wrapping(&ClaudeCodeParser)
        .on_read(move || {
            let _ = entered_tx.lock().expect("lock").send(());
            // A closed release channel releases every waiting read.
            let _ = release_rx.lock().expect("lock").recv();
        })
        .leak();
    Gate {
        stub,
        entered,
        release,
    }
}

fn many_plain_sessions(stub: &'static StubParser, copies: usize) -> Vec<ScanSession> {
    let corpus = corpus();
    let plain = claude_sessions(&corpus)
        .into_iter()
        .find(|s| s.key.session_key == "claude-plain")
        .expect("session");
    // The corpus directory must outlive the scan.
    std::mem::forget(corpus);
    with_parser(&vec![plain; copies], stub)
}

#[test]
fn cancelling_a_scan_parses_at_most_the_read_in_flight() {
    let gate = gated();
    let sessions = many_plain_sessions(gate.stub, 40);
    let handle = scan(
        sessions,
        query("hello"),
        ScanOptions {
            workers: 1,
            ..ScanOptions::default()
        },
    );
    gate.entered
        .recv_timeout(Duration::from_secs(10))
        .expect("the first read started");

    handle.cancel();
    drop(gate.release); // let the held read finish
    let events = drain(&handle);

    let Some(ScanEvent::Done { cancelled, .. }) = events.last() else {
        panic!("a cancelled scan still ends with Done: {events:?}");
    };
    assert!(cancelled);
    assert_eq!(
        gate.stub.reads(),
        1,
        "with one worker, only the read already in flight finishes"
    );
}

#[test]
fn dropping_the_handle_stops_the_scan_thread() {
    let gate = gated();
    let sessions = many_plain_sessions(gate.stub, 40);
    let handle = scan(
        sessions,
        query("hello"),
        ScanOptions {
            workers: 1,
            ..ScanOptions::default()
        },
    );
    gate.entered
        .recv_timeout(Duration::from_secs(10))
        .expect("the first read started");

    drop(handle);
    drop(gate.release);

    // No second read ever starts. A quiet half second is how the absence
    // of an event is observed; it can only miss a bug, never fail a
    // correct scan.
    assert!(
        gate.entered
            .recv_timeout(Duration::from_millis(500))
            .is_err(),
        "the scan kept reading after its handle was dropped"
    );
    assert_eq!(gate.stub.reads(), 1);
}

// --- what the prefilter skips ---

fn parsed(kinds: ContentKinds, prefilter: bool) -> usize {
    let corpus = corpus();
    let stub = StubParser::wrapping(&ClaudeCodeParser).leak();
    let sessions = with_parser(&claude_sessions(&corpus), stub);
    let summary = scan_blocking(
        &sessions,
        &ContentQuery::new("needle in a haystack", kinds),
        ScanOptions {
            prefilter,
            workers: 1,
            ..ScanOptions::default()
        },
        &CancelToken::new(),
    );
    assert_eq!(
        summary.results.len(),
        2,
        "the same sessions match however many are parsed"
    );
    stub.reads()
}

#[test]
fn a_selective_query_parses_only_the_files_that_could_match() {
    // claude-plain and claude-escaped say it; claude-quiet does not.
    assert_eq!(parsed(ContentKinds::default(), true), 2);
    assert_eq!(parsed(ContentKinds::default(), false), 3);
}

#[test]
fn searching_tools_thinking_hidden_or_aborted_turns_parses_everything() {
    // Those turns include text the parsers synthesize or re-serialize, so
    // the file's bytes can't rule a session out.
    for kinds in [
        ContentKinds {
            tools: true,
            ..ContentKinds::default()
        },
        ContentKinds {
            thinking: true,
            ..ContentKinds::default()
        },
        ContentKinds {
            hidden: true,
            ..ContentKinds::default()
        },
        ContentKinds {
            aborted: true,
            ..ContentKinds::default()
        },
    ] {
        assert_eq!(parsed(kinds, true), 3, "{kinds:?}");
    }
}

fn everything() -> ContentKinds {
    ContentKinds {
        messages: true,
        tools: true,
        thinking: true,
        aborted: true,
        hidden: true,
    }
}

#[test]
fn synthesized_text_is_found_with_the_prefilter_on() {
    let corpus = synthesized_corpus();
    for text in [
        "reasoning hidden by the model",
        "reasoning hidden",
        "model: acme/turbo-9",
        "label `checkpoint`",
        "Branch summary",
        "first block of text",
        "second block",
        "needle",
        "nEedle",
    ] {
        let with = scan_blocking(
            &corpus.sessions,
            &ContentQuery::new(text, everything()),
            ScanOptions::default(),
            &CancelToken::new(),
        );
        let without = scan_blocking(
            &corpus.sessions,
            &ContentQuery::new(text, everything()),
            ScanOptions {
                prefilter: false,
                ..ScanOptions::default()
            },
            &CancelToken::new(),
        );
        let keys = |s: &super::ScanSummary| {
            s.results
                .iter()
                .map(|(key, _)| key.session_key.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&with), keys(&without), "query {text:?}");
        assert!(!keys(&with).is_empty(), "{text:?} matches something");
    }
}

#[test]
fn ascii_written_as_escapes_is_found_with_the_default_kinds() {
    let corpus = synthesized_corpus();
    let summary = scan_blocking(
        &corpus.sessions,
        &query("needle"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    let found: Vec<_> = summary
        .results
        .iter()
        .map(|(key, _)| key.session_key.as_str())
        .collect();
    assert_eq!(found, ["claude-ascii"]);
}

// --- keeping the newest hits ---

fn hit(turn_index: usize, minute: Option<u32>) -> Hit {
    Hit {
        turn_index,
        role: TurnRole::User,
        kind: TurnKind::Message,
        timestamp: minute
            .and_then(|m| chrono::DateTime::from_timestamp(1_700_000_000 + i64::from(m) * 60, 0)),
        snippet: String::new(),
        highlight: 0..0,
    }
}

#[test]
fn the_cap_keeps_the_newest_hits_in_turn_order() {
    let hits: Vec<Hit> = (0..MAX_HITS_PER_SESSION + 100)
        .map(|i| hit(i, Some(i as u32)))
        .collect();
    let detail = keep_newest(hits);
    assert_eq!(detail.total, MAX_HITS_PER_SESSION + 100);
    assert!(detail.truncated());
    assert_eq!(detail.hits.len(), MAX_HITS_PER_SESSION);
    assert_eq!(detail.hits.first().map(|h| h.turn_index), Some(100));
    assert_eq!(
        detail.hits.last().map(|h| h.turn_index),
        Some(MAX_HITS_PER_SESSION + 99)
    );
    assert!(
        detail
            .hits
            .windows(2)
            .all(|w| w[0].turn_index < w[1].turn_index)
    );
}

#[test]
fn without_timestamps_the_cap_keeps_the_latest_turns() {
    let hits: Vec<Hit> = (0..MAX_HITS_PER_SESSION + 5)
        .map(|i| hit(i, None))
        .collect();
    let detail = keep_newest(hits);
    assert_eq!(detail.hits.first().map(|h| h.turn_index), Some(5));
}

#[test]
fn under_the_cap_nothing_is_dropped() {
    let detail = keep_newest(vec![hit(0, Some(1)), hit(1, Some(2))]);
    assert!(!detail.truncated());
    assert_eq!(detail.hits.len(), 2);
}

#[test]
fn a_session_with_more_matches_than_the_cap_reports_the_true_total() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("claude");
    let lines: Vec<String> = (0..MAX_HITS_PER_SESSION + 20)
        .map(|i| {
            serde_json::json!({
                "type": "user",
                "uuid": format!("u{i}"),
                "parentUuid": if i == 0 { serde_json::Value::Null } else { serde_json::json!(format!("u{}", i - 1)) },
                "timestamp": format!("2026-06-01T{:02}:{:02}:00Z", i / 60, i % 60),
                "message": {"role": "user", "content": format!("repeat marker {i}")},
            })
            .to_string()
        })
        .collect();
    write_lines(&root.join("projects/p/many.jsonl"), &lines);
    let session = super::test_support::session("claude-code", "many", &root, &ClaudeCodeParser);

    let detail = session_detail(&session, &query("repeat marker"));

    assert_eq!(detail.total, MAX_HITS_PER_SESSION + 20);
    assert_eq!(detail.hits.len(), MAX_HITS_PER_SESSION);
    assert_eq!(detail.hits[0].turn_index, 20, "the oldest 20 were dropped");
    let summary = scan_blocking(
        &[session],
        &query("repeat marker"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    assert_eq!(summary.results[0].1.count, MAX_HITS_PER_SESSION + 20);
}
