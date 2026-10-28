//! Git previews (ADR 0112 §1–2): a checkout's status, a repo's history
//! and remotes, a ref's recent commits, a commit's stat. Every call is a
//! read-only `git -C <dir>` with optional locks off, so even `status`
//! never rewrites the index (ADR 0143).

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

/// Run `query`. Notes and dates take `muted`, the theme's muted
/// modifier.
pub fn load_git_preview(query: &GitQuery, muted: Modifier) -> PreviewContent {
    let result = match query {
        GitQuery::Status { root } => status(root, muted),
        GitQuery::Repo { dir } => repo(dir, muted),
        GitQuery::Log {
            dir,
            rev,
            ahead_behind,
        } => log(dir, rev, ahead_behind.as_ref(), muted),
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

fn dim(text: impl Into<String>, muted: Modifier) -> Line<'static> {
    Line::styled(text.into(), Style::new().add_modifier(muted))
}

fn status(root: &Path, muted: Modifier) -> Result<Vec<Line<'static>>, GitError> {
    let out = git(root, &["status", "--short", "--branch"])?;
    let mut lines = Vec::new();
    let mut entries = out.lines();
    if let Some(branch) = entries.next().and_then(|l| l.strip_prefix("## ")) {
        lines.push(heading(branch.to_string()));
    }
    let changes: Vec<&str> = entries.collect();
    if changes.is_empty() {
        lines.push(dim("(clean)", muted));
    }
    for change in changes.iter().take(STATUS_LIMIT) {
        lines.push(Line::raw((*change).to_string()));
    }
    if changes.len() > STATUS_LIMIT {
        lines.push(dim(
            format!("… {} more", changes.len() - STATUS_LIMIT),
            muted,
        ));
    }
    lines.push(Line::raw(""));
    match super::path::load_path_preview(root, muted) {
        PreviewContent::Lines(listing) => lines.extend(listing),
        PreviewContent::Unavailable(reason) | PreviewContent::Failed(reason) => {
            lines.push(dim(reason, muted));
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

fn repo(dir: &Path, muted: Modifier) -> Result<Vec<Line<'static>>, GitError> {
    let branch = default_branch(dir)?;
    let mut lines = vec![heading(format!("recent commits on {branch}"))];
    match commits(dir, &branch, muted) {
        Ok(log) => lines.extend(log),
        Err(GitError::Failed(_)) => lines.push(dim("(no commits)", muted)),
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
        lines.push(dim("(none)", muted));
    }
    lines.extend(
        seen.into_iter()
            .map(|entry| Line::raw(entry.replace('\t', "  "))),
    );
    Ok(lines)
}

fn commits(dir: &Path, rev: &str, muted: Modifier) -> Result<Vec<Line<'static>>, GitError> {
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
                Span::styled(format!(" {date} "), Style::new().add_modifier(muted)),
                Span::raw(subject),
            ])
        })
        .collect())
}

fn log(
    dir: &Path,
    rev: &str,
    ahead_behind: Option<&(String, String)>,
    muted: Modifier,
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
            Err(_) => dim(format!("{upstream} not found"), muted),
        });
        lines.push(Line::raw(""));
    }
    lines.extend(commits(dir, rev, muted)?);
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
#[path = "git_tests.rs"]
mod tests;
