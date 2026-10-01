//! Forge discovery boundaries.
//!
//! Forge discovery associates remote pull requests with already-discovered
//! repos and branches. Each supported forge ships an adapter implementing
//! [`ForgeAdapter`]; the [`ForgeDiscovery`] coordinator is the
//! [`DiscoveryProvider`] that runs every registered adapter and merges
//! their [`GraphFragment`]s.
//!
//! GitHub discovery delegates to the `gh` CLI through an injectable
//! [`GhRunner`] seam, mirroring how tmux discovery uses
//! [`MuxBackend`](crate::discovery::tmux::MuxBackend).
//! Production runs use [`SystemGh`]; tests use [`FakeGh`] or any custom
//! implementation so they never need a real `gh` install or network call.
//! See ADR 0011 for the rationale.

#[cfg(test)]
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment, merge_fragments};

pub mod github;
pub mod gitlab;

pub use github::{GhPullRequestParser, PullRequestRecord, PullRequestState};

pub const GITHUB_PROVIDER: &str = crate::discovery::providers::GITHUB;
pub const GITHUB_DEFAULT_HOST: &str = "github.com";

/// JSON fields requested from `gh pr list --json <fields>`. Kept in one
/// place so the [`GhPullRequestParser`] and any `SystemGh` invocation
/// agree on the schema. See ADR 0011.
pub const GH_PR_LIST_FIELDS: &str = "number,state,url,headRefName,baseRefName,\
updatedAt,headRepositoryOwner,headRepository,isDraft";

/// Adapters implement provider-specific PR discovery and emit
/// provider-neutral [`GraphFragment`]s. Adapters must not perform
/// rendering, resolver scoring, or graph-wide cross-linking.
pub trait ForgeAdapter: Send + Sync {
    fn provider(&self) -> &str;

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment>;

    /// Should this adapter own PR discovery for the given remote
    /// origin URL (H-EXT-012)? A repo whose `origin` URL points
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

    /// H-EXT-012: register an already-boxed adapter. Used by the
    /// discovery driver, which takes ownership of adapters from
    /// `LocalDiscoveryConfig.forge_adapters` (already boxed) and
    /// hands them to the coordinator without re-boxing.
    pub fn with_boxed_adapter(mut self, adapter: Box<dyn ForgeAdapter>) -> Self {
        self.adapters.push(adapter);
        self
    }
}

impl DiscoveryProvider for ForgeDiscovery {
    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        // H-SERVE-PERF-005: TTL-cache the merged fragment so a
        // busy class thread doesn't respawn `gh pr list` on every
        // cycle. `GitHubForgeProvider` returns an empty fragment
        // when a repo has no PRs (or `gh` is unavailable), and an
        // empty fragment produces no `github` provenance stamps,
        // so `compute_freshness_gate` never marks `github` fresh
        // and every class cycle re-runs the forge coordinator →
        // one `gh pr list` spawn per cycle per root (0.5-0.8 Hz
        // on this operator's box). The TTL matches the Forge class
        // interval, so this skips only what the gate would have.
        let roots: Vec<PathBuf> = context.roots().to_vec();
        if let Some(cached) = context.caches().forge.get(&roots) {
            return Ok(cached);
        }

        let mut fragments = Vec::with_capacity(self.adapters.len());
        for adapter in &self.adapters {
            fragments.push(adapter.discover(context)?);
        }
        let merged = GraphFragment::from(merge_fragments(fragments));
        context.caches().forge.set(roots, merged.clone());
        Ok(merged)
    }
}

/// Time a cached [`ForgeDiscovery`] fragment is considered
/// fresh. Matches the Forge class TTL default in
/// `ServerIntervals` (5 minutes) — the gate that would have
/// already skipped this provider if stamps were present.
pub(crate) const FORGE_CACHE_TTL: Duration = Duration::from_secs(300);

/// Pluggable interface for invoking `gh` (or a fake equivalent). Mirrors
/// the [`MuxBackend`](crate::discovery::tmux::MuxBackend) seam so tests
/// stay offline.
pub trait GhRunner: Send + Sync {
    /// Run `gh pr list --json <fields>` for the given working directory.
    /// `cwd` is the repo working directory; runners that need a repo
    /// context use it as the spawn cwd. `fields` is the comma-separated
    /// list passed to `--json`.
    fn list_pull_requests(&self, cwd: &Path, fields: &str) -> Result<GhOutcome>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GhOutcome {
    /// `gh` returned successfully; payload is the raw, lossy UTF-8
    /// stdout. The body is expected to be a JSON array.
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
}

impl GhUnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinaryNotFound => "gh binary not found",
            Self::NotAuthenticated => "gh is not authenticated",
            Self::NotARepository => "cwd is not a GitHub repository",
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
    fn list_pull_requests(&self, cwd: &Path, fields: &str) -> Result<GhOutcome> {
        let output = Command::new(&self.binary)
            .args(["pr", "list", "--json", fields])
            .current_dir(cwd)
            .output();

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
    last_cwd: std::sync::Arc<std::sync::Mutex<Option<OsString>>>,
}

impl FakeGh {
    pub fn with_pull_requests(stdout: impl Into<String>) -> Self {
        Self {
            outcome: GhOutcome::PullRequests(stdout.into()),
            last_cwd: Default::default(),
        }
    }

    pub fn unavailable(reason: GhUnavailableReason) -> Self {
        Self {
            outcome: GhOutcome::Unavailable(reason),
            last_cwd: Default::default(),
        }
    }

    pub fn failed(code: Option<i32>, message: impl Into<String>) -> Self {
        Self {
            outcome: GhOutcome::Failed {
                code,
                message: message.into(),
            },
            last_cwd: Default::default(),
        }
    }

    /// Most-recent `cwd` passed to [`list_pull_requests`]. Useful in
    /// tests to confirm the adapter spawned `gh` from the right repo.
    pub fn last_cwd(&self) -> Option<PathBuf> {
        self.last_cwd
            .lock()
            .expect("lock")
            .clone()
            .map(PathBuf::from)
    }
}

impl GhRunner for FakeGh {
    fn list_pull_requests(&self, cwd: &Path, _fields: &str) -> Result<GhOutcome> {
        *self.last_cwd.lock().expect("lock") = Some(cwd.as_os_str().to_owned());
        Ok(self.outcome.clone())
    }
}

impl GhRunner for Box<dyn GhRunner> {
    fn list_pull_requests(&self, cwd: &Path, fields: &str) -> Result<GhOutcome> {
        (**self).list_pull_requests(cwd, fields)
    }
}

#[cfg(test)]
#[path = "forge_tests.rs"]
mod tests;
