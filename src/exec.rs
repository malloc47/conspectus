//! The one place Conspectus starts other programs (ADR 0117).
//!
//! Every subprocess is built by [`command`], which names the
//! [`Program`] it runs. `docs/capabilities.toml` lists what each program
//! may be asked to do. A source test fails on a `Command::new` anywhere
//! else, and a traced run of the built binary checks every program it
//! starts, and its arguments, against that manifest.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
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
    /// session outside tmux, or `claude agents --json`, polled to repair
    /// a live state (ADR 0218).
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

/// What Linux appends to the path of `/proc/self/exe` once the file it
/// names has been replaced or removed, as a rebuild of the binary does.
const DELETED_MARK: &str = " (deleted)";

/// The program that runs Conspectus again, and whether it is the one this
/// process started from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningBinary {
    pub path: PathBuf,
    pub fallback: Option<Fallback>,
}

/// Why [`RunningBinary::path`] is not where this process started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// The file this process started from was replaced, as a rebuild
    /// does: `path` is whatever is at that place now.
    Replaced,
    /// Nothing is at that place now: `path` runs the image this process
    /// is running, which stays reachable through `/proc` while it runs.
    Removed,
}

impl RunningBinary {
    /// The binary to run for a child that is Conspectus again. A TUI that
    /// has run for a while can be left with a path that no longer holds
    /// what it started from, since `std::env::current_exe` reports a
    /// replaced file as `<path> (deleted)`, a name nothing has.
    ///
    /// In order of preference: the path itself; the file now at that
    /// place, which is also what a restarted daemon and a new CLI run, so
    /// the child and the daemon agree; and last the running image, which
    /// still works but is the old version. A caller that gets a
    /// [`Fallback`] should tell the operator the TUI is out of date.
    pub fn locate() -> Self {
        locate(std::env::current_exe(), std::process::id(), Path::is_file)
    }
}

fn locate(current: io::Result<PathBuf>, pid: u32, exists: impl Fn(&Path) -> bool) -> RunningBinary {
    let Ok(current) = current else {
        // As before: whatever `conspectus` is on `PATH`.
        return RunningBinary {
            path: PathBuf::from("conspectus"),
            fallback: None,
        };
    };
    // A file that really has the marker in its name is checked first, so
    // it is not taken for a deleted one.
    if exists(&current) {
        return RunningBinary {
            path: current,
            fallback: None,
        };
    }
    let Some(original) = current.to_str().and_then(|s| s.strip_suffix(DELETED_MARK)) else {
        return RunningBinary {
            path: current,
            fallback: None,
        };
    };
    let original = PathBuf::from(original);
    if exists(&original) {
        return RunningBinary {
            path: original,
            fallback: Some(Fallback::Replaced),
        };
    }
    RunningBinary {
        path: proc_exe(pid),
        fallback: Some(Fallback::Removed),
    }
}

/// The running image of process `pid`, which `exec` accepts even after
/// its file is gone.
fn proc_exe(pid: u32) -> PathBuf {
    PathBuf::from(format!("/proc/{pid}/exe"))
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
