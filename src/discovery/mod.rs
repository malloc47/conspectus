//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use crate::config::ConfigLoader;
use crate::model::{Diagnostic, GraphLink, GraphNode, GraphSnapshot, NodeId, NodeProvenance};

pub mod agent_deck;
pub mod aliases;
pub mod atelier;
pub mod cache;
pub mod codex_log;
pub mod cross_link;
pub mod declared;
pub mod forge;
pub mod git;
pub mod harness;
pub mod hook_sidecar;
pub mod orchestrator;
pub mod pins;
pub mod providers;
pub mod tmux;
pub mod workspace;
pub mod zellij;

pub fn empty_graph() -> GraphSnapshot {
    GraphSnapshot::empty()
}

/// Unix epoch (seconds) captured from the wall clock. Used by each
/// discovery adapter to stamp the per-link `freshness_epoch` and
/// per-node `NodeProvenance.freshness_epoch` it emits (P7-002). The
/// implementation defaults to `0` when the clock is somehow before
/// the epoch — the schema treats that as the "unknown" sentinel.
pub fn current_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|delta| i64::try_from(delta.as_secs()).unwrap_or(0))
        .unwrap_or(0)
}

/// Fill in the `(provider, freshness_epoch)` defaults on a freshly
/// built fragment. Idempotent and first-write-wins:
///
/// * Links whose `source_metadata.adapter` is empty inherit
///   `provider`; links whose `freshness_epoch` is `None` inherit
///   `epoch`.
/// * Every node whose id is missing from `fragment.node_provenance`
///   gains a fresh entry with `(provider, Some(epoch))`.
///
/// Per-emit overrides (e.g. a harness adapter stamping a per-session
/// transcript mtime instead of the wall clock) survive intact —
/// the helper only fills in what nothing else set.
pub fn stamp_fragment(fragment: &mut GraphFragment, provider: &str, epoch: i64) {
    for link in &mut fragment.candidate_links {
        if link.source_metadata.adapter.is_empty() {
            link.source_metadata.adapter = provider.to_string();
        }
        if link.source_metadata.freshness_epoch.is_none() {
            link.source_metadata.freshness_epoch = Some(epoch);
        }
    }
    for node in &fragment.nodes {
        let id = node.id();
        fragment
            .node_provenance
            .entry(id)
            .or_insert_with(|| NodeProvenance {
                provider: provider.to_string(),
                freshness_epoch: Some(epoch),
            });
    }
}

/// Snapshot-flavored peer of [`stamp_fragment`]. Mutator passes
/// (cross-link inference, codex log attribution, hook sidecar
/// replay) edit the shared snapshot in place rather than returning a
/// fresh fragment; this helper lets them stamp any new
/// candidate-link or node they added without revisiting every
/// emit point. The same first-write-wins rule applies.
pub fn stamp_snapshot_mutations(snapshot: &mut GraphSnapshot, provider: &str, epoch: i64) {
    for link in &mut snapshot.candidate_links {
        if link.source_metadata.adapter.is_empty() {
            link.source_metadata.adapter = provider.to_string();
        }
        if link.source_metadata.freshness_epoch.is_none() {
            link.source_metadata.freshness_epoch = Some(epoch);
        }
    }
    for node in &snapshot.nodes {
        let id = node.id();
        snapshot
            .node_provenance
            .entry(id)
            .or_insert_with(|| NodeProvenance {
                provider: provider.to_string(),
                freshness_epoch: Some(epoch),
            });
    }
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
    /// Per-node producing-provider metadata (P7-002 / ADR 0037).
    /// Adapters populate this alongside `nodes`; `merge_fragments`
    /// folds the per-fragment map into [`GraphSnapshot::node_provenance`]
    /// at snapshot-assembly time. See [`crate::model::NodeProvenance`].
    pub node_provenance: BTreeMap<NodeId, NodeProvenance>,
}

impl GraphFragment {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn into_snapshot(self) -> GraphSnapshot {
        merge_fragments([self])
    }
}

/// H-HYG-001: consolidate 7 verbatim `snapshot_fragment`
/// helpers scattered across discovery adapter modules. Every
/// copy peeled the four struct fields off a `GraphSnapshot`;
/// this `From` impl makes the conversion callable via `.into()`
/// / `GraphFragment::from(snapshot)`.
impl From<GraphSnapshot> for GraphFragment {
    fn from(snapshot: GraphSnapshot) -> Self {
        GraphFragment {
            nodes: snapshot.nodes,
            candidate_links: snapshot.candidate_links,
            diagnostics: snapshot.diagnostics,
            node_provenance: snapshot.node_provenance,
        }
    }
}

/// H-HYG-001: consolidate 5 verbatim `path_string` helpers
/// scattered across discovery adapters + dev_scenarios. Every
/// copy did `path.to_string_lossy().to_string()`; this shared
/// helper is the single canonical version.
pub fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub trait DiscoveryProvider {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;
}

impl DiscoveryProvider for Box<dyn DiscoveryProvider> {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        (**self).discover(context)
    }
}

#[derive(Default)]
pub struct LocalDiscovery {
    providers: Vec<KeyedProvider>,
}

