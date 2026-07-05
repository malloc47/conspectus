// Extracted from git.rs H-HYG-011 rolling wave via #[path = "git_tests.rs"] mod tests;
use std::fs;

use tempfile::TempDir;

use super::*;

#[test]
fn probe_returns_none_outside_git_repo() {
    let temp = TempDir::new().expect("temp dir");

    let result = GitProbe::new().probe(temp.path()).expect("probe succeeds");

    assert!(result.is_none());
}

#[test]
fn probe_reads_plain_repo_identity_branch_remote_and_upstream() {
    let fixture = GitFixture::init();
    fixture.git(&["checkout", "-b", "feature"]);
    fixture.git(&["remote", "add", "origin", "git@example.com:owner/repo.git"]);
    fixture.git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);
    fixture.git(&["branch", "--set-upstream-to", "origin/feature", "feature"]);

    let result = GitProbe::new()
        .probe(fixture.root())
        .expect("probe succeeds")
        .expect("git repo discovered");

    assert_eq!(
        result.worktree_root,
        fixture.root().canonicalize().expect("canonical root")
    );
    assert_eq!(result.branch_ref.as_deref(), Some("refs/heads/feature"));
    assert_eq!(result.upstream.as_deref(), Some("origin/feature"));
    assert_eq!(result.local_branches, vec!["feature", "main"]);
    assert_eq!(
        result.remotes,
        vec![GitRemote {
            name: "origin".to_string(),
            url: "git@example.com:owner/repo.git".to_string(),
        }]
    );
    assert!(!result.is_linked_worktree());
}

#[test]
fn probe_allows_detached_head_without_branch() {
    let fixture = GitFixture::init();
    let commit = fixture.git_stdout(&["rev-parse", "HEAD"]);
    fixture.git(&["checkout", "--detach", &commit]);

    let result = GitProbe::new()
        .probe(fixture.root())
        .expect("probe succeeds")
        .expect("git repo discovered");

    assert_eq!(result.branch_ref, None);
    assert_eq!(result.upstream, None);
    assert_eq!(result.local_branches, vec!["main"]);
}

#[test]
fn probe_reads_linked_worktree_metadata() {
    let fixture = GitFixture::init();
    let linked = fixture.parent().join("linked");
    fixture.git(&["worktree", "add", "-b", "linked-branch", path_str(&linked)]);

    let main = GitProbe::new()
        .probe(fixture.root())
        .expect("main probe succeeds")
        .expect("main repo discovered");
    let linked_result = GitProbe::new()
        .probe(&linked)
        .expect("linked probe succeeds")
        .expect("linked worktree discovered");

    assert_eq!(linked_result.common_dir, main.common_dir);
    assert_ne!(linked_result.git_dir, main.git_dir);
    assert_eq!(
        linked_result.branch_ref.as_deref(),
        Some("refs/heads/linked-branch")
    );
    assert!(linked_result.is_linked_worktree());
}

#[test]
fn fragment_maps_git_probe_to_repo_worktree_branch_and_links() {
    let probe = GitProbeResult {
        common_dir: PathBuf::from("/workspace/repo/.git"),
        worktree_root: PathBuf::from("/workspace/repo"),
        git_dir: PathBuf::from("/workspace/repo/.git"),
        branch_ref: Some("refs/heads/main".to_string()),
        upstream: Some("origin/main".to_string()),
        remotes: vec![GitRemote {
            name: "origin".to_string(),
            url: "git@example.com:owner/repo.git".to_string(),
        }],
        local_branches: vec!["feature".to_string(), "main".to_string()],
    };

    let fragment = fragment_from_probe(&probe);

    assert_eq!(fragment.nodes.len(), 4);
    assert_eq!(fragment.candidate_links.len(), 2);
}

struct GitFixture {
    temp: TempDir,
    root: PathBuf,
}

impl GitFixture {
    fn init() -> Self {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("create repo dir");
        let fixture = Self { temp, root };
        fixture.git(&["init", "--initial-branch", "main"]);
        fixture.git(&["config", "user.name", "Conspectus Test"]);
        fixture.git(&["config", "user.email", "conspectus@example.invalid"]);
        fs::write(fixture.root.join("README.md"), "fixture\n").expect("write fixture file");
        fixture.git(&["add", "README.md"]);
        fixture.git(&["commit", "-m", "initial"]);
        fixture
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn parent(&self) -> &Path {
        self.temp.path()
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("run git command");

        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            trim_output(output.stderr)
        );
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .expect("run git command");

        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            trim_output(output.stderr)
        );
        trim_output(output.stdout)
    }
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}
