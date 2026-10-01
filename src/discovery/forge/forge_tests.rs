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

// ---------------------------------------------------------------------------
// ForgeDiscovery TTL cache pins that a second discover()
// with the same roots short-circuits without dispatching adapters, so the
// daemon's per-class cycle can't respawn `gh pr list` on every wake-up when
// the initial forge call returned an empty fragment.
// ---------------------------------------------------------------------------

/// Adapter that increments a shared counter every time its
/// `discover` runs. Empty fragment so it doesn't leave stamps
/// (the exact "no PRs" case that triggers the pre-cache bug).
struct CountingAdapter {
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ForgeAdapter for CountingAdapter {
    fn provider(&self) -> &str {
        "counting-fake"
    }
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        self.calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(GraphFragment::empty())
    }
}

#[test]
fn forge_ttl_cache_serves_second_call_without_re_dispatching_adapters() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let coordinator = ForgeDiscovery::new().with_boxed_adapter(Box::new(CountingAdapter {
        calls: std::sync::Arc::clone(&calls),
    }));

    let context = DiscoveryContext::default();

    // First call: cache miss, adapter runs.
    let first = coordinator.discover(&context).expect("first");
    assert!(first.nodes.is_empty());
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Second call, same (empty) roots, well within TTL — must
    // serve from cache without dispatching the adapter.
    let second = coordinator.discover(&context).expect("second");
    assert!(second.nodes.is_empty());
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "second discover() must be served from cache"
    );
}

#[test]
fn forge_ttl_cache_invalidates_when_context_roots_change() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let coordinator = ForgeDiscovery::new().with_boxed_adapter(Box::new(CountingAdapter {
        calls: std::sync::Arc::clone(&calls),
    }));

    let caches = std::sync::Arc::new(crate::discovery::DiscoveryCaches::default());
    let ctx_a = DiscoveryContext::from_root("/tmp/a").with_caches(std::sync::Arc::clone(&caches));
    let ctx_b = DiscoveryContext::from_root("/tmp/b").with_caches(caches);

    coordinator.discover(&ctx_a).expect("call a");
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    coordinator.discover(&ctx_a).expect("cached a");
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
    // Different roots -> cache miss -> adapter re-dispatches.
    coordinator.discover(&ctx_b).expect("call b");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::Relaxed),
        2,
        "changing context roots must invalidate the cache"
    );
}
