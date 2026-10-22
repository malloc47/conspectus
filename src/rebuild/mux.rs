//! The mux executor (ADR 0142): bring a mux back, or bring its agent
//! back, from the effective recipe of a slot, a history record, or a
//! live node.
//!
//! - gone or unknown: `new-session` with the effective argv, the most
//!   recent hosted session's resume spliced in (ADR 0098), then attach;
//! - live with its agent running: attach;
//! - live with a dead pane: end it, then as for gone (ADR 0103);
//! - live with its agent exited: type the Conspectus-built argv into
//!   the pane, but only when nothing shows a harness running there
//!   (ADR 0028, ADR 0102), then attach;
//! - owned by an orchestrator: refuse.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::discovery::harness::adapter_in_command;
use crate::discovery::orchestrator;
use crate::discovery::tmux::{MuxBackend, TmuxPaneStatus};
use crate::launch_default::DefaultSource;
use crate::model::{
    FieldSource, GraphLink, GraphNode, GraphSnapshot, LinkState, MuxRecord, MuxSessionId,
    MuxSessionNode, NodeId, RelationKind, SlotStatus,
};
use crate::output::render::format_relative_age;
use crate::resolve::slots::{ResolvedSlot, resolve_slot};

use super::resume::{NoResume, Resume, build_resume, session_to_resume};
use super::{
    Action, Env, Landing, MuxTarget, Options, Plan, RebuildError, Step, Subject, worktree,
};

/// Plan the rebuild of a mux subject.
pub fn plan(
    snapshot: &GraphSnapshot,
    subject: &Subject,
    options: &Options,
    env: &Env<'_>,
    now: i64,
) -> Result<Plan, RebuildError> {
    let resolved = resolve_slot(snapshot, &subject.slot);
    let record = resolved
        .record_id
        .as_deref()
        .and_then(|id| snapshot.history.mux(id));
    let live = live_node(snapshot, &resolved);
    let backend = record
        .map(|record| record.recipe.backend.clone())
        .or_else(|| live.map(|node| node.backend.clone()))
        .unwrap_or_else(|| crate::discovery::tmux::TMUX_BACKEND.to_string());

    // A live mux is addressed by the name it really has; one to create
    // takes the effective name and socket.
    let (name, socket) = match live {
        Some(node) => (node.session_name().to_string(), node.socket.clone()),
        None => (
            resolved
                .text("name")
                .unwrap_or(&subject.slot.id)
                .to_string(),
            resolved.text("socket").map(str::to_string),
        ),
    };
    if let Some(owner) = owner_of(record, &name) {
        return Err(RebuildError::Owned { name, owner });
    }
    let runner =
        (env.backend_for)(&backend).ok_or_else(|| RebuildError::NoBackend(backend.clone()))?;
    let target = MuxTarget {
        backend: backend.clone(),
        socket: socket.clone(),
        name: name.clone(),
    };

    let mut plan = Plan {
        subject: describe(&name, &resolved, record, now),
        steps: Vec::new(),
        warnings: Vec::new(),
        target: Some(target.clone()),
    };
    let harness = resolved.text("harness").map(str::to_string);
    let (argv, argv_default) = effective_argv(&resolved, harness.as_deref(), env);
    if let Some(source) = argv_default
        && source != DefaultSource::HarnessDefault
        && !argv.is_empty()
    {
        plan.warnings
            .push(format!("argv comes from {}", source.label()));
    }

    if resolved.status == SlotStatus::Live
        && let Some(node) = live
    {
        let pane = runner
            .pane_status(socket.as_deref(), &name)
            .unwrap_or(TmuxPaneStatus::Unsupported);
        let node_id = NodeId::MuxSession(node.id.clone());
        if matches!(pane, TmuxPaneStatus::Dead { .. }) {
            plan.warnings.push(format!(
                "the pane of `{name}` has died; its session is replaced"
            ));
            plan.steps.push(kill_step(&target));
            push_new_session(
                &mut plan,
                snapshot,
                &resolved,
                record,
                options,
                env,
                &target,
                &argv,
                harness.as_deref(),
            )?;
        } else if let Some(harness) = harness.as_deref()
            && !argv.is_empty()
            && !mux_runs_harness(snapshot, &node_id, node, harness)
        {
            // Asked to bring a particular session back into an idle
            // mux, type its resume; otherwise just the launch argv.
            let cwd = resolved.text("cwd").map(PathBuf::from).unwrap_or_default();
            let typed = match options.session.as_deref() {
                Some(session) => match build_resume(snapshot, harness, session, &cwd, &argv) {
                    Ok(resume) => resume.argv,
                    Err(why) => {
                        plan.warnings.extend(why.note());
                        argv.clone()
                    }
                },
                None => argv.clone(),
            };
            plan.steps.push(send_keys_step(&target, &typed));
        }
    } else {
        if !runner.can_create_sessions() {
            return Err(RebuildError::Cannot(format!(
                "the {backend} backend can't create sessions, so `{name}` can't be rebuilt"
            )));
        }
        if resolved.status == SlotStatus::Unknown {
            plan.warnings.push(format!(
                "`{name}` may still exist; its server couldn't be checked"
            ));
        }
        push_new_session(
            &mut plan,
            snapshot,
            &resolved,
            record,
            options,
            env,
            &target,
            &argv,
            harness.as_deref(),
        )?;
    }

    if options.attach {
        plan.steps.push(attach_step(&target));
    }
    Ok(plan)
}

