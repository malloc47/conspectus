//! opencode harness discovery.
//!
//! Reads modern `$STATE_ROOT/opencode.db` sessions plus legacy
//! `$STATE_ROOT/storage/session/<id>/info.json` records and emits one
//! `AgentSession` per discovered session. Records that fail to parse or are
//! missing the `id` field are skipped silently.
//!
//! Per ADR 0018 the SQLite-backed reader also extracts the `parent_id`
//! column when present and emits intra-harness `parent_session` candidate
//! links between two `AgentSession` endpoints. Older databases that lack
//! the column degrade by reading sessions without lineage rather than
//! dropping everything.
//!
//! Subagent sessions (openCode `@explore` / `@general` workers) are
//! classified via a schema probe for the `kind` column, with a title-
//! pattern heuristic as fallback when the column is absent. Subagent
//! sessions carry `session_kind: Subagent` on their `AgentSessionNode` so
//! downstream TUI and resolver code can nest, filter, or suppress them.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{
    AgentSessionId, AgentSessionNode, Confidence, Freshness, GraphLink, GraphNode, LinkEndpoint,
    LinkState, Metadata, NodeId, Provenance, RelationKind, SessionKind, SourceMetadata,
    UnresolvedEndpoint, normalize_last_message_preview,
};

pub const HARNESS_KEY: &str = crate::discovery::providers::OPENCODE;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenCodeAdapter;

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for OpenCodeAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let Some(state_root) = context.harness_state_root(self.harness_key()) else {
            return Ok(GraphFragment::empty());
        };
        let mut fragment = discover_state(state_root)?;
        crate::discovery::stamp_fragment(
            &mut fragment,
            HARNESS_KEY,
            crate::discovery::current_epoch(),
        );
        Ok(fragment)
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("opencode")]
    }

    fn resume_argv(
        &self,
        session_id: &str,
        _cwd: &std::path::Path,
    ) -> Option<Vec<std::ffi::OsString>> {
        // `opencode --session <id>` continues a specific session
        // (see `opencode --help`). The positional `[project]`
        // argument is left out so tmux's `-c <cwd>` handles the
        // working directory, mirroring the codex / claude-code
        // pattern.
        Some(vec![
            std::ffi::OsString::from("opencode"),
            std::ffi::OsString::from("--session"),
            std::ffi::OsString::from(session_id),
        ])
    }
}

fn discover_state(state_root: &Path) -> Result<GraphFragment> {
    let state_scope = state_root.to_string_lossy().to_string();
    let mut sessions = BTreeMap::new();

    for info in read_sqlite_sessions(&state_root.join("opencode.db")) {
        sessions.insert(info.id.clone(), info);
    }

    for info in read_legacy_sessions(state_root)? {
        sessions.entry(info.id.clone()).or_insert(info);
    }

    let mut nodes = Vec::with_capacity(sessions.len());
    let mut candidate_links = Vec::new();

    for info in sessions.values() {
        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, &info.id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: info.directory.clone(),
            title: info.title.clone(),
            last_message_preview: info.last_message_preview.clone(),
            last_active_epoch: info.last_active_epoch,
            session_kind: info.session_kind,
        }));
    }

    for info in sessions.values() {
        let Some(parent_id) = info.parent_id.as_deref() else {
            continue;
        };

        if parent_id.is_empty() || parent_id == info.id {
            // Empty parent strings are meaningless; self-parent links would
            // create a cycle that can't represent real lineage.
            continue;
        }

        let resolved_parent = sessions.get(parent_id).map(|_| parent_id);
        candidate_links.push(build_lineage_link(
            &info.id,
            parent_id,
            &state_scope,
            resolved_parent,
        ));
    }

    Ok(GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    })
}

#[derive(Deserialize)]
struct SessionInfo {
    id: String,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    title: Option<String>,
    /// Sourced from `session.parent_id` when the SQLite schema carries it.
    /// Legacy filesystem records do not expose lineage and leave this `None`.
    #[serde(default)]
    parent_id: Option<String>,
    /// Sourced from the most recent `type: "text"` row in the `part`
    /// table when present. Capped/normalized via
    /// [`normalize_last_message_preview`]. Legacy filesystem records
    /// and older schemas without the table leave this `None`.
    #[serde(default)]
    last_message_preview: Option<String>,
    /// Best-effort latest activity timestamp in Unix epoch seconds.
    /// SQLite rows store millisecond timestamps; legacy info.json stores
    /// the same shape under `time.updated` / `time.created`.
    #[serde(default)]
    last_active_epoch: Option<i64>,
    /// Harness-level session classification. Populated from the `kind`
    /// column when the schema carries it, otherwise inferred via a
    /// title-pattern heuristic for subagent detection.
    #[serde(default)]
    session_kind: Option<SessionKind>,
    #[serde(default)]
    time: Option<SessionTime>,
}

