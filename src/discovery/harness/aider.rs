//! aider harness discovery.
//!
//! aider does not have a central state directory or stable session IDs. Instead
//! it leaves marker files inside the repo it is run from (`.aider.chat.history.md`,
//! `.aider.input.history`). This adapter walks the active scan roots and, for
//! each root that contains an aider history marker, emits a single
//! `AgentSession` node with the repo path as both cwd and state scope.

use std::path::Path;

use anyhow::Result;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

pub const HARNESS_KEY: &str = "aider";
pub const SESSION_KEY: &str = "default";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AiderAdapter;

impl AiderAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for AiderAdapter {
    fn harness_key(&self) -> &str {
        HARNESS_KEY
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut nodes = Vec::new();

        for root in context.roots() {
            if !has_aider_state(root) {
                continue;
            }
            let scope = root.to_string_lossy().to_string();
            nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(HARNESS_KEY, &scope, SESSION_KEY),
                harness_key: HARNESS_KEY.to_string(),
                cwd: Some(scope),
                title: None,
            }));
        }

        Ok(GraphFragment {
            nodes,
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
        })
    }
}

fn has_aider_state(root: &Path) -> bool {
    root.join(".aider.chat.history.md").is_file() || root.join(".aider.input.history").is_file()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::discovery::harness::fixtures::write_aider_state;
    use crate::model::GraphNode;

    #[test]
    fn adapter_returns_empty_for_roots_without_markers() {
        let temp = TempDir::new().expect("temp");
        let context = DiscoveryContext::from_roots([temp.path()]).expect("context");

        let fragment = AiderAdapter::new().discover(&context).expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn adapter_emits_session_per_marked_root_with_stable_ids() {
        let temp = TempDir::new().expect("temp");
        let alpha = temp.path().join("alpha");
        let beta = temp.path().join("beta");
        let plain = temp.path().join("plain");
        fs::create_dir_all(&plain).expect("plain");
        write_aider_state(&alpha).expect("alpha");
        write_aider_state(&beta).expect("beta");

        let context = DiscoveryContext::from_roots([&alpha, &beta, &plain]).expect("context");

        let first = AiderAdapter::new().discover(&context).expect("first");
        let second = AiderAdapter::new().discover(&context).expect("second");

        assert_eq!(first, second);

        let sessions: Vec<_> = first
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::AgentSession(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(sessions.len(), 2, "only marked roots produce sessions");
        for session in &sessions {
            assert_eq!(session.harness_key, HARNESS_KEY);
            assert_eq!(session.id.session_key, SESSION_KEY);
            assert!(session.cwd.is_some());
        }
    }
}
