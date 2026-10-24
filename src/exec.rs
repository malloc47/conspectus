//! The one place Conspectus starts other programs (ADR 0117).
//!
//! Every subprocess is built by [`command`], which names the
//! [`Program`] it runs. `docs/capabilities.toml` lists what each program
//! may be asked to do. A source test fails on a `Command::new` anywhere
//! else, and a traced run of the built binary checks every program it
//! starts, and its arguments, against that manifest.

use std::ffi::OsStr;
use std::process::Command;

/// A program Conspectus may run. Each has an entry under `[programs]`
/// in `docs/capabilities.toml`, keyed by [`Program::key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    /// `git`: read-only queries of the repos discovery finds. The dev
    /// scenario builder also creates and commits to repos of its own.
    Git,
    /// `gh`: GitHub pull requests, the only network access (ADR 0011).
    Gh,
    /// `tmux`: session discovery and pane capture, plus the session
    /// lifecycle the operator starts (ADR 0143).
    Tmux,
    /// `zellij`: session discovery and attach.
    Zellij,
    /// `wt` (worktrunk): worktree changes the operator starts (ADR 0092).
    Worktrunk,
    /// A harness binary (`claude`, `codex`, ...) started to resume a
    /// session outside tmux.
    Harness,
    /// `claude-history`, the external viewer for Claude Code transcripts.
    ClaudeHistory,
    /// `$PAGER`, `less`, or `more` for long table output.
    Pager,
    /// Conspectus itself, run again by the TUI for a launch.
    Conspectus,
}

impl Program {
    pub const ALL: [Program; 9] = [
        Program::Git,
        Program::Gh,
        Program::Tmux,
        Program::Zellij,
        Program::Worktrunk,
        Program::Harness,
        Program::ClaudeHistory,
        Program::Pager,
        Program::Conspectus,
    ];

    /// The program's key in `docs/capabilities.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Program::Git => "git",
            Program::Gh => "gh",
            Program::Tmux => "tmux",
            Program::Zellij => "zellij",
            Program::Worktrunk => "wt",
            Program::Harness => "harness",
            Program::ClaudeHistory => "claude-history",
            Program::Pager => "pager",
            Program::Conspectus => "conspectus",
        }
    }
}

/// A [`Command`] that runs `program` as `binary`: a name looked up on
/// `PATH`, or a path. `program` says which manifest entry allows it.
pub fn command(program: Program, binary: impl AsRef<OsStr>) -> Command {
    let _ = program;
    Command::new(binary)
}
