//! Agent harness discovery boundaries.
//!
//! Each supported harness ships with a small adapter implementing
//! [`HarnessAdapter`]. Adapters take a single read-only state root and emit a
//! provider-neutral [`GraphFragment`] containing `AgentSession` nodes and the
//! source metadata needed to preserve provider provenance. Adapters must not
//! perform rendering, fork lineage resolution, or session/mux scoring – those
//! belong to higher layers.
//!
//! The [`HarnessDiscovery`] coordinator is a [`DiscoveryProvider`] that runs
//! every registered adapter, looks up its state root from
//! [`DiscoveryContext::harness_state_root`], and merges fragments
//! deterministically through [`merge_fragments`].

use std::path::Path;

use anyhow::Result;

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::GraphSnapshot;

pub trait HarnessAdapter: Send + Sync {
    fn harness_key(&self) -> &str;

    fn discover(&self, state_root: Option<&Path>) -> Result<GraphFragment>;
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
}

impl DiscoveryProvider for HarnessDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let mut fragments = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            let state_root = context.harness_state_root(adapter.harness_key());
            fragments.push(adapter.discover(state_root)?);
        }

        Ok(snapshot_fragment(merge_fragments(fragments)))
    }
}

fn snapshot_fragment(snapshot: GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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

        fn discover(&self, _state_root: Option<&Path>) -> Result<GraphFragment> {
            Ok(self.fragment.clone())
        }
    }

    struct StateAwareAdapter;

    impl HarnessAdapter for StateAwareAdapter {
        fn harness_key(&self) -> &str {
            "state-aware"
        }

        fn discover(&self, state_root: Option<&Path>) -> Result<GraphFragment> {
            let Some(root) = state_root else {
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

            fn discover(&self, state_root: Option<&Path>) -> Result<GraphFragment> {
                *self.captured.lock().expect("lock") = state_root.map(PathBuf::from);
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

        // merge_fragments canonicalizes order regardless of adapter registration order.
        assert_eq!(fragment.nodes, vec![session_alpha, session_beta]);
    }
}
