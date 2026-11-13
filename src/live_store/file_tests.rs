use super::*;
use crate::live_store::{LiveEventKind as K, SharedReducer, WaitKind};
use crate::model::live::{LiveDetail as D, LiveState as S};

const HOUR_MS: i64 = 60 * 60 * 1000;
const NOW: i64 = 1_800_000_000_000;

fn unknown_liveness(_: &LiveObservation) -> Liveness {
    Liveness::Unknown
}

fn event_in(session: &str, pid: Option<i64>, at_ms: i64, kind: K) -> LiveEvent {
    LiveEvent::new("harness-a", session, at_ms, kind).with_context(LiveContext {
        pid,
        ..LiveContext::default()
    })
}

fn apply(file: &mut LiveFile, events: &[LiveEvent], now_ms: i64) -> bool {
    file.apply_events(&SharedReducer, events, now_ms, &unknown_liveness)
}

fn state_of(file: &LiveFile, session: &str) -> (S, Option<D>) {
    let entry = file.get("harness-a", session).expect("an entry");
    (entry.observation.state, entry.observation.detail)
}

fn snapshot(instance: &str, seq: u64, observed_ms: i64, state: S) -> LiveSnapshot {
    LiveSnapshot {
        harness: "harness-b".into(),
        session_id: "session-1".into(),
        instance: instance.into(),
        seq,
        observed_ms,
        context: LiveContext::default(),
        state,
        detail: None,
        since_ms: observed_ms,
        subagents: 0,
        via_subagent: false,
        heartbeat_s: 60,
    }
}

fn snapshot_state(file: &LiveFile) -> (S, u64, i64) {
    let entry = file.get("harness-b", "session-1").expect("an entry");
    (
        entry.observation.state,
        entry.seq.unwrap(),
        entry.observation.observed_ms,
    )
}

#[test]
fn every_write_takes_the_next_revision_and_stamps_the_entries_it_changed() {
    let mut file = LiveFile::default();
    assert!(apply(
        &mut file,
        &[event_in("s1", None, 100, K::TurnStarted)],
        NOW
    ));
    assert!(apply(
        &mut file,
        &[
            event_in("s2", None, 200, K::TurnStarted),
            event_in("s2", None, 210, K::TurnEnded { interrupted: false }),
        ],
        NOW + 1
    ));
    assert_eq!(file.rev, 2);
    assert_eq!(file.get("harness-a", "s1").unwrap().rev, 1);
    let s2 = file.get("harness-a", "s2").unwrap();
    assert_eq!((s2.rev, s2.changed_ms), (2, NOW + 1));
    assert_eq!(state_of(&file, "s2"), (S::Ready, Some(D::Finished)));
    // An event that changes nothing is no write.
    assert!(!apply(
        &mut file,
        &[event_in(
            "s2",
            None,
            210,
            K::TurnEnded { interrupted: false }
        )],
        NOW + 2
    ));
    assert_eq!(file.rev, 2);
}

#[test]
fn a_new_session_id_from_the_same_pid_ends_the_old_entry() {
    let mut file = LiveFile::default();
    apply(
        &mut file,
        &[event_in("old", Some(42), 100, K::TurnStarted)],
        NOW,
    );
    // The operator cleared the conversation and the old session's end
    // was dropped.
    apply(
        &mut file,
        &[event_in(
            "new",
            Some(42),
            200,
            K::SessionStarted {
                after_compaction: false,
            },
        )],
        NOW + 1,
    );
    assert_eq!(state_of(&file, "old"), (S::Exited, Some(D::Ended)));
    assert_eq!(state_of(&file, "new"), (S::Ready, None));
    let old = file.get("harness-a", "old").unwrap();
    assert_eq!((old.rev, old.observation.since_ms), (2, 200));

    // A late event of the old session from that process neither revives
    // it nor ends the new one.
    assert!(!apply(
        &mut file,
        &[event_in("old", Some(42), 210, K::TurnStarted)],
        NOW + 2
    ));
    assert_eq!(state_of(&file, "new"), (S::Ready, None));
}

