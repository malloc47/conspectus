use std::fs;

use tempfile::TempDir;

use super::*;
use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture, write_malformed};
use crate::model::GraphNode;

fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
    let fixture = HarnessFixture::at(temp.path());
    let context = DiscoveryContext::default()
        .with_harness_state_root(HARNESS_KEY, fixture.codex_state_root());
    (context, fixture)
}

#[test]
fn adapter_returns_empty_when_no_state_root_configured() {
    let fragment = CodexAdapter::new()
        .discover(&DiscoveryContext::default())
        .expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn adapter_returns_empty_when_sessions_dir_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, _fixture) = context_with_state(&temp);

    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn discovers_codex_sessions_with_cwd_and_stable_ids() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("alpha-id").with_cwd("/work/alpha"))
        .expect("write alpha");
    fixture
        .write_codex_session(&CodexSessionRecord::new("beta-id"))
        .expect("write beta");

    let first = CodexAdapter::new().discover(&context).expect("first");
    let second = CodexAdapter::new().discover(&context).expect("second");

    assert_eq!(first, second, "discovery should be stable across runs");

    let sessions: Vec<_> = first
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(session) => Some(session.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sessions.len(), 2);
    let alpha = sessions
        .iter()
        .find(|s| s.id.session_key == "alpha-id")
        .expect("alpha session");
    assert_eq!(alpha.harness_key, HARNESS_KEY);
    assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
    assert_eq!(alpha.last_active_epoch, Some(1_700_000_000));
    assert_eq!(
        alpha.id.state_scope,
        fixture.codex_state_root().to_string_lossy()
    );

    let beta = sessions
        .iter()
        .find(|s| s.id.session_key == "beta-id")
        .expect("beta session");
    assert!(
        beta.cwd.is_none(),
        "missing optional cwd should remain None"
    );
}

#[test]
fn discovers_codex_sessions_in_nested_yyyy_mm_dd_subdirs() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let nested = fixture
        .codex_state_root()
        .join("sessions")
        .join("2026")
        .join("05")
        .join("09");
    fs::create_dir_all(&nested).expect("nested sessions dir");
    fs::write(
        nested.join("rollout-2026-05-09T00-07-57-nested-id.jsonl"),
        "{\"type\":\"session_meta\",\"payload\":{\"id\":\"nested-id\",\"cwd\":\"/work/nested\"}}\n",
    )
    .expect("write nested rollout");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id.session_key, "nested-id");
    assert_eq!(sessions[0].cwd.as_deref(), Some("/work/nested"));
}

fn lineage_links(fragment: &GraphFragment) -> Vec<&GraphLink> {
    fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::ParentSession)
        .collect()
}

#[test]
fn fork_lineage_resolves_when_parent_rollout_is_on_disk() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("parent-id").with_cwd("/work/repo"))
        .expect("parent");
    fixture
        .write_codex_session(
            &CodexSessionRecord::new("child-id")
                .with_cwd("/work/repo")
                .with_forked_from("parent-id"),
        )
        .expect("child");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);

    assert_eq!(lineage.len(), 1);
    let target = match &lineage[0].target {
        LinkEndpoint::Node { id } => id,
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved parent endpoint, got {other:?}")
        }
    };
    let NodeId::AgentSession(parent_id) = target else {
        panic!("expected AgentSession target");
    };
    assert_eq!(parent_id.session_key, "parent-id");

    assert_eq!(
        lineage[0].source_metadata.fields.get("lineage_kind"),
        Some(&json!("fork"))
    );
}

#[test]
fn fork_lineage_preserves_unresolved_parent_when_rollout_is_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(
            &CodexSessionRecord::new("orphan-id").with_forked_from("pruned-parent"),
        )
        .expect("orphan");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);

    assert_eq!(lineage.len(), 1);
    let evidence = match &lineage[0].target {
        LinkEndpoint::Unresolved { evidence } => evidence,
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved endpoint, got {other:?}")
        }
    };
    assert_eq!(evidence.harness_key.as_deref(), Some(HARNESS_KEY));
    assert_eq!(evidence.native_id.as_deref(), Some("pruned-parent"));
}

#[test]
fn self_fork_does_not_emit_lineage_cycle() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("loop-id").with_forked_from("loop-id"))
        .expect("loop");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    assert_eq!(fragment.nodes.len(), 1);
    assert!(lineage_links(&fragment).is_empty());
}

