// Extracted from codex_log.rs H-HYG-011 rolling wave via #[path = "codex_log_tests.rs"] mod tests;
use std::fs;

use tempfile::TempDir;

use super::*;
use crate::model::{MuxSessionId, MuxSessionNode};

fn now() -> i64 {
    1_700_000_000
}

fn write_logs_db(path: &Path, rows: &[(&str, &str, i64)]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("logs db parent");
    }
    let conn = Connection::open(path).expect("open logs db fixture");
    conn.execute(
        "CREATE TABLE logs (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                ts INTEGER NOT NULL, \
                ts_nanos INTEGER NOT NULL DEFAULT 0, \
                level TEXT NOT NULL DEFAULT 'INFO', \
                target TEXT NOT NULL DEFAULT 'test', \
                thread_id TEXT, \
                process_uuid TEXT)",
        [],
    )
    .expect("create logs");
    for (process_uuid, thread_id, ts) in rows {
        conn.execute(
            "INSERT INTO logs (ts, thread_id, process_uuid) VALUES (?1, ?2, ?3)",
            params![ts, thread_id, process_uuid],
        )
        .expect("insert log row");
    }
}

fn agent_session_node(state_scope: &str, key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new(CODEX_HARNESS_KEY, state_scope, key),
        CODEX_HARNESS_KEY.to_string(),
    ))
}

fn mux_node(native_id: &str) -> GraphNode {
    GraphNode::MuxSession(
        MuxSessionNode::new(
            MuxSessionId::new(format!("tmux:{native_id}")),
            "tmux".to_string(),
            native_id.to_string(),
        )
        .with_cwd("/work".to_string())
        .with_active_pane_command("codex".to_string())
        .with_active_pane_pid(100)
        .with_active_pane_current_path("/work".to_string())
        .with_active_pane_start_command("codex --resume stale-session".to_string())
        .with_activity_epoch(now()),
    )
}

fn stale_command_match_link(state_scope: &str, native_id: &str, session_key: &str) -> GraphLink {
    let source = NodeId::AgentSession(AgentSessionId::new(
        CODEX_HARNESS_KEY,
        state_scope,
        session_key,
    ));
    let target = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{native_id}")));
    let mut fields = Metadata::new();
    fields.insert(
        "match_kind".to_string(),
        serde_json::Value::String(
            crate::resolve::evidence::ACTIVE_PANE_COMMAND_SESSION_MATCH.to_string(),
        ),
    );
    GraphLink {
        id: format!("cross_link:cmd:{session_key}:{native_id}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "cross_link".to_string(),
            evidence: Some(crate::resolve::evidence::ACTIVE_PANE_COMMAND_SESSION_MATCH.to_string()),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn build_snapshot(native_id: &str) -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::default();
    snapshot.nodes.push(mux_node(native_id));
    snapshot
}

fn codex_pid_map(native_id: &str, pid: i64) -> BTreeMap<MuxSessionId, Vec<(String, i64)>> {
    let mut map = BTreeMap::new();
    map.insert(
        MuxSessionId::new(format!("tmux:{native_id}")),
        vec![(CODEX_HARNESS_KEY.to_string(), pid)],
    );
    map
}

fn codex_log_mux_links(snapshot: &GraphSnapshot) -> Vec<&GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .filter(|l| {
            l.source_metadata.adapter == ADAPTER_NAME && l.relation == RelationKind::LinkedToMux
        })
        .collect()
}

#[test]
fn emits_linked_to_mux_for_freshest_thread_per_pid() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[
            ("pid:100:uuid-a", "thread-old", now() - 60),
            ("pid:100:uuid-a", "thread-current", now() - 10),
        ],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    // Slice-A would normally produce the AgentSession for thread-current;
    // include it here so we exercise the existing-session path.
    snapshot
        .nodes
        .push(agent_session_node(&state_scope, "thread-current"));

    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let log_links = codex_log_mux_links(&snapshot);
    assert_eq!(log_links.len(), 1);
    let NodeId::AgentSession(session_id) = &log_links[0].source else {
        panic!("expected agent session source");
    };
    assert_eq!(session_id.session_key, "thread-current");
    let pid = log_links[0]
        .source_metadata
        .fields
        .get("process_pid")
        .and_then(serde_json::Value::as_i64);
    assert_eq!(pid, Some(100));
    let suffix = log_links[0]
        .source_metadata
        .fields
        .get("process_uuid_suffix")
        .and_then(serde_json::Value::as_str);
    assert_eq!(suffix, Some("uuid-a"));
    assert!(snapshot.nodes.iter().any(|node| {
        matches!(
            node,
            GraphNode::RuntimeProcess(process)
                if process.pid == Some(100)
                    && process.harness_key.as_deref() == Some(CODEX_HARNESS_KEY)
        )
    }));
    assert!(
        snapshot
            .candidate_links
            .iter()
            .any(|link| link.relation == RelationKind::ProcessIdentifiesSession)
    );
}

