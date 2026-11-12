use super::*;

#[test]
fn latest_store_round_trips_hook_record() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let record = HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: "session-1".to_string(),
        cwd: Some("/work".to_string()),
        pid: Some(1),
        ppid: Some(2),
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: Some("/tmp/transcript.jsonl".to_string()),
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: 1_700_000_000,
        harness_version: Some("1.0.0".to_string()),
        background_harness: false,
    };

    let outcome = store.write_record(&record).expect("write record");

    assert!(!outcome.replaced_existing);
    assert!(outcome.path.is_file());
    assert_eq!(store.read_records(), vec![record]);
}

#[test]
fn latest_store_keeps_only_newest_record_for_mux() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let mut older = HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: "old".to_string(),
        cwd: Some("/work".to_string()),
        pid: None,
        ppid: None,
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch: 100,
        harness_version: None,
        background_harness: false,
    };
    let mut newer = older.clone();
    newer.session_key = "new".to_string();
    newer.observed_epoch = 200;

    store.write_record(&newer).expect("write newer");
    older.session_key = "older-late-arrival".to_string();
    store.write_record(&older).expect("write older");

    assert_eq!(store.read_records(), vec![newer]);
}

/// A Claude Code record from pane `pane` of tmux session `editor`.
fn pane_record(session_key: &str, pane: &str, observed_epoch: i64) -> HookRecord {
    HookRecord {
        schema_version: SCHEMA_VERSION,
        harness_key: "claude-code".to_string(),
        session_key: session_key.to_string(),
        cwd: Some("/work".to_string()),
        pid: Some(4242),
        ppid: None,
        tmux: Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some(pane.to_string()),
            socket_path: None,
        }),
        transcript_path: None,
        hook_event_name: Some("SessionStart".to_string()),
        observed_epoch,
        harness_version: None,
        background_harness: false,
    }
}

fn sessions(store: &HookStore) -> Vec<String> {
    let mut sessions: Vec<String> = store
        .read_records()
        .into_iter()
        .map(|record| record.session_key)
        .collect();
    sessions.sort();
    sessions
}

fn spool_json(store: &HookStore) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(store.latest_path()).expect("read spool"))
        .expect("spool is JSON")
}

#[test]
fn two_panes_of_one_tmux_session_keep_separate_records() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());

    store
        .write_record(&pane_record("alpha", "%1", 100))
        .expect("write pane 1");
    store
        .write_record(&pane_record("beta", "%2", 200))
        .expect("write pane 2");
    assert_eq!(sessions(&store), ["alpha", "beta"]);

    // A newer session in pane 1 replaces only pane 1's record.
    let outcome = store
        .write_record(&pane_record("gamma", "%1", 300))
        .expect("write pane 1 again");
    assert!(outcome.replaced_existing);
    assert_eq!(sessions(&store), ["beta", "gamma"]);
    let keys: Vec<String> = spool_json(&store)["records"]
        .as_object()
        .expect("records")
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        keys,
        ["tmux:default:editor:pane:%1", "tmux:default:editor:pane:%2"]
    );
}

#[test]
fn a_mux_record_without_a_pane_keeps_one_slot_per_mux_session() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let mut older = pane_record("old", "%1", 100);
    older.tmux.as_mut().expect("tmux").pane_id = None;
    let mut newer = older.clone();
    newer.session_key = "new".to_string();
    newer.pid = Some(5151);
    newer.observed_epoch = 200;

    store.write_record(&older).expect("write older");
    store.write_record(&newer).expect("write newer");
    assert_eq!(sessions(&store), ["new"]);
}