#[test]
fn sessions_without_forked_from_id_emit_no_lineage() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("standalone"))
        .expect("standalone");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    assert!(lineage_links(&fragment).is_empty());
}

#[test]
fn skips_malformed_and_non_meta_records() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("good"))
        .expect("write good");
    let sessions_dir = fixture.codex_state_root().join("sessions");
    write_malformed(sessions_dir.join("rollout-bad.jsonl")).expect("bad");
    // Wrong envelope type: parses, but kind != session_meta.
    fs::write(
        sessions_dir.join("rollout-other.jsonl"),
        "{\"type\":\"chat\",\"payload\":{\"id\":\"other\"}}\n",
    )
    .expect("write other");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s.id.session_key.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(sessions, vec!["good".to_string()]);
}

/// Helper: append response_item lines to an existing rollout
/// (the fixture only writes the session_meta header).
fn append_rollout_lines(path: &Path, lines: &[&str]) {
    let mut body = fs::read_to_string(path).expect("read rollout");
    for line in lines {
        body.push_str(line);
        body.push('\n');
    }
    fs::write(path, body).expect("rewrite rollout");
}

fn discover_session(context: &DiscoveryContext, id: &str) -> AgentSessionNode {
    let fragment = CodexAdapter::new().discover(context).expect("discover");
    fragment
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .find(|s| s.id.session_key == id)
        .expect("matching session")
}

/// Plain assistant `output_text` at the tail wins.
#[test]
fn last_message_preview_returns_last_assistant_output_text() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("plain").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hello back"}]}}"#,
        ],
    );

    let session = discover_session(&context, "plain");
    assert_eq!(session.last_message_preview.as_deref(), Some("hello back"));
}

/// Tool / reasoning / event records at the tail are skipped; the
/// preceding text message wins.
#[test]
fn last_message_preview_skips_tool_reasoning_and_event_records() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("mixed").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"running checks"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"thinking"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call","name":"shell"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","output":"ok"}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","input":42}}"#,
        ],
    );

    let session = discover_session(&context, "mixed");
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("running checks"),
    );
}

/// Empty `text` strings should not win — the extractor keeps
/// walking until a non-empty block is found.
#[test]
fn last_message_preview_skips_empty_text_blocks() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("empty-text").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"valid"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":""}]}}"#,
        ],
    );

    let session = discover_session(&context, "empty-text");
    assert_eq!(session.last_message_preview.as_deref(), Some("valid"));
}

/// A rollout with only the session_meta header yields no preview.
#[test]
fn last_message_preview_returns_none_when_tail_has_no_messages() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("meta-only").with_cwd("/work"))
        .expect("write session");

    let session = discover_session(&context, "meta-only");
    assert_eq!(session.last_message_preview, None);
}

/// Long messages are capped via the shared normalizer.
#[test]
fn last_message_preview_is_capped_at_two_hundred_chars() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("long").with_cwd("/work"))
        .expect("write session");
    let long_text = "a".repeat(500);
    let line = format!(
        r#"{{"type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"{long_text}"}}]}}}}"#
    );
    append_rollout_lines(&path, &[&line]);

    let session = discover_session(&context, "long");
    let preview = session.last_message_preview.expect("non-empty");
    assert_eq!(preview.chars().count(), 200);
    assert!(preview.ends_with('…'));
}

/// Discovery degrades silently on corrupt body bytes inside the
/// tail — preview returns None, the session still discovers.
#[test]
fn last_message_preview_returns_none_when_tail_is_corrupt() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("corrupt").with_cwd("/work"))
        .expect("write session");
    // Append non-JSON garbage as the rollout's tail.
    let mut body = fs::read_to_string(&path).expect("read");
    body.push_str("this is not json\n");
    fs::write(&path, body).expect("rewrite");

    let session = discover_session(&context, "corrupt");
    assert_eq!(session.last_message_preview, None);
}

/// `<turn_aborted>` markers wrap the canned interrupt message
/// codex emits when the user cancels mid-turn. The preview
/// should reflect the body of that message, not the marker.
#[test]
fn last_message_preview_strips_turn_aborted_marker_prefix() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("aborted").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<turn_aborted> The user interrupted the previous turn on purpose."}]}}"#,
        ],
    );

    let session = discover_session(&context, "aborted");
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("The user interrupted the previous turn on purpose."),
    );
}

