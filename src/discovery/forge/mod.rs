//! Forge discovery boundaries.
//!
//! Forge discovery associates remote pull requests with already-discovered
//! repos and branches. Each supported forge ships an adapter implementing
//! [`ForgeAdapter`]; the [`ForgeDiscovery`] coordinator runs every
//! registered adapter over the repos the graph found, after the other
//! discovery passes (ADR 0115), and merges their [`GraphFragment`]s.
//!
//! GitHub discovery delegates to the `gh` CLI through an injectable
//! [`GhRunner`] seam, mirroring how tmux discovery uses
//! [`MuxBackend`](crate::discovery::tmux::MuxBackend).
//! Production runs use [`SystemGh`]; tests use [`FakeGh`] or any custom
//! implementation so they never need a real `gh` install or network call.
//! See ADR 0011 for the rationale.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::discovery::{DiscoveryContext, GraphFragment, merge_fragments};
use crate::model::{GraphNode, GraphSnapshot, RepoId};

pub mod github;
pub mod gitlab;

pub use github::{GhPullRequestParser, PullRequestRecord, PullRequestState};

pub const GITHUB_PROVIDER: &str = crate::discovery::providers::GITHUB;
pub const GITHUB_DEFAULT_HOST: &str = "github.com";

/// JSON fields requested from `gh pr list --json <fields>`. Kept in one
/// place so the [`GhPullRequestParser`] and any `SystemGh` invocation
/// agree on the schema. See ADR 0011.
pub const GH_PR_LIST_FIELDS: &str = "number,state,title,url,headRefName,baseRefName,\
updatedAt,headRepositoryOwner,headRepository,isDraft";

/// Gh queries run at once across every discovered repo (ADR 0115).
pub const MAX_CONCURRENT_GH: usize = 4;

/// Open PRs fetched per remote. Well above `gh`'s default of 30 so a
/// busy repo doesn't drop open PRs.
pub const OPEN_PR_LIMIT: u32 = 100;

/// PRs of any state fetched per remote from the recent window.
pub const RECENT_PR_LIMIT: u32 = 100;

/// Days back the recent-PR query reaches, so recently merged and closed
/// PRs enter the graph alongside open ones (ADR 0115).
pub const RECENT_PR_DAYS: i64 = 30;

/// A repo forge discovery asks about: what its local checkout knows
/// about remotes and branches, read through the cached git probe
/// (ADR 0115).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForgeRepo {
    pub repo_id: RepoId,
    pub remotes: Vec<crate::discovery::git::GitRemote>,
    /// Short local branch names (`feature/login`).
    pub local_branches: BTreeSet<String>,
}

/// Every repo in `snapshot` that forge discovery can ask about: one per
/// repo node, probed (cached) through a checkout root or source path
/// it has on disk. Repos without a path on disk are skipped.
pub fn forge_repos(snapshot: &GraphSnapshot, context: &DiscoveryContext) -> Vec<ForgeRepo> {
    let mut paths: BTreeMap<RepoId, Vec<PathBuf>> = BTreeMap::new();
    for node in &snapshot.nodes {
        match node {
            GraphNode::Checkout(checkout) => paths
                .entry(checkout.id.repo.clone())
                .or_default()
                .push(PathBuf::from(&checkout.root)),
            GraphNode::Repo(repo) => paths
                .entry(repo.id.clone())
                .or_default()
                .extend(repo.source_paths.iter().map(PathBuf::from)),
            _ => {}
        }
    }
    let probe = crate::discovery::git::GitProbe::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut repos = Vec::new();
    for candidates in paths.into_values() {
        let found = candidates
            .iter()
            .filter(|path| path.is_dir())
            .find_map(|path| probe.probe_cached(path, context.caches()).ok().flatten());
        let Some(found) = found else {
            continue;
        };
        if !seen.insert(found.common_dir.clone()) {
            continue;
        }
        repos.push(ForgeRepo {
            repo_id: RepoId::new(found.common_dir.to_string_lossy().to_string()),
            remotes: found.remotes,
            local_branches: found.local_branches.into_iter().collect(),
        });
    }
    repos
}

/// Adapters implement provider-specific PR discovery and emit
/// provider-neutral [`GraphFragment`]s. Adapters must not perform
/// rendering, resolver scoring, or graph-wide cross-linking.
pub trait ForgeAdapter: Send + Sync {
    fn provider(&self) -> &str;

    /// Discover PRs for `repos`, the repos the graph found (ADR 0115).
    fn discover(&self, repos: &[ForgeRepo], context: &DiscoveryContext) -> Result<GraphFragment>;

