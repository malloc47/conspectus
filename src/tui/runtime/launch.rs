//! Subprocess launches: mux new / launch, tmux attach, and the external viewer.

use super::*;
use crate::tui::messages::{CommandRecord, LogEntry, LogTarget};

/// Run `conspectus <args>` and capture its full output, both for the
/// caller's summary and as the command record of the message log entry
/// (ADR 0105). Output is captured and stdin is null, so the TUI stays
/// up and shows a spinner chip labelled `label` meanwhile (ADR 0127).
fn run_conspectus_subprocess(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    label: String,
    args: Vec<String>,
) -> (CommandRecord, std::io::Result<Output>) {
    let program =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("conspectus"));
    let output = {
        let program = program.clone();
        let args = args.clone();
        run_with_terminal_spinner(terminal, app, label, move || {
            crate::exec::command(crate::exec::Program::Conspectus, &program)
                .args(&args)
                .output()
        })
    };

    let mut argv = vec![program.display().to_string()];
    argv.extend(args);
    let record = match &output {
        Ok(output) => CommandRecord::from_output(argv, output),
        Err(err) => CommandRecord {
            argv,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("failed to spawn: {err}"),
        },
    };
    (record, output)
}

/// Log entry for a launch subprocess. A launch that succeeded but
/// wrote to stderr (a resume that fell back to a fresh launch, a
/// cleared sidecar) is a warning, so the note isn't lost.
pub(super) fn launch_entry(
    summary: &LaunchSummary,
    record: CommandRecord,
    target: LogTarget,
) -> LogEntry {
    let entry = if !summary.success {
        LogEntry::error(&summary.message)
    } else if record.stderr.trim().is_empty() {
        LogEntry::info(&summary.message)
    } else {
        LogEntry::warning(&summary.message)
    };
    entry.with_target(target).with_command(record)
}

