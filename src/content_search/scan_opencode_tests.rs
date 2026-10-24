//! The opencode path of the scan: sessions live in one SQLite database
//! (1.x `message`/`part`, 2.x `session_message`), and candidate sessions
//! are chosen in SQL before any are parsed.

use std::collections::BTreeSet;
use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::json;

use super::test_support::session;
use super::{CancelToken, ScanOptions, ScanSession, scan_blocking};
use crate::viewer::parser::HarnessParser;
use crate::viewer::parser::opencode::OpenCodeParser;
use crate::viewer::search::{ContentKinds, ContentQuery};

fn db(dir: &Path) -> Connection {
    let conn = Connection::open(dir.join("opencode.db")).expect("open db");
    conn.execute_batch(
        "CREATE TABLE session (id text PRIMARY KEY, directory text NOT NULL, \
           title text NOT NULL DEFAULT '', time_created integer NOT NULL DEFAULT 0);
         CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, \
           time_created integer NOT NULL, data text NOT NULL);
         CREATE TABLE part (id text PRIMARY KEY, message_id text NOT NULL, \
           session_id text, time_created integer NOT NULL, data text NOT NULL);
         CREATE TABLE session_v2 (id text PRIMARY KEY, directory text);
         CREATE TABLE session_message (id text PRIMARY KEY, session_id text NOT NULL, \
           type text NOT NULL, seq integer NOT NULL, time_created integer NOT NULL, \
           time_updated integer NOT NULL, data text NOT NULL);",
    )
    .expect("schema");
    conn
}

/// A 1.x session whose parts are `(type, json)` pairs.
fn v1_session(conn: &Connection, id: &str, parts: &[serde_json::Value]) {
    conn.execute(
        "INSERT INTO session (id, directory, time_created) VALUES (?1, '/oc', 1)",
        params![id],
    )
    .expect("session");
    for (i, part) in parts.iter().enumerate() {
        let message = format!("{id}-m{i}");
        conn.execute(
            "INSERT INTO message VALUES (?1, ?2, ?3, ?4)",
            params![
                message,
                id,
                100 + i as i64,
                json!({"role": "assistant"}).to_string()
            ],
        )
        .expect("message");
        conn.execute(
            "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                format!("{message}-p"),
                message,
                id,
                100 + i as i64,
                part.to_string()
            ],
        )
        .expect("part");
    }
}

/// A 2.x session of `(type, json)` messages.
fn v2_session(conn: &Connection, id: &str, messages: &[(&str, serde_json::Value)]) {
    conn.execute(
        "INSERT INTO session_v2 (id, directory) VALUES (?1, '/oc2')",
        params![id],
    )
    .expect("session_v2");
    for (seq, (kind, data)) in messages.iter().enumerate() {
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![
                format!("{id}-{seq}"),
                id,
                kind,
                seq as i64,
                1_792_300_000_000_i64 + seq as i64,
                data.to_string()
            ],
        )
        .expect("session_message");
    }
}

fn sessions(dir: &Path, keys: &[&str]) -> Vec<ScanSession> {
    keys.iter()
        .map(|key| session("opencode", key, &dir.join("opencode.db"), &OpenCodeParser))
        .collect()
}

fn found(
    sessions: &[ScanSession],
    text: &str,
    kinds: ContentKinds,
    prefilter: bool,
) -> BTreeSet<String> {
    scan_blocking(
        sessions,
        &ContentQuery::new(text, kinds),
        ScanOptions {
            prefilter,
            ..ScanOptions::default()
        },
        &CancelToken::new(),
    )
    .results
    .into_iter()
    .map(|(key, _)| key.session_key)
    .collect()
}

fn set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|k| (*k).to_string()).collect()
}

#[test]
fn a_two_point_x_session_is_found_through_session_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v2_session(
        &conn,
        "ses_v2",
        &[
            ("user", json!({"text": "please explain the quokka design"})),
            (
                "assistant",
                json!({"content": [{"type": "text", "text": "the quokka is a marsupial"}]}),
            ),
        ],
    );
    v2_session(
        &conn,
        "ses_v2_quiet",
        &[("user", json!({"text": "nothing relevant"}))],
    );
    drop(conn);
    let all = sessions(dir.path(), &["ses_v2", "ses_v2_quiet"]);

    assert_eq!(
        found(&all, "quokka design", ContentKinds::default(), true),
        set(&["ses_v2"])
    );
    assert_eq!(
        found(&all, "marsupial", ContentKinds::default(), true),
        set(&["ses_v2"])
    );
    assert_eq!(
        found(&all, "quokka", ContentKinds::default(), true),
        found(&all, "quokka", ContentKinds::default(), false)
    );
}

#[test]
fn a_mixed_one_point_x_and_two_point_x_database_is_searched_whole() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_one",
        &[json!({"type": "text", "text": "shared phrase old layout"})],
    );
    v2_session(
        &conn,
        "ses_two",
        &[("user", json!({"text": "shared phrase new layout"}))],
    );
    v1_session(
        &conn,
        "ses_other",
        &[json!({"type": "text", "text": "unrelated"})],
    );
    drop(conn);
    let all = sessions(dir.path(), &["ses_one", "ses_two", "ses_other"]);

    for prefilter in [true, false] {
        assert_eq!(
            found(&all, "shared phrase", ContentKinds::default(), prefilter),
            set(&["ses_one", "ses_two"]),
            "prefilter {prefilter}"
        );
    }
}