#[test]
fn synthesizes_sparse_session_when_state_has_not_seen_thread_yet() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "fresh-thread", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    // No agent session for fresh-thread yet.
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let synth = snapshot.nodes.iter().find_map(|n| match n {
        GraphNode::AgentSession(s) if s.id.session_key == "fresh-thread" => Some(s),
        _ => None,
    });
    let synth = synth.expect("synthesized agent session");
    assert_eq!(synth.harness_key, CODEX_HARNESS_KEY);
    assert_eq!(synth.last_active_epoch, Some(now() - 5));

    let log_link_count = codex_log_mux_links(&snapshot).len();
    assert_eq!(log_link_count, 1);
}

#[test]
fn demotes_stale_active_pane_command_session_match_for_same_mux() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "current-thread", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    // Stale command match pointing at a different session for the same mux.
    snapshot.candidate_links.push(stale_command_match_link(
        &state_scope,
        "main",
        "stale-session",
    ));
    snapshot
        .nodes
        .push(agent_session_node(&state_scope, "stale-session"));
    snapshot
        .nodes
        .push(agent_session_node(&state_scope, "current-thread"));

    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let stale = snapshot
        .candidate_links
        .iter()
        .find(|l| matches!(&l.source, NodeId::AgentSession(s) if s.session_key == "stale-session"))
        .expect("stale command link present");
    match &stale.state {
        LinkState::Overridden { reason, .. } => {
            assert!(reason.as_ref().unwrap().contains("codex log"));
        }
        other => panic!("expected Overridden state, got {other:?}"),
    }
}

#[test]
fn corroborating_command_match_for_same_session_stays_active() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "thread-x", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    // Command match for the SAME session the log resolves to. Should
    // remain Active — it's corroborating, not stale.
    snapshot
        .candidate_links
        .push(stale_command_match_link(&state_scope, "main", "thread-x"));
    snapshot
        .nodes
        .push(agent_session_node(&state_scope, "thread-x"));

    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let corroborating = snapshot
        .candidate_links
        .iter()
        .find(|l| {
            matches!(&l.source, NodeId::AgentSession(s) if s.session_key == "thread-x")
                && l.source_metadata.adapter == "cross_link"
        })
        .expect("command link present");
    assert!(matches!(corroborating.state, LinkState::Active));
}

#[test]
fn log_rows_outside_freshness_window_are_ignored() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();
    // Older than the 24h query-performance bound. The pid set is
    // already filtered to live codex processes, so this test only
    // protects the query-cost guard; it is not a correctness
    // freshness gate.
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "stale-thread", now() - 25 * 60 * 60)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let count = codex_log_mux_links(&snapshot).len();
    assert_eq!(count, 0);
}

#[test]
fn unrelated_pid_does_not_produce_a_link() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:999:uuid-z", "other-thread", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let count = codex_log_mux_links(&snapshot).len();
    assert_eq!(count, 0);
}

