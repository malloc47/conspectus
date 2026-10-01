use super::*;

#[test]
fn parses_empty_output_as_no_worktrees() {
    assert!(parse_worktree_porcelain("").is_empty());
    assert!(parse_worktree_porcelain("\n\n").is_empty());
}

#[test]
fn first_worktree_is_primary_rest_are_linked() {
    let stdout = "\
worktree /src/repo
HEAD aaaa
branch refs/heads/main

worktree /src/wt/feature
HEAD bbbb
branch refs/heads/feature

worktree /src/wt/bugfix
HEAD cccc
branch refs/heads/bugfix
";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 3);

    assert_eq!(records[0].path, "/src/repo");
    assert_eq!(records[0].kind, WorktreeKind::Primary);
    assert_eq!(records[0].branch.as_deref(), Some("refs/heads/main"));
    assert_eq!(records[0].head.as_deref(), Some("aaaa"));

    assert_eq!(records[1].kind, WorktreeKind::Linked);
    assert_eq!(records[1].branch.as_deref(), Some("refs/heads/feature"));
    assert_eq!(records[2].kind, WorktreeKind::Linked);
    assert_eq!(records[2].path, "/src/wt/bugfix");
}

#[test]
fn parses_detached_head_worktree() {
    let stdout = "\
worktree /src/repo
HEAD aaaa
branch refs/heads/main

worktree /src/wt/detached
HEAD deadbeef
detached
";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 2);
    assert!(records[1].detached);
    assert_eq!(records[1].branch, None);
    assert_eq!(records[1].head.as_deref(), Some("deadbeef"));
}

#[test]
fn parses_bare_main_worktree() {
    let stdout = "\
worktree /src/repo.git
bare

worktree /src/wt/feature
HEAD bbbb
branch refs/heads/feature
";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 2);
    assert!(records[0].bare);
    assert_eq!(records[0].kind, WorktreeKind::Primary);
    assert_eq!(records[0].branch, None);
    assert!(!records[1].bare);
}

#[test]
fn parses_locked_with_and_without_reason() {
    let stdout = "\
worktree /src/repo
HEAD aaaa
branch refs/heads/main

worktree /src/wt/locked-bare
HEAD bbbb
branch refs/heads/x
locked

worktree /src/wt/locked-reason
HEAD cccc
branch refs/heads/y
locked \"agent is running here\"
";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].locked, None);
    assert_eq!(records[1].locked.as_deref(), Some(""));
    assert_eq!(records[2].locked.as_deref(), Some("agent is running here"));
}

#[test]
fn parses_prunable_reason() {
    let stdout = "\
worktree /src/repo
HEAD aaaa
branch refs/heads/main

worktree /src/wt/gone
HEAD bbbb
branch refs/heads/z
prunable gitdir file points to non-existent location
";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[1].prunable.as_deref(),
        Some("gitdir file points to non-existent location"),
    );
}

#[test]
fn tolerates_trailing_record_without_blank_line() {
    let stdout = "worktree /src/repo\nHEAD aaaa\nbranch refs/heads/main";
    let records = parse_worktree_porcelain(stdout);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, WorktreeKind::Primary);
    assert_eq!(records[0].branch.as_deref(), Some("refs/heads/main"));
}

#[test]
fn git_backend_reports_no_mutation_capabilities() {
    let backend = SystemGitWorktree::new();
    assert_eq!(backend.backend_key(), GIT_BACKEND);
    let caps = backend.capabilities();
    assert!(!caps.can_create);
    assert!(!caps.can_remove);
}

#[test]
fn git_backend_missing_binary_degrades_to_empty() {
    let backend = SystemGitWorktree::with_binary("/definitely/not/here/git");
    let records = backend
        .list(std::path::Path::new("/tmp"))
        .expect("missing binary is non-fatal");
    assert!(records.is_empty());
}

#[test]
fn git_backend_lists_real_worktrees_when_git_available() {
    // Integration-ish: build a real repo + a linked worktree in a
    // tempdir and confirm the backend enumerates both. Skips cleanly
    // when git is not installed on the host.
    use std::process::Command;
    let tmp = tempfile::TempDir::new().expect("temp dir");
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).expect("mkdir repo");

    let run = |args: &[&str], cwd: &std::path::Path| -> Option<bool> {
        let status = Command::new("git").args(args).current_dir(cwd).status();
        match status {
            Ok(s) => Some(s.success()),
            Err(_) => None, // git not installed
        }
    };

    // `git init` — bail out of the test body if git is absent.
    match run(&["init", "-q", "-b", "main"], &repo) {
        None => return, // no git on host; nothing to assert
        Some(false) => return,
        Some(true) => {}
    }
    let _ = run(&["config", "user.email", "t@example.com"], &repo);
    let _ = run(&["config", "user.name", "t"], &repo);
    std::fs::write(repo.join("f"), "x").expect("write file");
    let _ = run(&["add", "-A"], &repo);
    let _ = run(&["commit", "-qm", "init"], &repo);

    let wt = tmp.path().join("wt-feature");
    let added = run(
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            wt.to_str().unwrap(),
        ],
        &repo,
    );
    if added != Some(true) {
        return; // worktree add unsupported/failed on this host
    }

    let backend = SystemGitWorktree::new();
    let records = backend.list(&repo).expect("list worktrees");
    assert_eq!(records.len(), 2, "primary + one linked worktree");
    assert_eq!(records[0].kind, WorktreeKind::Primary);
    assert!(records.iter().any(
        |r| r.kind == WorktreeKind::Linked && r.branch.as_deref() == Some("refs/heads/feature")
    ));
}

