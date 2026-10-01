//! GitLab forge adapter (skeleton).
//!
//! Second forge adapter that proves the `ForgeAdapter` registry
//! supports multiple entries with `claims_remote_url`-based
//! routing. **Real GitLab PR discovery is deferred** until the
//! multi-forge `ForgePr` identity model is settled; the open
//! questions are in backlog `H-DESIGN-002`.
//!
//! What ships today:
//!
//! - `GitLabForgeProvider` implements `ForgeAdapter` +
//!   `DiscoveryProvider`. `discover` returns an empty
//!   `GraphFragment` (no nodes, no diagnostics) so the
//!   provider is safe to register but doesn't emit
//!   speculative rows against an unsettled identity model.
//! - `claims_remote_url` matches `gitlab.com` substrings on
//!   both HTTPS and SSH remote-url shapes.
//! - No `GlabRunner` trait / `SystemGlab` impl yet. Those
//!   land alongside real discovery when the identity model
//!   is decided.
//!
//! The adapter is opt-in via `CONSPECTUS_ENABLE_GITLAB` in
//! `LocalDiscoveryConfig::from_env`. Default off keeps the
//! stub from cluttering warm-start caches with a spurious
//! `gitlab` provider slice.

use anyhow::Result;

use super::ForgeAdapter;
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment};

/// Provider stamp string for gitlab-derived nodes / links, so
/// tests can compare against a single source-of-truth string.
pub const GITLAB_PROVIDER: &str = crate::discovery::providers::GITLAB;

/// GitLab CLI host used for `claims_remote_url` matching.
/// Enterprise / self-hosted GitLab is not matched yet.
pub const GITLAB_DEFAULT_HOST: &str = "gitlab.com";

/// GitLab forge adapter (skeleton). Emits no PRs: `discover`
/// returns an empty fragment until the identity model is settled.
#[derive(Clone, Debug, Default)]
pub struct GitLabForgeProvider;

impl GitLabForgeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl DiscoveryProvider for GitLabForgeProvider {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        // Skeleton: no discovery until the ForgePr identity model
        // is settled. Present to prove the registry accepts a
        // second adapter and routes by `claims_remote_url`.
        Ok(GraphFragment::empty())
    }
}

impl ForgeAdapter for GitLabForgeProvider {
    fn provider(&self) -> &str {
        GITLAB_PROVIDER
    }

    fn discover(&self, context: &DiscoveryContext) -> Result<GraphFragment> {
        <Self as DiscoveryProvider>::discover(self, context)
    }

    fn claims_remote_url(&self, remote_url: &str) -> bool {
        remote_url_is_gitlab(remote_url)
    }
}

fn remote_url_is_gitlab(remote_url: &str) -> bool {
    // Substring match against `gitlab.com`. Handles both HTTPS
    // (`https://gitlab.com/owner/repo.git`) and SSH
    // (`git@gitlab.com:owner/repo.git`) shapes. Enterprise
    // GitLab hosts would need a config-driven host list.
    let lower = remote_url.to_ascii_lowercase();
    lower.contains("gitlab.com")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_gitlab_https_and_ssh_urls() {
        let adapter = GitLabForgeProvider::new();
        assert!(adapter.claims_remote_url("https://gitlab.com/foo/bar.git"));
        assert!(adapter.claims_remote_url("git@gitlab.com:foo/bar.git"));
        assert!(adapter.claims_remote_url("ssh://git@gitlab.com/foo/bar"));
    }

    #[test]
    fn does_not_claim_github_urls() {
        let adapter = GitLabForgeProvider::new();
        assert!(!adapter.claims_remote_url("https://github.com/foo/bar.git"));
        assert!(!adapter.claims_remote_url("git@github.com:foo/bar.git"));
    }

    #[test]
    fn provider_string_is_gitlab_not_github() {
        // Sanity anchor: the adapter's provider string must
        // differ from the GitHub adapter's so provenance
        // stamps distinguish them.
        let adapter = GitLabForgeProvider::new();
        assert_eq!(adapter.provider(), "gitlab");
        assert_ne!(adapter.provider(), crate::discovery::forge::GITHUB_PROVIDER);
    }

    #[test]
    fn discover_returns_empty_fragment_until_h_design_002_lands() {
        let adapter = GitLabForgeProvider::new();
        let ctx = DiscoveryContext::from_root("/tmp");
        let fragment = ForgeAdapter::discover(&adapter, &ctx).expect("discover");
        assert!(fragment.nodes.is_empty());
        assert!(fragment.candidate_links.is_empty());
        assert!(fragment.diagnostics.is_empty());
    }
}