/// Log entry for an attach that returned. When the pane is now dead,
/// the process inside exited non-zero, at startup or while attached
/// (ADR 0103, ADR 0133), and its output is the entry's detail.
pub(super) fn attach_return_entry(
    what: &str,
    outcome: AttachOutcome,
    target: LogTarget,
    mux: &AttachTarget,
    socket: Option<&str>,
) -> LogEntry {
    if let AttachOutcome::Failed(reason) = outcome {
        return LogEntry::error(format!("{what} attach failed: {reason}")).with_target(target);
    }
    let attached = LogEntry::info(format!("{what} attached/detached")).with_target(target.clone());
    let Some(runner) = crate::discovery::system_mux_backend(&mux.backend) else {
        return attached;
    };
    let native_id = mux.native_id.as_str();
    match runner.pane_status(socket, native_id) {
        Ok(crate::discovery::tmux::TmuxPaneStatus::Dead { status }) => {
            let status = status.map_or_else(String::new, |code| format!(" with status {code}"));
            LogEntry::error(format!("{what}: process in `{native_id}` exited{status}"))
                .with_target(target)
                .with_detail(crate::discovery::tmux::dead_pane_output(
                    runner.as_ref(),
                    socket,
                    native_id,
                ))
        }
        _ => attached,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LaunchSummary {
    pub(super) success: bool,
    pub(super) message: String,
}

/// Argv the TUI passes when re-execing into `conspectus mux new`
/// (ADR 0095). Split out so the shape is unit-testable
/// without running the actual subprocess. `--no-attach` is always
/// present so the subprocess exits after spawning; the TUI's own
/// attach path takes over once we're back in the alt screen.
pub(super) fn mux_new_argv(name: &str, cwd: &str) -> Vec<String> {
    vec![
        "mux".to_string(),
        "new".to_string(),
        name.to_string(),
        "--cwd".to_string(),
        cwd.to_string(),
        "--no-attach".to_string(),
    ]
}

/// Executor branch for `ExecSpec::MuxNew`: behind a spinner,
/// re-exec into `conspectus mux new <name> --cwd <cwd> --no-attach`,
/// then attach to the resulting tmux session on success (refreshing
/// after the attach returns, ADR 0133). No store write.
pub(super) fn execute_mux_new(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    name: &str,
    cwd: &str,
) {
    let args = mux_new_argv(name, cwd);
    let (record, output) =
        run_conspectus_subprocess(terminal, app, format!("Creating {name}"), args);

    let mux = MuxSessionId::new(format!("tmux:{name}"));
    let launch = summarize_mux_new_output(name, output);
    let entry = launch_entry(&launch, record, LogTarget::Mux(mux.clone()));
    if !launch.success {
        refresh_after_launch_with_spinner(terminal, app);
        app.report(entry);
        return;
    }
    app.report(entry);

    let attach_target = AttachTarget {
        mux: mux.clone(),
        backend: "tmux".to_string(),
        native_id: name.to_string(),
    };
    let outcome = run_tmux_attach(terminal, &attach_target);
    app.update(Msg::HandoffReturned(mux.clone()));
    app.report(attach_return_entry(
        &format!("mux `{name}`"),
        outcome,
        LogTarget::Mux(mux),
        &attach_target,
        None,
    ));
}

pub(super) fn summarize_mux_new_output(
    name: &str,
    output: std::io::Result<Output>,
) -> LaunchSummary {
    match output {
        Ok(output) if output.status.success() => {
            let detail = command_output_excerpt(&output);
            LaunchSummary {
                success: true,
                message: if detail.is_empty() {
                    format!("mux `{name}` created")
                } else {
                    format!("mux `{name}` created: {detail}")
                },
            }
        }
        Ok(output) => {
            let detail = command_output_excerpt(&output);
            LaunchSummary {
                success: false,
                message: if detail.is_empty() {
                    format!("mux `{name}` create exited with {}", output.status)
                } else {
                    format!("mux `{name}` create failed: {detail}")
                },
            }
        }
        Err(err) => LaunchSummary {
            success: false,
            message: format!("mux `{name}` create failed to spawn: {err}"),
        },
    }
}

/// Argv the TUI passes when re-execing into
/// `conspectus mux launch` (ADR 0096). Split out
/// so the shape is unit-testable without running the actual
/// subprocess. `--no-attach` is always present so the subprocess
/// exits after spawning; the TUI's own attach path takes over once
/// we're back in the alt screen.
pub(super) fn mux_launch_argv(
    request: &crate::tui::widgets::mux_launch::MuxLaunchRequest,
) -> Vec<String> {
    let mut args = vec![
        "mux".to_string(),
        "launch".to_string(),
        request.harness.clone(),
        "--name".to_string(),
        request.name.clone(),
        "--cwd".to_string(),
        request.cwd.clone(),
        "--no-attach".to_string(),
    ];
    if let Some(socket) = request.mux_socket.as_deref() {
        args.push("--socket".to_string());
        args.push(socket.to_string());
    }
    if let Some(branch) = request.worktree_branch.as_deref() {
        args.push("--worktree-branch".to_string());
        args.push(branch.to_string());
    }
    if let Some(repo) = request.worktree_repo.as_deref() {
        args.push("--worktree-repo".to_string());
        args.push(repo.to_string());
    }
    if !request.launch_argv.is_empty() {
        args.push("--argv".to_string());
        for arg in &request.launch_argv {
            args.push(arg.clone());
        }
    }
    args
}

/// Executor branch for `ExecSpec::MuxLaunch`: behind a spinner,
/// re-exec into `conspectus mux launch …`, then attach. The row tree
/// picks up the fresh mux and its harness session from the refresh
/// that runs after the attach returns (ADR 0133). Mirrors [`execute_launch_pin`] and
/// [`execute_mux_new`] but with the mux-launch argv shape.
pub(super) fn execute_mux_launch(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    request: crate::tui::widgets::mux_launch::MuxLaunchRequest,
) {
    let name = request.name.clone();
    let args = mux_launch_argv(&request);
    let (record, output) =
        run_conspectus_subprocess(terminal, app, format!("Launching {name}"), args);

    let mux = MuxSessionId::new(format!("tmux:{name}"));
    let launch = summarize_mux_launch_output(&name, output);
    let entry = launch_entry(&launch, record, LogTarget::Mux(mux.clone()));
    if !launch.success {
        refresh_after_launch_with_spinner(terminal, app);
        app.report(entry);
        return;
    }
    app.report(entry);

    let attach_target = AttachTarget {
        mux: mux.clone(),
        backend: "tmux".to_string(),
        native_id: name.clone(),
    };
    let outcome =
        run_tmux_attach_with_socket(terminal, &attach_target, request.mux_socket.as_deref());
    app.update(Msg::HandoffReturned(mux.clone()));
    app.report(attach_return_entry(
        &format!("mux `{name}` launch"),
        outcome,
        LogTarget::Mux(mux),
        &attach_target,
        request.mux_socket.as_deref(),
    ));
}

pub(super) fn summarize_mux_launch_output(
    name: &str,
    output: std::io::Result<Output>,
) -> LaunchSummary {
    match output {
        Ok(output) if output.status.success() => {
            let detail = command_output_excerpt(&output);
            LaunchSummary {
                success: true,
                message: if detail.is_empty() {
                    format!("mux `{name}` launched")
                } else {
                    format!("mux `{name}` launched: {detail}")
                },
            }
        }
        Ok(output) => {
            let detail = command_output_excerpt(&output);
            LaunchSummary {
                success: false,
                message: if detail.is_empty() {
                    format!("mux `{name}` launch exited with {}", output.status)
                } else {
                    format!("mux `{name}` launch failed: {detail}")
                },
            }
        }
        Err(err) => LaunchSummary {
            success: false,
            message: format!("mux `{name}` launch failed to spawn: {err}"),
        },
    }
}

pub(super) fn command_output_excerpt(output: &Output) -> String {
    let mut lines = String::new();
    lines.push_str(&String::from_utf8_lossy(&output.stderr));
    if lines.trim().is_empty() {
        lines.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    lines
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .chars()
        .take(180)
        .collect()
}

/// Outcome of a single attach attempt. Errors carry a
/// human-readable reason for the status bar.
#[derive(Debug)]
pub(super) enum AttachOutcome {
    /// `tmux attach-session` ran and exited (operator detached,
    /// pane was closed, tmux returned successfully, etc.).
    Detached,
    /// We never got to `tmux` cleanly, or `tmux` exited non-zero
    /// with a message worth surfacing.
    Failed(String),
}

/// Leave the alt screen + raw mode, spawn `tmux attach-session
/// -t <native_id>` inheriting the parent terminal, wait for it to
/// exit, then re-enter the alt screen. The caller's
/// `DefaultTerminal` is replaced in place so the resumed event
/// loop draws into the fresh terminal.
pub(super) fn run_tmux_attach(
    terminal: &mut DefaultTerminal,
    target: &AttachTarget,
) -> AttachOutcome {
    run_tmux_attach_with_socket(terminal, target, None)
}

/// How an attach target's backend is driven.
pub(super) enum AttachRoute {
    /// tmux, attached directly so nested clients get `switch-client`.
    Tmux,
    /// Any other registered backend, through its `MuxBackend` runner.
    Backend(Box<dyn MuxBackend>),
    /// A backend key this build has no runner for.
    Unknown,
}

pub(super) fn attach_route(backend_key: &str) -> AttachRoute {
    if backend_key == crate::discovery::tmux::TMUX_BACKEND {
        return AttachRoute::Tmux;
    }
    crate::discovery::system_mux_backend(backend_key)
        .map_or(AttachRoute::Unknown, AttachRoute::Backend)
}

/// Attach to `target` through the backend it belongs to.
pub(super) fn run_mux_attach(
    terminal: &mut DefaultTerminal,
    target: &AttachTarget,
    socket_name: Option<&str>,
) -> AttachOutcome {
    let runner = match attach_route(&target.backend) {
        AttachRoute::Tmux => return run_tmux_attach_with_socket(terminal, target, socket_name),
        AttachRoute::Backend(runner) => runner,
        AttachRoute::Unknown => {
            return AttachOutcome::Failed(format!(
                "no runner for mux backend `{}`",
                target.backend
            ));
        }
    };
    ratatui::restore();
    let outcome = runner.attach_session(socket_name, &target.native_id);
    *terminal = ratatui::init();
    let _ = terminal.clear();
    attach_outcome(&target.backend, outcome)
}

pub(super) fn attach_outcome(
    backend: &str,
    outcome: anyhow::Result<crate::discovery::tmux::TmuxAttachOutcome>,
) -> AttachOutcome {
    use crate::discovery::tmux::TmuxAttachOutcome;
    match outcome {
        Ok(TmuxAttachOutcome::Detached) => AttachOutcome::Detached,
        Ok(TmuxAttachOutcome::NoTarget) => {
            AttachOutcome::Failed(format!("{backend} session not found"))
        }
        Ok(TmuxAttachOutcome::Unavailable(reason)) => {
            AttachOutcome::Failed(format!("{backend} is unavailable: {}", reason.as_str()))
        }
        Ok(TmuxAttachOutcome::Failed { message, .. }) => AttachOutcome::Failed(message),
        Ok(TmuxAttachOutcome::Unsupported) => {
            AttachOutcome::Failed(format!("the {backend} runner does not support attach"))
        }
        Err(err) => AttachOutcome::Failed(format!("could not launch {backend}: {err}")),
    }
}

pub(super) fn run_tmux_attach_with_socket(
    terminal: &mut DefaultTerminal,
    target: &AttachTarget,
    socket_name: Option<&str>,
) -> AttachOutcome {
    // Suspend the ratatui terminal so tmux owns the real screen
    // for the duration of the attach.
    ratatui::restore();

    let nested = std::env::var_os("TMUX").is_some();
    let subcommand = if nested {
        "switch-client"
    } else {
        "attach-session"
    };
    let mut command = crate::exec::command(crate::exec::Program::Tmux, "tmux");
    if let Some(socket) = socket_name.filter(|socket| *socket != "default") {
        command.args(["-L", socket]);
    }
    let status = command.args([subcommand, "-t", &target.native_id]).status();

    // Re-enter the alt screen + raw mode and swap the terminal in
    // place. Failure to re-init is fatal for the TUI, but we
    // attempted restore() first so the shell stays usable.
    *terminal = ratatui::init();
    // Forget any cached frame state — the parent screen was
    // overwritten by tmux, and a clean clear avoids ghost cells
    // from the suspended buffer.
    let _ = terminal.clear();

    match status {
        Ok(s) if s.success() => AttachOutcome::Detached,
        Ok(s) => AttachOutcome::Failed(format!("tmux {subcommand} exited with {s}")),
        Err(err) => AttachOutcome::Failed(format!("could not launch tmux: {err}")),
    }
}

/// Outcome of a single viewer launch attempt. Errors carry a
/// human-readable reason for the status bar, parallel to
/// [`AttachOutcome`].
#[derive(Debug)]
pub(super) enum ViewerOutcome {
    /// The viewer ran and exited (operator closed it, viewer
    /// returned successfully, etc.).
    Exited,
    /// We never got to the viewer cleanly, or it exited non-zero
    /// with a message worth surfacing.
    Failed(String),
}

/// Leave the alt screen + raw mode, spawn the viewer inheriting the
/// parent terminal, wait for it to exit, then re-enter the alt
/// screen. Mirrors `run_tmux_attach` with two viewer-specific
/// tweaks:
///   1. Explicit `Clear(All)` + cursor-to-origin after restoring the
///      terminal. Plain `ratatui::restore()` is enough for local
///      terminals but on mosh / nested muxers the LeaveAlternateScreen
///      sequence can be coalesced with the child's first writes,
///      leaving the viewer's output overlaid on the dropped TUI
///      buffer. The clear forces a clean canvas.
///   2. On non-zero exit, hold for Enter before re-entering the alt
///      screen so the operator can read whatever the viewer
///      printed to stderr (e.g. `recall`'s "Session not found")
///      instead of having it wiped by the re-render.
pub(super) fn run_viewer_launch(
    terminal: &mut DefaultTerminal,
    plan: &LaunchPlan,
) -> ViewerOutcome {
    use ratatui::crossterm::{
        cursor::MoveTo,
        execute,
        terminal::{Clear, ClearType},
    };

    ratatui::restore();
    let _ = execute!(std::io::stdout(), Clear(ClearType::All), MoveTo(0, 0));

    let status = crate::exec::command(crate::exec::Program::ClaudeHistory, &plan.program)
        .args(&plan.args)
        .status();

    let outcome = match status {
        Ok(s) if s.success() => ViewerOutcome::Exited,
        Ok(s) => ViewerOutcome::Failed(format!("{} exited with {s}", plan.program)),
        Err(err) => ViewerOutcome::Failed(format!("could not launch {}: {err}", plan.program)),
    };

    if matches!(outcome, ViewerOutcome::Failed(_)) {
        eprintln!("\n[viewer exited non-zero — press Enter to return to conspectus]");
        let mut buf = String::new();
        let _ = std::io::stdin().read_line(&mut buf);
    }

    *terminal = ratatui::init();
    let _ = terminal.clear();
    outcome
}
