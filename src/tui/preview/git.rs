//! Git previews (ADR 0112 §1–2): a checkout's status, a repo's history
//! and remotes, a ref's recent commits, a commit's stat. Every call is a
//! read-only `git -C <dir>` with optional locks off, so even `status`
//! never rewrites the index (ADR 0087).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::PreviewContent;

/// Commits a log preview shows.
const LOG_LIMIT: &str = "30";
/// Status lines shown before summarizing the rest.
const STATUS_LIMIT: usize = 200;

/// A git read a preview runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitQuery {
    /// A checkout: its short status above a listing of its root.
    Status { root: PathBuf },
    /// A repo: recent commits on its default branch, then its remotes.
    Repo { dir: PathBuf },
    /// Recent commits at `rev`. With `ahead_behind`, how far the first
    /// ref is ahead of and behind the second.
    Log {
        dir: PathBuf,
        rev: String,
        ahead_behind: Option<(String, String)>,
    },
    /// `git show --stat` for a commit.
    Show { dir: PathBuf, commit: String },
}

pub fn load_git_preview(query: &GitQuery) -> PreviewContent {
    let result = match query {
        GitQuery::Status { root } => status(root),
        GitQuery::Repo { dir } => repo(dir),
        GitQuery::Log {
            dir,
            rev,
            ahead_behind,
        } => log(dir, rev, ahead_behind.as_ref()),
        GitQuery::Show { dir, commit } => show(dir, commit),
    };
    match result {
        Ok(lines) => PreviewContent::Lines(lines),
        Err(GitError::Unavailable(reason)) => PreviewContent::Unavailable(reason),
        Err(GitError::Failed(message)) => PreviewContent::Failed(message),
    }
}

#[derive(Debug)]
enum GitError {
    Unavailable(String),
    Failed(String),
}

/// Run a read-only git command in `dir` and return its stdout.
fn git(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    if !dir.exists() {
        return Err(GitError::Unavailable("no longer exists".to_string()));
    }
    let output = crate::exec::command(crate::exec::Program::Git, "git")
        .args(["--no-optional-locks", "-c", "color.ui=never", "-C"])
        .arg(dir)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => GitError::Unavailable("git not found".to_string()),
            _ => GitError::Failed(format!("git failed to start: {err}")),
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.contains("not a git repository") {
        return Err(GitError::Unavailable("not a git repository".to_string()));
    }
    Err(GitError::Failed(format!(
        "git {}: {stderr}",
        args.join(" ")
    )))
}

/// Whether `rev` names something in the repo at `dir`.
fn rev_exists(dir: &Path, rev: &str) -> Result<bool, GitError> {
    match git(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    ) {
        Ok(_) => Ok(true),
        Err(GitError::Failed(_)) => Ok(false),
        Err(err) => Err(err),
    }
}

fn heading(text: impl Into<String>) -> Line<'static> {
    Line::styled(text.into(), Style::new().add_modifier(Modifier::BOLD))
}

fn dim(text: impl Into<String>) -> Line<'static> {
    Line::styled(text.into(), Style::new().add_modifier(Modifier::DIM))
}

fn status(root: &Path) -> Result<Vec<Line<'static>>, GitError> {
    let out = git(root, &["status", "--short", "--branch"])?;
    let mut lines = Vec::new();
    let mut entries = out.lines();
    if let Some(branch) = entries.next().and_then(|l| l.strip_prefix("## ")) {
        lines.push(heading(branch.to_string()));
    }
    let changes: Vec<&str> = entries.collect();
    if changes.is_empty() {
        lines.push(dim("(clean)"));
    }
    for change in changes.iter().take(STATUS_LIMIT) {
        lines.push(Line::raw((*change).to_string()));
    }
    if changes.len() > STATUS_LIMIT {
        lines.push(dim(format!("… {} more", changes.len() - STATUS_LIMIT)));
    }
    lines.push(Line::raw(""));
    match super::path::load_path_preview(root) {
        PreviewContent::Lines(listing) => lines.extend(listing),
        PreviewContent::Unavailable(reason) | PreviewContent::Failed(reason) => {
            lines.push(dim(reason));
        }
        _ => {}
    }
    Ok(lines)
}