#[test]
fn supersede_needs_the_same_harness_and_a_pid() {
    let mut file = LiveFile::default();
    apply(
        &mut file,
        &[event_in("a", Some(42), 100, K::TurnStarted)],
        NOW,
    );
    let other_harness =
        LiveEvent::new("harness-z", "b", 200, K::TurnStarted).with_context(LiveContext {
            pid: Some(42),
            ..LiveContext::default()
        });
    apply(&mut file, &[other_harness], NOW);
    apply(&mut file, &[event_in("c", None, 300, K::TurnStarted)], NOW);
    apply(
        &mut file,
        &[event_in("d", Some(43), 300, K::TurnStarted)],
        NOW,
    );
    assert_eq!(state_of(&file, "a"), (S::Working, Some(D::Generating)));
}

#[test]
fn a_pid_reused_by_a_new_process_ends_the_old_entry_as_killed() {
    let mut file = LiveFile::default();
    let with_start = |session: &str, start: u64| {
        LiveEvent::new("harness-a", session, 100, K::TurnStarted).with_context(LiveContext {
            pid: Some(42),
            pid_start: Some(start),
            ..LiveContext::default()
        })
    };
    apply(&mut file, &[with_start("old", 1)], NOW);
    apply(&mut file, &[with_start("new", 2)], NOW);
    assert_eq!(state_of(&file, "old"), (S::Exited, Some(D::Killed)));
}

#[test]
fn a_snapshot_of_the_same_instance_replaces_the_entry_only_with_a_higher_seq() {
    let mut file = LiveFile::default();
    assert!(file.apply_snapshot(
        &snapshot("i1", 5, 1_000, S::Working),
        NOW,
        &unknown_liveness
    ));
    // A lower or equal seq loses even with a later time.
    assert!(!file.apply_snapshot(&snapshot("i1", 4, 9_000, S::Ready), NOW, &unknown_liveness));
    assert!(!file.apply_snapshot(&snapshot("i1", 5, 9_000, S::Ready), NOW, &unknown_liveness));
    assert_eq!(snapshot_state(&file), (S::Working, 5, 1_000));
    // A higher seq wins even with an earlier time.
    assert!(file.apply_snapshot(&snapshot("i1", 6, 900, S::Waiting), NOW, &unknown_liveness));
    assert_eq!(snapshot_state(&file), (S::Waiting, 6, 900));
    assert_eq!(file.rev, 2);
}

#[test]
fn a_snapshot_of_a_new_instance_replaces_the_entry_unless_it_is_older() {
    let mut file = LiveFile::default();
    file.apply_snapshot(
        &snapshot("i1", 50, 1_000, S::Working),
        NOW,
        &unknown_liveness,
    );
    // The plugin reloaded: its seq starts over.
    assert!(!file.apply_snapshot(&snapshot("i2", 1, 999, S::Ready), NOW, &unknown_liveness));
    assert!(file.apply_snapshot(&snapshot("i2", 1, 1_000, S::Ready), NOW, &unknown_liveness));
    assert_eq!(snapshot_state(&file), (S::Ready, 1, 1_000));
    assert_eq!(
        file.get("harness-b", "session-1")
            .unwrap()
            .instance
            .as_deref(),
        Some("i2")
    );
}

#[test]
fn a_heartbeat_refreshes_the_evidence_and_an_identical_resend_is_no_write() {
    let mut file = LiveFile::default();
    file.apply_snapshot(
        &snapshot("i1", 1, 1_000, S::Working),
        NOW,
        &unknown_liveness,
    );
    let mut heartbeat = snapshot("i1", 2, 61_000, S::Working);
    heartbeat.since_ms = 1_000;
    assert!(file.apply_snapshot(&heartbeat, NOW, &unknown_liveness));
    let entry = file.get("harness-b", "session-1").unwrap();
    assert_eq!(entry.observation.since_ms, 1_000);
    assert_eq!(entry.observation.observed_ms, 61_000);
    assert_eq!(entry.observation.heartbeat_s, 60);
    assert!(!file.apply_snapshot(&heartbeat, NOW, &unknown_liveness));
    assert_eq!(file.rev, 2);
}

