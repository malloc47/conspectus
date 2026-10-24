//! The repo executor (ADR 0142): `git clone <remote> <path>` for a repo
//! or primary checkout that is gone.
//!
//! This is the one place Conspectus runs a git command that creates
//! something, and ADR 0143 keeps it narrow: the act context only, into
//! a path that doesn't exist, from a remote the history record or the
//! operator's slot names, and nothing else.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::model::{GraphSnapshot, RepoRecord, SlotStatus};
use crate::resolve::slots::{ResolvedSlot, resolve_slot};

use super::{Action, Env, MuxTarget, Options, Plan, RebuildError, Step, Subject};

/// Runs the clone. A trait so tests don't touch the network.
pub trait RepoCloner: Send + Sync {
    fn clone_repo(&self, remote: &str, dest: &Path) -> Result<()>;
}

/// `git clone` through [`crate::exec::command`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCloner;

impl RepoCloner for SystemCloner {
    fn clone_repo(&self, remote: &str, dest: &Path) -> Result<()> {
        check_clone_args(remote, dest)?;
        let output = crate::exec::command(crate::exec::Program::Git, "git")
            .args(clone_args(remote, dest))
            .output()
            .context("failed to run git")?;
        if output.status.success() {
            return Ok(());
        }
        bail!(
            "git clone {remote} failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

/// The arguments after `git` that clone `remote` into `dest`. The
/// cloner runs these and a rebuild plan prints them.
pub fn clone_args(remote: &str, dest: &Path) -> Vec<OsString> {
    vec![
        OsString::from("clone"),
        OsString::from(remote),
        dest.as_os_str().to_os_string(),
    ]
}

/// What may be cloned where: a remote that can't be read as an option,
/// into an absolute path that doesn't exist yet.
pub fn check_clone_args(remote: &str, dest: &Path) -> Result<()> {
    if remote.trim().is_empty() || remote.starts_with('-') {
        bail!("`{remote}` isn't a remote to clone");
    }
    if !dest.is_absolute() {
        bail!(
            "the clone destination `{}` must be an absolute path",
            dest.display()
        );
    }
    if dest.exists() {
        bail!(
            "the clone destination `{}` already exists; a rebuild never clones over anything",
            dest.display()
        );
    }
    Ok(())
}

pub fn plan(
    snapshot: &GraphSnapshot,
    subject: &Subject,
    options: &Options,
    _env: &Env<'_>,
    _now: i64,
) -> Result<Plan, RebuildError> {
    let resolved = resolve_slot(snapshot, &subject.slot);
    let record = resolved
        .record_id
        .as_deref()
        .and_then(|id| snapshot.history.repo(id));
    let name = resolved
        .text("common_dir")
        .map_or_else(|| subject.slot.id.clone(), repo_label);
    let mut plan = Plan {
        subject: format!(
            "repo `{name}` ({})",
            resolved.status.label_for(crate::model::PinKind::Repo)
        ),
        steps: Vec::new(),
        warnings: Vec::new(),
        target: None::<MuxTarget>,
    };
    let hypothetical = options.assume_stopped && resolved.status == SlotStatus::Live;
    if resolved.status == SlotStatus::Live && !hypothetical {
        plan.warnings
            .push(format!("`{name}` exists; there is nothing to clone"));
        return Ok(plan);
    }
    if hypothetical {
        plan.subject = format!("repo `{name}` (exists; planned as if removed)");
        plan.warnings.push(format!(
            "hypothetical: `{name}` exists, so git would refuse to clone over it; \
             these are the commands that would recreate it if it were gone"
        ));
    }
    plan.steps
        .push(clone_step_with(&resolved, record, hypothetical)?);
    Ok(plan)
}

fn repo_label(common_dir: &str) -> String {
    crate::history::recorder::repo_name(common_dir)
}

/// The clone step for a repo's effective recipe.
pub fn clone_step(
    resolved: &ResolvedSlot,
    record: Option<&RepoRecord>,
) -> Result<Step, RebuildError> {
    clone_step_with(resolved, record, false)
}

/// As [`clone_step`]; `allow_existing` skips the check that the
/// destination is free, for a plan that only shows the commands.
pub fn clone_step_with(
    resolved: &ResolvedSlot,
    record: Option<&RepoRecord>,
    allow_existing: bool,
) -> Result<Step, RebuildError> {
    let remote = resolved
        .text("remote")
        .or_else(|| record.and_then(|r| r.recipe.remotes.first().map(|r| r.url.as_str())))
        .ok_or_else(|| {
            RebuildError::Cannot(
                "no remote is recorded for this repo; set one with `pin edit <pin> --set remote=<url>`"
                    .into(),
            )
        })?;
    let dest = resolved
        .text("path")
        .map(PathBuf::from)
        .or_else(|| record.and_then(|r| r.recipe.primary_root.as_ref().map(PathBuf::from)))
        .or_else(|| {
            resolved.text("common_dir").and_then(|dir| {
                let dir = Path::new(dir);
                (dir.file_name().is_some_and(|n| n == ".git"))
                    .then(|| dir.parent().map(Path::to_path_buf))?
            })
        })
        .ok_or_else(|| {
            RebuildError::Cannot(
                "no path is recorded for this repo; set one with `pin edit <pin> --set path=<dir>`"
                    .into(),
            )
        })?;
    if allow_existing {
        if remote.trim().is_empty() || remote.starts_with('-') {
            return Err(RebuildError::Cannot(format!(
                "`{remote}` isn't a remote to clone"
            )));
        }
    } else {
        check_clone_args(remote, &dest).map_err(|err| RebuildError::Cannot(err.to_string()))?;
    }
    Ok(Step::new(
        "clone the repo",
        Action::Clone {
            remote: remote.to_string(),
            dest,
        },
    ))
}

#[cfg(test)]
#[path = "clone_tests.rs"]
mod tests;
