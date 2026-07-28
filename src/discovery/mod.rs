//! Discovery adapter boundaries.
//!
//! This module will hold providers for git, agent harnesses, tmux, forge, and
//! workspace metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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
    // H-SERVE-PERF-001a (ADR 0091): defer eviction of the process-tree
    // mutator slices. Whether they re-run depends on whether the mux
    // or harness heavy providers actually ran this cycle, which we
    // only know after discovery below. On a git/forge-only cycle we
    // keep the prior agent↔pane links instead of paying a fresh
    // `/proc` walk. Every other stale/always-evict key evicts now.
    let deferred: std::collections::BTreeSet<&str> =
        cache::PROCESS_TREE_MUTATORS.iter().copied().collect();
    for key in gate.keys_to_evict() {
        if deferred.contains(key.as_str()) {
            continue;
        }
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
        .with_keyed_provider(&[providers::GIT], git::GitDiscovery::new())
        .with_keyed_provider(
            &[providers::ATELIER],
            atelier::AtelierWorkspaceDiscovery::new(),
        )
        .with_keyed_provider(
            &[providers::GENERIC_WORKSPACE],
            workspace::GenericWorkspaceDiscovery::new(),
        )
        .with_keyed_provider(
            &[
                providers::CLAUDE_CODE,
                providers::CODEX,
                providers::OPENCODE,
                providers::AIDER,
            ],
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
        providers = providers
            .with_keyed_provider(&[providers::TMUX], tmux::TmuxDiscovery::with_runner(runner));
    }

    if let Some(runner) = zellij_runner {
        providers = providers.with_keyed_provider(
            &[providers::ZELLIJ],
            zellij::ZellijDiscovery::with_runner(runner),
        );
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
        providers = providers.with_keyed_provider(&[providers::GITHUB], coordinator);
    }

    let mut fresh = providers.discover_skipping(&context, &gate.fresh)?;
    let cwd_git_fragment = observed_cwd_git_fragment(&fresh);
    fresh = merge_fragments([GraphFragment::from(fresh), cwd_git_fragment]);

    // H-SERVE-PERF-001a: did the mux or harness heavy providers
    // actually run this cycle? Their provenance in the freshly-run
    // result (which excludes fresh-skipped providers and the prior
    // backstop) is the robust signal — disabled providers stamp
    // nothing, a git-only cycle stamps only git provenance, and a
    // mux/harness cycle stamps mux/harness provenance.
    //
    // H-SERVE-PERF-003 refines this further: even when mux/harness
    // did run, the walk is only worth firing when their slice
    // *content* differs from last cycle. A busy-file watcher that
    // wakes the harness class at the throttle cap on every codex
    // SQLite write produces byte-identical fragments cycle over
    // cycle — the /proc walk on those is pure waste. The
    // fingerprint is computed with `freshness_epoch` stamps
    // excluded so wall-clock churn doesn't defeat equality.
    let current_fingerprint = mux_or_harness_slice_fingerprint(&fresh);
    let mut fingerprint_guard = LAST_MUX_HARNESS_FINGERPRINT.lock().unwrap();
    let run_process_tree = should_open_process_tree_gate(current_fingerprint, *fingerprint_guard);
    if let Some(fp) = current_fingerprint {
        *fingerprint_guard = Some(fp);
    }
    drop(fingerprint_guard);

    // Phase-2 backstop merge with the evicted prior. The fresh
    // fragment wins on every collision; the prior fills in
    // slices the live run did not emit (the providers we
    // skipped because they were fresh).
    let mut snapshot = merge_with_prior(fresh, prior);

    // When the process-tree pass refreshes, drop the deferred prior
    // slices so the mutator re-stamps a clean, current contribution
    // (matching the pre-H-SERVE-PERF-001a always-evict behavior for
    // these keys). When it's skipped, the prior slices stay in the
    // merged snapshot untouched.
    if run_process_tree {
        for key in cache::PROCESS_TREE_MUTATORS {
            snapshot.evict_provider(key);
        }
    }

    apply_mutators(&mut snapshot, &config, &context, run_process_tree);
    Ok(snapshot)
}

/// Process-wide last-seen mux/harness slice fingerprint
/// (H-SERVE-PERF-003). Consulted by
/// [`should_open_process_tree_gate`] to skip the `/proc` walk when
/// the mux/harness slice content is byte-identical to the prior
/// cycle. Reset to `None` on daemon startup (first cycle after
/// restart always opens the gate to seed the walk). Tests can
/// reset via [`reset_process_tree_fingerprint_for_tests`].
static LAST_MUX_HARNESS_FINGERPRINT: Mutex<Option<u64>> = Mutex::new(None);

/// Decide whether the process-tree pass should fire this cycle
/// (H-SERVE-PERF-003).
///
/// * `current == None` → no mux/harness content in the fresh
///   fragment; matches today's "no provenance stamped" path and
///   keeps the gate closed regardless of history. The deferred
///   prior slices carry through.
/// * `previous == None` → first cycle after startup, or the first
///   cycle to observe any mux/harness content. Open the gate to
///   seed the walk + populate downstream caches (codex_log,
///   cross_link inference).
/// * `current == previous` → mux/harness slice is byte-identical
///   to last cycle (with `freshness_epoch` stamps excluded). The
///   `/proc` walk would produce the same links; skip it.
/// * `current != previous` → real content change (session added
///   or removed, mux window opened, activity epoch advanced).
///   Fire the walk.
fn should_open_process_tree_gate(current: Option<u64>, previous: Option<u64>) -> bool {
    match (current, previous) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(c), Some(p)) => c != p,
    }
}

