//! Claude Code harness discovery.
//!
//! Walks `$STATE_ROOT/projects/<encoded-cwd>/<session>.jsonl` and emits one
//! `AgentSession` per discovered session. The first JSONL line is parsed to
//! recover the `sessionId`, `cwd`, and optional `summary` fields; malformed
//! files are skipped silently. Sessions whose first line is missing the
//! `sessionId` field are also skipped to avoid emitting fabricated identities.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

pub const HARNESS_KEY: &str = "claude-code";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for ClaudeCodeAdapter {
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
    let projects = state_root.join("projects");

    if !projects.exists() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut nodes = Vec::new();

    for project in fs::read_dir(&projects)? {
        let project_dir = project?.path();

        if !project_dir.is_dir() {
            continue;
        }

        for entry in fs::read_dir(&project_dir)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if !name.ends_with(".jsonl") {
                continue;
            }

            let Some(meta) = read_session_header(&path) else {
                continue;
            };

            nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(HARNESS_KEY, &state_scope, &meta.session_id),
                harness_key: HARNESS_KEY.to_string(),
                cwd: meta.cwd,
                title: meta.summary,
            }));
        }
    }

    Ok(GraphFragment {
        nodes,
        candidate_links: Vec::new(),
        diagnostics: Vec::new(),
    })
}

#[derive(Deserialize)]
struct SessionHeader {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    summary: Option<String>,
}

fn read_session_header(path: &Path) -> Option<SessionHeader> {
    let body = fs::read_to_string(path).ok()?;
    let first = body.lines().next()?;
    serde_json::from_str(first).ok()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::discovery::harness::fixtures::{
        ClaudeCodeSessionRecord, HarnessFixture, write_malformed,
    };
    use crate::model::GraphNode;

    fn context_with_state(temp: &TempDir) -> (DiscoveryContext, HarnessFixture) {
        let fixture = HarnessFixture::at(temp.path());
        let context = DiscoveryContext::default()
            .with_harness_state_root(HARNESS_KEY, fixture.claude_code_state_root());
        (context, fixture)
    }

    #[test]
    fn adapter_returns_empty_when_no_state_root_configured() {
        let fragment = ClaudeCodeAdapter::new()
            .discover(&DiscoveryContext::default())
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn adapter_returns_empty_when_projects_dir_missing() {
        let temp = TempDir::new().expect("temp");
        let (context, _) = context_with_state(&temp);

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn discovers_claude_sessions_with_summary_and_stable_ids() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_claude_code_session(
                &ClaudeCodeSessionRecord::new("session-a", "/work/alpha")
                    .with_summary("alpha work"),
            )
            .expect("write a");
        fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("session-b", "/work/beta"))
            .expect("write b");

        let first = ClaudeCodeAdapter::new().discover(&context).expect("first");
        let second = ClaudeCodeAdapter::new().discover(&context).expect("second");

        assert_eq!(first, second);

        let sessions: Vec<_> = first
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(sessions.len(), 2);
        let a = sessions
            .iter()
            .find(|s| s.id.session_key == "session-a")
            .expect("session-a");
        assert_eq!(a.cwd.as_deref(), Some("/work/alpha"));
        assert_eq!(a.title.as_deref(), Some("alpha work"));

        let b = sessions
            .iter()
            .find(|s| s.id.session_key == "session-b")
            .expect("session-b");
        assert!(b.title.is_none(), "missing summary should leave title None");
    }

    #[test]
    fn skips_malformed_records() {
        let temp = TempDir::new().expect("temp");
        let (context, fixture) = context_with_state(&temp);
        fixture
            .write_claude_code_session(&ClaudeCodeSessionRecord::new("good", "/work/repo"))
            .expect("write good");
        let project_dir = fixture
            .claude_code_state_root()
            .join("projects")
            .join("-work-other");
        std::fs::create_dir_all(&project_dir).expect("create project dir");
        write_malformed(project_dir.join("bad.jsonl")).expect("malformed");

        let fragment = ClaudeCodeAdapter::new()
            .discover(&context)
            .expect("discover");

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
}
