use std::process::Command;

use super::*;

/// `query` loaded with nothing to stop it.
fn load_git_preview(query: &GitQuery, muted: Modifier) -> PreviewContent {
    load_git_preview_unless(query, muted, &|| false).expect("nothing stops this load")
}

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
    let lines = rows(load_git_preview(
        &GitQuery::Status {
            root: repo.path().to_path_buf(),
        },
        crate::tui::Theme::default().muted,
    ));
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
    let lines = rows(load_git_preview(
        &GitQuery::Log {
            dir: repo.path().to_path_buf(),
            rev: "topic".to_string(),
            ahead_behind: Some(("topic".to_string(), "main".to_string())),
        },
        crate::tui::Theme::default().muted,
    ));
    assert_eq!(lines[0], "ahead 1 · behind 0 of main");
    assert!(lines[2].ends_with("topic work"), "{lines:?}");
    assert!(lines[4].ends_with("first commit"), "{lines:?}");
}

#[test]
fn a_repo_shows_its_default_branch_history_and_remotes() {
    let repo = repo_fixture();
    let lines = rows(load_git_preview(
        &GitQuery::Repo {
            dir: repo.path().join(".git"),
        },
        crate::tui::Theme::default().muted,
    ));
    assert_eq!(lines[0], "recent commits on HEAD");
    assert!(lines[1].ends_with("second commit"), "{lines:?}");
    assert_eq!(&lines[lines.len() - 2..], ["remotes", "(none)"]);
}

#[test]
fn a_commit_shows_its_stat() {
    let repo = repo_fixture();
    let lines = rows(load_git_preview(
        &GitQuery::Show {
            dir: repo.path().to_path_buf(),
            commit: "main".to_string(),
        },
        crate::tui::Theme::default().muted,
    ));
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
        load_git_preview(
            &GitQuery::Log {
                dir: repo.path().to_path_buf(),
                rev: "refs/heads/gone".to_string(),
                ahead_behind: None,
            },
            crate::tui::Theme::default().muted
        ),
        PreviewContent::Unavailable("refs/heads/gone not found".to_string())
    );
    assert_eq!(
        load_git_preview(
            &GitQuery::Status {
                root: repo.path().join("gone"),
            },
            crate::tui::Theme::default().muted
        ),
        PreviewContent::Unavailable("no longer exists".to_string())
    );
    let plain = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        load_git_preview(
            &GitQuery::Status {
                root: plain.path().to_path_buf(),
            },
            crate::tui::Theme::default().muted
        ),
        PreviewContent::Unavailable("not a git repository".to_string())
    );
}

/// Run `query` with a stop that lets `allowed` git commands start, and
/// say how it ended and how many commands asked to start (the last one
/// refused when the load was stopped).
fn load_stopping_after(
    query: &GitQuery,
    allowed: usize,
) -> (Result<PreviewContent, Stopped>, usize) {
    let asked = std::sync::atomic::AtomicUsize::new(0);
    let stop = || asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= allowed;
    let outcome = load_git_preview_unless(query, crate::tui::Theme::default().muted, &stop);
    (outcome, asked.load(std::sync::atomic::Ordering::SeqCst))
}

#[test]
fn a_stopped_load_starts_no_more_git_commands() {
    let repo = repo_fixture();
    let dir = repo.path().to_path_buf();
    let queries = [
        GitQuery::Status { root: dir.clone() },
        GitQuery::Repo { dir: dir.clone() },
        GitQuery::Log {
            dir: dir.clone(),
            rev: "topic".to_string(),
            ahead_behind: Some(("topic".to_string(), "main".to_string())),
        },
        GitQuery::Show {
            dir,
            commit: "HEAD".to_string(),
        },
    ];
    for query in &queries {
        // Nothing stops it: how many commands the preview runs.
        let (full, commands) = load_stopping_after(query, usize::MAX);
        assert!(full.is_ok(), "{query:?}");
        assert!(commands >= 1, "{query:?} ran no git");
        // Stopped after each number of commands: it ends there, with the
        // command it was refused never started.
        for allowed in 0..commands {
            let (outcome, asked) = load_stopping_after(query, allowed);
            assert_eq!(outcome, Err(Stopped), "{query:?} after {allowed} commands");
            assert_eq!(asked, allowed + 1, "{query:?} after {allowed} commands");
        }
    }
}