/// `<proposed_plan>` markers wrap a longer plan body. The
/// marker is stripped and the plan content survives the
/// preview pipeline.
#[test]
fn last_message_preview_strips_proposed_plan_marker_prefix() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("plan").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<proposed_plan> # Atelier Profiles V1 introduce declarative profiles"}]}}"#,
        ],
    );

    let session = discover_session(&context, "plan");
    let preview = session.last_message_preview.expect("preview present");
    assert!(
        preview.starts_with("# Atelier Profiles V1"),
        "unexpected preview content: {preview:?}",
    );
    assert!(!preview.contains("proposed_plan"));
}

/// A bare `<turn_aborted>` with no body should be skipped so the
/// preview reflects the previous real text message instead of
/// rendering an empty cell.
#[test]
fn last_message_preview_skips_bare_turn_aborted_message() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("bare-marker").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"earlier real reply"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"<turn_aborted>"}]}}"#,
        ],
    );

    let session = discover_session(&context, "bare-marker");
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("earlier real reply"),
    );
}

/// Unknown XML-shaped tags are *not* stripped — only the
/// known-codex-marker list is honored, so legitimate user
/// content like `<html>` or `<foo>` survives untouched.
#[test]
fn last_message_preview_leaves_unknown_xml_tags_alone() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let path = fixture
        .write_codex_session(&CodexSessionRecord::new("unknown").with_cwd("/work"))
        .expect("write session");

    append_rollout_lines(
        &path,
        &[
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<html>this is not a marker</html>"}]}}"#,
        ],
    );

    let session = discover_session(&context, "unknown");
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("<html>this is not a marker</html>"),
    );
}

// ── state-database reader tests (ADR 0048) ────────────────────────────

#[derive(Default)]
struct StateThreadFixture {
    id: String,
    cwd: Option<String>,
    title: Option<String>,
    first_user_message: Option<String>,
    updated_at_ms: Option<i64>,
    created_at_ms: Option<i64>,
}

impl StateThreadFixture {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            ..Self::default()
        }
    }

    fn cwd(mut self, cwd: &str) -> Self {
        self.cwd = Some(cwd.to_string());
        self
    }

    fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    fn first_user_message(mut self, message: &str) -> Self {
        self.first_user_message = Some(message.to_string());
        self
    }

    fn updated_at_ms(mut self, ms: i64) -> Self {
        self.updated_at_ms = Some(ms);
        self
    }
}

#[derive(Clone, Copy)]
struct StateColumns {
    cwd: bool,
    title: bool,
    first_user_message: bool,
    updated_at_ms: bool,
    created_at_ms: bool,
}

impl StateColumns {
    fn full() -> Self {
        Self {
            cwd: true,
            title: true,
            first_user_message: true,
            updated_at_ms: true,
            created_at_ms: true,
        }
    }
}

fn write_state_db(
    path: &Path,
    columns: StateColumns,
    threads: &[StateThreadFixture],
    spawn_edges: &[(&str, &str)],
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("state db parent");
    }
    let conn = Connection::open(path).expect("open state db fixture");

    let mut cols = vec!["id TEXT NOT NULL PRIMARY KEY"];
    if columns.cwd {
        cols.push("cwd TEXT");
    }
    if columns.title {
        cols.push("title TEXT");
    }
    if columns.first_user_message {
        cols.push("first_user_message TEXT");
    }
    if columns.updated_at_ms {
        cols.push("updated_at_ms INTEGER");
    }
    if columns.created_at_ms {
        cols.push("created_at_ms INTEGER");
    }
    conn.execute(&format!("CREATE TABLE threads ({})", cols.join(", ")), [])
        .expect("create threads");

    for row in threads {
        let mut names: Vec<&str> = vec!["id"];
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(row.id.clone())];
        if columns.cwd {
            names.push("cwd");
            params.push(Box::new(row.cwd.clone()));
        }
        if columns.title {
            names.push("title");
            params.push(Box::new(row.title.clone()));
        }
        if columns.first_user_message {
            names.push("first_user_message");
            params.push(Box::new(row.first_user_message.clone()));
        }
        if columns.updated_at_ms {
            names.push("updated_at_ms");
            params.push(Box::new(row.updated_at_ms));
        }
        if columns.created_at_ms {
            names.push("created_at_ms");
            params.push(Box::new(row.created_at_ms));
        }
        let placeholders: Vec<String> = (1..=names.len()).map(|n| format!("?{n}")).collect();
        let sql = format!(
            "INSERT INTO threads ({}) VALUES ({})",
            names.join(", "),
            placeholders.join(", ")
        );
        let refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        conn.execute(&sql, refs.as_slice()).expect("insert thread");
    }

    if !spawn_edges.is_empty() {
        conn.execute(
            "CREATE TABLE thread_spawn_edges (\
                    parent_thread_id TEXT NOT NULL, \
                    child_thread_id TEXT NOT NULL PRIMARY KEY, \
                    status TEXT NOT NULL)",
            [],
        )
        .expect("create spawn edges");
        for (parent, child) in spawn_edges {
            conn.execute(
                "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, status) \
                     VALUES (?1, ?2, 'closed')",
                [parent, child],
            )
            .expect("insert spawn edge");
        }
    }
}

