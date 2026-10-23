//! The worktree executor (ADR 0142): bring a linked worktree back
//! through the worktree backend (ADR 0092), so Conspectus never
//! changes git itself.
//!
//! `wt switch <branch>` when the branch still exists, `wt switch
//! --create --base <last head> <branch>` when it doesn't. Worktrunk's
//! path template decides where the worktree lands, which may differ
//! from the recorded root; the run records the new place, links it to
//! the old record, and moves pin slots over (the location rule is
//! provisional, ADR 0142).
//!
//! A worktree in a repo that is gone clones the repo first, and a
//! primary checkout is a clone of its repo.

use std::path::{Path, PathBuf};

use crate::model::{
    CheckoutRecord, GraphNode, GraphSnapshot, NodeId, PinKind, PinSlot, SlotFields, SlotStatus,
};
use crate::resolve::slots::{ResolvedSlot, resolve_slot};

use super::clone::clone_step;
use super::{Action, Env, Options, Plan, RebuildError, Step, Subject};

pub fn plan(
    snapshot: &GraphSnapshot,
    subject: &Subject,
    _options: &Options,
    env: &Env<'_>,
    _now: i64,
) -> Result<Plan, RebuildError> {
    let resolved = resolve_slot(snapshot, &subject.slot);
    let record = resolved
        .record_id
        .as_deref()
        .and_then(|id| snapshot.history.checkout(id));
    let root = resolved.text("root").unwrap_or_default().to_string();
    let label = Path::new(&root)
        .file_name()
        .map_or_else(|| root.clone(), |name| name.to_string_lossy().into_owned());
    let mut plan = Plan {
        subject: format!("worktree `{label}` ({})", resolved.status.as_str()),
        steps: Vec::new(),
        warnings: Vec::new(),
        target: None,
    };
    if resolved.status == SlotStatus::Live {
        plan.warnings
            .push(format!("`{root}` exists; there is nothing to rebuild"));
        return Ok(plan);
    }
    if let Some(location) = resolved.text("location") {
        plan.warnings.push(format!(
            "worktrunk decides where a worktree goes, so `location = {location}` may not be honored"
        ));
    }
    if record.is_some_and(|r| !r.recipe.linked) {
        // A primary checkout is its repo, cloned.
        let repo = repo_resolved(snapshot, &resolved);
        plan.steps
            .push(clone_into(&repo, snapshot, &resolved, &root)?);
        return Ok(plan);
    }
    let steps = switch_steps(snapshot, &resolved, record, env)?;
    plan.steps = steps;
    Ok(plan)
}

/// A transient repo slot for the repo a checkout record names.
fn repo_resolved(snapshot: &GraphSnapshot, checkout: &ResolvedSlot) -> ResolvedSlot {
    let repo_key = checkout.text("repo").unwrap_or_default();
    let target = snapshot.history.by_key(repo_key).map_or_else(
        || repo_key.to_string(),
        |(_, meta)| PinSlot::history_target(&meta.id),
    );
    let slot = PinSlot {
        id: repo_key.to_string(),
        kind: PinKind::Repo,
        target: Some(target),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: u32::MAX,
        legacy: false,
    };
    resolve_slot(snapshot, &slot)
}

/// Clone the repo a primary checkout belongs to into the checkout's
/// own root.
fn clone_into(
    repo: &ResolvedSlot,
    snapshot: &GraphSnapshot,
    checkout: &ResolvedSlot,
    root: &str,
) -> Result<Step, RebuildError> {
    let record = repo
        .record_id
        .as_deref()
        .and_then(|id| snapshot.history.repo(id));
    let mut repo = repo.clone();
    // The checkout's root is where the clone goes.
    repo.effective.retain(|field| field.name != "path");
    repo.effective.push(crate::model::RecipeField {
        name: "path".to_string(),
        value: crate::model::FieldValue::Text(root.to_string()),
        source: crate::model::FieldSource::History,
    });
    let _ = checkout;
    clone_step(&repo, record)
}

