//! Resume support: shell out to the harness binary to re-open an
//! un-muxed agent session. Per the P8-011 spec, the command is
//! harness-specific and best-effort — unsupported harnesses degrade
//! to a disabled-action reason.
//!
//! Supported harnesses:
//!   claude-code: `claude --resume <session_key>`
//!   codex:       `codex exec --resume <session_key>`
//!   opencode:    `opencode --session <session_key>`
//!   aider:       unsupported (per-cwd chat history, no
//!                single-command "resume this session" path)

use std::process::Command;

use crate::model::AgentSessionId;

/// Outcome of a resume target resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResumeTarget {
    /// Ready to launch; carries the command and the display label.
    Launch { command: String, label: String },
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
pub fn resolve_resume_target(session: &AgentSessionId) -> ResumeTarget {
    match session.harness_key.as_str() {
        "claude-code" => ResumeTarget::Launch {
            command: format!("claude --resume {}", session.session_key),
            label: session.session_key.clone(),
        },
        "codex" => ResumeTarget::Launch {
            command: format!("codex exec --resume {}", session.session_key),
            label: session.session_key.clone(),
        },
        "opencode" => ResumeTarget::Launch {
            command: format!("opencode --session {}", session.session_key),
            label: session.session_key.clone(),
        },
        "aider" => ResumeTarget::Unsupported {
            harness_key: session.harness_key.clone(),
        },
        other => ResumeTarget::Unsupported {
            harness_key: other.to_string(),
        },
    }
}

/// Launch a resume command in the background (does not replace the
/// TUI process). Returns true if the spawn succeeded.
pub fn launch_resume(target: &ResumeTarget) -> bool {
    let ResumeTarget::Launch { command, .. } = target else {
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
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}