    /// Should this adapter own PR discovery for the given remote
    /// origin URL? A repo whose `origin` URL points
    /// at `github.com` should be handled by the GitHub adapter;
    /// a repo whose origin points at `gitlab.com` should be
    /// handled by a GitLab adapter. Returning `false` means "not
    /// mine"; returning `true` means "mine — call `discover`
    /// against a context that includes this repo."
    ///
    /// Default returns `false`. GitHub adapter overrides for
    /// `github.com` hosts. Future forge adapters (GitLab,
    /// Gitea, custom hosted GitHub Enterprise) override with
    /// their own host matching.
    fn claims_remote_url(&self, remote_url: &str) -> bool {
        let _ = remote_url;
        false
    }
}

/// Coordinator that runs every registered [`ForgeAdapter`] and returns a
/// merged fragment.
#[derive(Default)]
pub struct ForgeDiscovery {
    adapters: Vec<Box<dyn ForgeAdapter>>,
}

impl ForgeDiscovery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter: impl ForgeAdapter + 'static) -> Self {
        self.adapters.push(Box::new(adapter));
        self
    }

    /// Register an already-boxed adapter. Used by the
    /// discovery driver, which takes ownership of adapters from
    /// `LocalDiscoveryConfig.forge_adapters` (already boxed) and
    /// hands them to the coordinator without re-boxing.
    pub fn with_boxed_adapter(mut self, adapter: Box<dyn ForgeAdapter>) -> Self {
        self.adapters.push(adapter);
        self
    }
}

impl ForgeDiscovery {
    /// Discover PRs for every repo in `snapshot` (ADR 0115). Runs after
    /// the working-directory git pass, so repos found only through a
    /// session's or mux's working directory are included. Adapters
    /// reuse their cached listings while they're younger than the
    /// forge interval (ADR 0116), so this is cheap to call every cycle.
    pub fn discover_graph(
        &self,
        snapshot: &GraphSnapshot,
        context: &DiscoveryContext,
    ) -> Result<GraphFragment> {
        let repos = forge_repos(snapshot, context);
        let mut fragments = Vec::with_capacity(self.adapters.len());
        for adapter in &self.adapters {
            fragments.push(adapter.discover(&repos, context)?);
        }
        Ok(GraphFragment::from(merge_fragments(fragments)))
    }
}

/// How soon a forge query that failed is retried, when the forge
/// interval is longer: a laptop back online shouldn't wait an hour for
/// its PRs (ADR 0116).
pub const FORGE_RETRY_AFTER: Duration = Duration::from_secs(300);

/// What one forge remote answered: the raw listing bodies, in query
/// order, and the reason for each query that failed.
#[derive(Clone, Debug)]
pub(crate) struct RemoteListing {
    /// When the listing was fetched, for its age.
    pub(crate) fetched: Instant,
    /// The same moment as a Unix epoch, stamped on what it produced.
    pub(crate) epoch: i64,
    pub(crate) bodies: Vec<String>,
    pub(crate) failures: Vec<String>,
}

/// Forge listings per remote (`HOST/OWNER/REPO`), kept for the life of
/// the process that runs discovery (ADR 0116). A listing is reused
/// while it is younger than the forge interval, or than
/// [`FORGE_RETRY_AFTER`] if a query failed. Remotes are cached
/// separately, so a newly discovered repo costs only its own queries.
#[derive(Debug, Default)]
pub(crate) struct ForgeListings {
    slots: Mutex<HashMap<String, RemoteListing>>,
    hosts: Mutex<Option<SignedInHosts>>,
}

/// The hosts `gh` reported it is signed in to, and when it said so
/// (ADR 0128).
#[derive(Clone, Debug)]
struct SignedInHosts {
    fetched: Instant,
    hosts: Vec<String>,
}

impl ForgeListings {
    /// The cached listing for `remote`, whether or not it is still
    /// fresh.
    pub(crate) fn get(&self, remote: &str) -> Option<RemoteListing> {
        self.lock().get(remote).cloned()
    }

    /// The cached listing for `remote` if it may be reused under
    /// `max_age`.
    pub(crate) fn fresh(&self, remote: &str, max_age: Duration) -> Option<RemoteListing> {
        let listing = self.get(remote)?;
        let max_age = if listing.failures.is_empty() {
            max_age
        } else {
            max_age.min(FORGE_RETRY_AFTER)
        };
        (listing.fetched.elapsed() < max_age).then_some(listing)
    }

    pub(crate) fn set(&self, remote: String, listing: RemoteListing) {
        self.lock().insert(remote, listing);
    }

    /// The signed-in hosts if they may be reused under `max_age`. An
    /// empty answer is reused for at most [`FORGE_RETRY_AFTER`], so a
    /// sign-in made after startup shows up without waiting an hour.
    pub(crate) fn fresh_hosts(&self, max_age: Duration) -> Option<Vec<String>> {
        let slot = self.hosts.lock().unwrap_or_else(PoisonError::into_inner);
        let known = slot.as_ref()?;
        let max_age = if known.hosts.is_empty() {
            max_age.min(FORGE_RETRY_AFTER)
        } else {
            max_age
        };
        (known.fetched.elapsed() < max_age).then(|| known.hosts.clone())
    }

