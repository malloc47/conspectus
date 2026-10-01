//! `conspectus mux` subcommand tree (ADR 0095, ADR 0096).
//!
//! Two verbs:
//! - `mux new` — bare tmux session, no pin, no agent, no worktree.
//! - `mux launch` — harness in a fresh tmux, no pin persistence.
//!
//! Both are sanctioned under ADR 0087 category 3 (operator-initiated
//! mux lifecycle) plus category 4 (Conspectus-constructed harness
//! argv for `mux launch`). Neither writes a pin or sidecar.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use crate::discovery::harness::launch_argv_for;
use crate::discovery::tmux::{MuxBackend, SystemTmux};

use super::pin::{attach_and_report, format_attach_command, report_new_session};

#[derive(Debug, Args)]
pub struct MuxArgs {
    #[command(subcommand)]
    command: MuxCommand,
}

impl MuxArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            MuxCommand::New(args) => args.run(),
            MuxCommand::Launch(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum MuxCommand {
    /// Create a bare tmux session (no pin, no agent, no worktree).
    /// The next discovery cycle picks the mux up; if you
    /// want a durable declaration, follow up with `conspectus pin
    /// adopt <name>`.
    New(MuxNewArgs),
    /// Launch a harness in a fresh tmux session with no pin
    /// persistence. Distinct from `pin launch` (which
    /// requires + writes a pin) and from `mux new` (which spawns a
    /// bare shell). The mux and its attributed harness session
    /// appear in discovery on the next refresh; `conspectus pin
    /// adopt <name>` remains the after-the-fact persistence path.
    Launch(MuxLaunchArgs),
}

#[derive(Debug, Args)]
struct MuxNewArgs {
    /// tmux session name.
    name: String,
    /// Working directory tmux passes as `-c`. Defaults to `$PWD`.
    /// Must exist on disk at invocation time.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Non-default tmux socket (`tmux -L <name>`). Absent ⇒ default
    /// socket.
    #[arg(long, value_name = "NAME")]
    socket: Option<String>,
    /// Skip the terminal hand-off. The new session is spawned
    /// detached and the attach command is printed for the operator
    /// to run by hand. Mirrors `pin launch --no-attach`.
    #[arg(long = "no-attach")]
    no_attach: bool,
}

impl MuxNewArgs {
    fn run(self) -> Result<()> {
        let runner = SystemTmux::new();
        self.run_with_runner(&runner)
    }

    fn run_with_runner(self, runner: &dyn MuxBackend) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("mux new requires a non-empty <NAME>");
        }
        let cwd = match self.cwd {
            Some(cwd) => cwd,
            None => std::env::current_dir()?,
        };
        if !cwd.is_dir() {
            bail!(
                "mux new --cwd `{}` is not an existing directory",
                cwd.display()
            );
        }
        let socket = self.socket.as_deref();
        let name = self.name.as_str();
        // ADR 0095: empty argv — tmux launches the operator's login
        // shell. No `send-keys` is used to seed the pane (ADR 0028).
        let outcome = runner
            .new_session(socket, name, &cwd, &[])
            .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
        report_new_session(outcome, name)?;
        if self.no_attach {
            let attach_cmd = format_attach_command(socket, name);
            println!("spawned `{name}` (detached); attach with: {attach_cmd}");
            return Ok(());
        }
        attach_and_report(runner, socket, name)
    }
}