#[test]
fn a_snapshot_keeps_known_attribution_and_drops_a_detail_its_state_lacks() {
    let mut file = LiveFile::default();
    let mut first = snapshot("i1", 1, 1_000, S::Working);
    first.context = LiveContext {
        cwd: Some("/work/app".into()),
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: Some("app".into()),
            pane: Some("%3".into()),
        }),
        pid: Some(42),
        pid_start: Some(9),
        ..LiveContext::default()
    };
    file.apply_snapshot(&first, NOW, &unknown_liveness);
    let mut second = snapshot("i1", 2, 2_000, S::Ready);
    second.detail = Some(D::Permission);
    second.subagents = 2;
    second.context.tmux = Some(LiveTmux {
        pane: Some("%3".into()),
        ..LiveTmux::default()
    });
    file.apply_snapshot(&second, NOW, &unknown_liveness);
    let entry = file.get("harness-b", "session-1").unwrap();
    assert_eq!(entry.observation.state, S::Ready);
    assert_eq!(entry.observation.detail, None);
    assert_eq!(entry.observation.subagents, 2);
    assert_eq!(entry.observation.pid, Some(42));
    assert_eq!(entry.observation.pid_start, Some(9));
    assert_eq!(entry.cwd.as_deref(), Some("/work/app"));
    assert_eq!(
        entry.tmux.as_ref().and_then(|tmux| tmux.session.as_deref()),
        Some("app")
    );
    assert!(entry.reducer.is_empty());
}

#[test]
fn a_new_pane_replaces_every_tmux_field_and_a_new_pid_forgets_the_old_start() {
    let mut entry = LiveEntry::new("harness-a", "s", 0);
    entry.merge_context(&LiveContext {
        cwd: Some("/a".into()),
        transcript_path: Some("/a/t.jsonl".into()),
        tmux: Some(LiveTmux {
            socket: Some("/tmp/sock".into()),
            session: Some("one".into()),
            pane: Some("%1".into()),
        }),
        pid: Some(10),
        pid_start: Some(5),
    });
    // Later events carry only what they know.
    entry.merge_context(&LiveContext {
        tmux: Some(LiveTmux {
            pane: Some("%1".into()),
            ..LiveTmux::default()
        }),
        pid: Some(10),
        ..LiveContext::default()
    });
    assert_eq!(entry.cwd.as_deref(), Some("/a"));
    assert_eq!(entry.observation.pid_start, Some(5));
    assert_eq!(entry.tmux.as_ref().unwrap().session.as_deref(), Some("one"));
    entry.merge_context(&LiveContext {
        tmux: Some(LiveTmux {
            pane: Some("%9".into()),
            ..LiveTmux::default()
        }),
        pid: Some(11),
        ..LiveContext::default()
    });
    assert_eq!(
        entry.tmux,
        Some(LiveTmux {
            pane: Some("%9".into()),
            ..LiveTmux::default()
        })
    );
    assert_eq!(
        (entry.observation.pid, entry.observation.pid_start),
        (Some(11), None)
    );
    assert_eq!(entry.transcript_path.as_deref(), Some("/a/t.jsonl"));
}

/// An entry for `session` that last changed at `changed_ms`.
fn aged(session: &str, state: S, changed_ms: i64, pid: Option<i64>) -> LiveEntry {
    let mut entry = LiveEntry::new("harness-a", session, changed_ms);
    entry.observation.state = state;
    entry.observation.pid = pid;
    entry.changed_ms = changed_ms;
    entry
}

fn file_of(entries: Vec<LiveEntry>) -> LiveFile {
    LiveFile {
        entries: entries.into_iter().map(|e| (e.key(), e)).collect(),
        ..LiveFile::default()
    }
}

