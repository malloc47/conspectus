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
    };
    let mut newer = older.clone();
    newer.session_key = "new".to_string();
    newer.observed_epoch = 200;

    store.write_record(&newer).expect("write newer");
    older.session_key = "older-late-arrival".to_string();
    store.write_record(&older).expect("write older");

    assert_eq!(store.read_records(), vec![newer]);
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
