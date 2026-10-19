use super::*;

#[test]
fn empty_body_parses_to_zero_records() {
    assert!(GhPullRequestParser::new().parse("").is_empty());
    assert!(GhPullRequestParser::new().parse("   \n  ").is_empty());
    assert!(GhPullRequestParser::new().parse("[]").is_empty());
}

#[test]
fn malformed_body_returns_no_records() {
    assert!(GhPullRequestParser::new().parse("not json").is_empty());
    assert!(GhPullRequestParser::new().parse("{}").is_empty());
}

#[test]
fn single_open_pull_request_parses_with_all_fields() {
    let body = r#"[{
            "number": 7,
            "state": "OPEN",
            "url": "https://github.com/octo/repo/pull/7",
            "headRefName": "feature/login",
            "baseRefName": "main",
            "updatedAt": "2026-03-05T12:34:56Z",
            "headRepositoryOwner": {"login": "octo"},
            "headRepository": {"name": "repo"},
            "isDraft": false
        }]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.number, 7);
    assert_eq!(record.state, PullRequestState::Open);
    assert_eq!(record.head_ref, "feature/login");
    assert_eq!(record.base_ref.as_deref(), Some("main"));
    assert_eq!(record.head_owner.as_deref(), Some("octo"));
    assert_eq!(record.head_repo.as_deref(), Some("repo"));
    assert!(!record.is_draft);
    assert_eq!(
        record.url.as_deref(),
        Some("https://github.com/octo/repo/pull/7")
    );
    assert_eq!(record.updated_at.as_deref(), Some("2026-03-05T12:34:56Z"));
    assert_eq!(record.updated_epoch, Some(1_772_714_096));
}

#[test]
fn multiple_pull_requests_preserve_input_order() {
    let body = r#"[
            {"number": 2, "state": "OPEN", "headRefName": "b"},
            {"number": 1, "state": "MERGED", "headRefName": "a"}
        ]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert_eq!(
        records.iter().map(|r| r.number).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert_eq!(records[0].state, PullRequestState::Open);
    assert_eq!(records[1].state, PullRequestState::Merged);
}

#[test]
fn rows_missing_required_fields_are_skipped() {
    let body = r#"[
            {"state": "OPEN", "headRefName": "no-number"},
            {"number": 5, "state": "OPEN"},
            {"number": 6, "state": "OPEN", "headRefName": "ok"}
        ]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].number, 6);
}

#[test]
fn draft_flag_is_preserved() {
    let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "wip", "isDraft": true}]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert!(records[0].is_draft);
}

#[test]
fn unknown_state_falls_through_to_other() {
    let body = r#"[{"number": 1, "state": "CONVERTED_TO_DISCUSSION", "headRefName": "x"}]"#;

    let records = GhPullRequestParser::new().parse(body);

    match &records[0].state {
        PullRequestState::Other(raw) => assert_eq!(raw, "CONVERTED_TO_DISCUSSION"),
        other => panic!("expected Other, got {other:?}"),
    }
}

#[test]
fn missing_optional_fields_yield_none() {
    let body = r#"[{"number": 9, "state": "OPEN", "headRefName": "minimal"}]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert!(record.url.is_none());
    assert!(record.base_ref.is_none());
    assert!(record.head_owner.is_none());
    assert!(record.head_repo.is_none());
    assert!(record.updated_at.is_none());
    assert!(record.updated_epoch.is_none());
    assert!(!record.is_draft);
}

#[test]
fn updated_at_with_offset_is_normalized_to_utc_epoch() {
    let utc = r#"[{"number": 1, "state": "OPEN", "headRefName": "a",
                       "updatedAt": "2026-03-05T12:00:00Z"}]"#;
    let offset = r#"[{"number": 2, "state": "OPEN", "headRefName": "b",
                          "updatedAt": "2026-03-05T14:00:00+02:00"}]"#;

    let utc_records = GhPullRequestParser::new().parse(utc);
    let offset_records = GhPullRequestParser::new().parse(offset);

    assert_eq!(
        utc_records[0].updated_epoch,
        offset_records[0].updated_epoch
    );
}

#[test]
fn updated_at_with_unsupported_shape_falls_back_to_raw_string() {
    let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "a",
                        "updatedAt": "not a timestamp"}]"#;

    let records = GhPullRequestParser::new().parse(body);

    assert_eq!(records[0].updated_at.as_deref(), Some("not a timestamp"));
    assert!(records[0].updated_epoch.is_none());
}