    pub(crate) fn set_hosts(&self, hosts: Vec<String>) {
        *self.hosts.lock().unwrap_or_else(PoisonError::into_inner) = Some(SignedInHosts {
            fetched: Instant::now(),
            hosts,
        });
    }

    /// Forget every listing and the signed-in hosts, so the next
    /// discovery asks every remote again: the TUI's hard refresh and
    /// the daemon's forge cycle.
    pub(crate) fn clear(&self) {
        self.lock().clear();
        *self.hosts.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, RemoteListing>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Which pull requests a `gh pr list` call asks for (ADR 0115).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrListFilter {
    /// Open PRs, up to `limit`.
    Open { limit: u32 },
    /// PRs of any state updated on or after `since` (`YYYY-MM-DD`), up
    /// to `limit`.
    UpdatedSince { since: String, limit: u32 },
}

impl PrListFilter {
    /// The `gh pr list` arguments for this filter.
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::Open { limit } => vec![
                "--state".to_string(),
                "open".to_string(),
                "--limit".to_string(),
                limit.to_string(),
            ],
            Self::UpdatedSince { since, limit } => vec![
                "--state".to_string(),
                "all".to_string(),
                "--limit".to_string(),
                limit.to_string(),
                "--search".to_string(),
                format!("updated:>={since}"),
            ],
        }
    }
}

/// Pluggable interface for invoking `gh` (or a fake equivalent). Mirrors
/// the [`MuxBackend`](crate::discovery::tmux::MuxBackend) seam so tests
/// stay offline.
pub trait GhRunner: Send + Sync {
    /// Run `gh pr list --repo <repo> --json <fields>` narrowed by
    /// `filter`, where `repo` is `[HOST/]OWNER/REPO` (ADR 0115).
    fn list_pull_requests(
        &self,
        repo: &str,
        filter: &PrListFilter,
        fields: &str,
    ) -> Result<GhOutcome>;

    /// Run `gh pr view <number> --repo <repo> --json <fields>`, where
    /// `repo` is `[HOST/]OWNER/REPO`. Success carries the JSON object
    /// in [`GhOutcome::PullRequests`].
    fn view_pull_request(&self, repo: &str, number: u64, fields: &str) -> Result<GhOutcome>;

    /// The hosts `gh` is signed in to, from `gh auth status --json
    /// hosts` (ADR 0128). Best effort: any failure yields no hosts,
    /// which leaves discovery on the built-in host names.
    fn authenticated_hosts(&self) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GhOutcome {
    /// `gh` returned successfully; payload is the raw, lossy UTF-8
    /// stdout: a JSON array from `pr list`, an object from `pr view`.
    PullRequests(String),
    /// `gh` is not usable on this host (binary missing or
    /// unauthenticated).
    Unavailable(GhUnavailableReason),
    /// `gh` returned a non-zero status for an unexpected reason. The
    /// message is the trimmed stderr; the code is the OS exit code when
    /// known.
    Failed { code: Option<i32>, message: String },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GhUnavailableReason {
    BinaryNotFound,
    NotAuthenticated,
    NotARepository,
    /// `gh` couldn't reach the forge: no network, or the host is down.
    Offline,
}

impl GhUnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinaryNotFound => "gh binary not found",
            Self::NotAuthenticated => "gh is not authenticated",
            Self::NotARepository => "cwd is not a GitHub repository",
            Self::Offline => "gh can't reach the forge",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemGh {
    binary: PathBuf,
}

impl Default for SystemGh {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("gh"),
        }
    }
}

impl SystemGh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }
}

impl GhRunner for SystemGh {
    fn list_pull_requests(
        &self,
        repo: &str,
        filter: &PrListFilter,
        fields: &str,
    ) -> Result<GhOutcome> {
        let mut args: Vec<String> = ["pr", "list", "--repo", repo, "--json", fields]
            .iter()
            .map(ToString::to_string)
            .collect();
        args.extend(filter.args());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&args, None)
    }

    fn view_pull_request(&self, repo: &str, number: u64, fields: &str) -> Result<GhOutcome> {
        let number = number.to_string();
        self.run(
            &["pr", "view", &number, "--repo", repo, "--json", fields],
            None,
        )
    }

    fn authenticated_hosts(&self) -> Vec<String> {
        match self.run(&["auth", "status", "--json", "hosts"], None) {
            Ok(GhOutcome::PullRequests(body)) => parse_authenticated_hosts(&body),
            _ => Vec::new(),
        }
    }
}