fn live_node<'a>(
    snapshot: &'a GraphSnapshot,
    resolved: &ResolvedSlot,
) -> Option<&'a MuxSessionNode> {
    if resolved.status != SlotStatus::Live {
        return None;
    }
    let id = resolved.target_node.as_ref()?;
    match snapshot.find_node(id)? {
        GraphNode::MuxSession(mux) => Some(mux),
        _ => None,
    }
}

fn owner_of(record: Option<&MuxRecord>, name: &str) -> Option<String> {
    record
        .and_then(|record| record.recipe.orchestrator.clone())
        .or_else(|| orchestrator::owner_of_mux(name).map(|o| o.display_name.to_string()))
}

/// The argv to start with. The slot resolver's own answer stands when
/// an override, the live pane, history, or the snapshot supplied it;
/// when it only fell back to the harness default, the launch profiles
/// and `atelier exec` get their say (ADR 0142). The second value is
/// the source of such a default.
fn effective_argv(
    resolved: &ResolvedSlot,
    harness: Option<&str>,
    env: &Env<'_>,
) -> (Vec<OsString>, Option<DefaultSource>) {
    let is_default = resolved
        .effective
        .iter()
        .find(|field| field.name == "argv")
        .is_none_or(|field| field.source == FieldSource::Default);
    if is_default && let Some(harness) = harness {
        let cwd = resolved.text("cwd").map(PathBuf::from);
        if let Some((argv, source)) = env.launch.default_argv(harness, cwd.as_deref()) {
            return (argv, Some(source));
        }
    }
    let argv = resolved
        .list("argv")
        .unwrap_or_default()
        .iter()
        .map(OsString::from)
        .collect();
    (argv, None)
}

fn describe(name: &str, resolved: &ResolvedSlot, record: Option<&MuxRecord>, now: i64) -> String {
    let state = match (resolved.status, record) {
        (SlotStatus::Gone | SlotStatus::Unknown, Some(record)) => format!(
            "{}, last seen {} ago",
            resolved.status.as_str(),
            format_relative_age(record.meta.last_seen, now)
        ),
        (status, _) => status.as_str().to_string(),
    };
    format!("mux `{name}` ({state})")
}

fn display_command(target: &MuxTarget, rest: &str) -> String {
    match target.socket.as_deref() {
        Some(socket) if target.backend == "tmux" => format!("tmux -L {socket} {rest}"),
        _ => format!("{} {rest}", target.backend),
    }
}

fn kill_step(target: &MuxTarget) -> Step {
    Step {
        title: "end the dead session".to_string(),
        command: display_command(target, &format!("kill-session -t {}", target.name)),
        action: Action::KillSession {
            backend: target.backend.clone(),
            socket: target.socket.clone(),
            name: target.name.clone(),
        },
    }
}

fn send_keys_step(target: &MuxTarget, argv: &[OsString]) -> Step {
    let literal = super::launch::format_argv_for_send_keys(argv);
    Step {
        title: "relaunch the agent".to_string(),
        command: display_command(
            target,
            &format!("send-keys -t {} {literal} Enter", target.name),
        ),
        action: Action::SendKeys {
            backend: target.backend.clone(),
            socket: target.socket.clone(),
            name: target.name.clone(),
            argv: argv.to_vec(),
        },
    }
}

