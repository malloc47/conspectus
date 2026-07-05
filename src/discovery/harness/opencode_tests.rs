// Extracted from opencode.rs H-HYG-011 rolling wave via #[path = "opencode_tests.rs"] mod tests;
use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;

use super::*;
use crate::discovery::harness::fixtures::{HarnessFixture, OpenCodeSessionRecord};
use crate::model::GraphNode;

fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
    let fixture = HarnessFixture::at(temp.path());
    let context = DiscoveryContext::default()
        .with_harness_state_root(HARNESS_KEY, fixture.opencode_state_root());
    (context, fixture)
}

#[test]
fn adapter_returns_empty_when_no_state_root_configured() {
    let fragment = OpenCodeAdapter::new()
        .discover(&DiscoveryContext::default())
        .expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn adapter_returns_empty_when_storage_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, _) = context_with_state(&temp);

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn discovers_opencode_sessions_with_directory_and_title() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_opencode_session(
            &OpenCodeSessionRecord::new("s1")
                .with_directory("/work/repo")
                .with_title("first work"),
        )
        .expect("write s1");
    fixture
        .write_opencode_session(&OpenCodeSessionRecord::new("s2"))
        .expect("write s2");

    let first = OpenCodeAdapter::new().discover(&context).expect("first");
    let second = OpenCodeAdapter::new().discover(&context).expect("second");

    assert_eq!(first, second);

    let sessions: Vec<_> = first
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sessions.len(), 2);
    let s1 = sessions
        .iter()
        .find(|s| s.id.session_key == "s1")
        .expect("s1");
    assert_eq!(s1.cwd.as_deref(), Some("/work/repo"));
    assert_eq!(s1.title.as_deref(), Some("first work"));

    let s2 = sessions
        .iter()
        .find(|s| s.id.session_key == "s2")
        .expect("s2");
    assert!(s2.cwd.is_none());
    assert!(s2.title.is_none());
}

#[test]
fn discovers_opencode_sessions_from_sqlite_store() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_session(
        &fixture.opencode_state_root().join("opencode.db"),
        "db-session",
        Some("/work/db"),
        Some("database work"),
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id.session_key, "db-session");
    assert_eq!(sessions[0].cwd.as_deref(), Some("/work/db"));
    assert_eq!(sessions[0].title.as_deref(), Some("database work"));
    assert_eq!(sessions[0].last_active_epoch, Some(1_700_000_000));
}

#[test]
fn sqlite_store_wins_over_legacy_session_for_duplicate_id() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fixture
        .write_opencode_session(
            &OpenCodeSessionRecord::new("same")
                .with_directory("/work/legacy")
                .with_title("legacy"),
        )
        .expect("write legacy");
    write_sqlite_session(
        &fixture.opencode_state_root().join("opencode.db"),
        "same",
        Some("/work/sqlite"),
        Some("sqlite"),
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    let sessions: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].cwd.as_deref(), Some("/work/sqlite"));
    assert_eq!(sessions[0].title.as_deref(), Some("sqlite"));
}

#[test]
fn malformed_sqlite_store_degrades_to_empty_fragment() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    fs::create_dir_all(fixture.opencode_state_root()).expect("state root");
    fs::write(
        fixture.opencode_state_root().join("opencode.db"),
        "not sqlite",
    )
    .expect("write malformed db");

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn skips_sessions_without_id() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let dir = fixture.opencode_state_root().join("storage/session/bad");
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("info.json"), "{\"directory\":\"/work\"}").expect("write");

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

fn write_sqlite_session(path: &Path, id: &str, directory: Option<&str>, title: Option<&str>) {
    write_sqlite_sessions(
        path,
        SqliteSchemaConfig::with_parent(),
        &[SqliteSessionRow {
            id,
            directory,
            title,
            parent_id: None,
            kind: None,
            time_created: Some(1_600_000_000_000),
            time_updated: Some(1_700_000_000_000),
        }],
    );
}

struct SqliteSchemaConfig {
    has_parent: bool,
    has_kind: bool,
}

impl SqliteSchemaConfig {
    fn with_parent() -> Self {
        Self {
            has_parent: true,
            has_kind: false,
        }
    }

    #[allow(dead_code)]
    fn with_parent_and_kind() -> Self {
        Self {
            has_parent: true,
            has_kind: true,
        }
    }
}

struct SqliteSessionRow<'a> {
    id: &'a str,
    directory: Option<&'a str>,
    title: Option<&'a str>,
    parent_id: Option<&'a str>,
    kind: Option<&'a str>,
    time_created: Option<i64>,
    time_updated: Option<i64>,
}

