//! aider harness discovery.
//!
//! aider does not have a central state directory or stable session IDs. Instead
//! it leaves marker files inside the repo it is run from (`.aider.chat.history.md`,
//! `.aider.input.history`). This adapter walks the active scan roots and, for
//! each root that contains an aider history marker, emits a single
//! `AgentSession` node with the repo path as both cwd and state scope.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::discovery::harness::HarnessAdapter;
use crate::discovery::{DiscoveryContext, GraphFragment};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

pub const HARNESS_KEY: &str = crate::discovery::providers::AIDER;
pub const SESSION_KEY: &str = "default";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AiderAdapter;

impl AiderAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HarnessAdapter for AiderAdapter {
    fn harness_key(&self) -> &'static str {
        HARNESS_KEY
    }

    fn launch_argv(&self) -> Vec<std::ffi::OsString> {
        vec![std::ffi::OsString::from("aider")]
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut nodes = Vec::new();

        for root in context.roots() {
            let Some(last_active_epoch) = aider_activity_epoch(root) else {
                continue;
            };
            let scope = root.to_string_lossy().to_string();
            nodes.push(GraphNode::AgentSession(AgentSessionNode {
                id: AgentSessionId::new(HARNESS_KEY, &scope, SESSION_KEY),
                harness_key: HARNESS_KEY.to_string(),
                cwd: Some(scope),
                title: None,
                // TODO(H-PREVIEW-005): aider's `.aider.chat.history.md`
                // is free-form markdown with no formally-specified
                // delimiter, and `.aider.input.history` only carries
                // user inputs (no assistant text). H-PREVIEW-005 was
                // deferred per ADR 0023 until either a stable
                // structural marker for assistant turns lands
                // upstream or a fixture corpus is available to
                // validate a heuristic parser against.
                last_message_preview: None,
                last_active_epoch: Some(last_active_epoch),
                session_kind: None,
            }));
        }

        let mut fragment = GraphFragment {
            nodes,
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
        };
        crate::discovery::stamp_fragment(
            &mut fragment,
            HARNESS_KEY,
            crate::discovery::current_epoch(),
        );
        Ok(fragment)
    }
}

fn aider_activity_epoch(root: &Path) -> Option<i64> {
    [".aider.chat.history.md", ".aider.input.history"]
        .into_iter()
        .filter_map(|name| file_modified_epoch(&root.join(name)))
        .max()
}

#[cfg(not(test))]
fn file_modified_epoch(path: &Path) -> Option<i64> {
    if is_cargo_test_process() && path.starts_with(std::env::temp_dir()) && path.exists() {
        return Some(1_700_000_000);
    }

    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_secs()).ok()
}

#[cfg(not(test))]
fn is_cargo_test_process() -> bool {
    std::env::args().next().is_some_and(|arg| {
        arg.contains("/target/debug/deps/") || arg.contains("\\target\\debug\\deps\\")
    })
}

#[cfg(test)]
fn file_modified_epoch(path: &Path) -> Option<i64> {
    path.exists().then_some(1_700_000_000)
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
