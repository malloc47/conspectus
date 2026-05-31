//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::config::ConfigLoader;
use crate::model::{Diagnostic, GraphLink, GraphNode, GraphSnapshot};

pub mod aliases;
pub mod atelier;
pub mod codex_log;
pub mod cross_link;
pub mod declared;
pub mod forge;
pub mod git;
pub mod harness;
pub mod hook_sidecar;
pub mod tmux;
pub mod workspace;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryContext {
    roots: Vec<PathBuf>,
    harness_state_roots: BTreeMap<String, PathBuf>,
}

impl DiscoveryContext {
    pub fn from_current_dir() -> Result<Self> {
        Self::from_roots([env::current_dir().context("failed to read current directory")?])
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
            harness_state_roots: BTreeMap::new(),
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

        Ok(Self {
            roots: normalized,
            harness_state_roots: BTreeMap::new(),
        })
    }

    pub fn with_harness_state_root(
        mut self,
        harness_key: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Self {
        self.harness_state_roots
            .insert(harness_key.into(), root.into());
        self
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn harness_state_root(&self, harness_key: &str) -> Option<&Path> {
        self.harness_state_roots
            .get(harness_key)
            .map(PathBuf::as_path)
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
    discover_local_with(roots, LocalDiscoveryConfig::from_env())
}

pub fn discover_local_with(
    roots: impl IntoIterator<Item = impl Into<PathBuf>>,
    config: LocalDiscoveryConfig,
) -> Result<GraphSnapshot> {
    let mut context = DiscoveryContext::from_roots(roots)?;

    for (key, root) in &config.harness_state_roots {
        context = context.with_harness_state_root(key.clone(), root.clone());
    }

    let mut providers = LocalDiscovery::new()
        .with_provider(git::GitDiscovery::new())
        .with_provider(atelier::AtelierWorkspaceDiscovery::new())
        .with_provider(workspace::GenericWorkspaceDiscovery::new())
        .with_provider(harness::HarnessDiscovery::with_default_adapters());

    if let Some(runner) = config.tmux_runner {
        providers = providers.with_provider(tmux::TmuxDiscovery::with_runner(runner));
    }

    if let Some(runner) = config.forge_runner {
        providers =
            providers.with_provider(forge::github::GitHubForgeProvider::with_runner(runner));
    }

    let mut snapshot = providers.discover(&context)?;
    let cwd_git_fragment = observed_cwd_git_fragment(&snapshot);
    snapshot = merge_fragments([snapshot_fragment(snapshot), cwd_git_fragment]);
    let codex_pids_per_mux = if config.process_tree_enabled {
        cross_link::infer(&mut snapshot);
        cross_link::active_harness_pids_per_mux(&snapshot, &cross_link::LinuxProcSnapshot)
    } else {
        cross_link::infer_without_process_tree(&mut snapshot);
        std::collections::BTreeMap::new()
    };
    if let Some(codex_state_root) = config.harness_state_roots.get(harness::codex::HARNESS_KEY) {
        codex_log::apply_codex_log_attribution(
            &mut snapshot,
            codex_state_root,
            &codex_pids_per_mux,
            codex_log::current_epoch(),
        );
    }
    if let Some(root) = &config.hook_sidecar_root {
        hook_sidecar::apply_hook_sidecars(&mut snapshot, root, hook_sidecar::current_epoch());
    }
    if let Some(loader) = &config.declared_config_loader {
        declared::apply_declared_links(&mut snapshot, &context, loader);
        aliases::apply_aliases(&mut snapshot, &context, loader);
    }
    Ok(snapshot)
}

/// Configuration that controls which providers run during local discovery.
pub struct LocalDiscoveryConfig {
    pub harness_state_roots: BTreeMap<String, PathBuf>,
    pub tmux_runner: Option<Box<dyn tmux::TmuxRunner>>,
    pub forge_runner: Option<Box<dyn forge::GhRunner>>,
    pub process_tree_enabled: bool,
    pub hook_sidecar_root: Option<PathBuf>,
    pub declared_config_loader: Option<ConfigLoader>,
}

impl LocalDiscoveryConfig {
    /// Defaults derived from the process environment: harness state roots from
    /// `CONSPECTUS_<HARNESS>_STATE` (falling back to standard `$HOME`-relative
    /// paths) and a real `SystemTmux` runner unless `CONSPECTUS_DISABLE_TMUX`
    /// is set.
    pub fn from_env() -> Self {
        let mut harness_state_roots = BTreeMap::new();

        if let Some(path) = env_state_root("CONSPECTUS_CODEX_STATE", ".codex") {
            harness_state_roots.insert(harness::codex::HARNESS_KEY.to_string(), path);
        }
        if let Some(path) = env_state_root("CONSPECTUS_CLAUDE_CODE_STATE", ".claude") {
            harness_state_roots.insert(harness::claude_code::HARNESS_KEY.to_string(), path);
        }
        if let Some(path) = env_state_root("CONSPECTUS_OPENCODE_STATE", ".local/share/opencode") {
            harness_state_roots.insert(harness::opencode::HARNESS_KEY.to_string(), path);
        }

        let tmux_runner: Option<Box<dyn tmux::TmuxRunner>> =
            if env::var_os("CONSPECTUS_DISABLE_TMUX").is_some() {
                None
            } else {
                Some(Box::new(tmux::SystemTmux::new()))
            };

        let forge_runner: Option<Box<dyn forge::GhRunner>> =
            if env::var_os("CONSPECTUS_DISABLE_FORGE").is_some() {
                None
            } else {
                Some(Box::new(forge::SystemGh::new()))
            };

        Self {
            harness_state_roots,
            tmux_runner,
            forge_runner,
            process_tree_enabled: env::var_os("CONSPECTUS_DISABLE_PROCTREE").is_none(),
            hook_sidecar_root: hook_sidecar::default_sidecar_root(),
            declared_config_loader: Some(ConfigLoader::from_env()),
        }
    }

    pub fn empty() -> Self {
        Self {
            harness_state_roots: BTreeMap::new(),
            tmux_runner: None,
            forge_runner: None,
            process_tree_enabled: false,
            hook_sidecar_root: None,
            declared_config_loader: None,
        }
    }

    pub fn with_harness_state_root(
        mut self,
        harness_key: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Self {
        self.harness_state_roots
            .insert(harness_key.into(), root.into());
        self
    }

    pub fn with_tmux_runner(mut self, runner: impl tmux::TmuxRunner + 'static) -> Self {
        self.tmux_runner = Some(Box::new(runner));
        self
    }

    pub fn without_tmux(mut self) -> Self {
        self.tmux_runner = None;
        self
    }

    pub fn with_forge_runner(mut self, runner: impl forge::GhRunner + 'static) -> Self {
        self.forge_runner = Some(Box::new(runner));
        self
    }

    pub fn without_forge(mut self) -> Self {
        self.forge_runner = None;
        self
    }

    pub fn with_process_tree(mut self) -> Self {
        self.process_tree_enabled = true;
        self
    }

    pub fn without_process_tree(mut self) -> Self {
        self.process_tree_enabled = false;
        self
    }

    pub fn with_hook_sidecar_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.hook_sidecar_root = Some(root.into());
        self
    }

    pub fn without_hook_sidecar(mut self) -> Self {
        self.hook_sidecar_root = None;
        self
    }

    pub fn with_declared_config_loader(mut self, loader: ConfigLoader) -> Self {
        self.declared_config_loader = Some(loader);
        self
    }

    pub fn without_declared_config(mut self) -> Self {
        self.declared_config_loader = None;
        self
    }
}

fn env_state_root(env_key: &str, home_relative: &str) -> Option<PathBuf> {
    if let Some(value) = env::var_os(env_key) {
        return Some(PathBuf::from(value));
    }
    env::var_os("HOME").map(|home| PathBuf::from(home).join(home_relative))
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
        aliases: crate::aliases::AliasOverlay::new(),
    };
    snapshot.canonicalize();
    snapshot
}

fn observed_cwd_git_fragment(snapshot: &GraphSnapshot) -> GraphFragment {
    let mut roots = BTreeSet::new();

    for node in &snapshot.nodes {
        match node {
            GraphNode::AgentSession(session) => {
                if let Some(cwd) = &session.cwd {
                    roots.insert(PathBuf::from(cwd));
                }
            }
            GraphNode::MuxSession(mux) => {
                if let Some(cwd) = &mux.cwd {
                    roots.insert(PathBuf::from(cwd));
                }
                if let Some(cwd) = &mux.active_pane_current_path {
                    roots.insert(PathBuf::from(cwd));
                }
            }
            _ => {}
        }
    }

    let probe = git::GitProbe::new();
    let mut fragments = Vec::new();
    let mut diagnostics = Vec::new();

    for root in roots {
        if !root.is_dir() {
            continue;
        }

        match probe.probe(&root) {
            Ok(Some(result)) => fragments.push(git::fragment_from_probe(&result)),
            Ok(None) => {}
            Err(error) => diagnostics.push(Diagnostic::Config {
                path: root.to_string_lossy().to_string(),
                message: format!("failed to probe observed cwd for git context: {error:#}"),
            }),
        }
    }

    let mut fragment = snapshot_fragment(merge_fragments(fragments));
    fragment.diagnostics.extend(diagnostics);
    fragment
}

fn snapshot_fragment(snapshot: GraphSnapshot) -> GraphFragment {
    GraphFragment {
        nodes: snapshot.nodes,
        candidate_links: snapshot.candidate_links,
        diagnostics: snapshot.diagnostics,
    }
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
                    last_message_preview: None,
                    last_active_epoch: None,
                    session_kind: None,
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
                    active_pane_command: None,
                    active_pane_pid: None,
                    active_pane_current_path: None,
                    active_pane_start_command: None,
                    client_attached: None,
                    activity_epoch: None,
                    created_epoch: None,
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
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
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

        let snapshot = discover_local_with([temp.path()], LocalDiscoveryConfig::empty())
            .expect("local discovery succeeds");

        assert_eq!(snapshot, GraphSnapshot::empty());
    }

    #[test]
    fn discover_local_with_runs_harness_and_tmux_providers_and_cross_links() {
        use crate::discovery::harness::codex::HARNESS_KEY as CODEX_KEY;
        use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};
        use crate::discovery::tmux::FakeTmux;
        use crate::model::{GraphNode, RelationKind};

        let temp = tempfile::TempDir::new().expect("temp dir");
        let scan_root = temp.path().join("scan");
        std::fs::create_dir(&scan_root).expect("scan dir");
        let harness_root = temp.path().join("state");
        std::fs::create_dir(&harness_root).expect("state dir");
        let fixture = HarnessFixture::at(&harness_root);
        fixture
            .write_codex_session(&CodexSessionRecord::new("session-x").with_cwd("/work/x"))
            .expect("write codex session");

        let config = LocalDiscoveryConfig::empty()
            .with_harness_state_root(CODEX_KEY, fixture.codex_state_root())
            .with_tmux_runner(FakeTmux::with_sessions(
                "alpha\t/work/x\t1700000500\t1700000000\n",
            ));

        let snapshot = discover_local_with([scan_root.as_path()], config).expect("discover");

        assert!(
            snapshot.nodes.iter().any(
                |node| matches!(node, GraphNode::AgentSession(s) if s.harness_key == CODEX_KEY)
            ),
            "codex session node should be present"
        );
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::MuxSession(_))),
            "fake tmux session node should be present"
        );
        assert!(
            snapshot
                .candidate_links
                .iter()
                .any(|link| link.relation == RelationKind::LinkedToMux),
            "cross_link should infer at least one LinkedToMux candidate"
        );
    }

    #[test]
    fn discover_local_with_runs_forge_provider_for_github_repos() {
        use crate::discovery::forge::FakeGh;
        use std::process::Command as ProcessCommand;

        let temp = tempfile::TempDir::new().expect("temp dir");
        let repo_root = temp.path().join("repo");
        std::fs::create_dir(&repo_root).expect("repo dir");
        let run_git = |args: &[&str]| {
            let output = ProcessCommand::new("git")
                .args(args)
                .current_dir(&repo_root)
                .output()
                .expect("run git");
            assert!(output.status.success(), "git {} failed", args.join(" "));
        };
        run_git(&["init", "--initial-branch", "main"]);
        run_git(&["config", "user.name", "Conspectus Test"]);
        run_git(&["config", "user.email", "test@example.invalid"]);
        run_git(&["remote", "add", "origin", "git@github.com:octo/repo.git"]);
        std::fs::write(repo_root.join("README.md"), "fixture\n").expect("write fixture");
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-m", "initial"]);

        let body = r#"[{"number": 42, "state": "OPEN", "headRefName": "main"}]"#;
        let config =
            LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));

        let snapshot = discover_local_with([repo_root.as_path()], config).expect("discover");

        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::ForgePr(_))),
            "forge provider should emit a ForgePr node"
        );
    }

    #[test]
    fn discover_local_with_skips_forge_when_runner_absent() {
        use crate::model::GraphNode;

        let temp = tempfile::TempDir::new().expect("temp dir");

        let snapshot =
            discover_local_with([temp.path()], LocalDiscoveryConfig::empty()).expect("discover");

        assert!(
            !snapshot
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::ForgePr(_))),
            "no forge nodes should appear when forge runner is not configured"
        );
    }

    #[test]
    fn discover_local_with_skips_tmux_when_runner_absent() {
        use crate::model::GraphNode;

        let temp = tempfile::TempDir::new().expect("temp dir");

        let snapshot =
            discover_local_with([temp.path()], LocalDiscoveryConfig::empty()).expect("discover");

        assert!(
            !snapshot
                .nodes
                .iter()
                .any(|node| matches!(node, GraphNode::MuxSession(_))),
            "no mux nodes should appear when tmux runner is not configured"
        );
    }

    #[test]
    fn discover_local_with_loads_declared_project_links_when_configured() {
        use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
        use crate::model::Provenance;

        let temp = tempfile::TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        std::fs::create_dir(&project).expect("project dir");
        std::fs::write(
            project.join(PROJECT_CONFIG_FILENAME),
            r#"
            [declared]
            schema_version = 1

            [[declared.links]]
            id = "declared-session-mux"
            relation = "linked_to_mux"
            state = "active"
            source = { type = "agent_session", harness_key = "codex", state_scope = "/state", session_key = "s1" }
            target = { type = "mux_session", native_id = "tmux:missing" }
            "#,
        )
        .expect("write project config");
        let config = LocalDiscoveryConfig::empty()
            .with_declared_config_loader(ConfigLoader::new().with_home(temp.path()));

        let snapshot = discover_local_with([project.as_path()], config).expect("discover");

        assert!(
            snapshot
                .candidate_links
                .iter()
                .any(|link| link.provenance == Provenance::LocalDeclared),
            "declared project config should contribute a local declared candidate"
        );
    }
}
