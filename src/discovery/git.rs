//! Read-only git discovery probes.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};
use crate::model::{
    BranchId, BranchNode, CheckoutId, CheckoutNode, Confidence, Freshness, GraphLink, GraphNode,
    LinkEndpoint, LinkState, NodeId, Provenance, RelationKind, RepoId, RepoNode, SourceMetadata,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProbe {
    git_bin: PathBuf,
}

impl Default for GitProbe {
    fn default() -> Self {
        Self {
            git_bin: PathBuf::from("git"),
        }
    }
}

impl GitProbe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn probe(&self, root: impl AsRef<Path>) -> Result<Option<GitProbeResult>> {
        let root = root.as_ref().to_path_buf();

        // H-SERVE-PERF-004: consult the process-wide probe cache
        // before spawning any git subprocesses. Each probe otherwise
        // fires 8-15 `git rev-parse`/`symbolic-ref`/`remote`/
        // `for-each-ref` invocations, and `observed_cwd_git_fragment`
        // calls this once per unique session cwd on every discovery
        // cycle. On operator boxes with a dozen active sessions
        // that's the dominant serve idle cost (~150 git spawns per
        // cycle, ~1MB of libc/pcre/git-shared-lib reads per spawn).
        if let Some(cached) = probe_cache_lookup(&root) {
            return Ok(cached);
        }

        let result = self.probe_uncached(&root)?;
        probe_cache_store(&root, &result);
        Ok(result)
    }

    fn probe_uncached(&self, root: &Path) -> Result<Option<GitProbeResult>> {
        if !self.is_inside_work_tree(root)? {
            return Ok(None);
        }

        let common_dir = self.required(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let worktree_root = self.required(root, &["rev-parse", "--show-toplevel"])?;
        let git_dir = self.required(root, &["rev-parse", "--path-format=absolute", "--git-dir"])?;
        let branch_ref = self.optional(root, &["symbolic-ref", "--quiet", "HEAD"])?;
        let upstream = self.optional(
            root,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )?;
        let remotes = self.remotes(root)?;
        let local_branches = self.local_branches(root)?;

        Ok(Some(GitProbeResult {
            common_dir: PathBuf::from(common_dir),
            worktree_root: PathBuf::from(worktree_root),
            git_dir: PathBuf::from(git_dir),
            branch_ref,
            upstream,
            remotes,
            local_branches,
        }))
    }

    fn is_inside_work_tree(&self, root: &Path) -> Result<bool> {
        match self.optional(root, &["rev-parse", "--is-inside-work-tree"])? {
            Some(value) => Ok(value == "true"),
            None => Ok(false),
        }
    }

    /// Enumerate local branch short refs (e.g. `main`,
    /// `feature/login`). Empty when the repo has no commits yet or
    /// `git for-each-ref` returns nothing. The forge adapter uses this
    /// to map `gh pr list` head refs onto local `Branch` nodes even
    /// when the branch is not the currently-checked-out one.
    fn local_branches(&self, root: &Path) -> Result<Vec<String>> {
        let Some(output) = self.optional(
            root,
            &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        )?
        else {
            return Ok(Vec::new());
        };
        let mut branches: Vec<String> = output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        branches.sort();
        branches.dedup();
        Ok(branches)
    }

    fn remotes(&self, root: &Path) -> Result<Vec<GitRemote>> {
        let Some(names) = self.optional(root, &["remote"])? else {
            return Ok(Vec::new());
        };
        let mut remotes = Vec::new();

        for name in names.lines().filter(|line| !line.trim().is_empty()) {
            let url = self
                .optional(root, &["remote", "get-url", name])?
                .unwrap_or_default();
            remotes.push(GitRemote {
                name: name.to_string(),
                url,
            });
        }

        remotes.sort();
        Ok(remotes)
    }

    fn required(&self, root: &Path, args: &[&str]) -> Result<String> {
        self.optional(root, args)?
            .with_context(|| format!("git command failed: git {}", args.join(" ")))
    }

    fn optional(&self, root: &Path, args: &[&str]) -> Result<Option<String>> {
        #[cfg(test)]
        GIT_SPAWN_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let output = Command::new(&self.git_bin)
            .args(args)
            .current_dir(root)
            .output()
            .with_context(|| format!("failed to run git {}", args.join(" ")))?;

        if output.status.success() {
            return Ok(Some(trim_output(output.stdout)));
        }

        if is_expected_absence(args, output.status.code()) {
            return Ok(None);
        }

        bail!(
            "git {} failed with status {}: {}",
            args.join(" "),
            output.status,
            trim_output(output.stderr)
        );
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitDiscovery {
    probe: GitProbe,
}

impl GitDiscovery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DiscoveryProvider for GitDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        let epoch = crate::discovery::current_epoch();
        let mut fragments = Vec::new();

        for root in context.roots() {
            if let Some(probe) = self.probe.probe(root)? {
                fragments.push(fragment_from_probe(&probe));
            }
        }

        let mut fragment = GraphFragment::from(merge_fragments(fragments));
        crate::discovery::stamp_fragment(&mut fragment, crate::discovery::providers::GIT, epoch);
        Ok(fragment)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProbeResult {
    pub common_dir: PathBuf,
    pub worktree_root: PathBuf,
    pub git_dir: PathBuf,
    pub branch_ref: Option<String>,
    pub upstream: Option<String>,
    pub remotes: Vec<GitRemote>,
    pub local_branches: Vec<String>,
}

impl GitProbeResult {
    pub fn is_linked_worktree(&self) -> bool {
        self.common_dir != self.git_dir
    }
}

pub fn fragment_from_probe(probe: &GitProbeResult) -> GraphFragment {
    let repo_id = RepoId::new(crate::discovery::path_to_string(&probe.common_dir));
    let checkout_id = CheckoutId::new(
        repo_id.clone(),
        crate::discovery::path_to_string(&probe.worktree_root),
    );
    let repo_node = repo_node(repo_id.clone(), probe);
    let checkout_node = checkout_node(checkout_id.clone(), probe);
    let mut nodes = vec![
        GraphNode::Repo(repo_node),
        GraphNode::Checkout(checkout_node),
    ];
    let mut candidate_links = vec![git_link(
        NodeId::Checkout(checkout_id.clone()),
        NodeId::Repo(repo_id.clone()),
        RelationKind::BelongsToRepo,
        "git common dir",
    )];

    for branch_ref in branch_refs(probe) {
        let branch_id = BranchId::new(repo_id.clone(), branch_ref.clone());
        nodes.push(GraphNode::Branch(BranchNode {
            id: branch_id.clone(),
            refname: branch_ref.clone(),
            current_commit: None,
            upstream: if probe.branch_ref.as_deref() == Some(branch_ref.as_str()) {
                probe.upstream.clone()
            } else {
                None
            },
        }));
    }

    if let Some(branch_ref) = &probe.branch_ref {
        let branch_id = BranchId::new(repo_id, branch_ref.clone());
        candidate_links.push(git_link(
            NodeId::Checkout(checkout_id),
            NodeId::Branch(branch_id),
            RelationKind::CheckedOutBranch,
            "symbolic HEAD",
        ));
    }

    GraphFragment {
        nodes,
        candidate_links,
        diagnostics: Vec::new(),
        node_provenance: BTreeMap::new(),
    }
}

fn repo_node(repo_id: RepoId, probe: &GitProbeResult) -> RepoNode {
    let mut repo = RepoNode::new(repo_id);
    repo.source_paths
        .push(crate::discovery::path_to_string(&probe.worktree_root));
    repo.remotes = probe
        .remotes
        .iter()
        .map(|remote| format!("{}={}", remote.name, remote.url))
        .collect();
    repo
}

fn checkout_node(checkout_id: CheckoutId, probe: &GitProbeResult) -> CheckoutNode {
    CheckoutNode {
        id: checkout_id,
        root: crate::discovery::path_to_string(&probe.worktree_root),
        git_dir: Some(crate::discovery::path_to_string(&probe.git_dir)),
        current_branch: probe.branch_ref.as_ref().map(|branch| {
            BranchId::new(
                RepoId::new(crate::discovery::path_to_string(&probe.common_dir)),
                branch.clone(),
            )
        }),
    }
}

fn branch_refs(probe: &GitProbeResult) -> Vec<String> {
    let mut refs = BTreeSet::new();
    if let Some(branch_ref) = &probe.branch_ref {
        refs.insert(branch_ref.clone());
    }
    refs.extend(
        probe
            .local_branches
            .iter()
            .map(|short| format!("refs/heads/{short}")),
    );
    refs.into_iter().collect()
}

fn git_link(source: NodeId, target: NodeId, relation: RelationKind, evidence: &str) -> GraphLink {
    let relation_name = relation_name(&relation);
    GraphLink {
        id: format!("git:{source}:{relation_name}:{target}"),
        source,
        target: LinkEndpoint::Node { id: target },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: crate::discovery::providers::GIT.to_string(),
            evidence: Some(evidence.to_string()),
            fields: Default::default(),
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

fn relation_name(relation: &RelationKind) -> String {
    serde_json::to_string(relation)
        .expect("relation serializes")
        .trim_matches('"')
        .to_string()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

fn trim_output(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}

fn is_expected_absence(args: &[&str], code: Option<i32>) -> bool {
    matches!(
        (args, code),
        (["rev-parse", "--is-inside-work-tree"], Some(128))
            | (["symbolic-ref", "--quiet", "HEAD"], Some(1))
            | (
                [
                    "rev-parse",
                    "--abbrev-ref",
                    "--symbolic-full-name",
                    "@{upstream}"
                ],
                Some(128)
            )
            | (["remote"], Some(_))
    )
}

// ---------------------------------------------------------------------------
// H-SERVE-PERF-004: cross-cycle cache for [`GitProbe::probe`].
// ---------------------------------------------------------------------------
//
// `observed_cwd_git_fragment` (in `discovery/mod.rs`) probes one cwd per
// mux/agent session on every discovery cycle. Each probe spawns 8-15 git
// subprocesses (`is-inside-work-tree`, `rev-parse` variants, `symbolic-ref`,
// `remote`, `remote get-url` per remote, `for-each-ref`), so a box with a
// dozen active sessions burns ~150 git spawns per cycle. Each spawn drags
// in libc/pcre/zlib/... through the dynamic linker plus reads the git
// config files — the dominant post-A/B serve idle cost (~167 MB/s of
// tmpfs reads, ~10k syscr/s on the harness thread).
//
// The probe result is a pure function of the underlying repo state, so a
// process-wide cache keyed on the mtimes of the .git files that back the
// probe outputs kills the repeat spawns entirely. Cache entries are
// invalidated when any of those files advance:
//
//   * `.git/HEAD`               — branch checkout (updates the ref symlink)
//   * `.git/config`             — remote add/rm, upstream tracking change
//   * `.git/refs/heads/`        — local branch create/delete (dir mtime)
//   * `.git/packed-refs`        — post `git gc` / `git pack-refs`
//
// Fingerprint entries are `Option<i128>` because worktrees, sparse checkouts,
// and freshly-init'd repos may not have every file present.

static PROBE_CACHE: Mutex<Option<HashMap<PathBuf, CachedProbeEntry>>> = Mutex::new(None);

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitStateFingerprint {
    head_mtime_ns: Option<i128>,
    config_mtime_ns: Option<i128>,
    refs_heads_mtime_ns: Option<i128>,
    packed_refs_mtime_ns: Option<i128>,
    /// mtime of the input `root` itself. Guards the "not a git dir"
    /// cache entry: if the caller adds `.git` to `root` between calls
    /// the parent dir's mtime advances, invalidating the cache.
    root_mtime_ns: Option<i128>,
}

struct CachedProbeEntry {
    fingerprint: GitStateFingerprint,
    result: Option<GitProbeResult>,
}

fn file_mtime_ns(path: &Path) -> Option<i128> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let duration = mtime.duration_since(UNIX_EPOCH).ok()?;
    Some(i128::from(duration.as_secs()) * 1_000_000_000 + i128::from(duration.subsec_nanos()))
}

fn fingerprint_for_repo(git_dir: &Path, common_dir: &Path, root: &Path) -> GitStateFingerprint {
    GitStateFingerprint {
        head_mtime_ns: file_mtime_ns(&git_dir.join("HEAD")),
        // `.git/config` on a linked worktree lives in the common dir; the
        // worktree's own git-dir carries only worktree-scoped state.
        config_mtime_ns: file_mtime_ns(&common_dir.join("config")),
        refs_heads_mtime_ns: file_mtime_ns(&common_dir.join("refs").join("heads")),
        packed_refs_mtime_ns: file_mtime_ns(&common_dir.join("packed-refs")),
        root_mtime_ns: file_mtime_ns(root),
    }
}

fn fingerprint_for_non_repo(root: &Path) -> GitStateFingerprint {
    GitStateFingerprint {
        head_mtime_ns: None,
        config_mtime_ns: None,
        refs_heads_mtime_ns: None,
        packed_refs_mtime_ns: None,
        root_mtime_ns: file_mtime_ns(root),
    }
}

fn probe_cache_lookup(root: &Path) -> Option<Option<GitProbeResult>> {
    let guard = PROBE_CACHE.lock().unwrap();
    let map = guard.as_ref()?;
    let entry = map.get(root)?;
    let current = match &entry.result {
        Some(result) => fingerprint_for_repo(&result.git_dir, &result.common_dir, root),
        None => fingerprint_for_non_repo(root),
    };
    (current == entry.fingerprint).then(|| entry.result.clone())
}

fn probe_cache_store(root: &Path, result: &Option<GitProbeResult>) {
    let fingerprint = match result {
        Some(r) => fingerprint_for_repo(&r.git_dir, &r.common_dir, root),
        None => fingerprint_for_non_repo(root),
    };
    let mut guard = PROBE_CACHE.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(
        root.to_path_buf(),
        CachedProbeEntry {
            fingerprint,
            result: result.clone(),
        },
    );
}

/// Clear the process-wide [`PROBE_CACHE`]. Tests that assert
/// against the cache short-circuit call this in setup so a prior
/// test's entries don't leak into their assertions.
#[cfg(test)]
pub(crate) fn reset_probe_cache_for_tests() {
    let mut guard = PROBE_CACHE.lock().unwrap();
    *guard = None;
}

/// Count of `git` subprocess invocations issued via
/// [`GitProbe::optional`]. Tests use
/// [`take_git_spawn_count_for_tests`] to assert the cache
/// short-circuit fires — a second `probe()` on an unchanged repo
/// should record zero further spawns.
#[cfg(test)]
static GIT_SPAWN_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn take_git_spawn_count_for_tests() -> usize {
    GIT_SPAWN_COUNT.swap(0, std::sync::atomic::Ordering::Relaxed)
}

/// Serial gate for cache-observing tests — the module-level
/// [`PROBE_CACHE`] and [`GIT_SPAWN_COUNT`] are process-global so
/// parallel test runs would cross-talk without a serial lock.
#[cfg(test)]
pub(crate) static PROBE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
