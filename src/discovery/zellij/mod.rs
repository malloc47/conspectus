//! Zellij mux backend (H-EXT-010).
//!
//! Zellij is the second mux backend Conspectus supports. This
//! module is the acceptance test for the H-EXT-008 `MuxBackend`
//! trait: adding zellij required *zero* edits outside this
//! module + its registration in
//! `discovery::providers::REGISTRY`,
//! `discovery::tmux::KNOWN_MUX_BACKENDS`, and
//! `LocalDiscoveryConfig::from_env`. Every downstream consumer
//! (pin validation, attach dispatch, TUI badge rendering,
//! provenance stamping) already routes by `backend_key`.
//!
//! Zellij's `list-sessions` output differs from tmux's — it's a
//! human-formatted line per session, with metadata blocks in
//! square brackets. Rather than parse the tmux format directly,
//! [`SystemZellij::list_sessions`] returns raw zellij stdout;
//! [`ZellijDiscovery`] wraps the backend and knows the zellij
//! parsing rules. Symmetric with the tmux side, where
//! `TmuxDiscovery` owns the tmux-format parser.
//!
//! Capabilities beyond `list_sessions` + `attach_session` stay
//! as their `MuxBackend` trait defaults (`Unsupported`) for now.
//! Rename, new_session, capture_pane, and send_keys can move
//! from `Unsupported` to real implementations as follow-up
//! stories decide zellij semantics for each (rename is not a
//! zellij-native op today; capture-pane requires a plugin;
//! send-keys is untested).

use std::io;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use crate::discovery::providers;
use crate::discovery::tmux::{MuxBackend, TmuxAttachOutcome, TmuxOutcome, UnavailableReason};
use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment};
use crate::model::{GraphNode, MuxSessionId, MuxSessionNode};

/// Backend identifier — matches the `backend_key()` this
/// backend returns. Aliased to
/// [`crate::discovery::providers::ZELLIJ`] so the model and
/// provenance stamps share the same literal string.
pub const ZELLIJ_BACKEND: &str = providers::ZELLIJ;

/// Raw-output backend that shells out to the `zellij` CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemZellij {
    binary: PathBuf,
}

impl Default for SystemZellij {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("zellij"),
        }
    }
}

impl SystemZellij {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn binary(&self) -> &std::path::Path {
        &self.binary
    }
}

impl MuxBackend for SystemZellij {
    fn backend_key(&self) -> &'static str {
        ZELLIJ_BACKEND
    }

    /// `zellij list-sessions --no-formatting` produces one
    /// session line per row without ANSI escapes. Zellij doesn't
    /// consume a format string the way tmux does, so the
    /// caller-supplied `_format` is ignored — the parser downstream
    /// ([`parse_zellij_sessions`]) is fixed against zellij's
    /// human-formatted default.
    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        let output = Command::new(&self.binary)
            .args(["list-sessions", "--no-formatting"])
            .output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxOutcome::Unavailable(UnavailableReason::BinaryNotFound));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn zellij binary at {}", self.binary.display())
                });
            }
        };

        if output.status.success() {
            return Ok(TmuxOutcome::Sessions(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_no_sessions(&stderr) {
            // Zellij reports "No active zellij sessions found"
            // as an error status. Treat it as an empty session
            // list rather than a failure — matches how tmux's
            // "no server running" collapses to an empty
            // discovery result upstream.
            return Ok(TmuxOutcome::Sessions(String::new()));
        }

        Ok(TmuxOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }

    fn attach_session(&self, _namespace: Option<&str>, name: &str) -> Result<TmuxAttachOutcome> {
        let status = Command::new(&self.binary).args(["attach", name]).status();

        let status = match status {
            Ok(status) => status,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxAttachOutcome::Unavailable(
                    UnavailableReason::BinaryNotFound,
                ));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn zellij binary at {}", self.binary.display())
                });
            }
        };

        if status.success() {
            Ok(TmuxAttachOutcome::Detached)
        } else {
            Ok(TmuxAttachOutcome::Failed {
                code: status.code(),
                message: format!("zellij attach `{name}` exited with {status}"),
            })
        }
    }
}

fn looks_like_no_sessions(stderr: &str) -> bool {
    let s = stderr.to_ascii_lowercase();
    s.contains("no active zellij sessions") || s.contains("no sessions")
}

