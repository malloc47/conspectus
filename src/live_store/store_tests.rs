use std::collections::BTreeSet;
use std::sync::{Arc, Barrier};

use super::*;
use crate::live_store::{LiveContext, LiveEventKind as K, LiveTmux};
use crate::model::live::{LiveDetail as D, LiveState as S};

fn event(session: &str, at_ms: i64, kind: K) -> LiveEvent {
    LiveEvent::new("harness-a", session, at_ms, kind)
}

fn turn(session: &str, at_ms: i64) -> LiveEvent {
    event(session, at_ms, K::TurnStarted)
}

#[test]
fn an_event_is_written_atomically_and_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path().join("conspectus"));
    let first = turn("s1", 100).with_context(LiveContext {
        cwd: Some("/work/app".into()),
        tmux: Some(LiveTmux {
            socket: Some("/tmp/tmux-1000/default".into()),
            session: Some("app".into()),
            pane: Some("%3".into()),
        }),
        pid: Some(4242),
        pid_start: Some(77),
        ..LiveContext::default()
    });
    assert_eq!(
        store.apply_event(&first).unwrap(),
        Applied::Written { rev: 1 }
    );
    assert_eq!(
        store
            .apply_event(&event("s1", 200, K::TurnEnded { interrupted: false }))
            .unwrap(),
        Applied::Written { rev: 2 }
    );

    let file = store.read();
    assert_eq!(file.rev, 2);
    let entry = file.get("harness-a", "s1").unwrap();
    assert_eq!(entry.observation.state, S::Ready);
    assert_eq!(entry.observation.detail, Some(D::Finished));
    assert_eq!(entry.observation.pid, Some(4242));
    assert_eq!(entry.cwd.as_deref(), Some("/work/app"));
    let tmux = entry.tmux.as_ref().unwrap();
    assert_eq!(
        (
            tmux.socket.as_deref(),
            tmux.session.as_deref(),
            tmux.pane.as_deref()
        ),
        (Some("/tmp/tmux-1000/default"), Some("app"), Some("%3"))
    );

    let text = fs::read_to_string(store.path()).unwrap();
    assert!(text.ends_with("}\n"));
    assert!(text.contains("\"harness-a:s1\""));
    assert!(store.dir().join(LIVE_STATE_LOCK).exists());
    // No temporary file is left beside the store.
    let names: BTreeSet<String> = fs::read_dir(store.dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from([LIVE_STATE_FILE.to_string(), LIVE_STATE_LOCK.to_string()])
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&store.path()), 0o600);
        assert_eq!(mode(&store.dir().join(LIVE_STATE_LOCK)), 0o600);
        assert_eq!(mode(store.dir()), 0o700);
    }
}

#[test]
fn an_unchanged_entry_is_not_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    store.apply_event(&turn("s1", 100)).unwrap();
    let before = fs::read_to_string(store.path()).unwrap();
    assert_eq!(
        store.apply_event(&turn("s1", 100)).unwrap(),
        Applied::Unchanged
    );
    assert_eq!(fs::read_to_string(store.path()).unwrap(), before);
}

#[test]
fn a_reader_finds_no_live_state_in_a_missing_corrupt_newer_or_unreadable_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    assert_eq!(store.read(), LiveFile::default());
    assert!(!store.path().exists(), "a reader never writes");

    fs::write(store.path(), "{ truncated").unwrap();
    assert_eq!(store.read(), LiveFile::default());

    store_newer_file(&store);
    assert_eq!(store.read(), LiveFile::default());

    fs::remove_file(store.path()).unwrap();
    fs::create_dir(store.path()).unwrap();
    assert_eq!(store.read(), LiveFile::default());
}

fn store_newer_file(store: &LiveStore) {
    fs::write(
        store.path(),
        "{\"schema_version\": 99, \"rev\": 5, \"entries\": {\"harness-a:s1\": {}}}\n",
    )
    .unwrap();
}

#[test]
fn a_writer_leaves_a_newer_file_alone_and_heals_a_corrupt_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    store_newer_file(&store);
    let newer = fs::read_to_string(store.path()).unwrap();
    assert_eq!(
        store.apply_event(&turn("s1", 100)).unwrap(),
        Applied::NewerSchema
    );
    assert_eq!(fs::read_to_string(store.path()).unwrap(), newer);

    fs::write(store.path(), "{ truncated").unwrap();
    assert_eq!(
        store.apply_event(&turn("s1", 100)).unwrap(),
        Applied::Written { rev: 1 }
    );
    assert!(store.read().get("harness-a", "s1").is_some());
}

#[test]
fn a_writer_reports_a_file_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    fs::create_dir(store.path()).unwrap();
    assert!(store.apply_event(&turn("s1", 100)).is_err());
}

