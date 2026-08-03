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