/// Parse zellij's `list-sessions --no-formatting` stdout into
/// [`ZellijSessionRow`] entries. Each line is a session; extra
/// metadata (`[Created ... ago]`, `(current)`,
/// `(EXITED - attach to resurrect)`) is tolerated by looking at
/// the first whitespace-separated token as the session name.
///
/// Sessions marked `EXITED` are still returned — the discovery
/// layer decides whether to skip them (today it does not; the
/// row's presence in the graph tells the operator the session
/// exists even if it's stopped).
pub fn parse_zellij_sessions(stdout: &str) -> Vec<ZellijSessionRow> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let name = trimmed.split_whitespace().next()?.to_string();
            if name.is_empty() {
                return None;
            }
            let exited = trimmed.contains("EXITED");
            let is_current = trimmed.contains("(current)");
            Some(ZellijSessionRow {
                name,
                exited,
                is_current,
            })
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZellijSessionRow {
    /// Session name — the primary key for `zellij attach`.
    pub name: String,
    /// True when zellij reports the session as EXITED (needs
    /// resurrection).
    pub exited: bool,
    /// True when zellij marked the session as `(current)` — the
    /// operator is currently attached to it.
    pub is_current: bool,
}

/// [`DiscoveryProvider`] that owns a [`SystemZellij`] (or any
/// [`MuxBackend`] returning zellij-formatted output) and turns
/// `list_sessions` output into `MuxSessionNode`s.
pub struct ZellijDiscovery<R: MuxBackend> {
    runner: R,
}

impl<R: MuxBackend> ZellijDiscovery<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }
}

impl<R: MuxBackend + 'static> DiscoveryProvider for ZellijDiscovery<R> {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        // The `format` string is ignored by [`SystemZellij`];
        // pass tmux's format anyway so a mixed backend list that
        // reuses this call surface stays uniform.
        let outcome = self.runner.list_sessions("")?;
        let stdout = match outcome {
            TmuxOutcome::Sessions(s) => s,
            TmuxOutcome::Unavailable(_) => return Ok(GraphFragment::empty()),
            TmuxOutcome::Failed { .. } => {
                // Degrade to an empty fragment when zellij
                // returned a non-zero status. The `Diagnostic`
                // enum today has no free-form info variant, so
                // failure telemetry stays out of the graph and
                // relies on stderr for surfacing. Matches how
                // the tmux path handles unexpected errors when
                // the shape is otherwise recoverable.
                return Ok(GraphFragment::empty());
            }
        };

        let mut fragment = GraphFragment::empty();
        for row in parse_zellij_sessions(&stdout) {
            // MuxSessionId is constructed via `new(&str)` with
            // the `<backend>:<name>` prefixed form (matches the
            // tmux side; see `MuxSessionNode.native_id` docstring
            // for the split between the prefixed id and the bare
            // node native_id).
            let node = MuxSessionNode {
                id: MuxSessionId::new(format!("{ZELLIJ_BACKEND}:{}", row.name)),
                backend: ZELLIJ_BACKEND.to_string(),
                native_id: row.name.clone(),
                cwd: None,
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                client_attached: Some(row.is_current),
                activity_epoch: None,
                created_epoch: None,
            };
            fragment.nodes.push(GraphNode::MuxSession(node));
        }

        crate::discovery::stamp_fragment(
            &mut fragment,
            ZELLIJ_BACKEND,
            crate::discovery::current_epoch(),
        );
        Ok(fragment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_zellij_output() {
        let stdout = "editor\nagent-work\n";
        let rows = parse_zellij_sessions(stdout);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "editor");
        assert!(!rows[0].exited);
        assert!(!rows[0].is_current);
        assert_eq!(rows[1].name, "agent-work");
    }

    #[test]
    fn parses_human_zellij_output() {
        let stdout = "\
sunburnt-shibolleth [Created 2h 5m ago]
active-session [Created 1h ago] (current)
exited-session [Created 24h ago] (EXITED - attach to resurrect)
";
        let rows = parse_zellij_sessions(stdout);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, "sunburnt-shibolleth");
        assert!(!rows[0].exited);
        assert!(!rows[0].is_current);
        assert_eq!(rows[1].name, "active-session");
        assert!(rows[1].is_current);
        assert_eq!(rows[2].name, "exited-session");
        assert!(rows[2].exited);
    }

    #[test]
    fn ignores_blank_lines() {
        let stdout = "\n\nfoo\n\n\n";
        assert_eq!(parse_zellij_sessions(stdout).len(), 1);
    }

    #[test]
    fn backend_key_is_zellij() {
        assert_eq!(SystemZellij::new().backend_key(), "zellij");
    }
}