#[test]
fn garbage_collection_drops_old_exits_and_long_dead_processes() {
    let mut file = file_of(vec![
        aged("exited-long-ago", S::Exited, NOW - HOUR_MS - 1, None),
        aged("exited-an-hour-ago", S::Exited, NOW - HOUR_MS, None),
        aged("dead-for-days", S::Working, NOW - 25 * HOUR_MS, Some(1)),
        aged(
            "dead-since-this-morning",
            S::Working,
            NOW - 2 * HOUR_MS,
            Some(1),
        ),
        aged("alive-for-days", S::Ready, NOW - 25 * HOUR_MS, Some(2)),
        aged("unchecked-for-days", S::Working, NOW - 25 * HOUR_MS, None),
    ]);
    let liveness = |observation: &LiveObservation| match observation.pid {
        Some(1) => Liveness::Dead,
        Some(_) => Liveness::Alive,
        None => Liveness::Unknown,
    };
    file.collect_garbage(NOW, &liveness);
    let kept: Vec<&str> = file
        .entries
        .values()
        .map(|e| e.session_id.as_str())
        .collect();
    assert_eq!(
        kept,
        vec![
            "alive-for-days",
            "dead-since-this-morning",
            "exited-an-hour-ago",
            "unchecked-for-days"
        ]
    );
}

#[test]
fn garbage_collection_keeps_the_most_recently_changed_entries() {
    let entries = (0..MAX_ENTRIES as i64 + 3)
        .map(|n| aged(&format!("s{n:04}"), S::Ready, NOW - 1_000 + n, None))
        .collect();
    let mut file = file_of(entries);
    file.collect_garbage(NOW, &unknown_liveness);
    assert_eq!(file.entries.len(), MAX_ENTRIES);
    for gone in ["s0000", "s0001", "s0002"] {
        assert!(file.get("harness-a", gone).is_none());
    }
    assert!(file.get("harness-a", "s0003").is_some());
}

#[test]
fn garbage_is_collected_on_every_write() {
    let mut file = file_of(vec![aged("old", S::Exited, NOW - 2 * HOUR_MS, None)]);
    apply(&mut file, &[event_in("s", None, 100, K::TurnStarted)], NOW);
    assert!(file.get("harness-a", "old").is_none());
}

#[test]
fn a_file_parses_tolerantly() {
    assert!(matches!(LiveFile::parse("not json"), Parsed::Corrupt));
    assert!(matches!(LiveFile::parse("{\"rev\": 3}"), Parsed::Corrupt));
    assert!(matches!(LiveFile::parse("[]"), Parsed::Corrupt));
    assert!(matches!(
        LiveFile::parse("{\"schema_version\": 2, \"entries\": {}}"),
        Parsed::Newer
    ));
    let Parsed::File(empty) = LiveFile::parse("{\"schema_version\": 1}") else {
        panic!("a bare header is an empty file");
    };
    assert!(empty.entries.is_empty());
}

#[test]
fn a_bad_entry_costs_only_itself_and_unknown_fields_are_ignored() {
    let mut file = LiveFile::default();
    apply(
        &mut file,
        &[
            event_in("good", Some(7), 100, K::TurnStarted),
            event_in(
                "good",
                Some(7),
                110,
                K::WaitStarted {
                    agent: None,
                    kind: WaitKind::Plan,
                },
            ),
        ],
        NOW,
    );
    let mut value = serde_json::to_value(&file).unwrap();
    value["entries"]["harness-a:good"]["added_by_a_later_version"] = serde_json::json!(true);
    value["entries"]["harness-a:good"]["observation"]["state"] = serde_json::json!("pondering");
    value["entries"]["harness-a:bad"] = serde_json::json!({ "harness": 3 });
    // A key that disagrees with its entry is read under the entry's own.
    let moved = value["entries"]["harness-a:good"].clone();
    value["entries"]["harness-a:mislabeled"] = moved;
    value["future_section"] = serde_json::json!({ "kept": true });
    let Parsed::File(read) = LiveFile::parse(&value.to_string()) else {
        panic!("a readable file");
    };
    assert_eq!(read.entries.len(), 1);
    let good = read.get("harness-a", "good").unwrap();
    assert_eq!(good.observation.state, S::Unknown);
    assert_eq!(good.observation.pid, Some(7));
    assert!(!good.reducer.is_empty());
    let rendered = read.render().unwrap();
    assert!(rendered.ends_with("}\n"));
    assert!(rendered.contains("\"future_section\""));
    assert!(!rendered.contains("added_by_a_later_version"));
}