#[derive(Debug, Args)]
struct MuxLaunchArgs {
    /// Harness key (e.g. `codex`, `claude-code`, `opencode`).
    /// Matched against `HarnessAdapter::harness_key`.
    harness: String,
    /// tmux session name for the new mux. Required.
    #[arg(long)]
    name: String,
    /// Working directory tmux passes as `-c`. Defaults to `$PWD`.
    /// Must exist on disk at invocation time. Ignored when
    /// `--worktree-branch` is set and the realized worktree path is
    /// used instead.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Non-default tmux socket (`tmux -L <name>`). Absent ⇒ default
    /// socket.
    #[arg(long, value_name = "NAME")]
    socket: Option<String>,
    /// Override the harness adapter's default launch argv. Every
    /// value after `--argv` up to the next flag is treated as a
    /// launch-argv token.
    #[arg(long, value_name = "ARG", num_args = 1..)]
    argv: Option<Vec<String>>,
    /// Realize the worktree for `<BRANCH>` under `--worktree-repo`
    /// at launch, and use the worktree path as the mux cwd
    /// (reuses the worktree if it already exists). Requires
    /// `--worktree-repo`.
    #[arg(long = "worktree-branch", value_name = "BRANCH")]
    worktree_branch: Option<String>,
    /// Repo anchor for the worktree realization. Required with
    /// `--worktree-branch`, ignored otherwise.
    #[arg(long = "worktree-repo", value_name = "PATH")]
    worktree_repo: Option<PathBuf>,
    /// Skip the terminal hand-off. Mirrors `pin launch --no-attach`
    /// and `mux new --no-attach`.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Accepted for symmetry with other verbs; `mux launch` has no
    /// discovery-driven state, so it's hidden from help.
    #[arg(long = "scan-root", value_name = "PATH", hide = true)]
    #[allow(dead_code)]
    scan_root: Vec<PathBuf>,
}

impl MuxLaunchArgs {
    fn run(self) -> Result<()> {
        let runner = SystemTmux::new();
        self.run_with_runner(&runner)
    }

    fn run_with_runner(self, runner: &dyn MuxBackend) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("mux launch requires a non-empty --name");
        }
        if self.harness.trim().is_empty() {
            bail!("mux launch requires a non-empty <HARNESS>");
        }
        // Resolve cwd: prefer worktree realization when --worktree-branch
        // is set; otherwise use --cwd or $PWD.
        let cwd = match (&self.worktree_branch, &self.worktree_repo) {
            (Some(branch), Some(repo)) => realize_worktree(repo, branch)?,
            (Some(_), None) => bail!("mux launch: --worktree-branch requires --worktree-repo"),
            (None, Some(_)) => {
                bail!("mux launch: --worktree-repo is only meaningful with --worktree-branch")
            }
            (None, None) => match self.cwd {
                Some(cwd) => cwd,
                None => std::env::current_dir()?,
            },
        };
        if !cwd.is_dir() {
            bail!(
                "mux launch --cwd `{}` is not an existing directory",
                cwd.display()
            );
        }
        // Resolve argv: --argv override, else the harness adapter's
        // default. Empty for both is a hard error since tmux would
        // start an unattributed process.
        let argv: Vec<std::ffi::OsString> = match self.argv {
            Some(argv) if !argv.is_empty() => {
                argv.into_iter().map(std::ffi::OsString::from).collect()
            }
            _ => launch_argv_for(self.harness.trim()),
        };
        if argv.is_empty() {
            bail!(
                "mux launch: no launch argv for harness `{}`; pass `--argv` explicitly",
                self.harness
            );
        }
        let socket = self.socket.as_deref();
        let name = self.name.as_str();
        let outcome = runner
            .new_session(socket, name, &cwd, &argv)
            .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
        report_new_session(outcome, name)?;
        if self.no_attach {
            let attach_cmd = format_attach_command(socket, name);
            println!("launched `{name}` (detached); attach with: {attach_cmd}");
            return Ok(());
        }
        attach_and_report(runner, socket, name)
    }
}

