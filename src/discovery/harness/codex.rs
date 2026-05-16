//! Codex harness discovery.
//!
//! Reads the first JSONL line (`session_meta`) of each rollout file under
//! `$STATE_ROOT/sessions/**/rollout-*.jsonl` and emits one `AgentSession` per
//! discovered session. Real Codex stores rollouts under
//! `sessions/YYYY/MM/DD/`, so the scanner walks the tree recursively rather
//! than only looking at the top-level directory. Malformed records and
//! rollouts without a `session_meta` envelope are skipped silently so a single
//! bad file cannot poison discovery.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

pub const HARNESS_KEY: &str = "codex";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for CodexAdapter {
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
    let sessions_dir = state_root.join("sessions");

    if !sessions_dir.exists() {
        return Ok(GraphFragment::empty());
    }

    let state_scope = state_root.to_string_lossy().to_string();
    let mut nodes = Vec::new();

    visit_rollouts(&sessions_dir, &mut |path| {
        let Some(meta) = read_session_meta(path) else {
            return;
        };

        nodes.push(GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new(HARNESS_KEY, &state_scope, &meta.id),
            harness_key: HARNESS_KEY.to_string(),
            cwd: meta.cwd,
            title: None,
        }));
    })?;

    Ok(GraphFragment {
        nodes,
        candidate_links: Vec::new(),
        diagnostics: Vec::new(),
    })
}

fn visit_rollouts(dir: &Path, on_rollout: &mut dyn FnMut(&Path)) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            visit_rollouts(&path, on_rollout)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            on_rollout(&path);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct SessionMetaEnvelope {
    #[serde(rename = "type")]
    kind: String,
    payload: SessionMetaPayload,
}

#[derive(Deserialize)]
struct SessionMetaPayload {
    id: String,
    #[serde(default)]
    cwd: Option<String>,
}

fn read_session_meta(path: &Path) -> Option<SessionMetaPayload> {
    let body = fs::read_to_string(path).ok()?;
    let first = body.lines().next()?;
    let envelope: SessionMetaEnvelope = serde_json::from_str(first).ok()?;

    if envelope.kind != "session_meta" {
        return None;
    }

    Some(envelope.payload)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::harness::fixtures::{
        CodexSessionRecord, HarnessFixture, write_malformed,
    };
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
}