#[derive(Deserialize)]
struct SessionTime {
    #[serde(default)]
    created: Option<i64>,
    #[serde(default)]
    updated: Option<i64>,
}

fn read_sqlite_sessions(path: &Path) -> Vec<SessionInfo> {
    if !path.exists() {
        return Vec::new();
    }

    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };

    let Some(mut query) = session_query(&connection) else {
        return Vec::new();
    };

    let Ok(rows) = query.statement.query_map([], |row| {
        let id: String = row.get(0)?;
        let directory: Option<String> = row.get(1)?;
        let title: Option<String> = row.get(2)?;
        let mut col: usize = 3;

        let parent_id = if query.has_parent {
            let val = row.get::<_, Option<String>>(col)?;
            col += 1;
            val
        } else {
            None
        };

        let raw_kind = if query.has_kind {
            let val = row.get::<_, Option<String>>(col)?;
            col += 1;
            val
        } else {
            None
        };

        let (time_updated, time_created) = if query.has_time {
            let u = row.get::<_, Option<i64>>(col)?;
            let c = row.get::<_, Option<i64>>(col + 1)?;
            (u, c)
        } else {
            (None, None)
        };

        let last_active_epoch = epoch_ms_to_seconds(time_updated.or(time_created));

        let session_kind =
            classify_session_kind(raw_kind.as_deref(), parent_id.as_deref(), title.as_deref());

        Ok(SessionInfo {
            id,
            directory,
            title,
            parent_id,
            last_active_epoch,
            last_message_preview: None,
            session_kind,
            time: None,
        })
    }) else {
        return Vec::new();
    };

    let mut sessions: Vec<SessionInfo> = rows
        .filter_map(|row| {
            let info = row.ok()?;
            if info.id.trim().is_empty() {
                None
            } else {
                Some(info)
            }
        })
        .collect();

    // Attach the most recent text-part preview per session. Best-effort:
    // schemas that don't carry the `part` table (or lack JSON1 support
    // in this rusqlite build, which the `bundled` feature ensures we
    // have) degrade to `None`.
    let previews = read_last_message_previews(&connection);
    for session in sessions.iter_mut() {
        if let Some(text) = previews.get(&session.id) {
            session.last_message_preview = normalize_last_message_preview(text);
        }
    }

    sessions
}

struct SessionQuery<'conn> {
    statement: rusqlite::Statement<'conn>,
    has_parent: bool,
    has_time: bool,
    has_kind: bool,
}

fn session_query(connection: &Connection) -> Option<SessionQuery<'_>> {
    // Probe for the `kind` column first (openCode ≥ some-future-version that
    // tags subagent sessions with a dedicated field). If the column exists the
    // adapter reads it directly; otherwise it falls back to a title-pattern
    // heuristic keyed on `parent_id` presence.
    let candidates = [
        (
            "SELECT id, directory, title, parent_id, kind, time_updated, time_created FROM session ORDER BY id",
            true,
            true,
            true,
        ),
        (
            "SELECT id, directory, title, parent_id, kind FROM session ORDER BY id",
            true,
            false,
            true,
        ),
        (
            "SELECT id, directory, title, parent_id, time_updated, time_created FROM session ORDER BY id",
            true,
            true,
            false,
        ),
        (
            "SELECT id, directory, title, parent_id FROM session ORDER BY id",
            true,
            false,
            false,
        ),
        (
            "SELECT id, directory, title, time_updated, time_created FROM session ORDER BY id",
            false,
            true,
            false,
        ),
        (
            "SELECT id, directory, title FROM session ORDER BY id",
            false,
            false,
            false,
        ),
    ];

    for (sql, has_parent, has_time, has_kind) in candidates {
        if let Ok(statement) = connection.prepare(sql) {
            return Some(SessionQuery {
                statement,
                has_parent,
                has_time,
                has_kind,
            });
        }
    }

    None
}

fn epoch_ms_to_seconds(epoch_ms: Option<i64>) -> Option<i64> {
    epoch_ms.map(|value| value / 1000)
}

/// Build a `session_id → most-recent-text` map by walking the `part`
/// table backward. Skips non-text parts (`step-start`, `step-finish`,
/// `reasoning`, `tool-call`, etc.) and empty text fields. The whole
/// query is best-effort — returns an empty map if the table is
/// missing, JSON1 isn't compiled in (it is under `bundled`), or any
/// row fails to parse.
fn read_last_message_previews(connection: &Connection) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let sql = "WITH ranked AS ( \
        SELECT session_id, json_extract(data, '$.text') AS text, \
               ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY time_created DESC, id DESC) AS rn \
        FROM part \
        WHERE json_extract(data, '$.type') = 'text' \
          AND json_extract(data, '$.text') IS NOT NULL \
          AND length(json_extract(data, '$.text')) > 0 \
    ) \
    SELECT session_id, text FROM ranked WHERE rn = 1";
    let Ok(mut statement) = connection.prepare(sql) else {
        return out;
    };
    let Ok(rows) = statement.query_map([], |row| {
        let session_id: String = row.get(0)?;
        let text: String = row.get(1)?;
        Ok((session_id, text))
    }) else {
        return out;
    };
    for row in rows.flatten() {
        out.insert(row.0, row.1);
    }
    out
}