fn attach_step(target: &MuxTarget) -> Step {
    Step {
        title: "attach".to_string(),
        command: if target.backend == "tmux" {
            super::launch::format_attach_command(target.socket.as_deref(), &target.name)
        } else {
            format!("{} attach {}", target.backend, target.name)
        },
        action: Action::Attach {
            backend: target.backend.clone(),
            socket: target.socket.clone(),
            name: target.name.clone(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn push_new_session(
    plan: &mut Plan,
    snapshot: &GraphSnapshot,
    resolved: &ResolvedSlot,
    record: Option<&MuxRecord>,
    options: &Options,
    env: &Env<'_>,
    target: &MuxTarget,
    argv: &[OsString],
    harness: Option<&str>,
) -> Result<(), RebuildError> {
    let (cwd, cwd_after, prerequisites) = resolve_cwd(snapshot, resolved, env, target)?;
    plan.steps.extend(prerequisites);

    let resume = match harness {
        Some(harness) if !argv.is_empty() => {
            if let Some(session) = session_to_resume(record, harness, options.session.as_deref()) {
                match build_resume(snapshot, harness, &session, &cwd, argv) {
                    Ok(resume) => Some(resume),
                    Err(why) => {
                        plan.warnings.extend(why.note());
                        None
                    }
                }
            } else {
                if options.session.is_some() {
                    plan.warnings.extend(NoResume::NoSession.note());
                }
                None
            }
        }
        _ => None,
    };

    let shown: Vec<OsString> = resume
        .as_ref()
        .map_or_else(|| argv.to_vec(), |r: &Resume| r.argv.clone());
    let command_tail = if shown.is_empty() {
        String::new()
    } else {
        format!(" {}", super::launch::format_argv_for_send_keys(&shown))
    };
    plan.steps.push(Step {
        title: match &resume {
            Some(resume) => format!("new session, resume {}", short(&resume.session_key)),
            None => "new session".to_string(),
        },
        command: display_command(
            target,
            &format!(
                "new-session -d -s {} -c {}{command_tail}",
                target.name,
                cwd.display()
            ),
        ),
        action: Action::NewSession {
            backend: target.backend.clone(),
            socket: target.socket.clone(),
            name: target.name.clone(),
            cwd,
            cwd_after,
            argv: argv.to_vec(),
            resume,
            harness: harness.map(str::to_string),
        },
    });
    Ok(())
}

/// The directory a new session starts in, and the steps that have to
/// run before it can: a `worktree_branch` override starts it in that
/// branch's worktree, and a working directory inside a worktree that is
/// gone rebuilds the worktree first (ADR 0142).
fn resolve_cwd(
    snapshot: &GraphSnapshot,
    resolved: &ResolvedSlot,
    env: &Env<'_>,
    target: &MuxTarget,
) -> Result<(PathBuf, Option<Landing>, Vec<Step>), RebuildError> {
    let no_cwd = || {
        RebuildError::Cannot(format!(
            "no working directory is recorded for `{0}`; set one with `pin edit {0} --set cwd=<path>`",
            target.name
        ))
    };
    let cwd = resolved.text("cwd").map(PathBuf::from).ok_or_else(no_cwd)?;

    if let Some(branch) = resolved.text("worktree_branch") {
        // The cwd names the repo; the branch picks the worktree.
        let checkout = worktree::checkout_containing(snapshot, &cwd).ok_or_else(|| {
            RebuildError::Cannot(format!(
                "`{}` isn't inside a repo Conspectus knows, so the worktree for `{branch}` can't be found",
                cwd.display()
            ))
        })?;
        let repo_key = NodeId::Repo(checkout.id.repo.clone()).to_string();
        if let Some(root) = worktree::live_worktree_for_branch(snapshot, &repo_key, branch) {
            return Ok((PathBuf::from(root), None, Vec::new()));
        }
        if env.worktree.is_none() {
            return Err(RebuildError::Cannot(
                "no worktree backend can change worktrees; install worktrunk (`wt`)".into(),
            ));
        }
        let create = !worktree::branch_exists(snapshot, &repo_key, branch);
        let key = format!("branch:{branch}");
        let step = worktree::switch_step(
            key.clone(),
            Path::new(&checkout.root),
            &repo_key,
            branch,
            create,
            None,
            None,
            None,
        );
        return Ok((
            cwd,
            Some(Landing {
                key,
                rel: PathBuf::new(),
            }),
            vec![step],
        ));
    }

    if cwd.is_dir() {
        return Ok((cwd, None, Vec::new()));
    }
    // A directory that's gone: is it inside a worktree history can bring back?
    let record = snapshot
        .history
        .checkouts
        .iter()
        .filter(|record| {
            crate::model::path_is_ancestor_of(Path::new(&record.recipe.root), &cwd)
                && !Path::new(&record.recipe.root).exists()
        })
        .max_by_key(|record| record.recipe.root.len());
    let Some(record) = record else {
        return Err(RebuildError::Cannot(format!(
            "the working directory `{}` doesn't exist",
            cwd.display()
        )));
    };
    let subject = Subject {
        slot: crate::model::PinSlot {
            id: record.meta.id.clone(),
            kind: crate::model::PinKind::Checkout,
            target: Some(crate::model::PinSlot::history_target(&record.meta.id)),
            pinned_at: None,
            overrides: crate::model::SlotFields::new(),
            snapshot: crate::model::SlotFields::new(),
            order: u32::MAX,
            legacy: false,
        },
        source: super::SubjectSource::History,
    };
    let sub = worktree::plan(snapshot, &subject, &Options::default(), env, 0)?;
    let root = PathBuf::from(&record.recipe.root);
    let rel = cwd
        .strip_prefix(&root)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    Ok((
        cwd,
        Some(Landing {
            key: root.display().to_string(),
            rel,
        }),
        sub.steps,
    ))
}

fn short(session: &str) -> &str {
    session.get(..8).unwrap_or(session)
}

/// Whether anything shows `harness` running in the mux: a harness
/// process in its panes, a session of the harness the resolver
/// attributed to it, or the harness as the pane's foreground command.
/// Without any of these the agent has exited, and with the process scan
/// off Conspectus can't tell, so it reads as exited only when the pane
/// is plainly not running it.
pub fn mux_runs_harness(
    snapshot: &GraphSnapshot,
    mux_id: &NodeId,
    mux: &MuxSessionNode,
    harness: &str,
) -> bool {
    let processes: std::collections::BTreeSet<&NodeId> = snapshot
        .candidate_links
        .iter()
        .filter(|link| {
            link.relation == RelationKind::MuxContainsProcess
                && matches!(link.state, LinkState::Active)
                && link.source == *mux_id
        })
        .filter_map(GraphLink::target_node_id)
        .collect();
    let process_runs = snapshot.nodes.iter().any(|node| match node {
        GraphNode::RuntimeProcess(process) => {
            process.harness_key.as_deref() == Some(harness)
                && processes.contains(&NodeId::RuntimeProcess(process.id.clone()))
        }
        _ => false,
    });
    if process_runs {
        return true;
    }
    let adapters: std::collections::BTreeMap<&str, &str> = snapshot
        .candidate_links
        .iter()
        .map(|link| (link.id.as_str(), link.source_metadata.adapter.as_str()))
        .collect();
    let session_attributed = snapshot.resolved_relationships.iter().any(|rel| {
        rel.relation == RelationKind::LinkedToMux
            && rel.target == *mux_id
            && rel.selected_link_id.as_ref().is_some_and(|link| {
                adapters.get(link.as_str()) != Some(&crate::history::HISTORY_ADAPTER)
            })
            && matches!(&rel.source, NodeId::AgentSession(s) if s.harness_key == harness)
    });
    if session_attributed {
        return true;
    }
    mux.active_pane_command
        .as_deref()
        .and_then(|command| adapter_in_command([command]))
        .is_some_and(|adapter| adapter.harness_key() == harness)
}

/// The id a rebuilt mux will have in the graph.
pub fn mux_node_id(target: &MuxTarget) -> NodeId {
    let native = match target.socket.as_deref() {
        Some(socket) => format!("{}:{socket}:{}", target.backend, target.name),
        None => format!("{}:{}", target.backend, target.name),
    };
    NodeId::MuxSession(MuxSessionId::new(native))
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
