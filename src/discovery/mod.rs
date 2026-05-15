//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::model::{Diagnostic, GraphLink, GraphNode, GraphSnapshot};

pub mod git;
pub mod workspace;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryContext {
    roots: Vec<PathBuf>,
}

impl DiscoveryContext {
    pub fn from_current_dir() -> Result<Self> {
        Self::from_roots([env::current_dir().context("failed to read current directory")?])
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
        }
    }

    pub fn from_roots(roots: impl IntoIterator<Item = impl Into<PathBuf>>) -> Result<Self> {
        let mut seen = BTreeSet::new();
        let mut normalized = Vec::new();

        for root in roots {
            let root = root.into();
            let normalized_root = normalize_scan_root(&root)?;

            if seen.insert(normalized_root.clone()) {
                normalized.push(normalized_root);
            }
        }

        Ok(Self { roots: normalized })
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

pub fn discover_local_at_roots(
    roots: impl IntoIterator<Item = impl Into<PathBuf>>,
) -> Result<GraphSnapshot> {
    LocalDiscovery::new()
        .with_provider(git::GitDiscovery::new())
        .with_provider(workspace::GenericWorkspaceDiscovery::new())
        .discover(&DiscoveryContext::from_roots(roots)?)
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

fn normalize_scan_root(root: &Path) -> Result<PathBuf> {
    if !root.exists() {
        bail!("scan root does not exist: {}", root.display());
    }

    if !root.is_dir() {
        bail!("scan root is not a directory: {}", root.display());
    }

    root.canonicalize()
        .with_context(|| format!("failed to canonicalize scan root: {}", root.display()))
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

    #[test]
    fn context_normalizes_and_deduplicates_scan_roots() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = temp.path();
        let nested = root.join("nested");
        std::fs::create_dir(&nested).expect("create nested dir");

        let context =
            DiscoveryContext::from_roots([root, root, nested.as_path()]).expect("roots normalize");

        assert_eq!(context.roots().len(), 2);
        assert!(context.roots()[0].is_absolute());
    }

    #[test]
    fn context_rejects_missing_scan_roots() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let missing = temp.path().join("missing");

        let error =
            DiscoveryContext::from_roots([missing]).expect_err("missing roots should be rejected");

        assert!(error.to_string().contains("scan root does not exist"));
    }

    #[test]
    fn local_discovery_accepts_existing_non_git_roots_as_sparse_graphs() {
        let temp = tempfile::TempDir::new().expect("temp dir");

        let snapshot = discover_local_at_roots([temp.path()]).expect("local discovery succeeds");

        assert_eq!(snapshot, GraphSnapshot::empty());
    }
}
