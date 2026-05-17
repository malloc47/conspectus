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
    LinkState, Metadata, NodeId, Provenance, RelationKind, SourceMetadata, UnresolvedEndpoint,
};

pub const HARNESS_KEY: &str = "opencode";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenCodeAdapter;

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for OpenCodeAdapter {
    fn harness_key(&self) -> &str {
        HARNESS_KEY
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let Some(state_root) = context.harness_state_root(self.harness_key()) else {
            return Ok(GraphFragment::empty());
        };
        discover_state(state_root)
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

    // Newer opencode schemas expose `parent_id`; older ones don't. Try the
    // richer query first and fall back to the lineage-less form rather than
    // silently dropping every session when the column is missing.
    let with_parent =
        connection.prepare("SELECT id, directory, title, parent_id FROM session ORDER BY id");

    let (mut statement, has_parent) = match with_parent {
        Ok(statement) => (statement, true),
        Err(_) => {
            match connection.prepare("SELECT id, directory, title FROM session ORDER BY id") {
                Ok(statement) => (statement, false),
                Err(_) => return Vec::new(),
            }
        }
    };

    let Ok(rows) = statement.query_map([], |row| {
        Ok(SessionInfo {
            id: row.get::<_, String>(0)?,
            directory: row.get::<_, Option<String>>(1)?,
            title: row.get::<_, Option<String>>(2)?,
            parent_id: if has_parent {
                row.get::<_, Option<String>>(3)?
            } else {
                None
            },
        })
    }) else {
        return Vec::new();
    };

    rows.filter_map(|row| {
        let info = row.ok()?;
        if info.id.trim().is_empty() {
            None
        } else {
            Some(info)
        }
    })
    .collect()
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
    serde_json::from_str(&body).ok()
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
            true,
            &[SqliteSessionRow {
                id,
                directory,
                title,
                parent_id: None,
            }],
        );
    }

    struct SqliteSessionRow<'a> {
        id: &'a str,
        directory: Option<&'a str>,
        title: Option<&'a str>,
        parent_id: Option<&'a str>,
    }

    fn write_sqlite_sessions(path: &Path, with_parent_column: bool, rows: &[SqliteSessionRow<'_>]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("db parent");
        }
        let connection = Connection::open(path).expect("open sqlite fixture");
        let schema = if with_parent_column {
            "CREATE TABLE session (
                id TEXT,
                directory TEXT,
                title TEXT,
                time_created INTEGER,
                time_updated INTEGER,
                parent_id TEXT
            )"
        } else {
            "CREATE TABLE session (
                id TEXT,
                directory TEXT,
                title TEXT,
                time_created INTEGER,
                time_updated INTEGER
            )"
        };
        connection
            .execute(schema, [])
            .expect("create session table");

        for row in rows {
            if with_parent_column {
                connection
                    .execute(
                        "INSERT INTO session (id, directory, title, parent_id) VALUES (?1, ?2, ?3, ?4)",
                        (row.id, row.directory, row.title, row.parent_id),
                    )
                    .expect("insert session row with parent_id");
            } else {
                connection
                    .execute(
                        "INSERT INTO session (id, directory, title) VALUES (?1, ?2, ?3)",
                        (row.id, row.directory, row.title),
                    )
                    .expect("insert session row");
            }
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
            true,
            &[
                SqliteSessionRow {
                    id: "parent",
                    directory: Some("/work/repo"),
                    title: Some("parent"),
                    parent_id: None,
                },
                SqliteSessionRow {
                    id: "child",
                    directory: Some("/work/repo"),
                    title: Some("child"),
                    parent_id: Some("parent"),
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
            true,
            &[SqliteSessionRow {
                id: "orphan",
                directory: Some("/work/repo"),
                title: None,
                parent_id: Some("pruned-parent"),
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
            true,
            &[SqliteSessionRow {
                id: "loop",
                directory: Some("/work/repo"),
                title: None,
                parent_id: Some("loop"),
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
            false,
            &[SqliteSessionRow {
                id: "legacy",
                directory: Some("/work/repo"),
                title: Some("legacy session"),
                parent_id: None,
            }],
        );

        let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

        assert_eq!(fragment.nodes.len(), 1);
        assert!(
            lineage_links(&fragment).is_empty(),
            "schemas without parent_id must not emit lineage"
        );
    }
}
