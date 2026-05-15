//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::model::{Diagnostic, GraphLink, GraphNode, GraphSnapshot};

pub mod git;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryContext {
    roots: Vec<PathBuf>,
}

impl DiscoveryContext {
    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
        }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphFragment {
    pub nodes: Vec<GraphNode>,
    pub candidate_links: Vec<GraphLink>,
    pub diagnostics: Vec<Diagnostic>,
}

impl GraphFragment {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn into_snapshot(self) -> GraphSnapshot {
        merge_fragments([self])
    }
}

pub trait DiscoveryProvider {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;
}

#[derive(Default)]
pub struct LocalDiscovery {
    providers: Vec<Box<dyn DiscoveryProvider>>,
}

impl LocalDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, provider: impl DiscoveryProvider + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    pub fn discover(&self, context: &DiscoveryContext) -> Result<GraphSnapshot> {
        let mut fragments = Vec::with_capacity(self.providers.len());

        for provider in &self.providers {
            fragments.push(provider.discover(context)?);
        }

        Ok(merge_fragments(fragments))
    }
}

pub fn discover_empty_at(root: impl AsRef<Path>) -> Result<GraphSnapshot> {
    LocalDiscovery::new().discover(&DiscoveryContext::from_root(root.as_ref()))
}

pub fn merge_fragments(fragments: impl IntoIterator<Item = GraphFragment>) -> GraphSnapshot {
    let mut nodes = BTreeMap::new();
    let mut candidate_links = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for fragment in fragments {
        for node in fragment.nodes {
            nodes.entry(node.id()).or_insert(node);
        }

        for link in fragment.candidate_links {
            candidate_links.entry(link.id.clone()).or_insert(link);
        }

        diagnostics.extend(fragment.diagnostics);
    }

    let mut snapshot = GraphSnapshot {
        nodes: nodes.into_values().collect(),
        candidate_links: candidate_links.into_values().collect(),
        resolved_relationships: Vec::new(),
        diagnostics,
    };
    snapshot.canonicalize();
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AgentSessionId, AgentSessionNode, GraphLink, LinkEndpoint, MuxSessionId, MuxSessionNode,
        NodeId, Provenance, RelationKind,
    };

    struct StaticProvider(GraphFragment);

    impl DiscoveryProvider for StaticProvider {
        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn empty_local_discovery_returns_empty_snapshot() {
        let snapshot = LocalDiscovery::new()
            .discover(&DiscoveryContext::from_root("/workspace"))
            .expect("empty discovery succeeds");

        assert_eq!(snapshot, GraphSnapshot::empty());
    }

    #[test]
    fn local_discovery_merges_provider_fragments_without_resolving() {
        let session = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
        let mux = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
        let link = GraphLink::new(
            "session-mux",
            session.clone(),
            LinkEndpoint::Node { id: mux.clone() },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );
        let discovery = LocalDiscovery::new()
            .with_provider(StaticProvider(GraphFragment {
                nodes: vec![GraphNode::AgentSession(AgentSessionNode {
                    id: AgentSessionId::new("codex", "global", "s1"),
                    harness_key: "codex".to_string(),
                    cwd: None,
                    title: None,
                })],
                candidate_links: vec![link.clone()],
                diagnostics: Vec::new(),
            }))
            .with_provider(StaticProvider(GraphFragment {
                nodes: vec![GraphNode::MuxSession(MuxSessionNode {
                    id: MuxSessionId::new("tmux:s1"),
                    backend: "tmux".to_string(),
                    native_id: "s1".to_string(),
                    cwd: None,
                })],
                candidate_links: Vec::new(),
                diagnostics: Vec::new(),
            }));

        let snapshot = discovery
            .discover(&DiscoveryContext::from_root("/workspace"))
            .expect("discovery succeeds");

        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.candidate_links, vec![link]);
        assert!(snapshot.resolved_relationships.is_empty());
    }

    #[test]
    fn merge_fragments_deduplicates_nodes_and_links_by_identity() {
        let node = GraphNode::MuxSession(MuxSessionNode {
            id: MuxSessionId::new("tmux:s1"),
            backend: "tmux".to_string(),
            native_id: "s1".to_string(),
            cwd: None,
        });
        let source = NodeId::AgentSession(AgentSessionId::new("codex", "global", "s1"));
        let target = NodeId::MuxSession(MuxSessionId::new("tmux:s1"));
        let link = GraphLink::new(
            "session-mux",
            source,
            LinkEndpoint::Node { id: target },
            RelationKind::LinkedToMux,
            Provenance::StrongDiscovered,
        );

        let snapshot = merge_fragments([
            GraphFragment {
                nodes: vec![node.clone()],
                candidate_links: vec![link.clone()],
                diagnostics: Vec::new(),
            },
            GraphFragment {
                nodes: vec![node],
                candidate_links: vec![link.clone()],
                diagnostics: Vec::new(),
            },
        ]);

        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.candidate_links, vec![link]);
    }
}