/// Realize the worktree for `branch` under `repo` and return the
/// worktree's absolute path. Idempotent — if the worktree already
/// exists, its path is returned unchanged. Mirrors ADR 0094 pin-side
/// realize-at-launch minus the pin-store read.
fn realize_worktree(repo: &std::path::Path, branch: &str) -> Result<PathBuf> {
    use crate::discovery::worktree::{
        WorktreeBackendSelection, WorktreeCreateRequest, WorktreeMutationOutcome,
        resolve_mutation_backend, worktrunk_available,
    };
    // If a worktree already exists for this branch, prefer its path.
    let backend_selection = WorktreeBackendSelection::default();
    let backend = resolve_mutation_backend(backend_selection, worktrunk_available())?
        .ok_or_else(|| anyhow!("mux launch: no worktree mutation backend configured"))?;
    if let Some(existing) = existing_worktree_for_branch(&*backend, repo, branch)? {
        return Ok(existing);
    }
    let request = WorktreeCreateRequest {
        repo_root: repo.to_path_buf(),
        branch: branch.to_string(),
        base: None,
    };
    let outcome = backend
        .create(&request)
        .map_err(|err| anyhow!("worktree create failed: {err}"))?;
    match outcome {
        WorktreeMutationOutcome::Succeeded { path: Some(path) } => Ok(PathBuf::from(path)),
        WorktreeMutationOutcome::Succeeded { path: None } => {
            // Re-list to locate the freshly created worktree.
            existing_worktree_for_branch(&*backend, repo, branch)?
                .ok_or_else(|| anyhow!("worktree created but not found in listing"))
        }
        WorktreeMutationOutcome::Unsupported => {
            bail!("mux launch: configured worktree backend does not support create")
        }
        WorktreeMutationOutcome::Failed { code, message } => {
            bail!(
                "mux launch: worktree create failed (exit {}): {message}",
                code.map_or_else(|| "?".to_string(), |c| c.to_string())
            )
        }
    }
}

