//! Terminal multiplexer (tmux) discovery boundaries.
//!
//! Production discovery shells out to `tmux list-sessions -F <format>` via
//! [`SystemTmux`]. Tests inject [`FakeTmux`] (or a closure-based runner) so they
//! never need a real tmux server. Both expose the same [`TmuxRunner`] surface.
//!
//! The runner only owns command execution and outcome classification. The
//! actual format parsing that turns `tmux` rows into [`MuxSession`] nodes will
//! land in a follow-up.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

pub trait TmuxRunner: Send + Sync {
    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxOutcome {
    /// `tmux list-sessions` returned successfully; payload is the raw, lossy
    /// UTF-8 stdout. Parsers should split on newlines.
    Sessions(String),
    /// tmux is not usable on this host (binary missing or no server running).
    Unavailable(UnavailableReason),
    /// tmux returned a non-zero status for an unexpected reason. The message is
    /// the trimmed stderr; the code is the OS exit code when known.
    Failed { code: Option<i32>, message: String },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    BinaryNotFound,
    NoServer,
}

impl UnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BinaryNotFound => "tmux binary not found",
            Self::NoServer => "tmux server not running",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemTmux {
    binary: PathBuf,
}

impl Default for SystemTmux {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("tmux"),
        }
    }
}

impl SystemTmux {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }
}

impl TmuxRunner for SystemTmux {
    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome> {
        let output = Command::new(&self.binary)
            .args(["list-sessions", "-F", format])
            .output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxOutcome::Unavailable(UnavailableReason::BinaryNotFound));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn tmux binary at {}", self.binary.display())
                });
            }
        };

        if output.status.success() {
            return Ok(TmuxOutcome::Sessions(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_no_server(&stderr) {
            return Ok(TmuxOutcome::Unavailable(UnavailableReason::NoServer));
        }

        Ok(TmuxOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }
}

fn looks_like_no_server(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("no server running") || lower.contains("no sessions")
}

/// Test runner that returns pre-canned outcomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeTmux {
    outcome: TmuxOutcome,
}

impl FakeTmux {
    pub fn with_sessions(stdout: impl Into<String>) -> Self {
        Self {
            outcome: TmuxOutcome::Sessions(stdout.into()),
        }
    }

    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self {
            outcome: TmuxOutcome::Unavailable(reason),
        }
    }

    pub fn failed(code: Option<i32>, message: impl Into<String>) -> Self {
        Self {
            outcome: TmuxOutcome::Failed {
                code,
                message: message.into(),
            },
        }
    }
}

impl TmuxRunner for FakeTmux {
    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(self.outcome.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_binary_reports_unavailable_binary_not_found() {
        let runner = SystemTmux::with_binary("/definitely/not/here/tmux");

        let outcome = runner.list_sessions("#{session_name}").expect("non-fatal");

        assert_eq!(
            outcome,
            TmuxOutcome::Unavailable(UnavailableReason::BinaryNotFound)
        );
    }

    #[test]
    fn fake_tmux_returns_pre_canned_sessions() {
        let runner = FakeTmux::with_sessions("alpha:/work\nbeta:/work\n");

        let outcome = runner.list_sessions("#{session_name}").expect("ok");

        assert_eq!(
            outcome,
            TmuxOutcome::Sessions("alpha:/work\nbeta:/work\n".to_string())
        );
    }

    #[test]
    fn fake_tmux_can_report_no_server() {
        let runner = FakeTmux::unavailable(UnavailableReason::NoServer);

        let outcome = runner.list_sessions("#{session_name}").expect("ok");

        assert_eq!(
            outcome,
            TmuxOutcome::Unavailable(UnavailableReason::NoServer)
        );
    }

    #[test]
    fn fake_tmux_can_surface_failed_runs() {
        let runner = FakeTmux::failed(Some(2), "permission denied");

        let outcome = runner.list_sessions("#{session_name}").expect("ok");

        assert_eq!(
            outcome,
            TmuxOutcome::Failed {
                code: Some(2),
                message: "permission denied".to_string(),
            }
        );
    }

    #[test]
    fn no_server_stderr_maps_to_unavailable() {
        assert!(looks_like_no_server(
            "no server running on /tmp/tmux-1000/default"
        ));
        assert!(looks_like_no_server(
            "error connecting to /tmp/tmux-1000/default (No sessions)"
        ));
        assert!(!looks_like_no_server("permission denied"));
    }

    #[test]
    fn unavailable_reason_has_stable_diagnostic_strings() {
        assert_eq!(
            UnavailableReason::BinaryNotFound.as_str(),
            "tmux binary not found"
        );
        assert_eq!(
            UnavailableReason::NoServer.as_str(),
            "tmux server not running"
        );
    }
}
