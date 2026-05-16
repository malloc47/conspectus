//! opencode harness discovery.
//!
//! Reads modern `$STATE_ROOT/opencode.db` sessions plus legacy
//! `$STATE_ROOT/storage/session/<id>/info.json` records and emits one
//! `AgentSession` per discovered session. Records that fail to parse or are
//! missing the `id` field are skipped silently.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

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

    let nodes = sessions
        .into_values()
        .map(|info| {
            GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(HARNESS_KEY, &state_scope, &info.id),
                harness_key: HARNESS_KEY.to_string(),
                cwd: info.directory,
                title: info.title,
            })
        })
        .collect();

    Ok(GraphFragment {
        nodes,
        candidate_links: Vec::new(),
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

    let Ok(mut statement) =
        connection.prepare("SELECT id, directory, title FROM session ORDER BY id")
    else {
        return Vec::new();
    };

    let Ok(rows) = statement.query_map([], |row| {
        Ok(SessionInfo {
            id: row.get::<_, String>(0)?,
            directory: row.get::<_, Option<String>>(1)?,
            title: row.get::<_, Option<String>>(2)?,
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("db parent");
        }
        let connection = Connection::open(path).expect("open sqlite fixture");
        connection
            .execute(
                "CREATE TABLE session (
                    id TEXT,
                    directory TEXT,
                    title TEXT,
                    time_created INTEGER,
                    time_updated INTEGER,
                    parent_id TEXT
                )",
                [],
            )
            .expect("create session table");
        connection
            .execute(
                "INSERT INTO session (id, directory, title) VALUES (?1, ?2, ?3)",
                (id, directory, title),
            )
            .expect("insert session row");
    }
}