#[test]
fn pull_request_state_round_trips_through_as_str() {
    assert_eq!(PullRequestState::Open.as_str(), "open");
    assert_eq!(PullRequestState::Closed.as_str(), "closed");
    assert_eq!(PullRequestState::Merged.as_str(), "merged");
    assert_eq!(PullRequestState::Other("DRAFT".into()).as_str(), "DRAFT");
}

fn context() -> RepoContext {
    RepoContext::github(RepoId::new("/workspace/repo/.git"), "octo", "repo")
}

/// The local repo's GitHub remotes, matching [`context`].
fn remotes() -> BTreeSet<(String, String)> {
    [("octo".to_string(), "repo".to_string())].into()
}

fn record(number: u64, head_ref: &str, state: PullRequestState) -> PullRequestRecord {
    PullRequestRecord {
        number,
        state,
        url: Some(format!("https://github.com/octo/repo/pull/{number}")),
        head_ref: head_ref.to_string(),
        head_owner: Some("octo".to_string()),
        head_repo: Some("repo".to_string()),
        base_ref: Some("main".to_string()),
        is_draft: false,
        updated_at: None,
        updated_epoch: None,
    }
}

#[test]
fn fragment_emits_forge_pr_node_per_record() {
    let context = context();
    let branches = BTreeSet::new();
    let records = vec![
        record(1, "feature/a", PullRequestState::Open),
        record(2, "feature/b", PullRequestState::Merged),
    ];

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &records);

    let pr_nodes: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(pr),
            _ => None,
        })
        .collect();
    assert_eq!(pr_nodes.len(), 2);
    assert_eq!(pr_nodes[0].id.number, 1);
    assert_eq!(pr_nodes[0].owner, "octo");
    assert_eq!(pr_nodes[0].repo, "repo");
    assert_eq!(pr_nodes[0].state.as_deref(), Some("open"));
    assert_eq!(pr_nodes[1].state.as_deref(), Some("merged"));
}

#[test]
fn matched_head_ref_emits_branch_node_endpoint() {
    let context = context();
    let mut branches = BTreeSet::new();
    branches.insert("feature/login".to_string());
    let records = vec![record(7, "feature/login", PullRequestState::Open)];

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &records);

    assert_eq!(fragment.candidate_links.len(), 1);
    let link = &fragment.candidate_links[0];
    assert_eq!(link.relation, RelationKind::BranchHasForgePr);
    match &link.target {
        LinkEndpoint::Node {
            id: NodeId::Branch(branch),
        } => {
            assert_eq!(branch.refname, "refs/heads/feature/login");
            assert_eq!(branch.repo, context.repo_id);
        }
        other => panic!("expected branch node target, got {other:?}"),
    }
}

#[test]
fn unknown_head_ref_emits_unresolved_endpoint() {
    let context = context();
    let branches = BTreeSet::new();
    let records = vec![record(9, "feature/missing", PullRequestState::Open)];

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &records);

    let link = &fragment.candidate_links[0];
    match &link.target {
        LinkEndpoint::Unresolved { evidence } => {
            assert_eq!(evidence.node_type, "branch");
            assert_eq!(evidence.native_id.as_deref(), Some("feature/missing"));
            assert_eq!(
                evidence.metadata.get("host"),
                Some(&Value::String("github.com".to_string()))
            );
        }
        other @ LinkEndpoint::Node { .. } => {
            panic!("expected unresolved branch evidence, got {other:?}")
        }
    }
}

#[test]
fn unresolved_branch_evidence_names_the_local_repo_it_was_queried_for() {
    let context = context();
    let records = vec![record(9, "feature/missing", PullRequestState::Merged)];

    let fragment = fragment_for_repo(&context, &BTreeSet::new(), &remotes(), &records);

    match &fragment.candidate_links[0].target {
        LinkEndpoint::Unresolved { evidence } => assert_eq!(
            evidence.metadata.get("local_repo"),
            Some(&Value::String(context.repo_id.common_dir))
        ),
        other @ LinkEndpoint::Node { .. } => panic!("expected unresolved evidence, got {other:?}"),
    }
}

