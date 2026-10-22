//! Running a [`Plan`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

use crate::discovery::worktree::{WorktreeMutationOutcome, WorktreeSwitchRequest};
use crate::history::LaunchedMux;

use super::launch::{
    Spawned, attach_and_report, discard_session, report_send_keys, resume_fallback_note,
    spawn_watched,
};
use super::{Action, Env, Plan};

/// What a finished run did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Executed {
    /// A line per step that ran.
    pub done: Vec<String>,
    /// Things worth telling the operator: a resume that died and was
    /// replaced, a history write that failed.
    pub notes: Vec<String>,
    /// The mux the run created, if it created one.
    pub created: Option<String>,
    /// Where each rebuilt worktree landed, by the key its step carries.
    pub landed: BTreeMap<String, PathBuf>,
    /// Slots moved onto a rebuilt worktree that landed at a new path,
    /// as `(slot, new history record)`.
    pub retargeted: Vec<(String, String)>,
}

/// A step failed. Earlier steps stay done; later ones didn't run.
#[derive(Debug)]
pub struct RunFailure {
    /// The step that failed, 1-based.
    pub step: usize,
    pub title: String,
    pub error: anyhow::Error,
    pub done: Vec<String>,
}

impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "step {} ({}) failed: {:#}",
            self.step, self.title, self.error
        )?;
        if self.done.is_empty() {
            write!(f, "; nothing had run yet")
        } else {
            write!(f, "; already done: {}", self.done.join("; "))
        }
    }
}

impl std::error::Error for RunFailure {}

/// Run `plan`'s steps in order, stopping at the first failure.
pub fn execute(plan: &Plan, env: &Env<'_>) -> Result<Executed, RunFailure> {
    let mut executed = Executed::default();
    for (index, step) in plan.steps.iter().enumerate() {
        match run_step(&step.action, env, &mut executed) {
            Ok(done) => executed.done.push(done),
            Err(error) => {
                return Err(RunFailure {
                    step: index + 1,
                    title: step.title.clone(),
                    error,
                    done: executed.done,
                });
            }
        }
    }
    Ok(executed)
}

fn run_step(action: &Action, env: &Env<'_>, executed: &mut Executed) -> Result<String> {
    match action {
        Action::KillSession {
            backend,
            socket,
            name,
        } => {
            let runner = runner(env, backend)?;
            discard_session(&*runner, socket.as_deref(), name)?;
            Ok(format!("ended the dead session `{name}`"))
        }
        Action::NewSession {
            backend,
            socket,
            name,
            cwd,
            cwd_after,
            argv,
            resume,
            harness,
        } => {
            let runner = runner(env, backend)?;
            let cwd = match cwd_after.as_ref().and_then(|landing| {
                executed
                    .landed
                    .get(&landing.key)
                    .map(|root| root.join(&landing.rel))
            }) {
                Some(landed) => landed,
                None => cwd.clone(),
            };
            let cwd = &cwd;
            let spawned = spawn_watched(
                &*runner,
                socket.as_deref(),
                name,
                cwd,
                argv,
                resume.as_ref().map(|resume| resume.argv.as_slice()),
                env.resume_watch,
            )?;
            let mut line = format!("created `{name}`");
            match (&spawned, resume) {
                (Spawned::FreshAfterFailedResume(dead), Some(resume)) => {
                    executed
                        .notes
                        .push(resume_fallback_note(&resume.argv, dead));
                    if !dead.output.trim().is_empty() {
                        executed.notes.push(dead.output.trim_end().to_string());
                    }
                    line.push_str(" fresh (the resume was rejected)");
                }
                (_, Some(resume)) => {
                    line.push_str(&format!(", resuming {}", resume.session_key));
                }
                _ => {}
            }
            executed.created = Some(name.clone());
            record_launch(
                env,
                executed,
                backend,
                socket.as_deref(),
                name,
                cwd,
                argv,
                harness.as_deref(),
            );
            Ok(line)
        }
        Action::WorktreeSwitch {
            key,
            repo_root,
            repo,
            branch,
            create,
            base,
            recorded_root,
            old_record,
        } => {
            let backend = env
                .worktree
                .context("no worktree backend can change worktrees")?;
            let outcome = backend.switch(&WorktreeSwitchRequest {
                repo_root: repo_root.clone(),
                branch: branch.clone(),
                create: *create,
                base: base.clone(),
            })?;
            match outcome {
                WorktreeMutationOutcome::Succeeded { .. } => {}
                WorktreeMutationOutcome::Unsupported => {
                    bail!(
                        "the {} backend can't change worktrees",
                        backend.backend_key()
                    )
                }
                WorktreeMutationOutcome::Failed { code, message } => {
                    bail!("wt switch failed (exit {code:?}): {message}")
                }
            }
            let landed = find_worktree(backend, repo_root, branch)?;
            let at = PathBuf::from(&landed.path);
            executed.landed.insert(key.clone(), at.clone());
            let moved = recorded_root.as_ref().is_some_and(|root| root != &at);
            record_worktree(
                env,
                executed,
                repo,
                &landed,
                branch,
                moved.then_some(old_record.as_deref()).flatten(),
            );
            Ok(if moved {
                format!(
                    "rebuilt `{branch}` at {} (it was {})",
                    at.display(),
                    recorded_root
                        .as_ref()
                        .map_or_else(String::new, |r| r.display().to_string())
                )
            } else {
                format!("rebuilt `{branch}` at {}", at.display())
            })
        }
        Action::Clone { remote, dest } => {
            env.cloner.clone_repo(remote, dest)?;
            Ok(format!("cloned {remote} to {}", dest.display()))
        }
        Action::SendKeys {
            backend,
            socket,
            name,
            argv,
        } => {
            let runner = runner(env, backend)?;
            let literal = super::launch::format_argv_for_send_keys(argv);
            let outcome = runner
                .send_keys(socket.as_deref(), name, &literal, true)
                .map_err(|err| anyhow!("{backend} send-keys failed: {err}"))?;
            report_send_keys(outcome, backend, name)?;
            Ok(format!("relaunched the agent in `{name}`"))
        }
        Action::Attach {
            backend,
            socket,
            name,
        } => {
            let runner = runner(env, backend)?;
            attach_and_report(&*runner, socket.as_deref(), name)?;
            Ok(format!("attached to `{name}`"))
        }
    }
}

