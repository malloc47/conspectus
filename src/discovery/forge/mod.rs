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
        let mut fragments = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            fragments.push(adapter.discover(context)?);
        }

        Ok(GraphFragment::from(merge_fragments(fragments)))
    }
}

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
mod tests {
    use super::*;

    #[test]
    fn missing_binary_reports_unavailable_binary_not_found() {
        let runner = SystemGh::with_binary("/definitely/not/here/gh");

        let outcome = runner
            .list_pull_requests(Path::new("/tmp"), "number,state")
            .expect("non-fatal");

        assert_eq!(
            outcome,
            GhOutcome::Unavailable(GhUnavailableReason::BinaryNotFound)
        );
    }

    #[test]
    fn fake_gh_returns_pre_canned_pull_requests() {
        let runner = FakeGh::with_pull_requests("[]");

        let outcome = runner
            .list_pull_requests(Path::new("/repo"), "number,state")
            .expect("ok");

        assert_eq!(outcome, GhOutcome::PullRequests("[]".to_string()));
        assert_eq!(runner.last_cwd().as_deref(), Some(Path::new("/repo")));
    }

    #[test]
    fn fake_gh_can_report_unauthenticated() {
        let runner = FakeGh::unavailable(GhUnavailableReason::NotAuthenticated);

        let outcome = runner
            .list_pull_requests(Path::new("/repo"), "number")
            .expect("ok");

        assert_eq!(
            outcome,
            GhOutcome::Unavailable(GhUnavailableReason::NotAuthenticated)
        );
    }

    #[test]
    fn fake_gh_can_surface_failed_runs() {
        let runner = FakeGh::failed(Some(2), "rate limited");

        let outcome = runner
            .list_pull_requests(Path::new("/repo"), "number")
            .expect("ok");

        assert_eq!(
            outcome,
            GhOutcome::Failed {
                code: Some(2),
                message: "rate limited".to_string(),
            }
        );
    }

    #[test]
    fn unauthenticated_stderr_maps_to_unavailable() {
        assert!(looks_like_unauthenticated(
            "error: not logged into any GitHub hosts"
        ));
        assert!(looks_like_unauthenticated(
            "gh: authentication required for github.com"
        ));
        assert!(!looks_like_unauthenticated("rate limited"));
    }

    #[test]
    fn not_a_repo_stderr_maps_to_unavailable() {
        assert!(looks_like_not_a_repo(
            "no GitHub repository found in current directory"
        ));
        assert!(looks_like_not_a_repo(
            "this is not a GitHub repository (no upstream remote)"
        ));
        assert!(!looks_like_not_a_repo("permission denied"));
    }

    #[test]
    fn unavailable_reasons_have_stable_diagnostic_strings() {
        assert_eq!(
            GhUnavailableReason::BinaryNotFound.as_str(),
            "gh binary not found"
        );
        assert_eq!(
            GhUnavailableReason::NotAuthenticated.as_str(),
            "gh is not authenticated"
        );
        assert_eq!(
            GhUnavailableReason::NotARepository.as_str(),
            "cwd is not a GitHub repository"
        );
    }

    struct StaticAdapter {
        provider: &'static str,
        fragment: GraphFragment,
    }