// ---- Worktrunk mutation backend ----

use std::sync::Mutex;

/// Records the args each `wt` invocation received and returns a
/// canned exit status, so create/remove argv is asserted without a
/// real `wt` binary.
struct FakeWt {
    calls: Mutex<Vec<Vec<String>>>,
    exit_code: i32,
    stderr: String,
}

impl FakeWt {
    fn ok() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            exit_code: 0,
            stderr: String::new(),
        }
    }

    fn failing(code: i32, stderr: &str) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            exit_code: code,
            stderr: stderr.to_string(),
        }
    }
}

impl WtRunner for FakeWt {
    fn run(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(std::string::ToString::to_string).collect());
        use std::os::unix::process::ExitStatusExt;
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(self.exit_code << 8),
            stdout: Vec::new(),
            stderr: self.stderr.clone().into_bytes(),
        })
    }
}

// A shared-state fake needs Arc so both the backend and the test can
// read `calls`. WtRunner is impl'd for Arc<FakeWt> via deref.
impl WtRunner for std::sync::Arc<FakeWt> {
    fn run(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        (**self).run(args)
    }
}

#[test]
fn worktrunk_reports_mutation_capabilities() {
    let backend = WorktrunkBackend::new();
    assert_eq!(backend.backend_key(), WORKTRUNK_BACKEND);
    let caps = backend.capabilities();
    assert!(caps.can_create);
    assert!(caps.can_remove);
}

#[test]
fn worktrunk_create_builds_switch_create_no_cd_argv() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    let outcome = backend
        .create(&WorktreeCreateRequest {
            repo_root: PathBuf::from("/src/app"),
            branch: "feature-a".to_string(),
            base: Some("main".to_string()),
        })
        .expect("create runs");
    assert_eq!(outcome, WorktreeMutationOutcome::Succeeded { path: None });

    let calls = fake.calls.lock().unwrap();
    assert_eq!(
        calls[0],
        vec![
            "-C",
            "/src/app",
            "switch",
            "--create",
            "--no-cd",
            "--base",
            "main",
            "feature-a",
        ],
    );
}

#[test]
fn worktrunk_create_omits_base_when_absent() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    backend
        .create(&WorktreeCreateRequest {
            repo_root: PathBuf::from("/src/app"),
            branch: "feature-a".to_string(),
            base: None,
        })
        .expect("create runs");
    let calls = fake.calls.lock().unwrap();
    assert!(!calls[0].iter().any(|a| a == "--base"));
    assert_eq!(calls[0].last().map(String::as_str), Some("feature-a"));
}

#[test]
fn worktrunk_remove_builds_yes_foreground_argv_and_force() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    backend
        .remove(&WorktreeRemoveRequest {
            repo_root: PathBuf::from("/src/app"),
            branch: "feature-a".to_string(),
            force: true,
        })
        .expect("remove runs");
    let calls = fake.calls.lock().unwrap();
    assert_eq!(
        calls[0],
        vec![
            "-C",
            "/src/app",
            "remove",
            "--yes",
            "--foreground",
            "--force",
            "feature-a",
        ],
    );
}

#[test]
fn worktrunk_remove_without_force_omits_force_flag() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    backend
        .remove(&WorktreeRemoveRequest {
            repo_root: PathBuf::from("/src/app"),
            branch: "feature-a".to_string(),
            force: false,
        })
        .expect("remove runs");
    let calls = fake.calls.lock().unwrap();
    assert!(!calls[0].iter().any(|a| a == "--force"));
}

#[test]
fn worktrunk_surfaces_command_failure() {
    let fake = std::sync::Arc::new(FakeWt::failing(1, "branch already exists"));
    let backend = WorktrunkBackend::with_runner(Box::new(fake));
    let outcome = backend
        .create(&WorktreeCreateRequest {
            repo_root: PathBuf::from("/src/app"),
            branch: "dupe".to_string(),
            base: None,
        })
        .expect("create runs");
    assert_eq!(
        outcome,
        WorktreeMutationOutcome::Failed {
            code: Some(1),
            message: "branch already exists".to_string(),
        },
    );
}