fn write_sqlite_sessions(path: &Path, config: SqliteSchemaConfig, rows: &[SqliteSessionRow<'_>]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("db parent");
    }
    let connection = Connection::open(path).expect("open sqlite fixture");

    let mut columns = vec!["id TEXT", "directory TEXT", "title TEXT"];
    if config.has_kind {
        columns.push("kind TEXT");
    }
    columns.push("time_created INTEGER");
    columns.push("time_updated INTEGER");
    if config.has_parent {
        columns.push("parent_id TEXT");
    }
    let schema = format!("CREATE TABLE session ({})", columns.join(", "));
    connection
        .execute(&schema, [])
        .expect("create session table");

    for row in rows {
        let mut col_names = vec!["id", "directory", "title"];
        if config.has_kind {
            col_names.push("kind");
        }
        col_names.push("time_created");
        col_names.push("time_updated");
        if config.has_parent {
            col_names.push("parent_id");
        }
        let placeholders: Vec<String> = (1..=col_names.len()).map(|n| format!("?{n}")).collect();
        let sql = format!(
            "INSERT INTO session ({}) VALUES ({})",
            col_names.join(", "),
            placeholders.join(", ")
        );

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(row.id),
            Box::new(row.directory),
            Box::new(row.title),
        ];
        if config.has_kind {
            params.push(Box::new(row.kind));
        }
        params.push(Box::new(row.time_created));
        params.push(Box::new(row.time_updated));
        if config.has_parent {
            params.push(Box::new(row.parent_id));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        connection
            .execute(&sql, param_refs.as_slice())
            .unwrap_or_else(|e| panic!("insert session row: {e}"));
    }
}

fn lineage_links(fragment: &GraphFragment) -> Vec<&GraphLink> {
    fragment
        .candidate_links
        .iter()
        .filter(|link| link.relation == RelationKind::ParentSession)
        .collect()
}

#[test]
fn lineage_resolves_when_parent_row_is_present() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[
            SqliteSessionRow {
                id: "parent",
                directory: Some("/work/repo"),
                title: Some("parent"),
                parent_id: None,
                kind: None,
                time_created: None,
                time_updated: None,
            },
            SqliteSessionRow {
                id: "child",
                directory: Some("/work/repo"),
                title: Some("child"),
                parent_id: Some("parent"),
                kind: None,
                time_created: None,
                time_updated: None,
            },
        ],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");
    let lineage = lineage_links(&fragment);

    assert_eq!(lineage.len(), 1);
    let target = match &lineage[0].target {
        LinkEndpoint::Node { id } => id,
        other @ LinkEndpoint::Unresolved { .. } => {
            panic!("expected resolved target, got {other:?}")
        }
    };
    let NodeId::AgentSession(parent_id) = target else {
        panic!("expected AgentSession target");
    };
    assert_eq!(parent_id.session_key, "parent");

    let NodeId::AgentSession(child_id) = &lineage[0].source else {
        panic!("expected AgentSession source");
    };
    assert_eq!(child_id.session_key, "child");

    assert_eq!(
        lineage[0].source_metadata.fields.get("lineage_kind"),
        Some(&json!("unknown"))
    );
    assert_eq!(
        lineage[0].source_metadata.fields.get("parent_native_id"),
        Some(&json!("parent"))
    );
}

#[test]
fn lineage_preserves_unresolved_parent_when_row_is_missing() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[SqliteSessionRow {
            id: "orphan",
            directory: Some("/work/repo"),
            title: None,
            parent_id: Some("pruned-parent"),
            kind: None,
            time_created: None,
            time_updated: None,
        }],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");
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
fn self_parent_row_does_not_emit_a_lineage_cycle() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[SqliteSessionRow {
            id: "loop",
            directory: Some("/work/repo"),
            title: None,
            parent_id: Some("loop"),
            kind: None,
            time_created: None,
            time_updated: None,
        }],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    assert_eq!(fragment.nodes.len(), 1, "session row still discovered");
    assert!(
        lineage_links(&fragment).is_empty(),
        "self-parent must not emit a lineage edge"
    );
}

#[test]
fn legacy_schema_without_parent_column_still_discovers_sessions() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig {
            has_parent: false,
            has_kind: false,
        },
        &[SqliteSessionRow {
            id: "legacy",
            directory: Some("/work/repo"),
            title: Some("legacy session"),
            parent_id: None,
            kind: None,
            time_created: None,
            time_updated: None,
        }],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

    assert_eq!(fragment.nodes.len(), 1);
    assert!(
        lineage_links(&fragment).is_empty(),
        "schemas without parent_id must not emit lineage"
    );
}