#[test]
fn draft_flag_is_carried_onto_node_and_link_metadata() {
    let context = context();
    let branches = BTreeSet::new();
    let mut draft = record(3, "wip", PullRequestState::Open);
    draft.is_draft = true;

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &[draft]);

    let pr = match &fragment.nodes[0] {
        GraphNode::ForgePr(pr) => pr,
        other => panic!("expected ForgePr node, got {other:?}"),
    };
    assert!(pr.is_draft);
    let link = &fragment.candidate_links[0];
    assert_eq!(
        link.source_metadata.fields.get("is_draft"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn state_is_preserved_for_open_closed_merged() {
    let context = context();
    let branches = BTreeSet::new();
    let records = vec![
        record(1, "open-branch", PullRequestState::Open),
        record(2, "closed-branch", PullRequestState::Closed),
        record(3, "merged-branch", PullRequestState::Merged),
    ];

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &records);

    let states: Vec<_> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => pr.state.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(states, vec!["open", "closed", "merged"]);
}

#[test]
fn node_and_link_ids_are_stable_across_repeated_runs() {
    let context = context();
    let mut branches = BTreeSet::new();
    branches.insert("feature/login".to_string());
    let records = vec![record(11, "feature/login", PullRequestState::Open)];

    let first = fragment_for_repo(&context, &branches, &remotes(), &records);
    let second = fragment_for_repo(&context, &branches, &remotes(), &records);

    let extract_ids = |fragment: &GraphFragment| -> (Vec<NodeId>, Vec<String>) {
        (
            fragment.nodes.iter().map(GraphNode::id).collect(),
            fragment
                .candidate_links
                .iter()
                .map(|link| link.id.clone())
                .collect(),
        )
    };
    assert_eq!(extract_ids(&first), extract_ids(&second));
}

#[test]
fn updated_epoch_is_preserved_on_the_node() {
    let context = context();
    let branches = BTreeSet::new();
    let mut rec = record(5, "head", PullRequestState::Open);
    rec.updated_epoch = Some(1_772_714_096);

    let fragment = fragment_for_repo(&context, &branches, &remotes(), &[rec]);

    let pr = match &fragment.nodes[0] {
        GraphNode::ForgePr(pr) => pr,
        other => panic!("expected ForgePr, got {other:?}"),
    };
    assert_eq!(pr.updated_epoch, Some(1_772_714_096));
}

#[test]
fn empty_records_yield_empty_fragment() {
    let fragment = fragment_for_repo(&context(), &BTreeSet::new(), &remotes(), &[]);

    assert!(fragment.nodes.is_empty());
    assert!(fragment.candidate_links.is_empty());
}

#[test]
fn parses_https_github_remotes() {
    assert_eq!(
        parse_github_remote("https://github.com/octo/repo.git"),
        Some((
            "github.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
    assert_eq!(
        parse_github_remote("https://github.com/octo/repo"),
        Some((
            "github.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
}

#[test]
fn parses_ssh_and_git_at_github_remotes() {
    assert_eq!(
        parse_github_remote("git@github.com:octo/repo.git"),
        Some((
            "github.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
    assert_eq!(
        parse_github_remote("ssh://git@github.com/octo/repo.git"),
        Some((
            "github.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
}

#[test]
fn parses_github_enterprise_hosts() {
    assert_eq!(
        parse_github_remote("https://github.acme.com/octo/repo.git"),
        Some((
            "github.acme.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
    assert_eq!(
        parse_github_remote("git@code.example.github.com:octo/repo.git"),
        Some((
            "code.example.github.com".to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
}

#[test]
fn rejects_non_github_remotes() {
    assert!(parse_github_remote("https://gitlab.com/octo/repo.git").is_none());
    assert!(parse_github_remote("git@bitbucket.org:octo/repo.git").is_none());
    assert!(parse_github_remote("file:///tmp/repo.git").is_none());
    assert!(parse_github_remote("").is_none());
}

#[test]
fn rejects_malformed_remote_paths() {
    assert!(parse_github_remote("https://github.com/octo").is_none());
    assert!(parse_github_remote("https://github.com//repo.git").is_none());
    assert!(parse_github_remote("https://github.com/octo/repo/extra").is_none());
}

use crate::discovery::forge::{FakeGh, ForgeAdapter, ForgeRepo, GhUnavailableReason};
use crate::discovery::git::GitRemote;

/// A local repo with `remotes` (`name`, `url`) and local `branches`.
fn forge_repo(remotes: &[(&str, &str)], branches: &[&str]) -> ForgeRepo {
    ForgeRepo {
        repo_id: RepoId::new("/workspace/repo/.git"),
        remotes: remotes
            .iter()
            .map(|(name, url)| GitRemote {
                name: (*name).to_string(),
                url: (*url).to_string(),
            })
            .collect(),
        local_branches: branches.iter().map(|b| (*b).to_string()).collect(),
    }
}

fn discover(runner: FakeGh, repos: &[ForgeRepo]) -> GraphFragment {
    GitHubForgeProvider::with_runner(runner)
        .discover(repos, &DiscoveryContext::default())
        .expect("discover")
}

fn pr_ids(fragment: &GraphFragment) -> Vec<String> {
    fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(format!("{}/{}#{}", pr.owner, pr.repo, pr.number)),
            _ => None,
        })
        .collect()
}

#[test]
fn provider_skips_repos_without_a_github_remote() {
    let runner = FakeGh::with_pull_requests(r#"[{"number": 1, "headRefName": "main"}]"#);
    let fragment = discover(
        runner.clone(),
        &[forge_repo(
            &[("origin", "git@gitlab.com:octo/repo.git")],
            &["main"],
        )],
    );
    assert!(fragment.nodes.is_empty());
    assert_eq!(runner.last_repo(), None, "gh is never asked");
}

#[test]
fn provider_names_the_repo_it_queries() {
    let runner = FakeGh::with_pull_requests("[]");
    discover(
        runner.clone(),
        &[forge_repo(
            &[("origin", "git@github.com:octo/repo.git")],
            &[],
        )],
    );
    assert_eq!(runner.last_repo().as_deref(), Some("github.com/octo/repo"));
}

#[test]
fn provider_merges_the_open_and_recent_queries_by_number() {
    // The fake answers both queries with the same body.
    let body = r#"[
            {"number": 1, "state": "OPEN", "headRefName": "main", "isDraft": false},
            {"number": 2, "state": "MERGED", "headRefName": "feature", "isDraft": false}
        ]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body),
        &[forge_repo(
            &[("origin", "git@github.com:octo/repo.git")],
            &[],
        )],
    );
    assert_eq!(pr_ids(&fragment), ["octo/repo#1", "octo/repo#2"]);
    assert_eq!(fragment.candidate_links.len(), 2);
}

#[test]
fn provider_queries_every_github_remote() {
    let body = r#"[{"number": 4, "state": "OPEN", "headRefName": "main"}]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body),
        &[forge_repo(
            &[
                ("origin", "git@github.com:me/repo.git"),
                ("upstream", "https://github.com/octo/repo.git"),
            ],
            &[],
        )],
    );
    let mut ids = pr_ids(&fragment);
    ids.sort();
    assert_eq!(ids, ["me/repo#4", "octo/repo#4"]);
}

fn branch_target(fragment: &GraphFragment) -> &LinkEndpoint {
    &fragment.candidate_links[0].target
}

#[test]
fn a_pr_from_this_repo_links_to_its_local_branch() {
    let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "feature",
        "headRepositoryOwner": {"login": "octo"}, "headRepository": {"name": "repo"}}]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body),
        &[forge_repo(
            &[("origin", "git@github.com:octo/repo.git")],
            &["feature"],
        )],
    );
    match branch_target(&fragment) {
        LinkEndpoint::Node {
            id: NodeId::Branch(branch),
        } => assert_eq!(branch.refname, "refs/heads/feature"),
        other => panic!("expected the local branch, got {other:?}"),
    }
}

#[test]
fn a_pr_from_my_fork_into_upstream_links_to_my_local_branch() {
    // The PR lives in `upstream`; its head is my fork, `origin`.
    let body = r#"[{"number": 9, "state": "OPEN", "headRefName": "fix",
        "headRepositoryOwner": {"login": "me"}, "headRepository": {"name": "repo"}}]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body),
        &[forge_repo(
            &[
                ("origin", "git@github.com:me/repo.git"),
                ("upstream", "git@github.com:octo/repo.git"),
            ],
            &["fix"],
        )],
    );
    assert!(
        fragment.candidate_links.iter().all(|link| matches!(
            link.target,
            LinkEndpoint::Node {
                id: NodeId::Branch(_)
            }
        )),
        "{:#?}",
        fragment.candidate_links
    );
}

#[test]
fn a_pr_from_a_strangers_fork_stays_unresolved_despite_a_same_named_branch() {
    let body = r#"[{"number": 3, "state": "OPEN", "headRefName": "main",
        "headRepositoryOwner": {"login": "stranger"}, "headRepository": {"name": "repo"}}]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body),
        &[forge_repo(
            &[("origin", "git@github.com:octo/repo.git")],
            &["main"],
        )],
    );
    match branch_target(&fragment) {
        LinkEndpoint::Unresolved { evidence } => {
            assert_eq!(evidence.native_id.as_deref(), Some("main"));
        }
        other @ LinkEndpoint::Node { .. } => panic!("expected an unresolved link, got {other:?}"),
    }
}

#[test]
fn an_unavailable_gh_is_reported_as_a_diagnostic() {
    let fragment = discover(
        FakeGh::unavailable(GhUnavailableReason::NotAuthenticated),
        &[forge_repo(
            &[("origin", "git@github.com:octo/repo.git")],
            &[],
        )],
    );
    assert!(fragment.nodes.is_empty());
    assert_eq!(
        fragment.diagnostics,
        [Diagnostic::ProviderUnavailable {
            provider: "github".to_string(),
            reason: "gh is not authenticated".to_string(),
        }]
    );
}

/// A runner that fails for one repo and answers for the rest.
struct OneRepoFails;

impl crate::discovery::forge::GhRunner for OneRepoFails {
    fn list_pull_requests(
        &self,
        repo: &str,
        _filter: &crate::discovery::forge::PrListFilter,
        _fields: &str,
    ) -> anyhow::Result<GhOutcome> {
        Ok(if repo.ends_with("/broken") {
            GhOutcome::Failed {
                code: Some(1),
                message: "HTTP 502".to_string(),
            }
        } else {
            GhOutcome::PullRequests(r#"[{"number": 5, "headRefName": "main"}]"#.to_string())
        })
    }

    fn view_pull_request(
        &self,
        _repo: &str,
        _number: u64,
        _fields: &str,
    ) -> anyhow::Result<GhOutcome> {
        Ok(GhOutcome::PullRequests("{}".to_string()))
    }
}

#[test]
fn a_failure_for_one_repo_keeps_the_others_prs() {
    let mut broken = forge_repo(&[("origin", "git@github.com:octo/broken.git")], &[]);
    broken.repo_id = RepoId::new("/workspace/broken/.git");
    let fragment = GitHubForgeProvider::with_runner(OneRepoFails)
        .discover(
            &[
                forge_repo(&[("origin", "git@github.com:octo/repo.git")], &[]),
                broken,
            ],
            &DiscoveryContext::default(),
        )
        .expect("discover");
    assert_eq!(pr_ids(&fragment), ["octo/repo#5"]);
    assert!(
        fragment.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            Diagnostic::ProviderUnavailable { reason, .. }
                if reason.contains("github.com/octo/broken") && reason.contains("HTTP 502")
        )),
        "{:?}",
        fragment.diagnostics
    );
}

/// A runner that records each `gh pr list` repo and, while `offline`
/// is set, reports the forge unreachable.
#[derive(Clone, Default)]
struct CountingGh {
    calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    offline: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CountingGh {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }

    fn go_offline(&self) {
        self.offline
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl crate::discovery::forge::GhRunner for CountingGh {
    fn list_pull_requests(
        &self,
        repo: &str,
        _filter: &crate::discovery::forge::PrListFilter,
        _fields: &str,
    ) -> anyhow::Result<GhOutcome> {
        self.calls.lock().expect("calls").push(repo.to_string());
        Ok(if self.offline.load(std::sync::atomic::Ordering::Relaxed) {
            GhOutcome::Unavailable(GhUnavailableReason::Offline)
        } else {
            GhOutcome::PullRequests(r#"[{"number": 5, "headRefName": "main"}]"#.to_string())
        })
    }

    fn view_pull_request(
        &self,
        _repo: &str,
        _number: u64,
        _fields: &str,
    ) -> anyhow::Result<GhOutcome> {
        Ok(GhOutcome::PullRequests("{}".to_string()))
    }
}

fn octo(name: &str) -> ForgeRepo {
    let mut repo = forge_repo(
        &[("origin", &format!("git@github.com:octo/{name}.git"))],
        &[],
    );
    repo.repo_id = RepoId::new(format!("/workspace/{name}/.git"));
    repo
}

#[test]
fn listings_are_reused_within_the_forge_interval() {
    let runner = CountingGh::default();
    let provider = GitHubForgeProvider::with_runner(runner.clone());
    let context = DiscoveryContext::default();
    for _ in 0..2 {
        let fragment = provider.discover(&[octo("a")], &context).expect("discover");
        assert_eq!(pr_ids(&fragment), ["octo/a#5"]);
    }
    // The open and the recent query, once.
    assert_eq!(runner.calls().len(), 2, "{:?}", runner.calls());
}

#[test]
fn a_new_remote_costs_only_its_own_queries() {
    let runner = CountingGh::default();
    let provider = GitHubForgeProvider::with_runner(runner.clone());
    let context = DiscoveryContext::default();
    provider.discover(&[octo("a")], &context).expect("a");
    let fragment = provider
        .discover(&[octo("a"), octo("b")], &context)
        .expect("a and b");
    assert_eq!(pr_ids(&fragment), ["octo/a#5", "octo/b#5"]);
    assert_eq!(
        runner.calls(),
        [
            "github.com/octo/a",
            "github.com/octo/a",
            "github.com/octo/b",
            "github.com/octo/b"
        ]
    );
}

#[test]
fn listings_past_the_forge_interval_or_cleared_are_fetched_again() {
    let runner = CountingGh::default();
    let provider = GitHubForgeProvider::with_runner(runner.clone());
    let expired = DiscoveryContext::default().with_forge_max_age(std::time::Duration::ZERO);
    provider.discover(&[octo("a")], &expired).expect("first");
    provider.discover(&[octo("a")], &expired).expect("expired");
    assert_eq!(runner.calls().len(), 4);

    let context = DiscoveryContext::default();
    provider.discover(&[octo("a")], &context).expect("cached");
    assert_eq!(runner.calls().len(), 6);
    context.caches().clear_forge();
    provider.discover(&[octo("a")], &context).expect("cleared");
    assert_eq!(runner.calls().len(), 8);
}

#[test]
fn a_failed_refetch_keeps_the_last_prs_and_reports_the_failure() {
    let runner = CountingGh::default();
    let provider = GitHubForgeProvider::with_runner(runner.clone());
    let context = DiscoveryContext::default().with_forge_max_age(std::time::Duration::ZERO);
    provider.discover(&[octo("a")], &context).expect("online");
    runner.go_offline();
    let fragment = provider.discover(&[octo("a")], &context).expect("offline");
    assert_eq!(pr_ids(&fragment), ["octo/a#5"]);
    assert_eq!(
        fragment.diagnostics,
        [Diagnostic::ProviderUnavailable {
            provider: "github".to_string(),
            reason: GhUnavailableReason::Offline.as_str().to_string(),
        }]
    );
}

#[test]
fn reused_prs_carry_the_time_the_forge_answered() {
    let runner = CountingGh::default();
    let context = DiscoveryContext::default();
    context.caches().forge.set(
        "github.com/octo/a".to_string(),
        crate::discovery::forge::RemoteListing {
            fetched: std::time::Instant::now(),
            epoch: 42,
            bodies: vec![r#"[{"number": 5, "headRefName": "main"}]"#.to_string()],
            failures: Vec::new(),
        },
    );
    let fragment = GitHubForgeProvider::with_runner(runner.clone())
        .discover(&[octo("a")], &context)
        .expect("discover");
    assert!(runner.calls().is_empty());
    let epochs: Vec<Option<i64>> = fragment
        .node_provenance
        .values()
        .map(|provenance| provenance.freshness_epoch)
        .collect();
    assert_eq!(epochs, [Some(42)]);
}

const ENTERPRISE_HOST: &str = "git.example.com";

fn enterprise_repo() -> ForgeRepo {
    forge_repo(
        &[
            ("origin", "git@git.example.com:me/repo.git"),
            ("upstream", "git@git.example.com:team/repo.git"),
        ],
        &["feature"],
    )
}

#[test]
fn a_signed_in_host_counts_as_github_whatever_its_name() {
    let url = "git@git.example.com:octo/repo.git";
    assert!(parse_github_remote(url).is_none());
    assert!(parse_github_remote_with(url, &GithubHosts::default()).is_none());

    let hosts = GithubHosts::with_signed_in(["Git.Example.com"]);
    assert_eq!(
        parse_github_remote_with(url, &hosts),
        Some((
            ENTERPRISE_HOST.to_string(),
            "octo".to_string(),
            "repo".to_string()
        ))
    );
    assert!(parse_github_remote_with("https://gitlab.com/octo/repo.git", &hosts).is_none());
}

#[test]
fn a_remote_on_a_signed_in_enterprise_host_is_queried_by_host() {
    let body = r#"[{"number": 2, "state": "MERGED", "headRefName": "main"}]"#;
    let runner = FakeGh::with_pull_requests(body).with_hosts(&[ENTERPRISE_HOST]);
    let fragment = discover(
        runner.clone(),
        &[forge_repo(
            &[("origin", "git@git.example.com:team/repo.git")],
            &[],
        )],
    );
    assert_eq!(
        runner.last_repo().as_deref(),
        Some("git.example.com/team/repo")
    );
    assert_eq!(pr_ids(&fragment), ["team/repo#2"]);
    let hosts: Vec<&str> = fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::ForgePr(pr) => Some(pr.host.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(hosts, [ENTERPRISE_HOST]);
}

#[test]
fn a_fork_on_an_enterprise_host_queries_origin_and_upstream() {
    let body = r#"[{"number": 3, "state": "OPEN", "headRefName": "feature",
                    "headRepositoryOwner": {"login": "me"},
                    "headRepository": {"name": "repo"}}]"#;
    let fragment = discover(
        FakeGh::with_pull_requests(body).with_hosts(&[ENTERPRISE_HOST]),
        &[enterprise_repo()],
    );
    let mut ids = pr_ids(&fragment);
    ids.sort();
    assert_eq!(ids, ["me/repo#3", "team/repo#3"]);
    // The PR from my fork into upstream links to my local branch.
    assert!(
        fragment
            .candidate_links
            .iter()
            .all(|link| matches!(link.target, LinkEndpoint::Node { .. })),
        "{:?}",
        fragment.candidate_links
    );
}

#[test]
fn a_host_gh_is_not_signed_in_to_is_left_alone_without_a_diagnostic() {
    let runner = FakeGh::with_pull_requests(r#"[{"number": 1, "headRefName": "main"}]"#)
        .with_hosts(&["github.com"]);
    let fragment = discover(runner.clone(), &[enterprise_repo()]);
    assert!(fragment.nodes.is_empty());
    assert!(fragment.diagnostics.is_empty());
    assert_eq!(runner.last_repo(), None, "gh is never asked");
}

#[test]
fn a_failed_host_lookup_falls_back_to_the_built_in_hosts() {
    // The fake reports no hosts, as a missing or old `gh` would.
    let runner = FakeGh::with_pull_requests(r#"[{"number": 1, "headRefName": "main"}]"#);
    let fragment = discover(runner.clone(), &[enterprise_repo(), octo("a")]);
    assert_eq!(pr_ids(&fragment), ["octo/a#1"]);
    assert_eq!(runner.host_lookups(), 1);
}

#[test]
fn hosts_are_not_looked_up_when_every_remote_is_built_in() {
    let runner = FakeGh::with_pull_requests("[]").with_hosts(&[ENTERPRISE_HOST]);
    discover(
        runner.clone(),
        &[
            octo("a"),
            forge_repo(&[("origin", "git@gitlab.com:o/r.git")], &[]),
        ],
    );
    // A GitLab remote has an unfamiliar host, so the lookup runs once;
    // a machine with only github.com remotes skips it.
    assert_eq!(runner.host_lookups(), 1);

    let runner = FakeGh::with_pull_requests("[]").with_hosts(&[ENTERPRISE_HOST]);
    discover(runner.clone(), &[octo("a"), octo("b")]);
    assert_eq!(runner.host_lookups(), 0);
}

#[test]
fn the_host_lookup_is_cached_until_a_hard_refresh() {
    let runner = FakeGh::with_pull_requests("[]").with_hosts(&[ENTERPRISE_HOST]);
    let provider = GitHubForgeProvider::with_runner(runner.clone());
    let context = DiscoveryContext::default();
    for _ in 0..3 {
        provider
            .discover(&[enterprise_repo()], &context)
            .expect("discover");
    }
    assert_eq!(runner.host_lookups(), 1);

    context.caches().clear_forge();
    provider
        .discover(&[enterprise_repo()], &context)
        .expect("after refresh");
    assert_eq!(runner.host_lookups(), 2);
}