    impl ForgeAdapter for StaticAdapter {
        fn provider(&self) -> &str {
            self.provider
        }

        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(self.fragment.clone())
        }
    }

    #[test]
    fn forge_discovery_without_adapters_returns_empty_fragment() {
        let fragment = ForgeDiscovery::new()
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment, GraphFragment::empty());
    }

    #[test]
    fn forge_discovery_merges_adapter_fragments_deterministically() {
        use crate::model::{ForgePrId, ForgePrNode, GraphNode};

        let pr_alpha = GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "octo", "repo", 1),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "repo".to_string(),
            number: 1,
            state: Some("open".to_string()),
            url: None,
            updated_epoch: None,
            is_draft: false,
        });
        let pr_beta = GraphNode::ForgePr(ForgePrNode {
            id: ForgePrId::new("github", "github.com", "octo", "repo", 2),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "repo".to_string(),
            number: 2,
            state: Some("open".to_string()),
            url: None,
            updated_epoch: None,
            is_draft: false,
        });

        let fragment = ForgeDiscovery::new()
            .with_adapter(StaticAdapter {
                provider: "github-beta",
                fragment: GraphFragment {
                    nodes: vec![pr_beta.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                    node_provenance: BTreeMap::new(),
                },
            })
            .with_adapter(StaticAdapter {
                provider: "github-alpha",
                fragment: GraphFragment {
                    nodes: vec![pr_alpha.clone()],
                    candidate_links: Vec::new(),
                    diagnostics: Vec::new(),
                    node_provenance: BTreeMap::new(),
                },
            })
            .discover(&DiscoveryContext::default())
            .expect("discovery succeeds");

        assert_eq!(fragment.nodes, vec![pr_alpha, pr_beta]);
    }

    // H-EXT-013 routing tests: prove that
    // `claims_remote_url` correctly partitions two adapters
    // that claim different hosts. Uses two `StaticAdapter`s
    // (test-only) whose `claims_remote_url` overrides target
    // distinct hosts; asserts each adapter's `claims_remote_url`
    // fires only for its own URLs.
    struct HostedStaticAdapter {
        provider: &'static str,
        host_match: &'static str,
    }

    impl ForgeAdapter for HostedStaticAdapter {
        fn provider(&self) -> &str {
            self.provider
        }

        fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
            Ok(GraphFragment::empty())
        }

        fn claims_remote_url(&self, remote_url: &str) -> bool {
            remote_url.contains(self.host_match)
        }
    }

    #[test]
    fn claims_remote_url_partitions_two_adapters_by_host() {
        let github_adapter = HostedStaticAdapter {
            provider: "github-fake",
            host_match: "github.example",
        };
        let gitlab_adapter = HostedStaticAdapter {
            provider: "gitlab-fake",
            host_match: "gitlab.example",
        };

        let github_url = "git@github.example:foo/bar.git";
        let gitlab_url = "https://gitlab.example/foo/bar.git";

        // Each adapter claims only its own URLs.
        assert!(github_adapter.claims_remote_url(github_url));
        assert!(!github_adapter.claims_remote_url(gitlab_url));
        assert!(!gitlab_adapter.claims_remote_url(github_url));
        assert!(gitlab_adapter.claims_remote_url(gitlab_url));

        // Non-matching URL is claimed by neither.
        let neither = "https://elsewhere.example/foo/bar";
        assert!(!github_adapter.claims_remote_url(neither));
        assert!(!gitlab_adapter.claims_remote_url(neither));
    }

    #[test]
    fn forge_discovery_accepts_two_adapters_via_boxed_registration() {
        // Register both adapters through the coordinator using
        // the H-EXT-012 `with_boxed_adapter` builder. Confirms
        // that `ForgeDiscovery` can carry a heterogeneous set
        // of `Box<dyn ForgeAdapter>`s (which is how
        // `LocalDiscoveryConfig::forge_adapters` flows through
        // to the coordinator).
        let github: Box<dyn ForgeAdapter> = Box::new(HostedStaticAdapter {
            provider: "github-fake",
            host_match: "github.example",
        });
        let gitlab: Box<dyn ForgeAdapter> = Box::new(HostedStaticAdapter {
            provider: "gitlab-fake",
            host_match: "gitlab.example",
        });
        let coordinator = ForgeDiscovery::new()
            .with_boxed_adapter(github)
            .with_boxed_adapter(gitlab);
        // Empty fragments merge cleanly; no panic proves the
        // coordinator dispatches both adapters.
        let fragment = coordinator
            .discover(&DiscoveryContext::default())
            .expect("discover");
        assert!(fragment.nodes.is_empty());
    }
}