#[test]
fn higher_log_db_suffix_wins_when_multiple_files_present() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();
    write_logs_db(
        &state_root.join("logs_1.sqlite"),
        &[("pid:100:uuid-old", "stale-from-old-db", now() - 5)],
    );
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-new", "current-from-new-db", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let link = codex_log_mux_links(&snapshot)
        .into_iter()
        .next()
        .expect("log link");
    let NodeId::AgentSession(session_id) = &link.source else {
        panic!("expected agent session source");
    };
    assert_eq!(session_id.session_key, "current-from-new-db");
}

#[test]
fn missing_log_db_returns_silently() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    let before = snapshot.candidate_links.len();
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    assert_eq!(snapshot.candidate_links.len(), before);
}

#[test]
fn empty_log_db_returns_silently() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let _state_scope = state_root.to_string_lossy().to_string();
    let db = state_root.join("logs_2.sqlite");
    Connection::open(&db).expect("empty db");

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    let before = snapshot.candidate_links.len();
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    assert_eq!(snapshot.candidate_links.len(), before);
}

#[test]
fn non_codex_pids_in_map_are_ignored() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "thread-x", now() - 5)],
    );

    let mut snapshot = build_snapshot("main");
    let mut pids: BTreeMap<MuxSessionId, Vec<(String, i64)>> = BTreeMap::new();
    pids.insert(
        MuxSessionId::new("tmux:main".to_string()),
        vec![("claude-code".to_string(), 100)],
    );
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );

    let count = codex_log_mux_links(&snapshot).len();
    assert_eq!(count, 0);
}

#[test]
fn caller_supplied_window_overrides_default_bound() {
    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        // Row is 30 minutes old; inside the 24h default but outside a
        // tight 5-minute custom bound.
        &[("pid:100:uuid-a", "thread-x", now() - 30 * 60)],
    );

    let mut snapshot = build_snapshot("main");
    let pids = codex_pid_map("main", 100);

    // Tight 5-minute window: row should be rejected as too old.
    apply_codex_log_attribution(&mut snapshot, state_root, &pids, now(), 5 * 60);
    let tight = codex_log_mux_links(&snapshot).len();
    assert_eq!(tight, 0);

    // Default window: same row is accepted.
    apply_codex_log_attribution(
        &mut snapshot,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    let wide = codex_log_mux_links(&snapshot).len();
    assert_eq!(wide, 1);
}

#[test]
fn cache_serves_second_call_without_re_querying_when_db_is_unchanged() {
    // H-SERVE-PERF-002: after the first call populates the cache,
    // a second call on the same DB with the same candidate pid set
    // and same ts_floor must serve entirely from the cache — no
    // SQLite query fires. Attributed to QUERY_COUNT so a future
    // regression that silently bypasses the cache would show up
    // as a nonzero second-call query count.
    let _serial = CACHE_TEST_LOCK.lock().unwrap();
    reset_query_cache_for_tests();
    let _ = take_query_count_for_tests();

    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[("pid:100:uuid-a", "thread-a", now() - 5)],
    );

    let pids = codex_pid_map("main", 100);
    let mut first = build_snapshot("main");
    apply_codex_log_attribution(&mut first, state_root, &pids, now(), DEFAULT_WINDOW_SECONDS);
    let first_queries = take_query_count_for_tests();
    assert!(
        first_queries >= 1,
        "first call must query at least once (was {first_queries})"
    );
    assert_eq!(codex_log_mux_links(&first).len(), 1);

    // Second call on the exact same inputs — must serve from cache.
    let mut second = build_snapshot("main");
    apply_codex_log_attribution(
        &mut second,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    let second_queries = take_query_count_for_tests();
    assert_eq!(
        second_queries, 0,
        "cache hit must skip every SQLite query on the second call"
    );
    // The links still land in the fresh snapshot because we replay
    // cached observations through the same emit path.
    assert_eq!(codex_log_mux_links(&second).len(), 1);
}

