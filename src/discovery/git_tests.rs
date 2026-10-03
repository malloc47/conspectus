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
fn probe_returns_none_for_a_non_directory_path() {
    // Probing a regular file must not spawn `git` with a
    // non-directory `current_dir` (which fails with ENOTDIR / "Not a
    // directory", os error 20). It reports no repo instead of a hard
    // error so a single bad path can't fail the whole discovery cycle.
    let temp = TempDir::new().expect("temp dir");
    let file = temp.path().join("plain.txt");
    fs::write(&file, b"contents").expect("write file");

    let result = GitProbe::new().probe(&file).expect("probe succeeds");

    assert!(result.is_none());
}

#[test]
fn probe_returns_none_for_a_symlink_to_a_file() {
    // `GenericWorkspaceDiscovery` reaches `probe` with symlink children
    // of a non-git scan root. A symlink that resolves to a file passes
    // its `is_symlink()` filter but is not a directory, so the probe
    // must degrade to `None` rather than surfacing the ENOTDIR spawn
    // failure that previously broke `conspectus serve` from a non-git
    // directory.
    let temp = TempDir::new().expect("temp dir");
    let file = temp.path().join("target.txt");
    fs::write(&file, b"contents").expect("write file");
    let link = temp.path().join("link-to-file");
    std::os::unix::fs::symlink(&file, &link).expect("symlink");

    let result = GitProbe::new().probe(&link).expect("probe succeeds");

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

// ---------------------------------------------------------------------------
// Probe cache short-circuits repeat probes on unchanged
// repos. observed_cwd_git_fragment calls probe() once per unique session
// cwd on every discovery cycle; without the cache each of those spawns
// 8-15 git subprocesses. The tests here pin that the second call on an
// unchanged repo is a cache hit (no re-probe, so no new cache entry) and
// that mutations (branch checkout, remote add) invalidate the cache.
// ---------------------------------------------------------------------------

#[test]
fn probe_cache_serves_second_call_without_spawning_git_on_unchanged_repo() {
    let caches = DiscoveryCaches::default();

    let fixture = GitFixture::init();
    let probe = GitProbe::new();

    // First call: cold, must spawn multiple git subprocesses.
    let first = probe
        .probe_cached(fixture.root(), &caches)
        .expect("first probe")
        .expect("repo");
    assert_eq!(caches.git_probes.take_inserts(), 1, "cold probe must miss");

    // Second call: cache hit with an identical result.
    let second = probe
        .probe_cached(fixture.root(), &caches)
        .expect("second probe")
        .expect("repo");
    assert_eq!(
        caches.git_probes.take_inserts(),
        0,
        "the second probe must be a cache hit"
    );
    assert_eq!(first, second);
}

#[test]
fn probe_cache_invalidates_on_branch_checkout() {
    // `git checkout` updates .git/HEAD's mtime, which the
    // fingerprint keys on. The second probe must observe the new
    // branch_ref rather than the cached one from before checkout.
    let caches = DiscoveryCaches::default();

    let fixture = GitFixture::init();
    let probe = GitProbe::new();
    let cached = probe
        .probe_cached(fixture.root(), &caches)
        .expect("initial")
        .expect("repo");
    assert_eq!(cached.branch_ref.as_deref(), Some("refs/heads/main"));
    let _ = caches.git_probes.take_inserts();

    fixture.git(&["checkout", "-b", "feature"]);

    let refreshed = probe
        .probe_cached(fixture.root(), &caches)
        .expect("post-checkout")
        .expect("repo");
    assert_eq!(
        caches.git_probes.take_inserts(),
        1,
        "branch checkout must invalidate the cache and force a re-probe"
    );
    assert_eq!(refreshed.branch_ref.as_deref(), Some("refs/heads/feature"));
}

#[test]
fn probe_cache_invalidates_on_remote_add() {
    // `git remote add` writes to .git/config; the config-mtime
    // fingerprint entry catches it. Second probe must observe the
    // new remote instead of returning the cached (empty) list.
    let caches = DiscoveryCaches::default();

    let fixture = GitFixture::init();
    let probe = GitProbe::new();
    let cached = probe
        .probe_cached(fixture.root(), &caches)
        .expect("initial")
        .expect("repo");
    assert!(cached.remotes.is_empty());
    let _ = caches.git_probes.take_inserts();

    fixture.git(&["remote", "add", "origin", "git@example.com:owner/repo.git"]);

    let refreshed = probe
        .probe_cached(fixture.root(), &caches)
        .expect("post-remote-add")
        .expect("repo");
    assert_eq!(
        caches.git_probes.take_inserts(),
        1,
        "remote add should invalidate the cache"
    );
    assert_eq!(refreshed.remotes.len(), 1);
    assert_eq!(refreshed.remotes[0].name, "origin");
}

#[test]
fn probe_cache_serves_none_for_non_repo_without_respawning() {
    // Non-repo cache path: fingerprint records `None` for every
    // .git file + the root's mtime. Second probe on the same
    // non-repo path must be served from the cache.
    let caches = DiscoveryCaches::default();

    let temp = TempDir::new().expect("temp");
    let probe = GitProbe::new();

    let first = probe
        .probe_cached(temp.path(), &caches)
        .expect("first probe");
    assert!(first.is_none());
    assert_eq!(
        caches.git_probes.take_inserts(),
        1,
        "cold non-repo probe must miss"
    );

    let second = probe
        .probe_cached(temp.path(), &caches)
        .expect("second probe");
    assert!(second.is_none());
    assert_eq!(
        caches.git_probes.take_inserts(),
        0,
        "the cached non-repo entry must serve the second call"
    );
}

fn wt_record(path: &str, branch: Option<&str>, kind: crate::model::WorktreeKind) -> WorktreeRecord {
    WorktreeRecord {
        path: path.to_string(),
        branch: branch.map(str::to_string),
        head: Some("abc123".to_string()),
        kind,
        locked: None,
        prunable: None,
        bare: false,
        detached: false,
    }
}

#[test]
fn sibling_worktree_fragment_skips_current_and_carries_metadata() {
    use crate::model::WorktreeKind;
    let probe = GitProbeResult {
        common_dir: PathBuf::from("/repo/.git"),
        worktree_root: PathBuf::from("/repo"),
        git_dir: PathBuf::from("/repo/.git"),
        branch_ref: Some("refs/heads/main".to_string()),
        upstream: None,
        remotes: Vec::new(),
        local_branches: Vec::new(),
    };
    let mut locked_sibling = wt_record(
        "/wt/bugfix",
        Some("refs/heads/bugfix"),
        WorktreeKind::Linked,
    );
    locked_sibling.locked = Some("agent running".to_string());
    let records = vec![
        // The current checkout — must be skipped (the probe owns it).
        wt_record("/repo", Some("refs/heads/main"), WorktreeKind::Primary),
        wt_record(
            "/wt/feature",
            Some("refs/heads/feature"),
            WorktreeKind::Linked,
        ),
        locked_sibling,
    ];

    let fragment = sibling_worktree_fragment(&probe, &records).expect("two siblings");
    let checkouts: Vec<&CheckoutNode> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::Checkout(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(checkouts.len(), 2, "current checkout skipped");
    assert!(
        !checkouts.iter().any(|c| c.root == "/repo"),
        "the probed checkout must not be re-emitted",
    );
    let bugfix = checkouts
        .iter()
        .find(|c| c.root == "/wt/bugfix")
        .expect("bugfix sibling");
    let meta = bugfix.worktree.as_ref().expect("worktree meta");
    assert_eq!(meta.kind, WorktreeKind::Linked);
    assert_eq!(meta.locked.as_deref(), Some("agent running"));
    // Each sibling links back to the repo.
    assert_eq!(fragment.candidate_links.len(), 2);
}

#[test]
fn sibling_worktree_fragment_is_none_without_siblings() {
    let probe = GitProbeResult {
        common_dir: PathBuf::from("/repo/.git"),
        worktree_root: PathBuf::from("/repo"),
        git_dir: PathBuf::from("/repo/.git"),
        branch_ref: Some("refs/heads/main".to_string()),
        upstream: None,
        remotes: Vec::new(),
        local_branches: Vec::new(),
    };
    // Only the current worktree — nothing to add.
    let records = vec![wt_record(
        "/repo",
        Some("refs/heads/main"),
        crate::model::WorktreeKind::Primary,
    )];
    assert!(sibling_worktree_fragment(&probe, &records).is_none());
}

#[test]
fn checkout_node_marks_primary_and_linked_from_probe() {
    use crate::model::WorktreeKind;
    // common_dir == git_dir → primary working tree.
    let primary = GitProbeResult {
        common_dir: PathBuf::from("/repo/.git"),
        worktree_root: PathBuf::from("/repo"),
        git_dir: PathBuf::from("/repo/.git"),
        branch_ref: None,
        upstream: None,
        remotes: Vec::new(),
        local_branches: Vec::new(),
    };
    let node = checkout_node(
        CheckoutId::new(RepoId::new("/repo/.git"), "/repo"),
        &primary,
    );
    assert_eq!(node.worktree.unwrap().kind, WorktreeKind::Primary);

    // common_dir != git_dir → linked worktree.
    let linked = GitProbeResult {
        git_dir: PathBuf::from("/repo/.git/worktrees/feature"),
        ..primary
    };
    let node = checkout_node(
        CheckoutId::new(RepoId::new("/repo/.git"), "/wt/feature"),
        &linked,
    );
    assert_eq!(node.worktree.unwrap().kind, WorktreeKind::Linked);
}

#[test]
fn git_discovery_enumerates_sibling_worktrees_end_to_end() {
    // Real git: a repo + a linked worktree, discovered through
    // GitDiscovery with the real backend. Skips cleanly if git can't
    // add a worktree on this host.
    let fixture = GitFixture::init();
    let wt = fixture.parent().join("wt-feature");
    let added = Command::new("git")
        .args(["worktree", "add", "-b", "feature", wt.to_str().unwrap()])
        .current_dir(fixture.root())
        .status()
        .is_ok_and(|s| s.success());
    if !added {
        return;
    }

    let discovery = GitDiscovery::new().with_worktree_backend(Box::new(
        crate::discovery::worktree::SystemGitWorktree::new(),
    ));
    let context = DiscoveryContext::from_root(fixture.root());
    let fragment = discovery.discover(&context).expect("discover");

    let worktree_checkouts: Vec<&CheckoutNode> = fragment
        .nodes
        .iter()
        .filter_map(|n| match n {
            GraphNode::Checkout(c) => Some(c),
            _ => None,
        })
        .filter(|c| c.worktree.is_some())
        .collect();
    // Both the primary (from the probe) and the linked worktree (from
    // enumeration) carry worktree metadata.
    assert!(
        worktree_checkouts
            .iter()
            .any(|c| c.worktree.as_ref().unwrap().kind == crate::model::WorktreeKind::Primary),
        "primary checkout marked",
    );
    assert!(
        worktree_checkouts
            .iter()
            .any(|c| c.worktree.as_ref().unwrap().is_linked()),
        "linked worktree enumerated and marked: {worktree_checkouts:?}",
    );
}

/// A directory with every permission removed for the life of the
/// guard. `None` when the process can still enter it (e.g. running as
/// root), in which case the caller can't exercise the denial.
struct LockedDir(std::path::PathBuf);

impl LockedDir {
    fn new(path: std::path::PathBuf) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir(&path).expect("create dir");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("lock dir");
        let locked = Self(path);
        locked.0.join(".").metadata().is_err().then_some(locked)
    }
}

impl Drop for LockedDir {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn probe_returns_none_for_a_directory_it_cannot_enter() {
    let temp = TempDir::new().expect("temp");
    let Some(locked) = LockedDir::new(temp.path().join("private")) else {
        return;
    };
    let caches = DiscoveryCaches::default();

    assert!(GitProbe::new().probe(&locked.0).expect("probe").is_none());
    assert!(
        GitProbe::new()
            .probe_cached(&locked.0, &caches)
            .expect("cached probe")
            .is_none()
    );
}
