use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::test_support::corpus;
use super::{CancelToken, ScanEvent, ScanOptions, ScanSession, hits_for, scan, scan_blocking};
use crate::viewer::search::{ContentKinds, ContentQuery};

fn query(text: &str) -> ContentQuery {
    ContentQuery::new(text, ContentKinds::default())
}

fn matched(sessions: &[ScanSession], text: &str, options: ScanOptions) -> BTreeSet<String> {
    scan_blocking(sessions, &query(text), options, &CancelToken::new())
        .results
        .into_iter()
        .map(|(key, _)| key.session_key)
        .collect()
}

#[test]
fn a_phrase_finds_every_session_that_says_it() {
    let corpus = corpus();
    let found = matched(
        &corpus.sessions,
        "needle in a haystack",
        ScanOptions::default(),
    );
    let expected: BTreeSet<String> = [
        "claude-plain",
        "claude-escaped",
        "codex-plain",
        "codex-escaped",
        "pi-plain",
        "pi-escaped",
        "ses_all",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn the_prefilter_changes_what_is_parsed_but_never_what_is_found() {
    let corpus = corpus();
    for text in [
        "needle in a haystack",
        "NEEDLE",
        "say \"hello\" to",
        "\"hello\"",
        "C:\\Users\\dev",
        "\\notes.txt",
        "line one\nline two",
        "caf\u{e9}",
        "\u{65e5}\u{672c}\u{8a9e}",
        "n\u{ef}code",
        "path/to/file.rs",
        "to/file",
        "tab\there",
        "\u{1f600} done",
        "a needle",
        "nothing here",
        "absent phrase",
    ] {
        let with = matched(&corpus.sessions, text, ScanOptions::default());
        let without = matched(
            &corpus.sessions,
            text,
            ScanOptions {
                prefilter: false,
                ..ScanOptions::default()
            },
        );
        assert_eq!(with, without, "query {text:?}");
    }
}

#[test]
fn results_do_not_depend_on_how_many_workers_run() {
    let corpus = corpus();
    let one = matched(
        &corpus.sessions,
        "hello",
        ScanOptions {
            workers: 1,
            ..ScanOptions::default()
        },
    );
    for workers in [2, 3, 8] {
        let many = matched(
            &corpus.sessions,
            "hello",
            ScanOptions {
                workers,
                ..ScanOptions::default()
            },
        );
        assert_eq!(one, many, "{workers} workers");
    }
    assert!(!one.is_empty());
}

#[test]
fn progress_counts_every_session_and_the_ones_it_could_not_read() {
    let mut corpus = corpus();
    let mut missing = corpus.sessions[0].clone();
    missing.key.session_key = "not-on-disk".to_string();
    missing.locator.session_key = "not-on-disk".to_string();
    corpus.sessions.push(missing);
    let total = corpus.sessions.len();

    let summary = scan_blocking(
        &corpus.sessions,
        &query("needle in a haystack"),
        ScanOptions::default(),
        &CancelToken::new(),
    );

    assert_eq!(summary.progress.total, total);
    assert_eq!(summary.progress.scanned, total);
    assert_eq!(summary.progress.skipped, 1);
    assert!(!summary.cancelled);
}

#[test]
fn a_database_that_is_not_there_skips_its_sessions_and_nothing_else() {
    let mut corpus = corpus();
    for session in corpus
        .sessions
        .iter_mut()
        .filter(|s| s.key.harness_key == "opencode")
    {
        session.locator.state_root = corpus.dir.path().join("gone.db");
    }
    let summary = scan_blocking(
        &corpus.sessions,
        &query("hello"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    assert_eq!(summary.progress.skipped, 2);
    assert!(summary.results.len() >= 6, "file harnesses still match");
}

#[test]
fn an_empty_query_finds_nothing_and_reads_nothing() {
    let corpus = corpus();
    let summary = scan_blocking(
        &corpus.sessions,
        &query("   "),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    assert!(summary.results.is_empty());
    assert_eq!(summary.progress.total, corpus.sessions.len());
}

#[test]
fn a_streamed_scan_stamps_its_generation_and_ends_with_done() {
    let corpus = corpus();
    let handle = scan(
        corpus.sessions.clone(),
        query("needle in a haystack"),
        ScanOptions {
            generation: 7,
            ..ScanOptions::default()
        },
    );
    let mut events = Vec::new();
    while let Some(event) = handle.recv() {
        events.push(event);
    }
    assert!(events.iter().all(|e| e.generation() == 7));
    let Some(ScanEvent::Done {
        progress,
        matched,
        cancelled,
        ..
    }) = events.last()
    else {
        panic!("the last event is Done: {events:?}");
    };
    assert_eq!(*matched, 7);
    assert_eq!(progress.scanned, corpus.sessions.len());
    assert!(!cancelled);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ScanEvent::Session { .. }))
            .count(),
        7
    );
}

#[test]
fn progress_events_never_go_backwards_in_total() {
    let corpus = corpus();
    let handle = scan(
        corpus.sessions.clone(),
        query("hello"),
        ScanOptions::default(),
    );
    let mut seen = 0;
    while let Some(event) = handle.recv() {
        if let ScanEvent::Progress { progress, .. } = event {
            assert_eq!(progress.total, corpus.sessions.len());
            assert!(progress.scanned <= progress.total);
            seen += 1;
        }
    }
    assert_eq!(
        seen,
        corpus.sessions.len(),
        "one progress event per session"
    );
}

/// A scan over many sessions that is cancelled after its first event stops
/// well short of the end, and says it was cancelled.
#[test]
fn cancelling_stops_the_scan_promptly() {
    let corpus = corpus();
    let many: Vec<ScanSession> = corpus
        .sessions
        .iter()
        .cycle()
        .take(corpus.sessions.len() * 600)
        .cloned()
        .collect();
    let total = many.len();
    let handle = scan(
        many,
        query("nothing matches this phrase at all zzz"),
        ScanOptions {
            workers: 1,
            ..ScanOptions::default()
        },
    );
    assert!(handle.recv().is_some(), "the scan started");
    let started = Instant::now();
    handle.cancel();
    let mut last = None;
    while let Some(event) = handle.recv() {
        last = Some(event);
    }
    let Some(ScanEvent::Done {
        progress,
        cancelled,
        ..
    }) = last
    else {
        panic!("a cancelled scan still ends with Done");
    };
    assert!(cancelled);
    assert!(
        progress.scanned < total,
        "stopped early: {}/{total}",
        progress.scanned
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stopped promptly: {:?}",
        started.elapsed()
    );
}

#[test]
fn dropping_the_handle_cancels_the_scan() {
    let corpus = corpus();
    let many: Vec<ScanSession> = corpus
        .sessions
        .iter()
        .cycle()
        .take(corpus.sessions.len() * 600)
        .cloned()
        .collect();
    let handle = scan(
        many,
        query("nothing matches this phrase at all zzz"),
        ScanOptions {
            workers: 1,
            ..ScanOptions::default()
        },
    );
    let token = handle.cancel_token();
    assert!(!token.is_cancelled());
    drop(handle);
    assert!(token.is_cancelled());
}

#[test]
fn a_scan_cancelled_before_it_starts_reads_nothing() {
    let corpus = corpus();
    let token = CancelToken::new();
    token.cancel();
    let summary = scan_blocking(
        &corpus.sessions,
        &query("hello"),
        ScanOptions::default(),
        &token,
    );
    assert!(summary.cancelled);
    assert!(summary.results.is_empty());
    assert_eq!(summary.progress.scanned, 0);
}

#[test]
fn hits_for_returns_every_matching_turn_of_one_session() {
    let corpus = corpus();
    let session = corpus
        .sessions
        .iter()
        .find(|s| s.key.session_key == "claude-plain")
        .expect("session");
    let hits = hits_for(session, &query("e"));
    assert!(hits.len() >= 5, "{} hits", hits.len());
    assert!(hits.windows(2).all(|w| w[0].turn_index < w[1].turn_index));
}

#[test]
fn the_summary_keeps_the_true_count_and_one_best_hit() {
    let corpus = corpus();
    let summary = scan_blocking(
        &corpus.sessions,
        &query("e"),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    let (_, hits) = summary
        .results
        .iter()
        .find(|(key, _)| key.session_key == "claude-plain")
        .expect("matched");
    assert_eq!(hits.count, hits_for(&corpus.sessions[0], &query("e")).len());
}

#[test]
fn tool_content_turns_the_prefilter_off() {
    // Tool bodies are re-serialized, so the filter only runs when they
    // aren't searched; with them searched every file is parsed.
    let corpus = corpus();
    let kinds = ContentKinds {
        tools: true,
        ..ContentKinds::default()
    };
    let with = scan_blocking(
        &corpus.sessions,
        &ContentQuery::new("needle in a haystack", kinds),
        ScanOptions::default(),
        &CancelToken::new(),
    );
    assert_eq!(with.results.len(), 7);
}

/// Generates a synthetic corpus of roughly `CONSPECTUS_SCAN_BENCH_MB`
/// megabytes (default 300) and times the scan on it. Run with
/// `CONSPECTUS_SCAN_BENCH_MB=300 cargo test --release scan_benchmark
/// -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: generates a large corpus; run with --ignored"]
fn scan_benchmark() {
    use super::test_support::Words;
    use crate::viewer::parser::claude_code::ClaudeCodeParser;
    use crate::viewer::parser::codex::CodexParser;
    use std::io::Write;

    let target_mb: usize = std::env::var("CONSPECTUS_SCAN_BENCH_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let dir = tempfile::tempdir().expect("tempdir");
    let claude = dir.path().join("claude");
    let codex = dir.path().join("codex");
    let mut sessions = Vec::new();
    let mut words = Words::new(42);
    let mut bytes_written = 0usize;
    let mut n = 0usize;
    let rare = "zebra-quartz-7731";
    while bytes_written < target_mb * 1024 * 1024 {
        n += 1;
        let is_claude = !n.is_multiple_of(3);
        let key = format!("bench-{n:05}");
        let size = if n.is_multiple_of(40) {
            24 << 20
        } else {
            1 << 20
        };
        let (path, root, harness, parser): (
            _,
            _,
            _,
            &'static dyn crate::viewer::parser::HarnessParser,
        ) = if is_claude {
            (
                claude.join("projects/p").join(format!("{key}.jsonl")),
                &claude,
                "claude-code",
                &ClaudeCodeParser,
            )
        } else {
            (
                codex
                    .join("sessions/2026/05/12")
                    .join(format!("rollout-2026-05-12T13-18-53-{key}.jsonl")),
                &codex,
                "codex",
                &CodexParser,
            )
        };
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let mut file = std::io::BufWriter::new(std::fs::File::create(&path).expect("create"));
        let mut written = 0usize;
        let mut i = 0usize;
        while written < size {
            i += 1;
            let mut text = words.text(60);
            if n.is_multiple_of(97) && i == 5 {
                text.push(' ');
                text.push_str(rare);
            }
            let line = if is_claude {
                if i.is_multiple_of(3) {
                    let result = words.text(400);
                    serde_json::json!({"type":"user","uuid":format!("u{i}"),"parentUuid":format!("u{}", i-1),"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":result}]}})
                } else if i.is_multiple_of(2) {
                    serde_json::json!({"type":"assistant","uuid":format!("u{i}"),"parentUuid":format!("u{}", i-1),"message":{"role":"assistant","content":[{"type":"text","text":text}]}})
                } else {
                    serde_json::json!({"type":"user","uuid":format!("u{i}"),"parentUuid":format!("u{}", i-1),"message":{"role":"user","content":text}})
                }
            } else {
                serde_json::json!({"type":"response_item","payload":{"type":"message","role":if i.is_multiple_of(2) {"assistant"} else {"user"},"content":[{"type":if i.is_multiple_of(2) {"output_text"} else {"input_text"},"text":text}]}})
            }
            .to_string();
            writeln!(file, "{line}").expect("write");
            written += line.len() + 1;
        }
        bytes_written += written;
        sessions.push(super::ScanSession {
            key: super::SessionKey {
                harness_key: harness.to_string(),
                session_key: key.clone(),
            },
            locator: crate::viewer::model::SessionLocator {
                harness_key: harness.to_string(),
                session_key: key,
                state_root: root.clone(),
            },
            parser,
        });
    }
    println!(
        "corpus: {} sessions, {:.0} MB, {} cpus",
        sessions.len(),
        bytes_written as f64 / 1_048_576.0,
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    );
    for (label, text, prefilter) in [
        ("rare phrase, prefilter", rare, true),
        ("rare phrase, full parse", rare, false),
        ("absent phrase, prefilter", "no-such-phrase-anywhere", true),
        ("common word, prefilter", "refactor module", true),
    ] {
        let started = Instant::now();
        let summary = scan_blocking(
            &sessions,
            &query(text),
            ScanOptions {
                prefilter,
                ..ScanOptions::default()
            },
            &CancelToken::new(),
        );
        println!(
            "{label:<28} {:>8.2?}  matched {:>4}  scanned {}",
            started.elapsed(),
            summary.results.len(),
            summary.progress.scanned
        );
    }
}