/// Content fingerprint of the mux/harness slice of `fresh`. Used
/// by [`should_open_process_tree_gate`] to detect quiet-cycle
/// re-runs where the operator's mux/harness state is unchanged
/// and the `/proc` walk would produce redundant work
/// (H-SERVE-PERF-003).
///
/// The hash covers only nodes owned by a mux/harness provider
/// (via `node_provenance[id].provider`) and only candidate_links
/// whose `source_metadata.adapter` is a mux/harness provider —
/// so noise from git/forge slices does not force a re-walk.
/// `SourceMetadata::freshness_epoch` is zeroed before hashing so
/// per-cycle wall-clock advances don't defeat equality; every
/// other field (including `NodeProvenance::provider` and
/// activity epochs recorded on the node itself) stays in.
///
/// Returns `None` when the mux/harness slice is empty — matches
/// today's gate semantics (no mux/harness provenance → no walk).
fn mux_or_harness_slice_fingerprint(fresh: &GraphSnapshot) -> Option<u64> {
    // Set of node ids owned by a mux/harness provider, for the
    // node-filter pass below.
    let mux_harness_node_ids: BTreeSet<NodeId> = fresh
        .node_provenance
        .iter()
        .filter(|(_, prov)| cache::is_mux_or_harness_provider(prov.provider.as_str()))
        .map(|(id, _)| id.clone())
        .collect();

    // Serialize each in-slice node to JSON, sort the byte
    // strings so `Vec` order can't affect the fingerprint.
    let mut node_bytes: Vec<Vec<u8>> = fresh
        .nodes
        .iter()
        .filter(|node| mux_harness_node_ids.contains(&node.id()))
        .filter_map(|node| serde_json::to_vec(node).ok())
        .collect();
    node_bytes.sort();

    // Same for candidate_links owned by a mux/harness provider,
    // with `freshness_epoch` zeroed so wall-clock churn doesn't
    // defeat equality.
    let mut link_bytes: Vec<Vec<u8>> = fresh
        .candidate_links
        .iter()
        .filter(|link| cache::is_mux_or_harness_provider(link.source_metadata.adapter.as_str()))
        .filter_map(|link| {
            let mut cloned = link.clone();
            cloned.source_metadata.freshness_epoch = None;
            serde_json::to_vec(&cloned).ok()
        })
        .collect();
    link_bytes.sort();

    if node_bytes.is_empty() && link_bytes.is_empty() {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    for bytes in &node_bytes {
        bytes.hash(&mut hasher);
    }
    // Domain-separator between node section and link section so a
    // node's serialized bytes and a link's serialized bytes can't
    // accidentally hash-collide across the boundary.
    b"__links__".hash(&mut hasher);
    for bytes in &link_bytes {
        bytes.hash(&mut hasher);
    }
    Some(hasher.finish())
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
    run_process_tree: bool,
) {
    // H-SERVE-PERF-001a (ADR 0091): the cross-link inference and the
    // pid-fed harness aux attribution are the expensive `/proc`-walking
    // passes. Run them only when the mux or harness slice re-ran this
    // cycle; on a git/forge-only cycle the caller preserved the prior
    // cross_link/codex_log slices in the merged snapshot, so skipping
    // here keeps agent↔pane links visible without a fresh walk.
    if run_process_tree {
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
    }
    if let Some(root) = &config.hook_sidecar_root {
        hook_sidecar::apply_hook_sidecars(snapshot, root, hook_sidecar::current_epoch());
    }
    if let Some(loader) = &config.declared_config_loader {
        declared::apply_declared_links(snapshot, context, loader);
        aliases::apply_aliases(snapshot, context, loader);
        // Fold in any project pin stores recorded by the registry
        // sidecar (H-PIN-ROOT-001) so pins registered outside the scan
        // root stay visible. Read fresh each cycle so a pin created
        // during a live session appears on the next refresh.
        let registry_stores = config
            .pin_store_registry
            .as_ref()
            .map(|registry| registry.read())
            .unwrap_or_default();
        pins::apply_pins(snapshot, context, loader, &registry_stores);
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
    /// Resolver for the pin-store registry sidecar (H-PIN-ROOT-001,
    /// ADR 0090). When present, each discovery cycle reads the
    /// registered project `.conspectus.toml` store paths and folds them
    /// into the pin loader's search set so pins registered in repos
    /// outside the scan root stay visible. `None` disables the registry
    /// (tests and headless fixtures that don't want state-home I/O).
    pub pin_store_registry: Option<crate::pin_store_registry::PinStoreRegistry>,
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
            pin_store_registry: Some(crate::pin_store_registry::PinStoreRegistry::from_env()),
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
            pin_store_registry: None,
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
        self.with_orchestrator_root(providers::AGENT_DECK, root)
    }

    /// Deprecated alias for
    /// [`Self::without_orchestrator`]`("agent_deck")`.
    pub fn without_agent_deck(self) -> Self {
        self.without_orchestrator(providers::AGENT_DECK)
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
#[path = "discovery_tests.rs"]
mod tests;
