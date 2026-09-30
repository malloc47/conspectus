// Extracted from github.rs H-HYG-011 rolling wave via #[path = "github_tests.rs"] mod tests;
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

    let fragment = fragment_for_repo(&context, &branches, &records);

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

    let fragment = fragment_for_repo(&context, &branches, &records);

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

    let fragment = fragment_for_repo(&context, &branches, &records);

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
fn draft_flag_is_carried_onto_node_and_link_metadata() {
    let context = context();
    let branches = BTreeSet::new();
    let mut draft = record(3, "wip", PullRequestState::Open);
    draft.is_draft = true;

    let fragment = fragment_for_repo(&context, &branches, &[draft]);

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

    let fragment = fragment_for_repo(&context, &branches, &records);

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

    let first = fragment_for_repo(&context, &branches, &records);
    let second = fragment_for_repo(&context, &branches, &records);

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

    let fragment = fragment_for_repo(&context, &branches, &[rec]);

    let pr = match &fragment.nodes[0] {
        GraphNode::ForgePr(pr) => pr,
        other => panic!("expected ForgePr, got {other:?}"),
    };
    assert_eq!(pr.updated_epoch, Some(1_772_714_096));
}

#[test]
fn empty_records_yield_empty_fragment() {
    let fragment = fragment_for_repo(&context(), &BTreeSet::new(), &[]);

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

use crate::discovery::forge::{FakeGh, GhUnavailableReason};
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

fn make_git_repo(temp: &TempDir, remote: &str) -> std::path::PathBuf {
    let root = temp.path().join("repo");
    std::fs::create_dir(&root).expect("create repo dir");
    run_git(&root, &["init", "--initial-branch", "main"]);
    run_git(&root, &["config", "user.name", "Conspectus Test"]);
    run_git(&root, &["config", "user.email", "test@example.invalid"]);
    run_git(&root, &["remote", "add", "origin", remote]);
    std::fs::write(root.join("README.md"), "fixture\n").expect("write fixture");
    run_git(&root, &["add", "README.md"]);
    run_git(&root, &["commit", "-m", "initial"]);
    root
}

fn run_git(root: &std::path::Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn provider_emits_no_rows_when_repo_has_no_github_remote() {
    let temp = TempDir::new().expect("temp dir");
    let root = make_git_repo(&temp, "git@gitlab.com:octo/repo.git");
    let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests("[]"));

    let context = DiscoveryContext::from_roots([root]).expect("context");
    let fragment = provider.discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn provider_emits_no_rows_when_gh_unavailable() {
    let temp = TempDir::new().expect("temp dir");
    let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
    let provider = GitHubForgeProvider::with_runner(FakeGh::unavailable(
        GhUnavailableReason::NotAuthenticated,
    ));

    let context = DiscoveryContext::from_roots([root]).expect("context");
    let fragment = provider.discover(&context).expect("discover");

    assert!(fragment.nodes.is_empty());
}

#[test]
fn provider_emits_forge_pr_nodes_for_github_repo() {
    let temp = TempDir::new().expect("temp dir");
    let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
    let body = r#"[
            {"number": 1, "state": "OPEN", "headRefName": "main", "isDraft": false},
            {"number": 2, "state": "MERGED", "headRefName": "feature", "isDraft": false}
        ]"#;
    let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

    let context = DiscoveryContext::from_roots([root]).expect("context");
    let fragment = provider.discover(&context).expect("discover");

    assert_eq!(
        fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::ForgePr(pr) => Some(pr),
                _ => None,
            })
            .count(),
        2
    );
    assert_eq!(fragment.candidate_links.len(), 2);
}

#[test]
fn provider_emits_branch_node_endpoint_when_head_ref_matches_current_branch() {
    let temp = TempDir::new().expect("temp dir");
    let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
    let body = r#"[{"number": 1, "state": "OPEN", "headRefName": "main"}]"#;
    let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

    let context = DiscoveryContext::from_roots([root]).expect("context");
    let fragment = provider.discover(&context).expect("discover");

    let link = &fragment.candidate_links[0];
    match &link.target {
        LinkEndpoint::Node {
            id: NodeId::Branch(_),
        } => {}
        other => panic!("expected branch node endpoint, got {other:?}"),
    }
}

#[test]
fn provider_emits_branch_node_endpoint_when_head_ref_matches_sibling_local_branch() {
    let temp = TempDir::new().expect("temp dir");
    let root = make_git_repo(&temp, "git@github.com:octo/repo.git");
    run_git(&root, &["branch", "feature/sibling"]);
    let body = r#"[{"number": 2, "state": "OPEN", "headRefName": "feature/sibling"}]"#;
    let provider = GitHubForgeProvider::with_runner(FakeGh::with_pull_requests(body));

    let context = DiscoveryContext::from_roots([root]).expect("context");
    let fragment = provider.discover(&context).expect("discover");

    let link = &fragment.candidate_links[0];
    match &link.target {
        LinkEndpoint::Node {
            id: NodeId::Branch(branch),
        } => assert_eq!(branch.refname, "refs/heads/feature/sibling"),
        other => panic!("expected sibling branch node endpoint, got {other:?}"),
    }
}
