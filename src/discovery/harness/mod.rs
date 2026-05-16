//! Agent harness discovery boundaries.
//!
//! Each supported harness ships with a small adapter implementing
//! [`HarnessAdapter`]. Adapters take the active [`DiscoveryContext`] and emit a
//! provider-neutral [`GraphFragment`] containing `AgentSession` nodes and the
//! source metadata needed to preserve provider provenance. Adapters must not
//! perform rendering, fork lineage resolution, or session/mux scoring – those
//! belong to higher layers.
//!
//! Most adapters look up a single state root via
//! [`DiscoveryContext::harness_state_root`]; per-repo harnesses such as aider
//! walk the configured scan roots instead. The [`HarnessDiscovery`] coordinator
//! is a [`DiscoveryProvider`] that runs every registered adapter and merges
//! fragments deterministically through [`merge_fragments`].

use anyhow::Result;

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::GraphSnapshot;

pub mod aider;
pub mod claude_code;
pub mod codex;
#[doc(hidden)]
pub mod fixtures;
pub mod opencode;

pub use aider::AiderAdapter;
pub use claude_code::ClaudeCodeAdapter;
pub use codex::CodexAdapter;
pub use opencode::OpenCodeAdapter;

pub trait HarnessAdapter: Send + Sync {
    fn harness_key(&self) -> &str;

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;
}

#[derive(Default)]
pub struct HarnessDiscovery {
    adapters: Vec<Box<dyn HarnessAdapter>>,
}

impl HarnessDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter: impl HarnessAdapter + 'static) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    pub fn with_default_adapters() -> Self {
        Self::new()
            .with_adapter(CodexAdapter::new())
            .with_adapter(ClaudeCodeAdapter::new())
            .with_adapter(OpenCodeAdapter::new())
            .with_adapter(AiderAdapter::new())
    }
}

impl DiscoveryProvider for HarnessDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut fragments = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            fragments.push(adapter.discover(context)?);
        }

        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

pub(crate) fn snapshot_fragment(snapshot: GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::model::{AgentSessionId, AgentSessionNode, GraphNode};

    struct StaticAdapter {
        key: &'static str,
        fragment: GraphFragment,
    }

    impl HarnessAdapter for StaticAdapter {
        fn harness_key(&self) -> &str {
            self.key
        }

        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(self.fragment.clone())
        }
    }

    struct StateAwareAdapter;

    impl HarnessAdapter for StateAwareAdapter {
        fn harness_key(&self) -> &str {
            "state-aware"
        }

        fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
            let Some(root) = context.harness_state_root(self.harness_key()) else {
                return Ok(GraphFragment::empty());
            };

            if !root.exists() {
                return Ok(GraphFragment::empty());
            }

            Ok(GraphFragment {
                nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("state-aware", "scope", "s1"),
                    harness_key: "state-aware".to_string(),
                    cwd: None,
                    title: None,
                })],
                candidate_links: Vec::new(),
                diagnostics: Vec::new(),
            })
        }
    }

    #[test]
    fn harness_discovery_without_adapters_returns_empty_fragment() {
        let fragment = HarnessDiscovery::new()
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment, GraphFragment::empty());
    }

    #[test]
    fn missing_state_directory_yields_empty_fragment_without_error() {
        let context =
            DiscoveryContext::default().with_harness_state_root("state-aware", "/does/not/exist");

        let fragment = HarnessDiscovery::new()
            .with_adapter(StateAwareAdapter)
            .discover(&context)
            .expect("missing state root does not error");

        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
    }

    #[test]
    fn adapter_receives_harness_state_root_from_context() {
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));

        struct Echo {
            captured: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
        }

        impl HarnessAdapter for Echo {
            fn harness_key(&self) -> &str {
                "codex"
            }

            fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
                *self.captured.lock().expect("lock") = context
                    .harness_state_root(self.harness_key())
                    .map(Path::to_path_buf);
                Ok(GraphFragment::empty())
            }
        }

        let context = DiscoveryContext::default().with_harness_state_root("codex", "/state/codex");

        HarnessDiscovery::new()
            .with_adapter(Echo {
                captured: captured.clone(),
            })
            .discover(&context)
            .expect("discovery succeeds");

        let observed = captured
            .lock()
            .expect("lock")
            .clone()
            .expect("state root passed to adapter");
        assert_eq!(observed, PathBuf::from("/state/codex"));
    }

    #[test]
    fn harness_discovery_merges_fragments_deterministically() {
        let session_alpha = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "scope", "alpha"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
        });
        let session_beta = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "scope", "beta"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
        });

        let fragment = HarnessDiscovery::new()
            .with_adapter(StaticAdapter {
                key: "codex-beta",
                fragment: GraphFragment {
                    nodes: vec![session_beta.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                },
            })
            .with_adapter(StaticAdapter {
                key: "codex-alpha",
                fragment: GraphFragment {
                    nodes: vec![session_alpha.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                },
            })
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment.nodes, vec![session_alpha, session_beta]);
    }
}