fn runner(env: &Env<'_>, backend: &str) -> Result<Box<dyn crate::discovery::tmux::MuxBackend>> {
    (env.backend_for)(backend)
        .with_context(|| format!("the `{backend}` mux backend isn't available"))
}

#[allow(clippy::too_many_arguments)]
fn record_launch(
    env: &Env<'_>,
    executed: &mut Executed,
    backend: &str,
    socket: Option<&str>,
    name: &str,
    cwd: &std::path::Path,
    argv: &[std::ffi::OsString],
    harness: Option<&str>,
) {
    let Some(writer) = env.history else {
        return;
    };
    let recorded: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let cwd = cwd.to_string_lossy();
    let launch = LaunchedMux {
        backend,
        socket,
        name,
        cwd: &cwd,
        argv: &recorded,
        harness,
    };
    if let Err(err) = writer.record_launched_mux(&launch) {
        executed.notes.push(format!(
            "could not record `{name}` in history at {}: {err}",
            writer.store.dir().display()
        ));
    }
}

/// The worktree now checking out `branch`, from the backend's listing.
fn find_worktree(
    backend: &dyn crate::discovery::worktree::WorktreeBackend,
    repo_root: &std::path::Path,
    branch: &str,
) -> Result<crate::discovery::worktree::WorktreeRecord> {
    let wanted = format!("refs/heads/{branch}");
    backend
        .list(repo_root)?
        .into_iter()
        .find(|record| record.branch.as_deref() == Some(wanted.as_str()))
        .with_context(|| format!("the backend ran but no worktree checks out `{branch}`"))
}

/// Write the rebuilt worktree's history record and move the slots that
/// pointed at the one it replaced (ADR 0142).
fn record_worktree(
    env: &Env<'_>,
    executed: &mut Executed,
    repo: &str,
    landed: &crate::discovery::worktree::WorktreeRecord,
    branch: &str,
    replaced: Option<&str>,
) {
    let Some(writer) = env.history else {
        return;
    };
    let new_id = match writer.record_rebuilt_checkout(
        repo,
        &landed.path,
        Some(branch),
        landed.head.as_deref(),
        replaced,
    ) {
        Ok(id) => id,
        Err(err) => {
            executed.notes.push(format!(
                "could not record the worktree in history at {}: {err}",
                writer.store.dir().display()
            ));
            return;
        }
    };
    let (Some(old), Some(store)) = (replaced, env.slots) else {
        return;
    };
    let old_target = crate::model::PinSlot::history_target(old);
    for slot in store.load().slots {
        if slot.target.as_deref() != Some(old_target.as_str()) {
            continue;
        }
        let new_target = crate::model::PinSlot::history_target(&new_id);
        match store.update(&slot.id, |slot| {
            slot.target = Some(new_target.clone());
            Ok(())
        }) {
            Ok(_) => executed.retargeted.push((slot.id.clone(), new_id.clone())),
            Err(err) => executed.notes.push(format!(
                "could not move pin `{}` to the rebuilt worktree: {err}",
                slot.id
            )),
        }
    }
    if !executed.retargeted.is_empty() {
        executed.notes.push(format!(
            "the worktree landed at {} instead of its recorded place; moved pin(s) {} to it",
            landed.path,
            executed
                .retargeted
                .iter()
                .map(|(slot, _)| format!("`{slot}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
}