/// The steps that bring a checkout's worktree back, the clone of its
/// repo first when the repo is gone.
pub fn switch_steps(
    snapshot: &GraphSnapshot,
    resolved: &ResolvedSlot,
    record: Option<&CheckoutRecord>,
    env: &Env<'_>,
) -> Result<Vec<Step>, RebuildError> {
    if env.worktree.is_none() {
        return Err(RebuildError::Cannot(
            "no worktree backend can change worktrees; install worktrunk (`wt`) or set \
             `[worktree] backend = \"worktrunk\"`"
                .into(),
        ));
    }
    let branch = resolved.text("branch").ok_or_else(|| {
        RebuildError::Cannot(
            "no branch is recorded for this worktree; set one with `pin edit <pin> --set branch=<name>`"
                .into(),
        )
    })?;
    let repo_key = resolved.text("repo").unwrap_or_default().to_string();
    let common_dir = repo_key
        .strip_prefix("repo:")
        .unwrap_or(&repo_key)
        .to_string();
    let recorded_root = resolved.text("root").map(PathBuf::from);

    let mut steps = Vec::new();
    let anchor = if let Some(anchor) = live_repo_anchor(snapshot, &repo_key, &common_dir) {
        anchor
    } else {
        // The repo is gone: clone it first, then work from the clone.
        let repo = repo_resolved(snapshot, resolved);
        let repo_record = repo
            .record_id
            .as_deref()
            .and_then(|id| snapshot.history.repo(id));
        let step = clone_step(&repo, repo_record)?;
        let Action::Clone { dest, .. } = &step.action else {
            unreachable!("clone_step makes a clone")
        };
        let dest = dest.clone();
        steps.push(step);
        dest
    };

    let exists = branch_exists(snapshot, &repo_key, branch);
    let create = !exists && !steps_clone_first(&steps);
    let base = if create {
        resolved
            .text("base")
            .or_else(|| resolved.text("head"))
            .map(str::to_string)
    } else {
        None
    };
    let key = recorded_root.as_ref().map_or_else(
        || format!("branch:{branch}"),
        |root| root.display().to_string(),
    );
    steps.push(switch_step(
        key,
        &anchor,
        &repo_key,
        branch,
        create,
        base,
        recorded_root,
        record.map(|r| r.meta.id.clone()),
    ));
    Ok(steps)
}

fn steps_clone_first(steps: &[Step]) -> bool {
    steps
        .iter()
        .any(|step| matches!(step.action, Action::Clone { .. }))
}

#[allow(clippy::too_many_arguments)]
pub fn switch_step(
    key: String,
    repo_root: &Path,
    repo: &str,
    branch: &str,
    create: bool,
    base: Option<String>,
    recorded_root: Option<PathBuf>,
    old_record: Option<String>,
) -> Step {
    let mut command = format!("wt -C {} switch --no-cd", repo_root.display());
    if create {
        command.push_str(" --create");
        if let Some(base) = &base {
            command.push_str(&format!(" --base {base}"));
        }
    }
    command.push_str(&format!(" {branch}"));
    Step {
        title: if create {
            format!("recreate branch {branch}")
        } else {
            format!("rebuild worktree {branch}")
        },
        command,
        action: Action::WorktreeSwitch {
            key,
            repo_root: repo_root.to_path_buf(),
            repo: repo.to_string(),
            branch: branch.to_string(),
            create,
            base,
            recorded_root,
            old_record,
        },
    }
}

/// A directory inside the live repo `repo_key` to run the backend from:
/// a checkout of it that exists, else the directory holding its `.git`.
pub fn live_repo_anchor(
    snapshot: &GraphSnapshot,
    repo_key: &str,
    common_dir: &str,
) -> Option<PathBuf> {
    let present = |path: &Path| path.is_dir().then(|| path.to_path_buf());
    for node in snapshot.live_nodes() {
        if let GraphNode::Checkout(checkout) = node
            && NodeId::Repo(checkout.id.repo.clone()).to_string() == repo_key
            && let Some(path) = present(Path::new(&checkout.root))
        {
            return Some(path);
        }
    }
    let dir = Path::new(common_dir);
    if dir.is_dir() {
        let parent = dir
            .parent()
            .filter(|_| dir.file_name().is_some_and(|n| n == ".git"));
        return parent.and_then(present).or_else(|| present(dir));
    }
    None
}

/// Whether a local branch `branch` exists in the repo, from the
/// graph's branch nodes.
pub fn branch_exists(snapshot: &GraphSnapshot, repo_key: &str, branch: &str) -> bool {
    snapshot.live_nodes().any(|node| match node {
        GraphNode::Branch(b) => {
            NodeId::Repo(b.id.repo.clone()).to_string() == repo_key
                && crate::history::recorder::short_ref(&b.refname) == branch
        }
        _ => false,
    })
}

/// The path of the live worktree checking out `branch` in the repo.
pub fn live_worktree_for_branch(
    snapshot: &GraphSnapshot,
    repo_key: &str,
    branch: &str,
) -> Option<String> {
    snapshot.live_nodes().find_map(|node| match node {
        GraphNode::Checkout(c)
            if NodeId::Repo(c.id.repo.clone()).to_string() == repo_key
                && c.current_branch
                    .as_ref()
                    .is_some_and(|b| crate::history::recorder::short_ref(&b.refname) == branch) =>
        {
            Some(c.root.clone())
        }
        _ => None,
    })
}

/// The repo and branch a path is a worktree of, from the checkouts
/// whose root contains it.
pub fn checkout_containing<'a>(
    snapshot: &'a GraphSnapshot,
    path: &Path,
) -> Option<&'a crate::model::CheckoutNode> {
    snapshot
        .live_nodes()
        .filter_map(|node| match node {
            GraphNode::Checkout(c) => Some(c),
            _ => None,
        })
        .filter(|c| crate::model::path_is_ancestor_of(Path::new(&c.root), path))
        .max_by_key(|c| c.root.len())
}

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod tests;