#[test]
fn git_backend_create_and_remove_are_unsupported() {
    // The built-in git backend never mutates (ADR 0087 prohibition 6).
    let backend = SystemGitWorktree::new();
    assert_eq!(
        backend
            .create(&WorktreeCreateRequest {
                repo_root: PathBuf::from("/src/app"),
                branch: "x".to_string(),
                base: None,
            })
            .unwrap(),
        WorktreeMutationOutcome::Unsupported,
    );
    assert_eq!(
        backend
            .remove(&WorktreeRemoveRequest {
                repo_root: PathBuf::from("/src/app"),
                branch: "x".to_string(),
                force: false,
            })
            .unwrap(),
        WorktreeMutationOutcome::Unsupported,
    );
}

// ---- Mutation backend resolver ----

#[test]
fn resolver_git_selection_is_always_read_only() {
    for available in [true, false] {
        let backend =
            resolve_mutation_backend(WorktreeBackendSelection::Git, available).expect("ok");
        assert!(backend.is_none(), "git selection never mutates");
    }
}

#[test]
fn resolver_auto_uses_worktrunk_only_when_available() {
    let present = resolve_mutation_backend(WorktreeBackendSelection::Auto, true).expect("ok");
    assert_eq!(present.unwrap().backend_key(), WORKTRUNK_BACKEND);

    let absent = resolve_mutation_backend(WorktreeBackendSelection::Auto, false).expect("ok");
    assert!(absent.is_none(), "auto stays read-only without wt");
}

#[test]
fn resolver_worktrunk_requires_wt_present() {
    let present = resolve_mutation_backend(WorktreeBackendSelection::Worktrunk, true).expect("ok");
    assert_eq!(present.unwrap().backend_key(), WORKTRUNK_BACKEND);

    let Err(err) = resolve_mutation_backend(WorktreeBackendSelection::Worktrunk, false) else {
        panic!("worktrunk selection must require wt on PATH")
    };
    assert!(err.to_string().contains("not on PATH"), "{err}");
}

#[test]
fn backend_selection_parse_round_trips() {
    for sel in [
        WorktreeBackendSelection::Auto,
        WorktreeBackendSelection::Git,
        WorktreeBackendSelection::Worktrunk,
    ] {
        assert_eq!(WorktreeBackendSelection::parse(sel.as_str()), Some(sel));
    }
    assert_eq!(WorktreeBackendSelection::parse("nonsense"), None);
    assert_eq!(
        WorktreeBackendSelection::default(),
        WorktreeBackendSelection::Auto
    );
}

// ---- Merge ----

#[test]
fn worktrunk_reports_merge_capability() {
    assert!(WorktrunkBackend::new().capabilities().can_merge);
    assert!(!SystemGitWorktree::new().capabilities().can_merge);
}

#[test]
fn worktrunk_merge_runs_in_worktree_with_optional_target() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    backend
        .merge(&WorktreeMergeRequest {
            worktree_root: PathBuf::from("/wt/feature"),
            target: Some("main".to_string()),
        })
        .expect("merge runs");
    backend
        .merge(&WorktreeMergeRequest {
            worktree_root: PathBuf::from("/wt/feature"),
            target: None,
        })
        .expect("merge runs");

    let calls = fake.calls.lock().unwrap();
    assert_eq!(calls[0], vec!["-C", "/wt/feature", "merge", "main"]);
    assert_eq!(calls[1], vec!["-C", "/wt/feature", "merge"]);
}

#[test]
fn git_backend_merge_is_unsupported() {
    assert_eq!(
        SystemGitWorktree::new()
            .merge(&WorktreeMergeRequest {
                worktree_root: PathBuf::from("/wt/feature"),
                target: None,
            })
            .unwrap(),
        WorktreeMutationOutcome::Unsupported,
    );
}

#[test]
fn worktrunk_reports_prune_capability() {
    assert!(WorktrunkBackend::new().capabilities().can_prune);
    assert!(!SystemGitWorktree::new().capabilities().can_prune);
}

#[test]
fn worktrunk_prune_runs_step_prune_with_yes_or_dry_run() {
    let fake = std::sync::Arc::new(FakeWt::ok());
    let backend = WorktrunkBackend::with_runner(Box::new(fake.clone()));
    backend
        .prune(&WorktreePruneRequest {
            repo_root: PathBuf::from("/repo"),
            dry_run: false,
        })
        .expect("prune runs");
    backend
        .prune(&WorktreePruneRequest {
            repo_root: PathBuf::from("/repo"),
            dry_run: true,
        })
        .expect("prune runs");

    let calls = fake.calls.lock().unwrap();
    assert_eq!(
        calls[0],
        vec!["-C", "/repo", "step", "prune", "--foreground", "--yes"]
    );
    assert_eq!(
        calls[1],
        vec!["-C", "/repo", "step", "prune", "--foreground", "--dry-run"]
    );
}

#[test]
fn git_backend_prune_is_unsupported() {
    assert_eq!(
        SystemGitWorktree::new()
            .prune(&WorktreePruneRequest {
                repo_root: PathBuf::from("/repo"),
                dry_run: false,
            })
            .unwrap(),
        WorktreeMutationOutcome::Unsupported,
    );
}