/// True when the title contains an openCode-convention subagent marker:
/// `(@<name> subagent)`.  Known variants are `@explore` and `@general`;
/// the pattern matches any `(@`-prefixed name followed by ` subagent)` to
/// survive subagent-type additions without code changes.
fn title_contains_subagent_pattern(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    // Fast path: known fixed patterns.
    if lower.contains("(@explore subagent)") || lower.contains("(@general subagent)") {
        return true;
    }
    // Catch any future `(@<name> subagent)` variant.
    lower.contains("(@") && lower.contains(" subagent)")
}

/// Classify an openCode session as human-driven or a subagent worker.
///
/// Prefers the dedicated `kind` column when the schema carries it.
/// Falls back to a title-pattern heuristic when only `parent_id` is
/// available (e.g. `(@explore subagent)` / `(@general subagent)`).
fn classify_session_kind(
    raw_kind: Option<&str>,
    parent_id: Option<&str>,
    title: Option<&str>,
) -> Option<SessionKind> {
    if let Some(kind) = raw_kind {
        return match kind {
            "subagent" => Some(SessionKind::Subagent),
            _ => Some(SessionKind::Human),
        };
    }

    if parent_id.is_some()
        && let Some(title) = title
        && title_contains_subagent_pattern(title)
    {
        return Some(SessionKind::Subagent);
    }

    None
}

fn build_lineage_link(
    child_session_key: &str,
    parent_session_key: &str,
    state_scope: &str,
    resolved_parent: Option<&str>,
) -> GraphLink {
    // ADR 0018 vocabulary: opencode `parent_id` is a tree-shape pointer
    // without a documented operation type, so `unknown` is the honest label
    // until opencode publishes lineage semantics we can map more precisely.
    let lineage_kind = "unknown";

    let mut fields: Metadata = Metadata::new();
    fields.insert("harness_key".to_string(), json!(HARNESS_KEY));
    fields.insert("lineage_kind".to_string(), json!(lineage_kind));
    fields.insert(
        "parent_native_id".to_string(),
        json!(parent_session_key.to_string()),
    );

    let source = NodeId::AgentSession(AgentSessionId::new(
        HARNESS_KEY,
        state_scope,
        child_session_key,
    ));

    let (target, link_id) = match resolved_parent {
        Some(parent_key) => {
            let parent_id = AgentSessionId::new(HARNESS_KEY, state_scope, parent_key);
            (
                LinkEndpoint::Node {
                    id: NodeId::AgentSession(parent_id),
                },
                format!("opencode:lineage:{child_session_key}:parent_session:{parent_key}"),
            )
        }
        None => {
            let evidence = UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some(HARNESS_KEY.to_string()),
                native_id: Some(parent_session_key.to_string()),
                state_scope: Some(state_scope.to_string()),
                path: None,
                metadata: fields.clone(),
            };
            (
                LinkEndpoint::Unresolved { evidence },
                format!(
                    "opencode:lineage:{child_session_key}:parent_session:unresolved:{parent_session_key}"
                ),
            )
        }
    };

    GraphLink {
        id: link_id,
        source,
        target,
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: HARNESS_KEY.to_string(),
            evidence: Some(format!("opencode session.parent_id {lineage_kind}")),
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn read_legacy_sessions(state_root: &Path) -> Result<Vec<SessionInfo>> {
    let sessions = state_root.join("storage").join("session");

    if !sessions.exists() {
        return Ok(Vec::new());
    }

    let mut infos = Vec::new();

    for entry in fs::read_dir(&sessions)? {
        let session_dir = entry?.path();

        if !session_dir.is_dir() {
            continue;
        }

        let info_path = session_dir.join("info.json");

        if let Some(info) = read_info(&info_path) {
            infos.push(info);
        }
    }

    Ok(infos)
}

fn read_info(path: &Path) -> Option<SessionInfo> {
    let body = fs::read_to_string(path).ok()?;
    let mut info: SessionInfo = serde_json::from_str(&body).ok()?;
    if info.last_active_epoch.is_none() {
        info.last_active_epoch = info
            .time
            .as_ref()
            .and_then(|time| epoch_ms_to_seconds(time.updated.or(time.created)));
    }
    Some(info)
}

#[cfg(test)]
mod tests {
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

    fn write_sqlite_sessions(
        path: &Path,
        config: SqliteSchemaConfig,
        rows: &[SqliteSessionRow<'_>],
    ) {
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
            let placeholders: Vec<String> =
                (1..=col_names.len()).map(|n| format!("?{n}")).collect();
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
            other => panic!("expected resolved target, got {other:?}"),
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
            other => panic!("expected unresolved endpoint, got {other:?}"),
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
}