fn session_by_id(fragment: &GraphFragment, id: &str) -> Option<AgentSessionNode> {
    fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s.clone()),
            _ => None,
        })
        .find(|s| s.id.session_key == id)
}

fn ensure_sessions_dir(fixture: &HarnessFixture) {
    let dir = fixture.codex_state_root().join("sessions");
    fs::create_dir_all(&dir).expect("sessions dir");
}

#[test]
fn state_db_threads_emit_sessions_with_cwd_title_and_activity() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("alpha")
            .cwd("/work/alpha")
            .title("Alpha thread")
            .updated_at_ms(1_700_000_500_000)],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let alpha = session_by_id(&fragment, "alpha").expect("alpha");
    assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
    assert_eq!(alpha.title.as_deref(), Some("Alpha thread"));
    // Millisecond updated_at_ms should land as second-precision epoch.
    assert_eq!(alpha.last_active_epoch, Some(1_700_000_500));
}

#[test]
fn state_db_title_falls_back_to_first_user_message_when_title_empty() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("bare")
            .title("   ")
            .first_user_message("Investigate flaky test on CI")],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let bare = session_by_id(&fragment, "bare").expect("bare");
    assert_eq!(bare.title.as_deref(), Some("Investigate flaky test on CI"),);
}

#[test]
fn state_db_higher_numeric_suffix_wins_when_multiple_state_files_present() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    // Older file: contains only a stale row.
    write_state_db(
        &fixture.codex_state_root().join("state_3.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("stale").cwd("/old")],
        &[],
    );
    // Active file: contains the row we want to see.
    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("fresh").cwd("/new")],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    assert!(session_by_id(&fragment, "fresh").is_some());
    assert!(
        session_by_id(&fragment, "stale").is_none(),
        "stale lower-version row must not leak through"
    );
}

#[test]
fn state_db_and_rollout_merge_state_authoritative_for_cwd_and_title() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);

    // Rollout supplies cwd /from-rollout.
    fixture
        .write_codex_session(&CodexSessionRecord::new("shared").with_cwd("/from-rollout"))
        .expect("rollout");

    // State row for same id overrides cwd and provides a title.
    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("shared")
            .cwd("/from-state")
            .title("State title")
            .updated_at_ms(1_750_000_000_000)],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let shared = session_by_id(&fragment, "shared").expect("shared");
    assert_eq!(shared.cwd.as_deref(), Some("/from-state"));
    assert_eq!(shared.title.as_deref(), Some("State title"));
    // State's ms timestamp wins over rollout's mtime-derived epoch.
    assert_eq!(shared.last_active_epoch, Some(1_750_000_000));
}

#[test]
fn state_only_session_emits_node_when_no_rollout_present() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("state-only").cwd("/sso")],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let node = session_by_id(&fragment, "state-only").expect("state-only");
    assert_eq!(node.cwd.as_deref(), Some("/sso"));
    assert!(node.last_message_preview.is_none());
}

#[test]
fn state_spawn_edges_emit_parent_session_with_spawn_lineage_kind() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[
            StateThreadFixture::new("parent"),
            StateThreadFixture::new("child"),
        ],
        &[("parent", "child")],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);
    assert_eq!(lineage.len(), 1);
    assert_eq!(
        lineage[0].source_metadata.fields.get("lineage_kind"),
        Some(&json!("spawn"))
    );
    let target = match &lineage[0].target {
        LinkEndpoint::Node { id } => id,
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved parent, got {other:?}")
        }
    };
    let NodeId::AgentSession(parent_id) = target else {
        panic!("expected AgentSession target");
    };
    assert_eq!(parent_id.session_key, "parent");
}