#[test]
fn a_record_outside_a_mux_takes_its_process_slot_then_its_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let outside = |session: &str, pid: Option<i64>, epoch: i64| HookRecord {
        tmux: None,
        pid,
        ..pane_record(session, "%1", epoch)
    };

    // Two processes in one directory keep a record each.
    store
        .write_record(&outside("first", Some(10), 100))
        .expect("write");
    store
        .write_record(&outside("second", Some(11), 200))
        .expect("write");
    assert_eq!(sessions(&store), ["first", "second"]);
    // A process that moves on to a new session replaces its own record.
    store
        .write_record(&outside("third", Some(10), 300))
        .expect("write");
    assert_eq!(sessions(&store), ["second", "third"]);
    // With no pid, records share their directory's slot.
    store
        .write_record(&outside("fourth", None, 400))
        .expect("write");
    store
        .write_record(&outside("fifth", None, 500))
        .expect("write");
    assert_eq!(sessions(&store), ["fifth", "second", "third"]);
}

#[test]
fn a_spool_under_the_per_session_key_moves_each_record_to_its_pane() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    // What a binary from before the pane slots wrote: one record per
    // tmux session, whichever pane last reported.
    fs::write(
        store.latest_path(),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "records": { "tmux:default:editor": pane_record("alpha", "%1", 100) }
        }))
        .expect("serialize"),
    )
    .expect("write old spool");
    assert_eq!(sessions(&store), ["alpha"]);

    // Another pane's record joins it instead of replacing it.
    store
        .write_record(&pane_record("beta", "%2", 200))
        .expect("write pane 2");
    assert_eq!(sessions(&store), ["alpha", "beta"]);
    let keys: Vec<String> = spool_json(&store)["records"]
        .as_object()
        .expect("records")
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        keys,
        ["tmux:default:editor:pane:%1", "tmux:default:editor:pane:%2"],
        "the next write stores every record under its pane slot"
    );
}

#[test]
fn an_old_key_and_a_pane_key_for_one_pane_keep_the_newer_record() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let write_spool = |old: HookRecord, current: HookRecord| {
        fs::write(
            store.latest_path(),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": 1,
                "records": {
                    "tmux:default:editor": old,
                    "tmux:default:editor:pane:%1": current,
                }
            }))
            .expect("serialize"),
        )
        .expect("write spool");
    };

    write_spool(
        pane_record("old-key-newer", "%1", 200),
        pane_record("pane-key-older", "%1", 100),
    );
    assert_eq!(sessions(&store), ["old-key-newer"]);
    write_spool(
        pane_record("old-key-older", "%1", 100),
        pane_record("pane-key-newer", "%1", 200),
    );
    assert_eq!(sessions(&store), ["pane-key-newer"]);
    write_spool(
        pane_record("old-key-tie", "%1", 100),
        pane_record("pane-key-tie", "%1", 100),
    );
    assert_eq!(sessions(&store), ["pane-key-tie"]);
}

#[test]
fn a_binary_from_before_the_pane_slots_adds_its_record_without_dropping_any() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    store
        .write_record(&pane_record("alpha", "%1", 100))
        .expect("write pane 1");
    store
        .write_record(&pane_record("beta", "%2", 200))
        .expect("write pane 2");

    // The older binary reads the same schema version, inserts its record
    // under the per-session key, and writes the map back.
    let mut spool = spool_json(&store);
    assert_eq!(spool["schema_version"], 1, "the schema version stays 1");
    spool["records"]["tmux:default:editor"] =
        serde_json::to_value(pane_record("gamma", "%3", 300)).expect("record");
    fs::write(
        store.latest_path(),
        serde_json::to_vec_pretty(&spool).expect("serialize"),
    )
    .expect("old binary write");

    assert_eq!(sessions(&store), ["alpha", "beta", "gamma"]);
}

#[test]
fn a_spool_from_a_newer_schema_is_read_as_empty_and_never_overwritten() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let newer = r#"{"schema_version": 2, "records": {"anything": {"shape": "unknown"}}}"#;
    fs::write(store.latest_path(), newer).expect("write newer spool");

    assert!(store.read_records().is_empty());
    let err = store
        .write_record(&pane_record("alpha", "%1", 100))
        .expect_err("a newer spool is left alone");
    assert!(err.to_string().contains("left untouched"), "{err}");
    assert_eq!(
        fs::read_to_string(store.latest_path()).expect("read"),
        newer
    );
}

