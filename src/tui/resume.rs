//! Resume support: shell out to the harness binary to re-open an
//! un-muxed agent session. The command is harness-specific and
//! best-effort — unsupported harnesses degrade
//! to a disabled-action reason.
//!
//! The per-harness resume command is derived from
//! [`crate::discovery::harness::resume_argv_for`] (which delegates
//! to the registered [`crate::discovery::harness::HarnessAdapter::resume_argv`]).
//! Supported harnesses today: claude-code, codex, opencode.
//! Aider is unsupported because it tracks chat history per-cwd
//! rather than per-session.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::discovery::harness::resume_argv_for;
use crate::model::{AgentSessionId, GraphNode, GraphSnapshot, NodeId};

/// Outcome of a resume target resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResumeTarget {
    /// Ready to launch; carries the command, the display label, and
    /// the session's working directory the command runs in.
    Launch {
        command: String,
        label: String,
        cwd: PathBuf,
    },
    /// Resume is not supported for this harness.
    Unsupported { harness_key: String },
    /// The session has no cwd and can't be safely resumed in-place.
    NoCwd,
}

/// Describe why resume is disabled for status-bar surfacing.
pub fn resume_disabled_reason(target: &ResumeTarget) -> String {
    match target {
        ResumeTarget::Unsupported { harness_key } => {
            format!("resume: {harness_key} does not expose a resume command")
        }
        ResumeTarget::NoCwd => "resume: session has no working directory".to_string(),
        ResumeTarget::Launch { .. } => unreachable!("called disabled_reason on valid target"),
    }
}

/// Resolve the resume command for an agent session.
///
/// Delegates to
/// [`crate::discovery::harness::resume_argv_for`] so the per-
/// harness "does this expose a resume command" answer lives on
/// the adapter (not in a hand-rolled match here). The command
/// string surfaced to the operator is the argv joined by
/// spaces — same shape as before, and `launch_resume` splits it
/// back on whitespace to spawn.
///
/// The command runs in `cwd`, the session's recorded working directory,
/// never the TUI's own (ADR 0111). A session without one resolves to
/// [`ResumeTarget::NoCwd`]. Existence isn't checked here, since status
/// hints call this on every render; a missing directory fails the spawn.
pub fn resolve_resume_target(session: &AgentSessionId, cwd: Option<&Path>) -> ResumeTarget {
    match resume_argv_for(
        &session.harness_key,
        &session.session_key,
        cwd.unwrap_or_else(|| Path::new("")),
    ) {
        Some(_) if cwd.is_none() => ResumeTarget::NoCwd,
        Some(argv) => {
            let command = argv
                .into_iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            ResumeTarget::Launch {
                command,
                label: session.session_key.clone(),
                cwd: cwd.map(Path::to_path_buf).unwrap_or_default(),
            }
        }
        None => ResumeTarget::Unsupported {
            harness_key: session.harness_key.clone(),
        },
    }
}

/// [`resolve_resume_target`] using the session's cwd from `snapshot`.
pub fn resolve_resume_target_in(
    snapshot: &GraphSnapshot,
    session: &AgentSessionId,
) -> ResumeTarget {
    let cwd = snapshot
        .find_node(&NodeId::AgentSession(session.clone()))
        .and_then(|node| match node {
            GraphNode::AgentSession(node) => node.cwd.as_deref(),
            _ => None,
        });
    resolve_resume_target(session, cwd.map(Path::new))
}

/// Launch a resume command in the background (does not replace the
/// TUI process). Returns true if the spawn succeeded.
pub fn launch_resume(target: &ResumeTarget) -> bool {
    let ResumeTarget::Launch { command, cwd, .. } = target else {
        return false;
    };
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }
    let program = parts[0];
    let args = &parts[1..];
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

#[cfg(test)]
#[path = "resume_tests.rs"]
mod tests;