#[test]
fn a_subagent_session_in_the_same_database_is_searched_too() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_parent",
        &[json!({"type": "text", "text": "delegating the audit"})],
    );
    v1_session(
        &conn,
        "ses_child",
        &[json!({"type": "text", "text": "audit finished: two findings"})],
    );
    drop(conn);
    let all = sessions(dir.path(), &["ses_parent", "ses_child"]);

    assert_eq!(
        found(&all, "audit", ContentKinds::default(), true),
        set(&["ses_parent", "ses_child"])
    );
}

#[test]
fn tool_parts_are_searched_only_when_asked() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_tool",
        &[json!({
            "type": "tool",
            "tool": "bash",
            "state": {"input": {"command": "ls /tmp/scratch"}, "output": "scratch-listing"}
        })],
    );
    drop(conn);
    let all = sessions(dir.path(), &["ses_tool"]);
    let tools = ContentKinds {
        tools: true,
        ..ContentKinds::default()
    };

    assert!(found(&all, "scratch-listing", ContentKinds::default(), true).is_empty());
    assert_eq!(
        found(&all, "scratch-listing", tools, true),
        set(&["ses_tool"])
    );
    assert_eq!(found(&all, "/tmp/scratch", tools, true), set(&["ses_tool"]));
}

#[test]
fn accented_text_is_found_whether_or_not_the_json_escapes_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_plain",
        &[json!({"type": "text", "text": "un café noir"})],
    );
    // The same text written the way an encoder that escapes non-ASCII does.
    let escaped = r#"{"type":"text","text":"un caf\u00e9 noir"}"#;
    conn.execute(
        "INSERT INTO session (id, directory) VALUES ('ses_escaped', '/oc')",
        [],
    )
    .expect("session");
    conn.execute(
        "INSERT INTO message VALUES ('e-m', 'ses_escaped', 1, '{\"role\":\"user\"}')",
        [],
    )
    .expect("message");
    conn.execute(
        "INSERT INTO part VALUES ('e-p', 'e-m', 'ses_escaped', 1, ?1)",
        params![escaped],
    )
    .expect("part");
    drop(conn);
    let all = sessions(dir.path(), &["ses_plain", "ses_escaped"]);

    for text in ["café", "caf", "un café noir", "noir"] {
        assert_eq!(
            found(&all, text, ContentKinds::default(), true),
            found(&all, text, ContentKinds::default(), false),
            "query {text:?}"
        );
    }
    assert_eq!(
        found(&all, "café", ContentKinds::default(), true),
        set(&["ses_plain", "ses_escaped"])
    );
}

#[test]
fn candidates_are_the_sessions_with_one_row_holding_every_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    // The two words are in different parts of one session: no single turn
    // holds both, so the session can't match and need not be parsed.
    v1_session(
        &conn,
        "ses_split",
        &[
            json!({"type": "text", "text": "only alpha here"}),
            json!({"type": "text", "text": "only beta here"}),
        ],
    );
    v1_session(
        &conn,
        "ses_both",
        &[json!({"type": "text", "text": "alpha and beta together"})],
    );
    drop(conn);
    let runs = vec!["alpha".to_string(), "beta".to_string()];

    let candidates = OpenCodeParser
        .candidates(&dir.path().join("opencode.db"), &runs, &|| false)
        .expect("can narrow");

    assert_eq!(
        candidates.into_iter().collect::<BTreeSet<_>>(),
        set(&["ses_both"])
    );
}

#[test]
fn a_database_without_the_expected_tables_cannot_narrow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = Connection::open(dir.path().join("opencode.db")).expect("open");
    conn.execute_batch("CREATE TABLE unrelated (a text);")
        .expect("schema");
    drop(conn);

    let candidates = OpenCodeParser.candidates(
        &dir.path().join("opencode.db"),
        &["needle".to_string()],
        &|| false,
    );

    assert!(candidates.is_none(), "no table is not an empty answer");
}

#[test]
fn a_row_that_cannot_be_read_cannot_narrow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_ok",
        &[json!({"type": "text", "text": "needle here"})],
    );
    // A part with no session id: reading its row fails.
    conn.execute(
        "INSERT INTO part VALUES ('p-null', 'm', NULL, 1, '{\"type\":\"text\",\"text\":\"needle too\"}')",
        [],
    )
    .expect("part");
    drop(conn);

    let candidates = OpenCodeParser.candidates(
        &dir.path().join("opencode.db"),
        &["needle".to_string()],
        &|| false,
    );

    assert!(
        candidates.is_none(),
        "a failed row must not become 'no candidates'"
    );
}

#[test]
fn a_cancelled_selection_gives_up_narrowing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let conn = db(dir.path());
    v1_session(
        &conn,
        "ses_ok",
        &[json!({"type": "text", "text": "needle here"})],
    );
    drop(conn);

    let candidates = OpenCodeParser.candidates(
        &dir.path().join("opencode.db"),
        &["needle".to_string()],
        &|| true,
    );

    assert!(candidates.is_none());
}

#[test]
fn the_worker_connection_cannot_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    drop(db(dir.path()));
    assert!(
        crate::viewer::parser::opencode::worker_connection_refuses_writes(
            &dir.path().join("opencode.db")
        )
    );
}
