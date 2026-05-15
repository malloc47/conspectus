//! opencode harness discovery.
//!
//! Reads `$STATE_ROOT/storage/session/<id>/info.json` and emits one
//! `AgentSession` per discovered session. Files that fail to parse or are
//! missing the `id` field are skipped silently.

use std::fs;
use std::path::Path;

use anyhow::Result;
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
    let sessions = state_root.join("storage").join("session");

    if !sessions.exists() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut nodes = Vec::new();

    for entry in fs::read_dir(&sessions)? {
        let session_dir = entry?.path();

        if !session_dir.is_dir() {
            continue;
        }

        let info_path = session_dir.join("info.json");

        let Some(info) = read_info(&info_path) else {
            continue;
        };

        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, &info.id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: info.directory,
            title: info.title,
        }));
    }

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

fn read_info(path: &Path) -> Option<SessionInfo> {
    let body = fs::read_to_string(path).ok()?;
    serde_json::from_str(&body).ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

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
    fn skips_sessions_without_id() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        let dir = fixture.opencode_state_root().join("storage/session/bad");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("info.json"), "{\"directory\":\"/work\"}").expect("write");

        let fragment = OpenCodeAdapter::new().discover(&context).expect("discover");

        assert!(fragment.nodes.is_empty());
    }
}