/// Build a minimal `part` table next to the session row so the
/// preview extractor can find a text row.
fn write_part_table(connection: &Connection, parts: &[(&str, &str, i64, &str)]) {
    connection
        .execute(
            "CREATE TABLE part (\
                    id TEXT, \
                    message_id TEXT, \
                    session_id TEXT, \
                    time_created INTEGER, \
                    time_updated INTEGER, \
                    data TEXT \
                )",
            [],
        )
        .expect("create part table");
    for (id, session_id, time_created, data) in parts {
        connection
            .execute(
                "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (id, "msg-x", session_id, time_created, time_created, data),
            )
            .expect("insert part");
    }
}

fn discover_session(context: &DiscoveryContext, id: &str) -> AgentSessionNode {
    let fragment = OpenCodeAdapter::new().discover(context).expect("discover");
    fragment
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::AgentSession(s) => Some(s),
            _ => None,
        })
        .find(|s| s.id.session_key == id)
        .expect("session present")
}

/// The most recent `type: "text"` part wins; non-text parts
/// (`step-start`, `step-finish`, `reasoning`, `tool-call`, …)
/// are skipped via the SQL `WHERE` clause.
#[test]
fn last_message_preview_returns_most_recent_text_part() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let db = fixture.opencode_state_root().join("opencode.db");
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    let connection = Connection::open(&db).expect("open db");
    connection
        .execute(
            "CREATE TABLE session (\
                    id TEXT, directory TEXT, title TEXT, parent_id TEXT)",
            [],
        )
        .expect("session table");
    connection
            .execute(
                "INSERT INTO session (id, directory, title, parent_id) VALUES ('s1', '/work', 't', NULL)",
                [],
            )
            .expect("session row");
    write_part_table(
        &connection,
        &[
            ("p1", "s1", 100, r#"{"type":"step-start"}"#),
            ("p2", "s1", 200, r#"{"type":"text","text":"first answer"}"#),
            (
                "p3",
                "s1",
                300,
                r#"{"type":"reasoning","text":"thinking aloud"}"#,
            ),
            ("p4", "s1", 400, r#"{"type":"text","text":"final answer"}"#),
            ("p5", "s1", 500, r#"{"type":"step-finish","reason":"stop"}"#),
        ],
    );
    drop(connection);

    let session = discover_session(&context, "s1");
    assert_eq!(
        session.last_message_preview.as_deref(),
        Some("final answer")
    );
}

/// Empty `text` strings and missing `text` fields are filtered by
/// the SQL `length` / `IS NOT NULL` guards.
#[test]
fn last_message_preview_skips_empty_text_parts() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let db = fixture.opencode_state_root().join("opencode.db");
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    let connection = Connection::open(&db).expect("open db");
    connection
        .execute(
            "CREATE TABLE session (id TEXT, directory TEXT, title TEXT, parent_id TEXT)",
            [],
        )
        .expect("session table");
    connection
        .execute("INSERT INTO session VALUES ('s1', '/work', 't', NULL)", [])
        .expect("session row");
    write_part_table(
        &connection,
        &[
            ("p1", "s1", 100, r#"{"type":"text","text":"valid"}"#),
            ("p2", "s1", 200, r#"{"type":"text","text":""}"#),
            ("p3", "s1", 300, r#"{"type":"text"}"#), // missing text
        ],
    );
    drop(connection);

    let session = discover_session(&context, "s1");
    assert_eq!(session.last_message_preview.as_deref(), Some("valid"));
}

/// Multi-session DBs surface a preview per session independently.
#[test]
fn last_message_preview_is_per_session() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let db = fixture.opencode_state_root().join("opencode.db");
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    let connection = Connection::open(&db).expect("open db");
    connection
        .execute(
            "CREATE TABLE session (id TEXT, directory TEXT, title TEXT, parent_id TEXT)",
            [],
        )
        .expect("session table");
    connection
        .execute("INSERT INTO session VALUES ('a', '/w', 't', NULL)", [])
        .expect("a");
    connection
        .execute("INSERT INTO session VALUES ('b', '/w', 't', NULL)", [])
        .expect("b");
    write_part_table(
        &connection,
        &[
            ("p1", "a", 100, r#"{"type":"text","text":"hello from a"}"#),
            ("p2", "b", 100, r#"{"type":"text","text":"hello from b"}"#),
        ],
    );
    drop(connection);

    let a = discover_session(&context, "a");
    let b = discover_session(&context, "b");
    assert_eq!(a.last_message_preview.as_deref(), Some("hello from a"));
    assert_eq!(b.last_message_preview.as_deref(), Some("hello from b"));
}

/// Schemas without the `part` table degrade silently — sessions
/// still discover, previews are `None`.
#[test]
fn last_message_preview_returns_none_without_part_table() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_session(
        &fixture.opencode_state_root().join("opencode.db"),
        "s1",
        Some("/work"),
        Some("t"),
    );

    let session = discover_session(&context, "s1");
    assert_eq!(session.last_message_preview, None);
}

/// Long text values are normalized and capped through the shared
/// helper.
#[test]
fn last_message_preview_is_capped_at_two_hundred_chars() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    let db = fixture.opencode_state_root().join("opencode.db");
    if let Some(parent) = db.parent() {
        fs::create_dir_all(parent).expect("parent dir");
    }
    let connection = Connection::open(&db).expect("open db");
    connection
        .execute(
            "CREATE TABLE session (id TEXT, directory TEXT, title TEXT, parent_id TEXT)",
            [],
        )
        .expect("session table");
    connection
        .execute("INSERT INTO session VALUES ('s1', '/w', 't', NULL)", [])
        .expect("session row");
    let long = "a".repeat(500);
    let data = format!(r#"{{"type":"text","text":"{long}"}}"#);
    write_part_table(&connection, &[("p1", "s1", 100, &data)]);
    drop(connection);

    let session = discover_session(&context, "s1");
    let preview = session.last_message_preview.expect("non-empty");
    assert_eq!(preview.chars().count(), 200);
    assert!(preview.ends_with('…'));
}

// --- Subagent detection tests ---

#[test]
fn subagent_detected_via_kind_column() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent_and_kind(),
        &[
            SqliteSessionRow {
                id: "parent",
                directory: Some("/work/repo"),
                title: Some("main session"),
                parent_id: None,
                kind: Some("human"),
                time_created: None,
                time_updated: None,
            },
            SqliteSessionRow {
                id: "sub",
                directory: Some("/work/repo"),
                title: Some("Find exact_cwd_match code (@explore subagent)"),
                parent_id: Some("parent"),
                kind: Some("subagent"),
                time_created: None,
                time_updated: None,
            },
        ],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");
    let sessions: BTreeMap<&str, &AgentSessionNode> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some((s.id.session_key.as_str(), s)),
            _ => None,
        })
        .collect();

    assert_eq!(sessions["parent"].session_kind, Some(SessionKind::Human));
    assert_eq!(sessions["sub"].session_kind, Some(SessionKind::Subagent));
}

#[test]
fn subagent_detected_via_title_heuristic() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[
            SqliteSessionRow {
                id: "parent",
                directory: Some("/work/repo"),
                title: Some("main session"),
                parent_id: None,
                kind: None,
                time_created: None,
                time_updated: None,
            },
            SqliteSessionRow {
                id: "explore-sub",
                directory: Some("/work/repo"),
                title: Some("Find exact_cwd_match code (@explore subagent)"),
                parent_id: Some("parent"),
                kind: None,
                time_created: None,
                time_updated: None,
            },
            SqliteSessionRow {
                id: "general-sub",
                directory: Some("/work/repo"),
                title: Some("General task (@general subagent)"),
                parent_id: Some("parent"),
                kind: None,
                time_created: None,
                time_updated: None,
            },
        ],
    );

    let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");
    let sessions: BTreeMap<&str, &AgentSessionNode> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::AgentSession(s) => Some((s.id.session_key.as_str(), s)),
            _ => None,
        })
        .collect();

    assert_eq!(sessions["parent"].session_kind, None);
    assert_eq!(
        sessions["explore-sub"].session_kind,
        Some(SessionKind::Subagent)
    );
    assert_eq!(
        sessions["general-sub"].session_kind,
        Some(SessionKind::Subagent)
    );
}

#[test]
fn session_without_parent_id_is_not_classified_as_subagent() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[SqliteSessionRow {
            id: "standalone",
            directory: Some("/work/repo"),
            title: Some("This looks like (@explore subagent) but no parent"),
            parent_id: None,
            kind: None,
            time_created: None,
            time_updated: None,
        }],
    );

    let session = discover_session(&context, "standalone");
    assert_eq!(session.session_kind, None);
}

#[test]
fn subagent_title_pattern_matches_future_subagent_types() {
    let temp = TempDir::new().expect("temp");
    let (context, fixture) = context_with_state(&temp);
    write_sqlite_sessions(
        &fixture.opencode_state_root().join("opencode.db"),
        SqliteSchemaConfig::with_parent(),
        &[SqliteSessionRow {
            id: "future-sub",
            directory: Some("/work/repo"),
            title: Some("(@future subagent) something"),
            parent_id: Some("parent"),
            kind: None,
            time_created: None,
            time_updated: None,
        }],
    );

    let session = discover_session(&context, "future-sub");
    assert_eq!(session.session_kind, Some(SessionKind::Subagent));
}