#[test]
fn a_busy_lock_drops_the_event_after_the_wait() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let (outcome, waited) = with_lock(store.dir(), || {
        let started = Instant::now();
        let outcome = store.apply_event(&turn("s1", 100)).unwrap();
        (outcome, started.elapsed())
    })
    .unwrap();
    assert_eq!(outcome, Applied::Dropped);
    assert!(waited >= LOCK_WAIT, "gave up after {waited:?}");
    assert!(waited < LOCK_WAIT * 10, "waited {waited:?}");
    assert!(!store.path().exists());

    // Once the lock is free the next event goes through.
    assert_eq!(
        store.apply_event(&turn("s1", 200)).unwrap(),
        Applied::Written { rev: 1 }
    );
}

#[test]
fn a_writer_waits_for_a_lock_freed_within_the_wait() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let held = Arc::new(Barrier::new(2));
    let holder = {
        let dir = store.dir().to_path_buf();
        let held = Arc::clone(&held);
        thread::spawn(move || {
            with_lock(&dir, || {
                held.wait();
                thread::sleep(Duration::from_millis(30));
            })
            .unwrap();
        })
    };
    held.wait();
    assert_eq!(
        store.apply_event(&turn("s1", 100)).unwrap(),
        Applied::Written { rev: 1 }
    );
    holder.join().unwrap();
}

#[test]
fn concurrent_writers_each_take_their_own_revision_in_lock_order() {
    const WRITERS: usize = 8;
    const EVENTS: i64 = 26;
    let dir = tempfile::tempdir().unwrap();
    // A generous wait, so a loaded machine drops nothing.
    let store = LiveStore::at(dir.path()).with_lock_wait(Duration::from_secs(30));
    let start = Arc::new(Barrier::new(WRITERS));
    let writers: Vec<_> = (0..WRITERS)
        .map(|writer| {
            let store = store.clone();
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                let session = format!("s{writer}");
                let mut revs = Vec::new();
                for n in 0..EVENTS {
                    // Each writer alternates its own session between two
                    // states and counts up a shared session's evidence.
                    let kind = if n % 2 == 0 {
                        K::TurnStarted
                    } else {
                        K::TurnEnded { interrupted: false }
                    };
                    for event in [
                        event(&session, n, kind),
                        turn("shared", n * 100 + writer as i64),
                    ] {
                        match store.apply_event(&event).unwrap() {
                            Applied::Written { rev } => revs.push(rev),
                            Applied::Unchanged => {}
                            other => panic!("writer {writer}: {other:?}"),
                        }
                    }
                }
                revs
            })
        })
        .collect();
    let mut per_writer = Vec::new();
    for writer in writers {
        per_writer.push(writer.join().unwrap());
    }

    let all: Vec<u64> = per_writer.iter().flatten().copied().collect();
    let distinct: BTreeSet<u64> = all.iter().copied().collect();
    assert_eq!(distinct.len(), all.len(), "two writes took one revision");
    let file = store.read();
    assert_eq!(file.rev, all.len() as u64, "a write was lost");
    assert_eq!(distinct, (1..=file.rev).collect());
    for revs in &per_writer {
        assert!(revs.windows(2).all(|pair| pair[0] < pair[1]));
    }
    for writer in 0..WRITERS {
        let entry = file.get("harness-a", &format!("s{writer}")).unwrap();
        // The last event each writer sent ended its turn.
        assert_eq!(entry.observation.state, S::Ready);
        assert_eq!(entry.observation.observed_ms, EVENTS - 1);
    }
    let shared = file.get("harness-a", "shared").unwrap();
    assert_eq!(
        shared.observation.observed_ms,
        (EVENTS - 1) * 100 + WRITERS as i64 - 1
    );
}

#[test]
fn an_event_with_an_id_that_is_not_an_id_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let not_an_id = event("two words", 100, K::TurnStarted);
    assert_eq!(store.apply_event(&not_an_id).unwrap(), Applied::Rejected);
    // One bad event refuses its whole batch.
    assert_eq!(
        store
            .apply_events(&SharedReducer, &[turn("s1", 100), not_an_id])
            .unwrap(),
        Applied::Rejected
    );
    assert!(!store.path().exists());
}

#[test]
fn snapshots_are_written_through_the_same_lock() {
    let dir = tempfile::tempdir().unwrap();
    let store = LiveStore::at(dir.path());
    let snapshot = LiveSnapshot {
        harness: "harness-b".into(),
        session_id: "session-1".into(),
        instance: "i1".into(),
        seq: 1,
        observed_ms: 1_000,
        context: LiveContext::default(),
        state: S::Waiting,
        detail: Some(D::Question),
        since_ms: 900,
        subagents: 0,
        via_subagent: false,
        heartbeat_s: 60,
    };
    assert_eq!(
        store.apply_snapshot(&snapshot).unwrap(),
        Applied::Written { rev: 1 }
    );
    assert_eq!(store.apply_snapshot(&snapshot).unwrap(), Applied::Unchanged);
    let dropped = with_lock(store.dir(), || store.apply_snapshot(&snapshot).unwrap()).unwrap();
    assert_eq!(dropped, Applied::Dropped);
    let entry = store.read();
    let entry = entry.get("harness-b", "session-1").unwrap();
    assert_eq!(entry.observation.detail, Some(D::Question));
    assert_eq!(entry.observation.since_ms, 900);
}
