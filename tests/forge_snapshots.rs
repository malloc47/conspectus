//! End-to-end JSON snapshots for forge discovery and table-projection
//! renderers. Each test sets up a git repo, injects a `FakeGh` runner
//! with a fixed `gh pr list --json` body, and snapshots the rendered
//! output after path normalization so reruns are byte-stable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use conspectus::config::Projection;
use conspectus::discovery::forge::FakeGh;
use conspectus::discovery::{LocalDiscoveryConfig, discover_local_with};
use conspectus::output::{render_graph_json, table};
use conspectus::resolve::resolve_snapshot;

#[test]
fn forge_zero_pull_requests_json_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests("[]"));
    let rendered = render_json(&fixture, config);

    insta::assert_snapshot!("forge_zero_pull_requests_json", rendered);
}

#[test]
fn forge_single_open_pull_request_json_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");
    let body = r#"[{
        "number": 7,
        "state": "OPEN",
        "url": "https://github.com/octo/repo/pull/7",
        "headRefName": "main",
        "baseRefName": "main",
        "updatedAt": "2026-03-05T12:34:56Z",
        "headRepositoryOwner": {"login": "octo"},
        "headRepository": {"name": "repo"},
        "isDraft": false
    }]"#;

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));
    let rendered = render_json(&fixture, config);

    insta::assert_snapshot!("forge_single_open_pull_request_json", rendered);
}

#[test]
fn forge_multiple_pull_requests_json_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");
    let body = r#"[
        {"number": 1, "state": "OPEN",   "headRefName": "main",    "updatedAt": "2026-03-05T12:00:00Z"},
        {"number": 2, "state": "MERGED", "headRefName": "feature", "updatedAt": "2026-03-04T12:00:00Z"},
        {"number": 3, "state": "CLOSED", "headRefName": "stale",   "updatedAt": "2026-03-03T12:00:00Z"}
    ]"#;

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));
    let rendered = render_json(&fixture, config);

    insta::assert_snapshot!("forge_multiple_pull_requests_json", rendered);
}

#[test]
fn session_table_agent_projection_with_pr_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");
    let body = r#"[{"number": 7, "state": "OPEN", "headRefName": "main", "isDraft": false}]"#;

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));
    let rendered = render_table(&fixture, config, Projection::Agent);

    insta::assert_snapshot!("session_table_agent_projection_with_pr", rendered);
}

#[test]
fn session_table_mux_projection_empty_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");
    let body = r#"[]"#;

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));
    let rendered = render_table(&fixture, config, Projection::Mux);

    insta::assert_snapshot!("session_table_mux_projection_empty", rendered);
}

#[test]
fn session_table_union_projection_with_pr_snapshot() {
    let fixture = RepoFixture::new("git@github.com:octo/repo.git");
    let body = r#"[{"number": 7, "state": "OPEN", "headRefName": "main"}]"#;

    let config = LocalDiscoveryConfig::empty().with_forge_runner(FakeGh::with_pull_requests(body));
    let rendered = render_table(&fixture, config, Projection::Union);

    insta::assert_snapshot!("session_table_union_projection_with_pr", rendered);
}

fn render_json(fixture: &RepoFixture, config: LocalDiscoveryConfig) -> String {
    let snapshot = discover_local_with([fixture.path()], config).expect("discover");
    let rendered = render_graph_json(&resolve_snapshot(snapshot)).expect("render json");
    fixture.normalize(&rendered)
}

fn render_table(
    fixture: &RepoFixture,
    config: LocalDiscoveryConfig,
    projection: Projection,
) -> String {
    let snapshot = discover_local_with([fixture.path()], config).expect("discover");
    let snapshot = resolve_snapshot(snapshot);
    let rendered = table::render(&snapshot, projection);
    fixture.normalize(&rendered)
}

struct RepoFixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl RepoFixture {
    fn new(remote: &str) -> Self {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("create repo dir");
        let fixture = Self { temp, root };
        fixture.git(&["init", "--initial-branch", "main"]);
        fixture.git(&["config", "user.name", "Conspectus Test"]);
        fixture.git(&["config", "user.email", "conspectus@example.invalid"]);
        fixture.git(&["remote", "add", "origin", remote]);
        fs::write(fixture.root.join("README.md"), "fixture\n").expect("write fixture");
        fixture.git(&["add", "README.md"]);
        fixture.git(&["commit", "-m", "initial"]);
        fixture
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn normalize(&self, rendered: &str) -> String {
        let canonical = self
            .temp
            .path()
            .canonicalize()
            .unwrap_or_else(|_| self.temp.path().to_path_buf());
        let mut out = rendered.replace(&canonical.to_string_lossy().to_string(), "/fixture");
        out = out.replace(&self.temp.path().to_string_lossy().to_string(), "/fixture");
        if let Some(name) = self.temp.path().file_name().and_then(|s| s.to_str()) {
            out = out.replace(name, "fixture");
        }
        out
    }
}
