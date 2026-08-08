//! `conspectus mux` subcommand tree (H-MUX-NEW-001 / ADR 0095).
//!
//! Only `mux new` today — bare tmux session creation with no pin, no
//! agent, no worktree. Sanctioned under the existing ADR 0087
//! category-3 (operator-initiated mux lifecycle) via the same
//! `MuxBackend::new_session` primitive pin launch uses; the difference
//! is empty argv (tmux runs the operator's login shell) and no pin
//! persistence.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use conspectus::discovery::tmux::{MuxBackend, SystemTmux};

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
        }
    }
}

#[derive(Debug, Subcommand)]
enum MuxCommand {
    /// Create a bare tmux session (no pin, no agent, no worktree).
    /// See ADR 0095. The next discovery cycle picks the mux up; if you
    /// want a durable declaration, follow up with `conspectus pin
    /// adopt <name>`.
    New(MuxNewArgs),
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

#[cfg(test)]
mod tests {
    use super::*;
    use conspectus::discovery::tmux::{FakeTmux, TmuxNewSessionOutcome};

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
}