struct KeyedProvider {
    /// Provider keys this entry emits. Empty means "unkeyed" — the
    /// entry always runs and is never skippable. The warm-start
    /// path's [`discover_skipping`](LocalDiscovery::discover_skipping)
    /// drops the entry iff *every* key is in the skip set, so a
    /// bundled provider (e.g. [`harness::HarnessDiscovery`] emitting
    /// `claude-code`/`codex`/`opencode`/`aider`) survives unless
    /// every one of its harnesses is fresh in the prior snapshot.
    keys: Vec<&'static str>,
    inner: Box<dyn DiscoveryProvider>,
}

impl LocalDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, provider: impl DiscoveryProvider + 'static) -> Self {
        self.providers.push(KeyedProvider {
            keys: Vec::new(),
            inner: Box::new(provider),
        });
        self
    }

    /// Tag this provider with the granular per-emit provider keys
    /// it stamps onto its outputs (see ADR 0079). The warm-start
    /// path uses these to decide whether the prior cache covers
    /// the entry's slice. Pass an empty slice (or use
    /// [`with_provider`]) to opt out of TTL gating.
    pub fn with_keyed_provider(
        mut self,
        keys: &[&'static str],
        provider: impl DiscoveryProvider + 'static,
    ) -> Self {
        self.providers.push(KeyedProvider {
            keys: keys.to_vec(),
            inner: Box::new(provider),
        });
        self
    }

    pub fn discover(&self, context: &DiscoveryContext) -> Result<GraphSnapshot> {
        self.discover_skipping(context, &BTreeSet::new())
    }

    /// Run every provider whose key set is *not* fully covered by
    /// `skip`. Unkeyed providers (empty key set) always run. The
    /// returned snapshot has the same shape as
    /// [`Self::discover`]'s output; downstream merge with a prior
    /// cache supplies the slices for skipped providers.
    pub fn discover_skipping(
        &self,
        context: &DiscoveryContext,
        skip: &BTreeSet<String>,
    ) -> Result<GraphSnapshot> {
        let mut fragments = Vec::with_capacity(self.providers.len());
        for keyed in &self.providers {
            if !keyed.keys.is_empty() && keyed.keys.iter().all(|k| skip.contains(*k)) {
                continue;
            }
            fragments.push(keyed.inner.discover(context)?);
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
    discover_local_warm_with(roots, config, GraphSnapshot::empty(), &Default::default())
}

/// Warm-start discovery driver (P7-003 phase 3). The thin
/// wrapper [`discover_local_with`] passes an empty `prior` and
/// default intervals, which collapses the freshness gate to "no
/// providers are fresh" and runs every adapter cold.
///
/// With a non-empty `prior` and real `intervals`, the warm-start
/// path:
///
/// 1. Asks [`cache::compute_freshness_gate`] which provider keys
///    are fresh (skip running), stale (evict from prior + re-run),
///    or always-evict (mutators + unmapped keys).
/// 2. Evicts every stale + always-evict slice from `prior` via
///    [`GraphSnapshot::evict_provider`] so the prior backstop
///    contributes only the fresh slices.
/// 3. Runs the heavy-provider chain with
///    [`LocalDiscovery::discover_skipping`], passing the fresh
///    set as the skip filter.
/// 4. Folds the observed-cwd git probe in, merges with the
///    evicted prior via [`merge_with_prior`] (fresh wins on every
///    collision), and re-runs every mutator pass against the
///    merged snapshot.
///
/// Mutators always re-run because their outputs are derived from
/// whatever the heavy providers produced this invocation. The
/// always-evict bucket pulls their cached slices so deleted
/// upstream state cannot linger.
pub fn discover_local_warm_with(
    roots: impl IntoIterator<Item = impl Into<PathBuf>>,
    config: LocalDiscoveryConfig,
    prior: GraphSnapshot,
    intervals: &crate::config::ServerIntervals,
) -> Result<GraphSnapshot> {
    let mut context = DiscoveryContext::from_roots(roots)?;

    for (key, root) in &config.harness_state_roots {
        context = context.with_harness_state_root(key.clone(), root.clone());
    }

    let now = current_epoch();
    let gate = cache::compute_freshness_gate(&prior, intervals, now);
    let mut prior = prior;
    for key in gate.keys_to_evict() {
        prior.evict_provider(&key);
    }

    // The tmux + forge runners are owned trait objects we move
    // into the per-provider constructors. Take them out of the
    // config now so `apply_mutators` below can still borrow the
    // remaining fields without a partial-move issue.
    let mut config = config;
    // H-EXT-008 / H-EXT-010: mux backends live in a registry
    // list; each is pulled by key and wrapped in its
    // per-backend `DiscoveryProvider`.
    let tmux_runner = config.take_mux_backend_by_key(tmux::TMUX_BACKEND);
    let zellij_runner = config.take_mux_backend_by_key(zellij::ZELLIJ_BACKEND);
    // H-EXT-012: forge adapters live in a registry list; drain
    // them here so they can be wrapped in a `ForgeDiscovery`
    // coordinator that fans a repo out to every registered
    // adapter. The adapter registry keeps the pre-H-EXT-012
    // "GitHub-only" single-slot behavior intact for consumers
    // that ship one adapter; multi-forge hosts extend it via
    // `LocalDiscoveryConfig::with_forge_adapter`.
    let forge_adapters: Vec<Box<dyn forge::ForgeAdapter>> =
        std::mem::take(&mut config.forge_adapters);

    let mut providers = LocalDiscovery::new()
        .with_keyed_provider(&["git"], git::GitDiscovery::new())
        .with_keyed_provider(&["atelier"], atelier::AtelierWorkspaceDiscovery::new())
        .with_keyed_provider(
            &["generic_workspace"],
            workspace::GenericWorkspaceDiscovery::new(),
        )
        .with_keyed_provider(
            &["claude-code", "codex", "opencode", "aider"],
            harness::HarnessDiscovery::with_default_adapters(),
        );

    // H-EXT-014: iterate the orchestrator registry and build a
    // provider per configured root. The registry names each
    // descriptor's builder, so a new orchestrator (dmux /
    // herdr / pertmux / workmux, gated on `H-AGENTMUX-*`
    // evidence audits) becomes a `REGISTRY` entry + a
    // per-orchestrator `[orchestrators.<key>].root = "..."` in
    // config (config-table parsing lands in a follow-up).
    for descriptor in orchestrator::REGISTRY {
        if let Some(root) = config.orchestrator_roots.get(descriptor.key) {
            let provider = (descriptor.build)(root.clone());
            providers = providers.with_keyed_provider(&[descriptor.key], provider);
        }
    }

    if let Some(runner) = tmux_runner {
        providers =
            providers.with_keyed_provider(&["tmux"], tmux::TmuxDiscovery::with_runner(runner));
    }

    if let Some(runner) = zellij_runner {
        providers = providers
            .with_keyed_provider(&["zellij"], zellij::ZellijDiscovery::with_runner(runner));
    }

    if !forge_adapters.is_empty() {
        // H-EXT-012: fan every registered adapter through the
        // ForgeDiscovery coordinator. The coordinator already
        // knew how to merge multiple adapters; the registry
        // list finally has more than one entry (or the room
        // for one).
        let mut coordinator = forge::ForgeDiscovery::new();
        for adapter in forge_adapters {
            coordinator = coordinator.with_boxed_adapter(adapter);
        }
        // Provider key list matches the pre-H-EXT-012 single
        // `"github"` string until a second forge lands.
        providers = providers.with_keyed_provider(&["github"], coordinator);
    }

    let mut fresh = providers.discover_skipping(&context, &gate.fresh)?;
    let cwd_git_fragment = observed_cwd_git_fragment(&fresh);
    fresh = merge_fragments([GraphFragment::from(fresh), cwd_git_fragment]);

    // Phase-2 backstop merge with the evicted prior. The fresh
    // fragment wins on every collision; the prior fills in
    // slices the live run did not emit (the providers we
    // skipped because they were fresh).
    let mut snapshot = merge_with_prior(fresh, prior);

    apply_mutators(&mut snapshot, &config, &context);
    Ok(snapshot)
}

/// Always-rerun mutator block extracted from
/// [`discover_local_warm_with`]. Runs against the merged
/// (fresh + prior backstop) snapshot so cross-references,
/// codex-log attribution, hook sidecar replay, and the declared
/// link/alias/pin overlays land against the canonical merged
/// state regardless of which heavy providers were skipped.
///
/// The corresponding slices in `prior` are pre-evicted by
/// [`discover_local_warm_with`]'s gate, so each mutator stamps
/// its fresh outputs into a clean slot via first-write-wins.
fn apply_mutators(
    snapshot: &mut GraphSnapshot,
    config: &LocalDiscoveryConfig,
    context: &DiscoveryContext,
) {
    let harness_pids_per_mux = if config.process_tree_enabled {
        cross_link::infer(snapshot);
        cross_link::active_harness_pids_per_mux(snapshot, &cross_link::LinuxProcSnapshot)
    } else {
        cross_link::infer_without_process_tree(snapshot);
        std::collections::BTreeMap::new()
    };
    // H-EXT-007: iterate registered adapters and let each one
    // apply its aux-attribution pass (opt-in via trait override
    // + state root configured + not in disabled_aux_harnesses).
    // The pre-H-EXT-007 hardcoded codex-log branch now lives on
    // `CodexAdapter::apply_aux_attribution`.
    let now_epoch = codex_log::current_epoch();
    for adapter in harness::registered_adapters() {
        if config
            .disabled_aux_harnesses
            .contains(adapter.harness_key())
        {
            continue;
        }
        let Some(state_root) = config.harness_state_roots.get(adapter.harness_key()) else {
            continue;
        };
        let ctx = harness::AuxAttributionContext {
            state_root,
            harness_pids_per_mux: &harness_pids_per_mux,
            now_epoch,
        };
        adapter.apply_aux_attribution(snapshot, &ctx);
    }
    if let Some(root) = &config.hook_sidecar_root {
        hook_sidecar::apply_hook_sidecars(snapshot, root, hook_sidecar::current_epoch());
    }
    if let Some(loader) = &config.declared_config_loader {
        declared::apply_declared_links(snapshot, context, loader);
        aliases::apply_aliases(snapshot, context, loader);
        pins::apply_pins(snapshot, context, loader);
    }
}

/// Configuration that controls which providers run during local discovery.
pub struct LocalDiscoveryConfig {
    pub harness_state_roots: BTreeMap<String, PathBuf>,
    /// Registered mux backends (H-EXT-008, ADR 0089). Adding a
    /// second backend (zellij per H-EXT-010, screen etc.) is a
    /// matter of pushing another entry. Each backend implements
    /// [`tmux::MuxBackend`] and returns its own `backend_key`.
    /// v1 ships with a single tmux entry populated by
    /// [`Self::from_env`]; multi-backend hosts push additional
    /// entries via [`Self::with_mux_backend`].
    pub mux_backends: Vec<Box<dyn tmux::MuxBackend>>,
    /// Registered forge adapters (H-EXT-012). Adding a second
    /// adapter (GitLab per H-EXT-013, Gitea, hosted GitHub
    /// Enterprise) is a matter of pushing another entry. Each
    /// adapter implements [`forge::ForgeAdapter`] and reports
    /// which remote URLs it claims via
    /// [`forge::ForgeAdapter::claims_remote_url`]. v1 ships with
    /// a single GitHub entry populated by
    /// [`Self::from_env`]; multi-forge hosts push additional
    /// entries via [`Self::with_forge_adapter`].
    pub forge_adapters: Vec<Box<dyn forge::ForgeAdapter>>,
    pub process_tree_enabled: bool,
    pub hook_sidecar_root: Option<PathBuf>,
    /// Registered orchestrator adapter roots (H-EXT-014).
    /// Keyed by orchestrator descriptor key
    /// (`agent_deck` today; dmux / herdr / pertmux / workmux
    /// slot in via `orchestrator::REGISTRY` follow-ups).
    /// `from_env` populates each entry from the descriptor's
    /// `env_root_var` (falling back to `home_relative_default`)
    /// unless the descriptor's `env_disable_var` is set.
    /// Callers add or remove entries via
    /// [`Self::with_orchestrator_root`] /
    /// [`Self::without_orchestrator`].
    pub orchestrator_roots: BTreeMap<String, PathBuf>,
    pub declared_config_loader: Option<ConfigLoader>,
    /// Harness keys whose optional aux-attribution mutator pass
    /// (H-EXT-007) should be skipped, even when the harness has a
    /// state root configured. Populated by
    /// [`Self::from_env`] from `CONSPECTUS_DISABLE_<KEY>_LOG` /
    /// `CONSPECTUS_DISABLE_CODEX_LOG` (the pre-H-EXT-007 codex-log
    /// disable env var, kept as-is for wire compatibility) and by
    /// callers via [`Self::without_aux_harness`].
    pub disabled_aux_harnesses: BTreeSet<String>,
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

        let mut mux_backends: Vec<Box<dyn tmux::MuxBackend>> = Vec::new();
        if env::var_os("CONSPECTUS_DISABLE_TMUX").is_none() {
            mux_backends.push(Box::new(tmux::SystemTmux::new()));
        }
        // H-EXT-010: zellij backend, opt-out via
        // `CONSPECTUS_DISABLE_ZELLIJ`. Registered unconditionally
        // by default; missing `zellij` binary surfaces as
        // `TmuxOutcome::Unavailable(BinaryNotFound)` and the
        // discovery layer degrades to an empty session set.
        if env::var_os("CONSPECTUS_DISABLE_ZELLIJ").is_none() {
            mux_backends.push(Box::new(zellij::SystemZellij::new()));
        }

        // H-EXT-012: forge adapters live in a registry list.
        // GitHub is the default entry; a second forge (GitLab
        // per H-EXT-013) is opt-in via `CONSPECTUS_ENABLE_GITLAB`
        // because the skeleton adapter doesn't yet emit real
        // PRs (H-DESIGN-002 blocks real gitlab discovery).
        // `CONSPECTUS_DISABLE_FORGE` still zeroes the list for
        // wire compatibility.
        let mut forge_adapters: Vec<Box<dyn forge::ForgeAdapter>> = Vec::new();
        if env::var_os("CONSPECTUS_DISABLE_FORGE").is_none() {
            forge_adapters.push(Box::new(forge::github::GitHubForgeProvider::with_runner(
                forge::SystemGh::new(),
            )));
            if env::var_os("CONSPECTUS_ENABLE_GITLAB").is_some() {
                forge_adapters.push(Box::new(forge::gitlab::GitLabForgeProvider::new()));
            }
        }

        // H-EXT-007: codex_log-specific `codex_log_window_seconds`
        // env parsing moves into `CodexAdapter::apply_aux_attribution`.
        // The pre-H-EXT-007 disable flag (`CONSPECTUS_DISABLE_CODEX_LOG`)
        // stays as a general "disable this harness's aux
        // attribution" knob via the `disabled_aux_harnesses` set,
        // preserving wire compatibility with operator env configs.
        let mut disabled_aux_harnesses: BTreeSet<String> = BTreeSet::new();
        if env::var_os("CONSPECTUS_DISABLE_CODEX_LOG").is_some() {
            disabled_aux_harnesses.insert(harness::codex::HARNESS_KEY.to_string());
        }

        // H-EXT-014: walk the orchestrator registry and
        // materialize each descriptor's default root. The
        // agent_deck env-var contract
        // (`CONSPECTUS_AGENT_DECK_ROOT` /
        // `CONSPECTUS_DISABLE_AGENT_DECK`) stays wire-compatible
        // because it's now driven by the descriptor entry.
        let mut orchestrator_roots: BTreeMap<String, PathBuf> = BTreeMap::new();
        for descriptor in orchestrator::REGISTRY {
            if let Some(root) = descriptor.resolve_default_root() {
                orchestrator_roots.insert(descriptor.key.to_string(), root);
            }
        }

        Self {
            harness_state_roots,
            mux_backends,
            forge_adapters,
            process_tree_enabled: env::var_os("CONSPECTUS_DISABLE_PROCTREE").is_none(),
            hook_sidecar_root: hook_sidecar::default_sidecar_root(),
            orchestrator_roots,
            declared_config_loader: Some(ConfigLoader::from_env()),
            disabled_aux_harnesses,
        }
    }

    pub fn empty() -> Self {
        Self {
            harness_state_roots: BTreeMap::new(),
            mux_backends: Vec::new(),
            forge_adapters: Vec::new(),
            process_tree_enabled: false,
            hook_sidecar_root: None,
            orchestrator_roots: BTreeMap::new(),
            declared_config_loader: None,
            disabled_aux_harnesses: BTreeSet::new(),
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

    /// Push a mux backend onto the registry (H-EXT-008, ADR 0089).
    /// Backends can be added in any order; discovery iterates them
    /// in registration order and picks the first one whose
    /// [`tmux::MuxBackend::backend_key`] matches a pin or session's
    /// `backend` field.
    pub fn with_mux_backend(mut self, backend: impl tmux::MuxBackend + 'static) -> Self {
        self.mux_backends.push(Box::new(backend));
        self
    }

    /// Deprecated alias for [`Self::with_mux_backend`] (H-EXT-008).
    /// Kept so pre-H-EXT-008 test call sites and scenario builders
    /// keep compiling without a mass rename in this commit.
    pub fn with_tmux_runner(self, runner: impl tmux::MuxBackend + 'static) -> Self {
        self.with_mux_backend(runner)
    }

    /// Clear every backend whose `backend_key()` matches the tmux
    /// backend string. Retained under the historical name because
    /// tests use it as `.without_tmux()` (H-EXT-008 rewired it
    /// against the backend list; the disable env var
    /// `CONSPECTUS_DISABLE_TMUX` still produces the same result
    /// through `from_env`).
    pub fn without_tmux(mut self) -> Self {
        self.mux_backends
            .retain(|b| b.backend_key() != tmux::TMUX_BACKEND);
        self
    }

    /// Find the first registered backend whose `backend_key()`
    /// matches `key` (H-EXT-008). Used by CLI + TUI dispatch to
    /// resolve a pin's / mux session's `backend` field to a
    /// concrete runner.
    pub fn mux_backend_by_key(&self, key: &str) -> Option<&dyn tmux::MuxBackend> {
        self.mux_backends
            .iter()
            .find(|b| b.backend_key() == key)
            .map(|b| b.as_ref())
    }

    /// Consume and return the first registered backend whose
    /// `backend_key()` matches `key` (H-EXT-008). Used by
    /// `discover_local_warm_with` when routing a backend into a
    /// `TmuxDiscovery` wrapper that owns it.
    pub fn take_mux_backend_by_key(&mut self, key: &str) -> Option<Box<dyn tmux::MuxBackend>> {
        let idx = self
            .mux_backends
            .iter()
            .position(|b| b.backend_key() == key)?;
        Some(self.mux_backends.remove(idx))
    }

    /// Push a forge adapter onto the registry (H-EXT-012).
    /// Adapters are iterated in registration order; each one
    /// receives every repo whose `origin` remote it claims via
    /// [`forge::ForgeAdapter::claims_remote_url`].
    pub fn with_forge_adapter(mut self, adapter: impl forge::ForgeAdapter + 'static) -> Self {
        self.forge_adapters.push(Box::new(adapter));
        self
    }

    /// Deprecated alias for [`Self::with_forge_adapter`] (H-EXT-012).
    /// Kept so pre-H-EXT-012 test call sites (`.with_forge_runner(
    /// FakeGh::with_pull_requests(...))`) compile without a mass
    /// rename. The runner gets wrapped in a
    /// [`forge::github::GitHubForgeProvider`] before it's pushed
    /// so the adapter list stays uniform.
    pub fn with_forge_runner(mut self, runner: impl forge::GhRunner + 'static) -> Self {
        self.forge_adapters
            .push(Box::new(forge::github::GitHubForgeProvider::with_runner(
                runner,
            )));
        self
    }

    /// Clear every registered forge adapter. Matches the historical
    /// `.without_forge()` semantic (H-EXT-012 rewired it against
    /// the adapter list; `CONSPECTUS_DISABLE_FORGE` still zeroes
    /// the list through `from_env`).
    pub fn without_forge(mut self) -> Self {
        self.forge_adapters.clear();
        self
    }

    pub fn with_process_tree(mut self) -> Self {
        self.process_tree_enabled = true;
        self
    }

    /// Skip a registered harness's aux-attribution mutator pass
    /// (H-EXT-007). Adds the key to
    /// [`Self::disabled_aux_harnesses`]; the caller doesn't need
    /// to know whether the harness actually has an aux surface
    /// (a `None` `apply_aux_attribution` override + a disable
    /// flag are both no-ops at run time).
    pub fn without_aux_harness(mut self, harness_key: impl Into<String>) -> Self {
        self.disabled_aux_harnesses.insert(harness_key.into());
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

    /// Register an orchestrator's on-disk root (H-EXT-014).
    /// `key` is the descriptor key
    /// (`orchestrator::OrchestratorDescriptor::key`); `root` is
    /// the resolved filesystem root the adapter's
    /// `DiscoveryProvider` reads from.
    pub fn with_orchestrator_root(
        mut self,
        key: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Self {
        self.orchestrator_roots.insert(key.into(), root.into());
        self
    }

    /// Unregister an orchestrator by key (H-EXT-014). No-op
    /// when the key isn't currently in the map.
    pub fn without_orchestrator(mut self, key: &str) -> Self {
        self.orchestrator_roots.remove(key);
        self
    }

    /// Deprecated alias for
    /// [`Self::with_orchestrator_root`]`("agent_deck", root)`
    /// (H-EXT-014). Kept so pre-H-EXT-014 test call sites
    /// (`.with_agent_deck_root(...)`) keep compiling.
    pub fn with_agent_deck_root(self, root: impl Into<PathBuf>) -> Self {
        self.with_orchestrator_root("agent_deck", root)
    }

    /// Deprecated alias for
    /// [`Self::without_orchestrator`]`("agent_deck")`.
    pub fn without_agent_deck(self) -> Self {
        self.without_orchestrator("agent_deck")
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

// `default_agent_deck_root` retired in H-EXT-014; the same
// env-var contract now lives in
// `orchestrator::REGISTRY[..].resolve_default_root()`.

pub fn merge_fragments(fragments: impl IntoIterator<Item = GraphFragment>) -> GraphSnapshot {
    let mut nodes = BTreeMap::new();
    let mut candidate_links = BTreeMap::new();
    let mut diagnostics = Vec::new();
    // First-write-wins on node provenance, matching the first-write-
    // wins semantics already in place for `nodes` above. The
    // fragment that contributed the node also owns the canonical
    // provenance entry; later fragments don't override that even if
    // a downstream mutator (cross_link, hook_sidecar, …) revisits
    // the node id.
    let mut node_provenance: BTreeMap<NodeId, NodeProvenance> = BTreeMap::new();

    for fragment in fragments {
        for node in fragment.nodes {
            nodes.entry(node.id()).or_insert(node);
        }

        for link in fragment.candidate_links {
            candidate_links.entry(link.id.clone()).or_insert(link);
        }

        for (id, prov) in fragment.node_provenance {
            node_provenance.entry(id).or_insert(prov);
        }

        diagnostics.extend(fragment.diagnostics);
    }

    let mut snapshot = GraphSnapshot {
        nodes: nodes.into_values().collect(),
        candidate_links: candidate_links.into_values().collect(),
        resolved_relationships: Vec::new(),
        diagnostics,
        aliases: crate::aliases::AliasOverlay::new(),
        pins: Vec::new(),
        node_provenance,
    };
    snapshot.canonicalize();
    snapshot
}

/// Warm-start backstop merge (P7-003 phase 2). Folds `prior` into
/// `fresh` so live discovery results override the persisted cache
/// wherever they collide, and the cache only contributes nodes,
/// candidate-links, and per-node provenance the fresh run did not
/// emit (e.g. things discovered against a previous cwd).
///
/// Implemented on top of [`merge_fragments`] with the fresh
/// fragment listed first — the first-write-wins rule already in
/// place for unioned providers gives us exactly the override
/// semantics the warm-start path wants. `resolved_relationships`,
/// `pins`, and `aliases` from the prior snapshot are intentionally
/// dropped: the resolver re-runs on the merged candidate set, and
/// pins / aliases reload from their on-disk configs alongside the
/// fresh discovery pass.
///
/// Phase 3 will graduate this from "always merge everything" to
/// per-provider TTL comparison + selective re-run via the P7-005
/// eviction primitive.
pub fn merge_with_prior(fresh: GraphSnapshot, prior: GraphSnapshot) -> GraphSnapshot {
    // `merge_fragments` already drops `resolved_relationships`,
    // `pins`, and `aliases` on the returned snapshot; the CLI
    // re-resolves and reloads pins/aliases from the live config
    // loader after this helper returns. Keeping that off the
    // warm-start path avoids round-tripping stale derived state.
    merge_fragments([GraphFragment::from(fresh), GraphFragment::from(prior)])
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

    let mut fragment = GraphFragment::from(merge_fragments(fragments));
    fragment.diagnostics.extend(diagnostics);
    // Tag observed-cwd-derived nodes/links as a distinct provider so
    // partial eviction (P7-005) can refresh them without touching the
    // primary `git` slice. First-write-wins on the per-node sidecar
    // keeps the canonical `git` provenance for nodes that surfaced
    // through both paths.
    stamp_fragment(&mut fragment, providers::GIT_CWD, current_epoch());
    fragment
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
            session,
            LinkEndpoint::Node { id: mux },
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
                node_provenance: BTreeMap::new(),
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
                node_provenance: BTreeMap::new(),
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
                node_provenance: BTreeMap::new(),
            },
            GraphFragment {
                nodes: vec![node],
                candidate_links: vec![link.clone()],
                diagnostics: Vec::new(),
                node_provenance: BTreeMap::new(),
            },
        ]);

        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.candidate_links, vec![link]);
    }

    #[test]
    fn merge_fragments_folds_node_provenance_first_write_wins() {
        // Two fragments contribute the same node id with different
        // provenance entries. First-write-wins, matching the
        // dedup-on-node-id semantics one block above. The
        // unique-to-fragment-B node carries its provider through.
        let mux_id = MuxSessionId::new("tmux:s1");
        let mux_node = GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
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
        let agent_node = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "alpha"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        });
        let mux_node_id = NodeId::MuxSession(mux_id);
        let agent_node_id = agent_node.id();

        let mut prov_a = BTreeMap::new();
        prov_a.insert(
            mux_node_id.clone(),
            NodeProvenance {
                provider: "tmux".to_string(),
                freshness_epoch: Some(1_700_000_100),
            },
        );
        let mut prov_b = BTreeMap::new();
        // B contributes a competing entry for the mux node — it
        // should lose to A's earlier write — plus a fresh entry for
        // the agent node that A did not touch.
        prov_b.insert(
            mux_node_id.clone(),
            NodeProvenance {
                provider: "cross_link".to_string(),
                freshness_epoch: Some(1_700_000_999),
            },
        );
        prov_b.insert(
            agent_node_id.clone(),
            NodeProvenance {
                provider: "harness::codex".to_string(),
                freshness_epoch: Some(1_700_000_200),
            },
        );

        let snapshot = merge_fragments([
            GraphFragment {
                nodes: vec![mux_node.clone()],
                candidate_links: Vec::new(),
                diagnostics: Vec::new(),
                node_provenance: prov_a,
            },
            GraphFragment {
                nodes: vec![mux_node, agent_node],
                candidate_links: Vec::new(),
                diagnostics: Vec::new(),
                node_provenance: prov_b,
            },
        ]);

        assert_eq!(snapshot.node_provenance.len(), 2);
        assert_eq!(
            snapshot.node_provenance.get(&mux_node_id).unwrap().provider,
            "tmux",
            "mux node provenance should come from the first fragment, not the second"
        );
        assert_eq!(
            snapshot
                .node_provenance
                .get(&agent_node_id)
                .unwrap()
                .provider,
            "harness::codex"
        );
    }

    #[test]
    fn merge_with_prior_lets_fresh_win_and_keeps_prior_only_nodes() {
        // Backstop merge semantics: collisions resolve to `fresh`,
        // prior-only nodes survive as a stale-but-better-than-empty
        // fallback. The persisted cache continues to surface state
        // the live scan did not see (e.g. a repo from yesterday's
        // cwd) without overriding anything the live scan refreshed.
        let mux_id = MuxSessionId::new("tmux:keep");
        let fresh_mux = GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            backend: "tmux".to_string(),
            native_id: "keep".to_string(),
            cwd: Some("/fresh/cwd".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        });
        let prior_mux = GraphNode::MuxSession(MuxSessionNode {
            id: mux_id.clone(),
            backend: "tmux".to_string(),
            native_id: "keep".to_string(),
            cwd: Some("/stale/cwd".to_string()),
            active_pane_command: None,
            active_pane_pid: None,
            active_pane_current_path: None,
            active_pane_start_command: None,
            client_attached: None,
            activity_epoch: None,
            created_epoch: None,
        });
        let prior_only = GraphNode::AgentSession(AgentSessionNode {
            id: AgentSessionId::new("codex", "/state", "from-cache"),
            harness_key: "codex".to_string(),
            cwd: None,
            title: None,
            last_message_preview: None,
            last_active_epoch: None,
            session_kind: None,
        });
        let prior_only_id = prior_only.id();

        let mut fresh = GraphSnapshot::empty();
        fresh.nodes.push(fresh_mux);
        fresh.node_provenance.insert(
            NodeId::MuxSession(mux_id.clone()),
            NodeProvenance {
                provider: "tmux".to_string(),
                freshness_epoch: Some(1_700_000_900),
            },
        );

        let mut prior = GraphSnapshot::empty();
        prior.nodes.push(prior_mux);
        prior.nodes.push(prior_only);
        prior.node_provenance.insert(
            NodeId::MuxSession(mux_id.clone()),
            NodeProvenance {
                provider: "tmux".to_string(),
                freshness_epoch: Some(1_700_000_100),
            },
        );
        prior.node_provenance.insert(
            prior_only_id.clone(),
            NodeProvenance {
                provider: "harness::codex".to_string(),
                freshness_epoch: Some(1_700_000_050),
            },
        );

        let merged = merge_with_prior(fresh, prior);

        // The fresh mux's cwd survives the merge — prior loses on
        // the collision.
        let merged_mux = merged
            .nodes
            .iter()
            .find_map(|node| match node {
                GraphNode::MuxSession(s) if s.id == mux_id => Some(s),
                _ => None,
            })
            .expect("mux node present in merged snapshot");
        assert_eq!(merged_mux.cwd.as_deref(), Some("/fresh/cwd"));

        // The prior-only agent session still appears.
        assert!(
            merged.nodes.iter().any(|node| node.id() == prior_only_id),
            "prior-only node should survive the backstop merge"
        );

        // Provenance for the collision uses fresh's epoch.
        let mux_prov = merged
            .node_provenance
            .get(&NodeId::MuxSession(mux_id.clone()))
            .expect("mux provenance");
        assert_eq!(mux_prov.freshness_epoch, Some(1_700_000_900));

        // Resolver re-runs on the merged snapshot — warm-start
        // never carries forward resolved relationships.
        assert!(merged.resolved_relationships.is_empty());
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
    fn discover_local_warm_with_keeps_fresh_provider_slice_from_prior() {
        // Stage a prior snapshot with a `github` slice timestamped
        // *just now* (well within the 5-minute forge TTL). The
        // forge runner is *not* installed in the config, so a cold
        // rebuild would have no github nodes; the warm-start path
        // should observe github is fresh, skip running it (vacuous
        // here), and let the prior slice flow through the
        // backstop merge so the result still carries it.
        //
        // Uses a `RepoNode` as a stand-in payload because the
        // freshness gate runs on `node_provenance.provider`, not
        // the node type; the warm-start contract only cares about
        // the provider key, not which node variant carries it.
        use crate::config::ServerIntervals;
        use crate::model::RepoNode;

        let temp = tempfile::TempDir::new().expect("temp dir");
        let mut prior = GraphSnapshot::empty();
        let repo_id = crate::model::RepoId::new("/fresh-github/.git");
        let node_id = NodeId::Repo(repo_id.clone());
        prior.nodes.push(GraphNode::Repo(RepoNode::new(repo_id)));
        prior.node_provenance.insert(
            node_id.clone(),
            NodeProvenance {
                provider: "github".to_string(),
                freshness_epoch: Some(current_epoch()),
            },
        );

        let snapshot = discover_local_warm_with(
            [temp.path()],
            LocalDiscoveryConfig::empty(),
            prior,
            &ServerIntervals::default(),
        )
        .expect("warm-start discovery");

        assert!(
            snapshot.nodes.iter().any(|node| node.id() == node_id),
            "fresh github slice should survive the warm-start merge"
        );
    }

    #[test]
    fn discover_local_warm_with_evicts_stale_slice_and_re_runs_cold() {
        // Same setup but the prior github slice is stamped at
        // epoch 0 — well past the 5-minute TTL relative to
        // wall-clock `now`. The gate marks it stale and the
        // warm-start path evicts it from the prior; since no forge
        // runner is wired up, the live run emits no github node
        // either, so the result has zero github nodes (correctly
        // reflecting deleted upstream state).
        use crate::config::ServerIntervals;
        use crate::model::RepoNode;

        let temp = tempfile::TempDir::new().expect("temp dir");
        let mut prior = GraphSnapshot::empty();
        let repo_id = crate::model::RepoId::new("/stale-github/.git");
        let node_id = NodeId::Repo(repo_id.clone());
        prior.nodes.push(GraphNode::Repo(RepoNode::new(repo_id)));
        prior.node_provenance.insert(
            node_id.clone(),
            NodeProvenance {
                provider: "github".to_string(),
                freshness_epoch: Some(0),
            },
        );

        let snapshot = discover_local_warm_with(
            [temp.path()],
            LocalDiscoveryConfig::empty(),
            prior,
            &ServerIntervals::default(),
        )
        .expect("warm-start discovery");

        assert!(
            !snapshot.nodes.iter().any(|node| node.id() == node_id),
            "stale github slice should be evicted; no forge runner means nothing replaces it"
        );
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

    #[test]
    fn discover_local_with_loads_project_pins_from_observed_session_cwd() {
        use crate::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
        use crate::discovery::harness::codex::HARNESS_KEY as CODEX_KEY;
        use crate::discovery::harness::fixtures::{CodexSessionRecord, HarnessFixture};

        let temp = tempfile::TempDir::new().expect("temp dir");
        let scan_root = temp.path().join("scan");
        let project = temp.path().join("project");
        std::fs::create_dir(&scan_root).expect("scan dir");
        std::fs::create_dir(&project).expect("project dir");
        std::fs::write(
            project.join(PROJECT_CONFIG_FILENAME),
            format!(
                r#"
                [pins]
                schema_version = 1

                [[pins.entries]]
                id = "observed"
                display_name = "observed"
                harness = "codex"
                cwd = "{}"
                mux = {{ backend = "tmux", name = "observed" }}
                "#,
                project.display()
            ),
        )
        .expect("write project config");

        let fixture = HarnessFixture::at(temp.path().join("state"));
        fixture
            .write_codex_session(
                &CodexSessionRecord::new("session-observed")
                    .with_cwd(project.to_string_lossy().into_owned()),
            )
            .expect("write codex session");

        let config = LocalDiscoveryConfig::empty()
            .with_harness_state_root(CODEX_KEY, fixture.codex_state_root())
            .with_declared_config_loader(ConfigLoader::new().with_home(temp.path()));

        let snapshot = discover_local_with([scan_root.as_path()], config).expect("discover");

        assert!(
            snapshot
                .pins
                .iter()
                .any(|pin| pin.id == "observed" && pin.provenance == Provenance::LocalPin),
            "project pin should load from the observed session cwd, not only the scan root"
        );
    }
}