/// The host names in `gh auth status --json hosts` output: the keys of
/// its `hosts` object, lower-cased. Anything else yields none.
pub(crate) fn parse_authenticated_hosts(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(hosts) = value.get("hosts").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let mut hosts: Vec<String> = hosts
        .keys()
        .map(|host| host.trim().to_ascii_lowercase())
        .filter(|host| !host.is_empty())
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

impl SystemGh {
    /// Run `gh` read-only, from `cwd` when given, and classify the
    /// failures callers degrade on.
    fn run(&self, args: &[&str], cwd: Option<&Path>) -> Result<GhOutcome> {
        let mut command = crate::exec::command(crate::exec::Program::Gh, &self.binary);
        command
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(std::process::Stdio::null());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = command.output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(GhOutcome::Unavailable(GhUnavailableReason::BinaryNotFound));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn gh binary at {}", self.binary.display())
                });
            }
        };

        if output.status.success() {
            return Ok(GhOutcome::PullRequests(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_unauthenticated(&stderr) {
            return Ok(GhOutcome::Unavailable(
                GhUnavailableReason::NotAuthenticated,
            ));
        }
        if looks_like_not_a_repo(&stderr) {
            return Ok(GhOutcome::Unavailable(GhUnavailableReason::NotARepository));
        }
        if looks_like_offline(&stderr) {
            return Ok(GhOutcome::Unavailable(GhUnavailableReason::Offline));
        }

        Ok(GhOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }
}

fn looks_like_unauthenticated(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("not logged into") || lower.contains("authentication required")
}

fn looks_like_offline(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "error connecting to",
        "could not resolve host",
        "no such host",
        "network is unreachable",
        "connection refused",
        "i/o timeout",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn looks_like_not_a_repo(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("no github repository") || lower.contains("not a github repository")
}

/// Test runner that returns the same pre-canned outcome for every
/// `cwd`. Tests that need cwd-aware behavior can implement
/// [`GhRunner`] directly.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct FakeGh {
    outcome: GhOutcome,
    last_repo: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    hosts: Vec<String>,
    host_lookups: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeGh {
    pub fn with_pull_requests(stdout: impl Into<String>) -> Self {
        Self {
            outcome: GhOutcome::PullRequests(stdout.into()),
            last_repo: Default::default(),
            hosts: Vec::new(),
            host_lookups: Default::default(),
        }
    }

    pub fn unavailable(reason: GhUnavailableReason) -> Self {
        Self {
            outcome: GhOutcome::Unavailable(reason),
            last_repo: Default::default(),
            hosts: Vec::new(),
            host_lookups: Default::default(),
        }
    }

    pub fn failed(code: Option<i32>, message: impl Into<String>) -> Self {
        Self {
            outcome: GhOutcome::Failed {
                code,
                message: message.into(),
            },
            last_repo: Default::default(),
            hosts: Vec::new(),
            host_lookups: Default::default(),
        }
    }

    /// Hosts this fake reports `gh` as signed in to (ADR 0128).
    pub fn with_hosts(mut self, hosts: &[&str]) -> Self {
        self.hosts = hosts.iter().map(ToString::to_string).collect();
        self
    }

    /// How many times [`GhRunner::authenticated_hosts`] has run, across
    /// clones.
    pub fn host_lookups(&self) -> usize {
        self.host_lookups.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Most-recent repo passed to [`GhRunner::list_pull_requests`].
    /// Useful in tests to confirm the adapter named the right repo.
    pub fn last_repo(&self) -> Option<String> {
        self.last_repo.lock().expect("lock").clone()
    }
}

impl GhRunner for FakeGh {
    fn list_pull_requests(
        &self,
        repo: &str,
        _filter: &PrListFilter,
        _fields: &str,
    ) -> Result<GhOutcome> {
        *self.last_repo.lock().expect("lock") = Some(repo.to_string());
        Ok(self.outcome.clone())
    }

    fn view_pull_request(&self, _repo: &str, _number: u64, _fields: &str) -> Result<GhOutcome> {
        Ok(self.outcome.clone())
    }

    fn authenticated_hosts(&self) -> Vec<String> {
        self.host_lookups
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.hosts.clone()
    }
}

impl GhRunner for Box<dyn GhRunner> {
    fn list_pull_requests(
        &self,
        repo: &str,
        filter: &PrListFilter,
        fields: &str,
    ) -> Result<GhOutcome> {
        (**self).list_pull_requests(repo, filter, fields)
    }

    fn view_pull_request(&self, repo: &str, number: u64, fields: &str) -> Result<GhOutcome> {
        (**self).view_pull_request(repo, number, fields)
    }

    fn authenticated_hosts(&self) -> Vec<String> {
        (**self).authenticated_hosts()
    }
}

#[cfg(test)]
#[path = "forge_tests.rs"]
mod tests;