#[test]
fn a_corrupt_spool_is_replaced_by_the_next_write() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    fs::write(store.latest_path(), "{ not json").expect("write corrupt spool");

    store
        .write_record(&pane_record("alpha", "%1", 100))
        .expect("write");
    assert_eq!(sessions(&store), ["alpha"]);
}

#[test]
fn the_spool_keeps_the_newest_records_up_to_its_bound() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = HookStore::new(temp.path());
    let total = MAX_SPOOL_RECORDS + 2;
    for index in 0..total {
        let epoch = 1_000 + i64::try_from(index).expect("small index");
        store
            .write_record(&pane_record(
                &format!("s{index}"),
                &format!("%{index}"),
                epoch,
            ))
            .expect("write");
    }

    let records = store.read_records();
    assert_eq!(records.len(), MAX_SPOOL_RECORDS);
    for gone in ["s0", "s1"] {
        assert!(
            records.iter().all(|record| record.session_key != gone),
            "{gone} was observed longest ago"
        );
    }
    let last = format!("s{}", total - 1);
    assert!(records.iter().any(|record| record.session_key == last));
}

#[test]
fn claude_payload_requires_session_id() {
    let err = hook_record_from_payload(
        "claude-code",
        &serde_json::json!({"cwd": "/work"}),
        Some(1),
        Some(2),
        None,
        None,
        100,
    )
    .expect_err("missing id");

    assert!(err.to_string().contains("session_id"));
}

#[test]
fn codex_payload_builds_hook_record() {
    let record = hook_record_from_payload(
        "codex",
        &serde_json::json!({
            "session_id": "019e531f-19ee-7823-816f-4526ef89d70b",
            "transcript_path": "/home/me/.codex/sessions/2026/05/23/rollout.jsonl",
            "cwd": "/work",
            "hook_event_name": "SessionStart"
        }),
        Some(10),
        Some(9),
        Some(HookTmuxRecord {
            session_name: Some("editor".to_string()),
            native_id: None,
            pane_id: Some("%1".to_string()),
            socket_path: None,
        }),
        Some("0.128.0".to_string()),
        100,
    )
    .expect("record");

    assert_eq!(record.harness_key, "codex");
    assert_eq!(record.session_key, "019e531f-19ee-7823-816f-4526ef89d70b");
    assert_eq!(
        record.transcript_path.as_deref(),
        Some("/home/me/.codex/sessions/2026/05/23/rollout.jsonl")
    );
    assert_eq!(record.cwd.as_deref(), Some("/work"));
    assert_eq!(record.hook_event_name.as_deref(), Some("SessionStart"));
    assert_eq!(record.pid, Some(10));
    assert_eq!(record.ppid, Some(9));
}

#[test]
fn opencode_payload_builds_hook_record() {
    let record = hook_record_from_payload(
        "opencode",
        &serde_json::json!({
            "session_id": "ses_01HZX2J5Y",
            "cwd": "/home/me/src/proj",
            "hook_event_name": "session.updated"
        }),
        Some(42),
        Some(41),
        Some(HookTmuxRecord {
            session_name: Some("work".to_string()),
            native_id: Some("$3".to_string()),
            pane_id: Some("%5".to_string()),
            socket_path: Some("/run/user/1000/tmux-1000/default".to_string()),
        }),
        Some("1.14.19".to_string()),
        1_700_000_500,
    )
    .expect("record");

    assert_eq!(record.harness_key, "opencode");
    assert_eq!(record.session_key, "ses_01HZX2J5Y");
    assert_eq!(record.cwd.as_deref(), Some("/home/me/src/proj"));
    assert_eq!(record.hook_event_name.as_deref(), Some("session.updated"));
    assert_eq!(record.pid, Some(42));
    assert_eq!(record.ppid, Some(41));
    assert_eq!(record.harness_version.as_deref(), Some("1.14.19"));
    assert!(record.transcript_path.is_none());
    let tmux = record.tmux.as_ref().expect("tmux carried through");
    assert_eq!(tmux.pane_id.as_deref(), Some("%5"));
}

