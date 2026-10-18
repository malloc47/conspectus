use std::process::Command;

use super::*;

/// A repo with two commits on `main` and a branch `topic` one
/// commit ahead.
fn repo_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("git");
        assert!(status.status.success(), "{status:?}");
    };
    run(&["init", "-q", "-b", "main"]);
    std::fs::write(dir.path().join("a.txt"), "a").expect("write");
    run(&["add", "a.txt"]);
    run(&["commit", "-q", "-m", "first commit"]);
    std::fs::write(dir.path().join("b.txt"), "b").expect("write");
    run(&["add", "b.txt"]);
    run(&["commit", "-q", "-m", "second commit"]);
    run(&["checkout", "-q", "-b", "topic"]);
    std::fs::write(dir.path().join("c.txt"), "c").expect("write");
    run(&["add", "c.txt"]);
    run(&["commit", "-q", "-m", "topic work"]);
    run(&["checkout", "-q", "main"]);
    dir
}

fn rows(content: PreviewContent) -> Vec<String> {
    match content {
        PreviewContent::Lines(lines) => lines.iter().map(ToString::to_string).collect(),
        other => panic!("expected lines, got {other:?}"),
    }
}

#[test]
fn a_checkout_shows_its_status_above_a_listing() {
    let repo = repo_fixture();
    std::fs::write(repo.path().join("new.txt"), "n").expect("write");
    let index = std::fs::metadata(repo.path().join(".git/index"))
        .and_then(|m| m.modified())
        .expect("index");
    let lines = rows(load_git_preview(&GitQuery::Status {
        root: repo.path().to_path_buf(),
    }));
    assert_eq!(lines[0], "main");
    assert_eq!(lines[1], "?? new.txt");
    assert!(lines.iter().any(|l| l.starts_with("a.txt")), "{lines:?}");
    let after = std::fs::metadata(repo.path().join(".git/index"))
        .and_then(|m| m.modified())
        .expect("index");
    assert_eq!(index, after, "status left the index alone");
}

#[test]
fn a_ref_shows_its_recent_commits_and_ahead_behind() {
    let repo = repo_fixture();
    let lines = rows(load_git_preview(&GitQuery::Log {
        dir: repo.path().to_path_buf(),
        rev: "topic".to_string(),
        ahead_behind: Some(("topic".to_string(), "main".to_string())),
    }));
    assert_eq!(lines[0], "ahead 1 · behind 0 of main");
    assert!(lines[2].ends_with("topic work"), "{lines:?}");
    assert!(lines[4].ends_with("first commit"), "{lines:?}");
}

#[test]
fn a_repo_shows_its_default_branch_history_and_remotes() {
    let repo = repo_fixture();
    let lines = rows(load_git_preview(&GitQuery::Repo {
        dir: repo.path().join(".git"),
    }));
    assert_eq!(lines[0], "recent commits on HEAD");
    assert!(lines[1].ends_with("second commit"), "{lines:?}");
    assert_eq!(&lines[lines.len() - 2..], ["remotes", "(none)"]);
}

#[test]
fn a_commit_shows_its_stat() {
    let repo = repo_fixture();
    let lines = rows(load_git_preview(&GitQuery::Show {
        dir: repo.path().to_path_buf(),
        commit: "main".to_string(),
    }));
    assert!(
        lines.iter().any(|l| l.contains("second commit")),
        "{lines:?}"
    );
    assert!(lines.iter().any(|l| l.contains("b.txt")), "{lines:?}");
}

#[test]
fn a_missing_ref_or_checkout_says_so() {
    let repo = repo_fixture();
    assert_eq!(
        load_git_preview(&GitQuery::Log {
            dir: repo.path().to_path_buf(),
            rev: "refs/heads/gone".to_string(),
            ahead_behind: None,
        }),
        PreviewContent::Unavailable("refs/heads/gone not found".to_string())
    );
    assert_eq!(
        load_git_preview(&GitQuery::Status {
            root: repo.path().join("gone"),
        }),
        PreviewContent::Unavailable("no longer exists".to_string())
    );
    let plain = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        load_git_preview(&GitQuery::Status {
            root: plain.path().to_path_buf(),
        }),
        PreviewContent::Unavailable("not a git repository".to_string())
    );
}