fn existing_worktree_for_branch(
    backend: &dyn crate::discovery::worktree::WorktreeBackend,
    repo: &std::path::Path,
    branch: &str,
) -> Result<Option<PathBuf>> {
    let records = backend
        .list(repo)
        .map_err(|err| anyhow!("worktree list failed: {err}"))?;
    let short = branch.trim_start_matches("refs/heads/");
    Ok(records
        .into_iter()
        .find(|rec| {
            rec.branch
                .as_deref()
                .is_some_and(|b| b.trim_start_matches("refs/heads/") == short)
        })
        .map(|rec| PathBuf::from(rec.path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::tmux::{FakeTmux, TmuxNewSessionOutcome};

    #[test]
    fn mux_new_spawns_bare_session_with_empty_argv() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("bare", TmuxNewSessionOutcome::Created);
        let cwd = std::env::temp_dir();
        let args = MuxNewArgs {
            name: "bare".to_string(),
            cwd: Some(cwd.clone()),
            socket: None,
            no_attach: true,
        };
        args.run_with_runner(&runner).expect("bare mux create");
        let calls = runner.new_session_calls();
        assert_eq!(calls.len(), 1);
        let (socket, name, argv_cwd, argv) = &calls[0];
        assert_eq!(socket.as_deref(), None);
        assert_eq!(name, "bare");
        assert_eq!(argv_cwd, &cwd);
        assert!(argv.is_empty(), "bare mux new must pass empty argv");
    }

    #[test]
    fn mux_new_rejects_empty_name() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxNewArgs {
            name: "   ".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            no_attach: true,
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("empty name rejected");
        assert!(err.to_string().contains("non-empty"));
        assert!(runner.new_session_calls().is_empty());
    }

    #[test]
    fn mux_new_rejects_missing_cwd() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxNewArgs {
            name: "bare".to_string(),
            cwd: Some(PathBuf::from("/nonexistent/definitely-not-here-12345")),
            socket: None,
            no_attach: true,
        };
        let err = args.run_with_runner(&runner).expect_err("bad cwd rejected");
        assert!(err.to_string().contains("not an existing directory"));
        assert!(runner.new_session_calls().is_empty());
    }

    #[test]
    fn mux_new_threads_socket_argument() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("boxed", TmuxNewSessionOutcome::Created);
        let args = MuxNewArgs {
            name: "boxed".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: Some("iso".to_string()),
            no_attach: true,
        };
        args.run_with_runner(&runner).expect("socketed create");
        let (socket, _, _, _) = &runner.new_session_calls()[0];
        assert_eq!(socket.as_deref(), Some("iso"));
    }

    #[test]
    fn mux_new_surfaces_name_taken_error() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("dup", TmuxNewSessionOutcome::NameTaken);
        let args = MuxNewArgs {
            name: "dup".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            no_attach: true,
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("duplicate name should error");
        let msg = err.to_string();
        assert!(msg.contains("already exists"), "unexpected msg: {msg}");
    }

    // -- mux launch ---------------------------------------------------

    #[test]
    fn mux_launch_spawns_with_harness_default_argv() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("adhoc", TmuxNewSessionOutcome::Created);
        let cwd = std::env::temp_dir();
        let args = MuxLaunchArgs {
            harness: "codex".to_string(),
            name: "adhoc".to_string(),
            cwd: Some(cwd.clone()),
            socket: None,
            argv: None,
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        args.run_with_runner(&runner).expect("mux launch");
        let calls = runner.new_session_calls();
        assert_eq!(calls.len(), 1);
        let (socket, name, argv_cwd, argv) = &calls[0];
        assert_eq!(socket.as_deref(), None);
        assert_eq!(name, "adhoc");
        assert_eq!(argv_cwd, &cwd);
        assert!(
            !argv.is_empty(),
            "mux launch must pass the harness's default argv"
        );
        // Codex adapter's default argv is `["codex"]`.
        assert_eq!(argv[0], std::ffi::OsString::from("codex"));
    }

    #[test]
    fn mux_launch_argv_override_overrides_harness_default() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("adhoc", TmuxNewSessionOutcome::Created);
        let args = MuxLaunchArgs {
            harness: "codex".to_string(),
            name: "adhoc".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            argv: Some(vec![
                "codex".to_string(),
                "--model".to_string(),
                "gpt-5".to_string(),
            ]),
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        args.run_with_runner(&runner).expect("mux launch");
        let (_, _, _, argv) = &runner.new_session_calls()[0];
        assert_eq!(argv.len(), 3);
        assert_eq!(argv[1], std::ffi::OsString::from("--model"));
        assert_eq!(argv[2], std::ffi::OsString::from("gpt-5"));
    }

    #[test]
    fn mux_launch_rejects_empty_name() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxLaunchArgs {
            harness: "codex".to_string(),
            name: "   ".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            argv: None,
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("empty name rejected");
        assert!(err.to_string().contains("non-empty"));
        assert!(runner.new_session_calls().is_empty());
    }

    #[test]
    fn mux_launch_rejects_empty_harness() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxLaunchArgs {
            harness: "  ".to_string(),
            name: "adhoc".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            argv: None,
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("empty harness rejected");
        assert!(err.to_string().contains("HARNESS"));
    }

    #[test]
    fn mux_launch_rejects_worktree_branch_without_repo() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxLaunchArgs {
            harness: "codex".to_string(),
            name: "adhoc".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            argv: None,
            worktree_branch: Some("feat/foo".to_string()),
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("branch without repo rejected");
        assert!(err.to_string().contains("--worktree-repo"));
    }

    #[test]
    fn mux_launch_threads_socket_argument() {
        let runner =
            FakeTmux::with_sessions("").with_new_session("boxed", TmuxNewSessionOutcome::Created);
        let args = MuxLaunchArgs {
            harness: "codex".to_string(),
            name: "boxed".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: Some("iso".to_string()),
            argv: None,
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        args.run_with_runner(&runner).expect("socketed launch");
        let (socket, _, _, _) = &runner.new_session_calls()[0];
        assert_eq!(socket.as_deref(), Some("iso"));
    }

    #[test]
    fn mux_launch_rejects_unknown_harness_with_no_argv() {
        let runner = FakeTmux::with_sessions("");
        let args = MuxLaunchArgs {
            harness: "not-a-real-harness".to_string(),
            name: "adhoc".to_string(),
            cwd: Some(std::env::temp_dir()),
            socket: None,
            argv: None,
            worktree_branch: None,
            worktree_repo: None,
            no_attach: true,
            scan_root: Vec::new(),
        };
        let err = args
            .run_with_runner(&runner)
            .expect_err("unknown harness w/o argv rejected");
        assert!(err.to_string().contains("no launch argv"));
    }
}