#[test]
fn cache_invalidates_when_db_mtime_advances() {
    // Rewriting the DB (fresh mtime, potentially different size)
    // must invalidate the cache so the next call re-queries and
    // picks up the new row rather than serving the stale cached
    // observation. Verified two ways: (1) the emitted link names
    // the new thread; (2) the query counter records a re-query.
    let _serial = CACHE_TEST_LOCK.lock().unwrap();
    reset_query_cache_for_tests();
    let _ = take_query_count_for_tests();

    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    let db_path = state_root.join("logs_2.sqlite");
    write_logs_db(&db_path, &[("pid:100:uuid-a", "thread-a", now() - 5)]);

    let mut first = build_snapshot("main");
    let pids = codex_pid_map("main", 100);
    apply_codex_log_attribution(&mut first, state_root, &pids, now(), DEFAULT_WINDOW_SECONDS);
    assert_eq!(codex_log_mux_links(&first).len(), 1);
    let _ = take_query_count_for_tests();

    // Rewrite the DB. Filesystem mtime advances, size may change —
    // either alone flips the fingerprint.
    fs::remove_file(&db_path).expect("remove db");
    write_logs_db(&db_path, &[("pid:100:uuid-a", "thread-b", now() - 5)]);

    let mut second = build_snapshot("main");
    apply_codex_log_attribution(
        &mut second,
        state_root,
        &pids,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    let second_queries = take_query_count_for_tests();
    assert!(
        second_queries >= 1,
        "advancing the DB must force a fresh query (was {second_queries})"
    );
    let links = codex_log_mux_links(&second);
    assert_eq!(links.len(), 1);
    let NodeId::AgentSession(id) = &links[0].source else {
        panic!();
    };
    assert_eq!(id.session_key, "thread-b");
}

#[test]
fn cache_invalidates_when_candidate_pid_set_changes() {
    // Cache is keyed on (fingerprint, sorted candidate pid list).
    // Adding a new candidate pid must trigger a re-query so the
    // new pid's row (which was never queried on the first call) is
    // picked up. Without the pid-set check the second call would
    // silently miss the new attribution.
    let _serial = CACHE_TEST_LOCK.lock().unwrap();
    reset_query_cache_for_tests();
    let _ = take_query_count_for_tests();

    let temp = TempDir::new().expect("temp");
    let state_root = temp.path();
    write_logs_db(
        &state_root.join("logs_2.sqlite"),
        &[
            ("pid:100:uuid-a", "thread-a", now() - 5),
            ("pid:200:uuid-b", "thread-b", now() - 5),
        ],
    );

    let pids_single = codex_pid_map("main", 100);
    let mut first = build_snapshot("main");
    apply_codex_log_attribution(
        &mut first,
        state_root,
        &pids_single,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    assert_eq!(codex_log_mux_links(&first).len(), 1);
    let _ = take_query_count_for_tests();

    // Second call with a superset pid list — DB unchanged but the
    // candidate list differs. Cache must miss and re-query.
    let mut pids_both = codex_pid_map("main", 100);
    pids_both
        .get_mut(&MuxSessionId::new("tmux:main".to_string()))
        .unwrap()
        .push((CODEX_HARNESS_KEY.to_string(), 200));

    let mut second = build_snapshot("main");
    apply_codex_log_attribution(
        &mut second,
        state_root,
        &pids_both,
        now(),
        DEFAULT_WINDOW_SECONDS,
    );
    let second_queries = take_query_count_for_tests();
    assert!(
        second_queries >= 2,
        "expanded pid set must trigger a fresh query per pid; got {second_queries}"
    );

    let links = codex_log_mux_links(&second);
    let session_keys: Vec<&str> = links
        .iter()
        .filter_map(|link| match &link.source {
            NodeId::AgentSession(id) => Some(id.session_key.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(session_keys.len(), 2);
    assert!(session_keys.contains(&"thread-a"));
    assert!(session_keys.contains(&"thread-b"));
}