#[test]
fn opencode_payload_requires_session_id() {
    let err = hook_record_from_payload(
        "opencode",
        &serde_json::json!({"cwd": "/work"}),
        Some(1),
        Some(2),
        None,
        None,
        100,
    )
    .expect_err("missing id");
    assert!(err.to_string().contains("session_id"));
}

#[test]
fn opencode_payload_rejects_empty_session_id() {
    let err = hook_record_from_payload(
        "opencode",
        &serde_json::json!({"session_id": ""}),
        Some(1),
        Some(2),
        None,
        None,
        100,
    )
    .expect_err("empty id");
    assert!(err.to_string().contains("empty"));
}

#[test]
fn pi_payload_builds_hook_record() {
    let record = hook_record_from_payload(
        "pi",
        &serde_json::json!({
            "session_id": "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d",
            "cwd": "/home/me/src/proj",
            "transcript_path": "/home/me/.pi/agent/sessions/--home-me-src-proj--/2026-10-16T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl",
            "hook_event_name": "session_start:resume"
        }),
        Some(42),
        Some(41),
        None,
        Some("1".to_string()),
        1_700_000_500,
    )
    .expect("record");

    assert_eq!(record.harness_key, "pi");
    assert_eq!(record.session_key, "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d");
    assert_eq!(record.cwd.as_deref(), Some("/home/me/src/proj"));
    assert_eq!(
        record.transcript_path.as_deref(),
        Some(
            "/home/me/.pi/agent/sessions/--home-me-src-proj--/2026-10-16T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl"
        )
    );
    assert_eq!(
        record.hook_event_name.as_deref(),
        Some("session_start:resume")
    );
    assert_eq!(record.harness_version.as_deref(), Some("1"));
}

#[test]
fn pi_payload_without_transcript_path_builds_hook_record() {
    let record = hook_record_from_payload(
        "pi",
        &serde_json::json!({"session_id": "s1", "cwd": "/work"}),
        None,
        None,
        None,
        None,
        100,
    )
    .expect("record");
    assert_eq!(record.session_key, "s1");
    assert!(record.transcript_path.is_none());
}

#[test]
fn pi_payload_requires_a_nonempty_session_id() {
    for payload in [
        serde_json::json!({"cwd": "/work"}),
        serde_json::json!({"session_id": ""}),
    ] {
        let err = hook_record_from_payload("pi", &payload, None, None, None, None, 100)
            .expect_err("no usable id");
        assert!(err.to_string().contains("session_id"), "{err}");
    }
}

/// The payloads the opencode plugin's 2.x TUI entrypoint
/// (`plugins/opencode-hook/src/v2/tui.ts`) sends: one per route change
/// and one per forwarded lifecycle event of the routed session.
#[test]
fn opencode_2x_tui_payloads_build_hook_records() {
    for event in [
        "tui.route",
        "session.created",
        "session.renamed",
        "session.status",
        "session.idle",
        "session.compacted",
    ] {
        let record = hook_record_from_payload(
            "opencode",
            &serde_json::json!({
                "session_id": "ses_eb2483494ffe5iKYZYXs2LCXKS",
                "cwd": "/home/user/proj",
                "hook_event_name": event
            }),
            Some(4242),
            Some(4241),
            Some(HookTmuxRecord {
                session_name: Some("paneA".to_string()),
                native_id: None,
                pane_id: Some("%0".to_string()),
                socket_path: Some("/run/user/1000/tmux-1000/default".to_string()),
            }),
            Some("0.2.0".to_string()),
            1_700_000_500,
        )
        .expect("record");

        assert_eq!(
            record.session_key, "ses_eb2483494ffe5iKYZYXs2LCXKS",
            "{event}"
        );
        assert_eq!(record.hook_event_name.as_deref(), Some(event));
        assert_eq!(record.cwd.as_deref(), Some("/home/user/proj"));
        assert_eq!(record.harness_version.as_deref(), Some("0.2.0"));
        assert!(!record.background_harness);
        assert_eq!(
            record.tmux.as_ref().and_then(|t| t.pane_id.as_deref()),
            Some("%0")
        );
    }
}