#[test]
fn state_spawn_edges_with_unknown_parent_emit_unresolved_endpoint() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[StateThreadFixture::new("orphan")],
        &[("missing-parent", "orphan")],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);
    assert_eq!(lineage.len(), 1);
    let evidence = match &lineage[0].target {
        LinkEndpoint::Unresolved { evidence } => evidence,
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved endpoint, got {other:?}")
        }
    };
    assert_eq!(evidence.native_id.as_deref(), Some("missing-parent"));
}

#[test]
fn state_spawn_edges_skip_self_and_empty_pointers() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    // child_thread_id is PK in production, so use distinct children for
    // the two degenerate cases instead of reusing the same one.
    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[
            StateThreadFixture::new("self-ref"),
            StateThreadFixture::new("empty-parent-child"),
        ],
        &[("self-ref", "self-ref"), ("", "empty-parent-child")],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    assert!(lineage_links(&fragment).is_empty());
}

#[test]
fn state_fork_and_spawn_coexist_on_same_session() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);

    // Rollout drives the fork lineage path.
    fixture
        .write_codex_session(&CodexSessionRecord::new("fork-parent"))
        .expect("fork parent rollout");
    fixture
        .write_codex_session(&CodexSessionRecord::new("multi").with_forked_from("fork-parent"))
        .expect("multi rollout");

    // State drives the spawn lineage path for the same child.
    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns::full(),
        &[
            StateThreadFixture::new("spawn-parent"),
            StateThreadFixture::new("multi"),
            StateThreadFixture::new("fork-parent"),
        ],
        &[("spawn-parent", "multi")],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);
    let kinds: Vec<_> = lineage
        .iter()
        .filter_map(|link| link.source_metadata.fields.get("lineage_kind"))
        .cloned()
        .collect();
    assert!(
        kinds.contains(&json!("fork")) && kinds.contains(&json!("spawn")),
        "expected both fork and spawn kinds; got {kinds:?}"
    );
    // Two parent_session candidates from the same child are fine — they
    // describe different lineage operations.

    assert_eq!(
        lineage
            .iter()
            .filter(|link| match &link.source {
                NodeId::AgentSession(id) => id.session_key == "multi",
                _ => false,
            })
            .count(),
        2
    );
}

#[test]
fn state_db_missing_optional_columns_degrades_to_supported_subset() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    ensure_sessions_dir(&fixture);

    // Old schema: only id + cwd, none of the millisecond timestamps or
    // first_user_message exist yet.
    write_state_db(
        &fixture.codex_state_root().join("state_5.sqlite"),
        StateColumns {
            cwd: true,
            title: false,
            first_user_message: false,
            updated_at_ms: false,
            created_at_ms: false,
        },
        &[StateThreadFixture::new("slim").cwd("/slim")],
        &[],
    );

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let slim = session_by_id(&fragment, "slim").expect("slim");
    assert_eq!(slim.cwd.as_deref(), Some("/slim"));
    assert!(slim.title.is_none());
    assert!(slim.last_active_epoch.is_none());
}

#[test]
fn state_db_unreadable_falls_back_to_rollout_only() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("rollout-only").with_cwd("/r"))
        .expect("rollout");

    // Write a junk file at the state_5.sqlite path so open fails.
    fs::write(
        fixture.codex_state_root().join("state_5.sqlite"),
        b"not a sqlite database",
    )
    .expect("junk state db");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let rollout = session_by_id(&fragment, "rollout-only").expect("rollout-only");
    assert_eq!(rollout.cwd.as_deref(), Some("/r"));
}

#[test]
fn state_db_missing_threads_table_degrades_to_rollout_only() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_codex_session(&CodexSessionRecord::new("rollout-id").with_cwd("/r"))
        .expect("rollout");

    // Empty but valid sqlite (no threads table).
    let db_path = fixture.codex_state_root().join("state_5.sqlite");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("state parent");
    }
    Connection::open(&db_path).expect("create empty db");

    let fragment = CodexAdapter::new().discover(&context).expect("discover");
    let rollout = session_by_id(&fragment, "rollout-id").expect("rollout-id");
    assert_eq!(rollout.cwd.as_deref(), Some("/r"));
}