/// The repo's default branch: the local branch `origin/HEAD` points at,
/// that remote branch when there's no local one, else `HEAD`.
fn default_branch(dir: &Path) -> Result<String, GitError> {
    let Ok(remote_head) = git(
        dir,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) else {
        return Ok("HEAD".to_string());
    };
    let remote_head = remote_head.trim().to_string();
    let local = remote_head.trim_start_matches("origin/");
    if rev_exists(dir, &format!("refs/heads/{local}"))? {
        Ok(local.to_string())
    } else {
        Ok(remote_head)
    }
}

fn repo(dir: &Path) -> Result<Vec<Line<'static>>, GitError> {
    let branch = default_branch(dir)?;
    let mut lines = vec![heading(format!("recent commits on {branch}"))];
    match commits(dir, &branch) {
        Ok(log) => lines.extend(log),
        Err(GitError::Failed(_)) => lines.push(dim("(no commits)")),
        Err(err) => return Err(err),
    }
    lines.push(Line::raw(""));
    lines.push(heading("remotes"));
    let remotes = git(dir, &["remote", "-v"])?;
    let mut seen = Vec::new();
    for remote in remotes.lines() {
        // `remote -v` lists each remote's fetch and push URLs; one line
        // per name and URL is enough.
        let entry = remote
            .trim_end_matches(" (fetch)")
            .trim_end_matches(" (push)")
            .to_string();
        if !seen.contains(&entry) {
            seen.push(entry);
        }
    }
    if seen.is_empty() {
        lines.push(dim("(none)"));
    }
    lines.extend(
        seen.into_iter()
            .map(|entry| Line::raw(entry.replace('\t', "  "))),
    );
    Ok(lines)
}

fn commits(dir: &Path, rev: &str) -> Result<Vec<Line<'static>>, GitError> {
    let out = git(
        dir,
        &[
            "log",
            "--format=%h%x09%as%x09%s",
            "-n",
            LOG_LIMIT,
            rev,
            "--",
        ],
    )?;
    Ok(out
        .lines()
        .map(|line| {
            let mut parts = line.splitn(3, '\t');
            let hash = parts.next().unwrap_or_default().to_string();
            let date = parts.next().unwrap_or_default().to_string();
            let subject = parts.next().unwrap_or_default().to_string();
            Line::from(vec![
                Span::styled(hash, Style::new().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!(" {date} "),
                    Style::new().add_modifier(Modifier::DIM),
                ),
                Span::raw(subject),
            ])
        })
        .collect())
}

fn log(
    dir: &Path,
    rev: &str,
    ahead_behind: Option<&(String, String)>,
) -> Result<Vec<Line<'static>>, GitError> {
    if !rev_exists(dir, rev)? {
        return Err(GitError::Unavailable(format!("{rev} not found")));
    }
    let mut lines = Vec::new();
    if let Some((branch, upstream)) = ahead_behind {
        let counts = git(
            dir,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{branch}...{upstream}"),
                "--",
            ],
        );
        lines.push(match counts {
            Ok(counts) => {
                let mut parts = counts.split_whitespace();
                let ahead = parts.next().unwrap_or("0");
                let behind = parts.next().unwrap_or("0");
                heading(format!("ahead {ahead} · behind {behind} of {upstream}"))
            }
            Err(_) => dim(format!("{upstream} not found")),
        });
        lines.push(Line::raw(""));
    }
    lines.extend(commits(dir, rev)?);
    Ok(lines)
}

fn show(dir: &Path, commit: &str) -> Result<Vec<Line<'static>>, GitError> {
    if !rev_exists(dir, commit)? {
        return Err(GitError::Unavailable(format!("commit {commit} not found")));
    }
    let out = git(dir, &["show", "--stat", commit, "--"])?;
    Ok(out
        .lines()
        .map(|line| Line::raw(line.replace('\t', "    ")))
        .collect())
}

#[cfg(test)]
mod tests {
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
}
