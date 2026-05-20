//! Terminal multiplexer (tmux) discovery boundaries.
//!
//! Production discovery shells out to `tmux list-sessions -F <format>` via
//! [`SystemTmux`]. Tests inject [`FakeTmux`] (or any other [`TmuxRunner`]) so
//! they never need a real tmux server. [`TmuxDiscovery`] is the
//! [`DiscoveryProvider`] that asks a runner for sessions, parses the rows, and
//! emits provider-neutral `MuxSession` nodes.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment};
use crate::model::{GraphNode, MuxSessionId, MuxSessionNode};

pub const TMUX_BACKEND: &str = "tmux";

/// Format string used with `tmux list-sessions -F`. Fields are tab-separated so
/// session roots can safely contain spaces.
pub const TMUX_LIST_FORMAT: &str =
    "#{session_name}\t#{session_path}\t#{session_activity}\t#{session_created}";

pub trait TmuxRunner: Send + Sync {
    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome>;

    /// Capture the visible content of pane `target` (e.g. a session
    /// name like `editor`, or a fuller `session:window.pane`
    /// selector). Default returns [`TmuxCaptureOutcome::Unsupported`]
    /// so existing test runners don't need to change.
    fn capture_pane(&self, _target: &str) -> Result<TmuxCaptureOutcome> {
        Ok(TmuxCaptureOutcome::Unsupported)
    }
}

/// Outcome of a `tmux capture-pane` call. Mirrors the shape of
/// [`TmuxOutcome`] but for the per-pane capture path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxCaptureOutcome {
    /// `tmux capture-pane -p -t <target>` succeeded; payload is
    /// the visible pane content as captured.
    Captured(String),
    /// tmux returned successfully but the target doesn't exist
    /// (a session/window/pane lookup miss).
    NoTarget,
    /// tmux itself isn't usable on this host.
    Unavailable(UnavailableReason),
    /// tmux returned a non-zero status for an unexpected reason.
    Failed { code: Option<i32>, message: String },
    /// The runner doesn't implement capture (e.g. fakes that only
    /// care about `list_sessions`). Treated as "no preview
    /// available" by the TUI.
    Unsupported,
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

    fn capture_pane(&self, target: &str) -> Result<TmuxCaptureOutcome> {
        // `-p` prints to stdout instead of leaving the capture in
        // the buffer; `-J` joins wrapped lines so the result reads
        // naturally in a fixed-width preview pane; `-e` emits the
        // pane's ANSI escape sequences so the TUI preview can
        // render with the same colours the operator sees in the
        // source pane (ADR 0025).
        let output = Command::new(&self.binary)
            .args(["capture-pane", "-p", "-J", "-e", "-t", target])
            .output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxCaptureOutcome::Unavailable(
                    UnavailableReason::BinaryNotFound,
                ));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn tmux binary at {}", self.binary.display())
                });
            }
        };

        if output.status.success() {
            return Ok(TmuxCaptureOutcome::Captured(
                String::from_utf8_lossy(&output.stdout).into_owned(),
            ));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_no_server(&stderr) {
            return Ok(TmuxCaptureOutcome::Unavailable(UnavailableReason::NoServer));
        }
        if looks_like_no_target(&stderr) {
            return Ok(TmuxCaptureOutcome::NoTarget);
        }

        Ok(TmuxCaptureOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }
}

fn looks_like_no_server(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("no server running") || lower.contains("no sessions")
}

fn looks_like_no_target(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("can't find session")
        || lower.contains("can't find window")
        || lower.contains("can't find pane")
        || lower.contains("no such session")
}

/// Test runner that returns pre-canned outcomes.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeTmux {
    outcome: TmuxOutcome,
    /// Per-target canned captures. A missing target falls through
    /// to [`TmuxCaptureOutcome::Unsupported`].
    captures: std::collections::BTreeMap<String, TmuxCaptureOutcome>,
}

impl FakeTmux {
    pub fn with_sessions(stdout: impl Into<String>) -> Self {
        Self {
            outcome: TmuxOutcome::Sessions(stdout.into()),
            captures: std::collections::BTreeMap::new(),
        }
    }

    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self {
            outcome: TmuxOutcome::Unavailable(reason),
            captures: std::collections::BTreeMap::new(),
        }
    }

    pub fn failed(code: Option<i32>, message: impl Into<String>) -> Self {
        Self {
            outcome: TmuxOutcome::Failed {
                code,
                message: message.into(),
            },
            captures: std::collections::BTreeMap::new(),
        }
    }

    /// Register a canned capture-pane response for `target`.
    pub fn with_capture(mut self, target: impl Into<String>, capture: TmuxCaptureOutcome) -> Self {
        self.captures.insert(target.into(), capture);
        self
    }
}

impl TmuxRunner for FakeTmux {
    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(self.outcome.clone())
    }

    fn capture_pane(&self, target: &str) -> Result<TmuxCaptureOutcome> {
        Ok(self
            .captures
            .get(target)
            .cloned()
            .unwrap_or(TmuxCaptureOutcome::Unsupported))
    }
}

impl TmuxRunner for Box<dyn TmuxRunner> {
    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome> {
        (**self).list_sessions(format)
    }

    fn capture_pane(&self, target: &str) -> Result<TmuxCaptureOutcome> {
        (**self).capture_pane(target)
    }
}

/// One row of `tmux list-sessions` output. Activity and creation epochs are
/// optional so the parser can keep using rows even when tmux is configured with
/// a custom format or when fields are blank.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxSessionRow {
    pub name: String,
    pub path: Option<String>,
    pub activity_epoch: Option<i64>,
    pub created_epoch: Option<i64>,
}

pub fn parse_list_sessions(stdout: &str) -> Vec<TmuxSessionRow> {
    stdout.lines().filter_map(parse_session_line).collect()
}

fn parse_session_line(line: &str) -> Option<TmuxSessionRow> {
    if line.trim().is_empty() {
        return None;
    }
    let mut fields = line.split('\t');
    let name = fields.next()?.trim().to_string();

    if name.is_empty() {
        return None;
    }

    let path = optional_string(fields.next());
    let activity_epoch = optional_epoch(fields.next());
    let created_epoch = optional_epoch(fields.next());

    Some(TmuxSessionRow {
        name,
        path,
        activity_epoch,
        created_epoch,
    })
}

fn optional_string(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();

    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn optional_epoch(value: Option<&str>) -> Option<i64> {
    optional_string(value)?.parse().ok()
}

#[derive(Clone, Debug)]
pub struct TmuxDiscovery<R: TmuxRunner> {
    runner: R,
}

impl Default for TmuxDiscovery<SystemTmux> {
    fn default() -> Self {
        Self {
            runner: SystemTmux::new(),
        }
    }
}

impl TmuxDiscovery<SystemTmux> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<R: TmuxRunner> TmuxDiscovery<R> {
    pub fn with_runner(runner: R) -> Self {
        Self { runner }
    }

    pub fn rows(&self) -> Result<TmuxDiscoveryRows> {
        let outcome = self.runner.list_sessions(TMUX_LIST_FORMAT)?;
        Ok(match outcome {
            TmuxOutcome::Sessions(stdout) => TmuxDiscoveryRows {
                rows: parse_list_sessions(&stdout),
                status: TmuxStatus::Available,
            },
            TmuxOutcome::Unavailable(reason) => TmuxDiscoveryRows {
                rows: Vec::new(),
                status: TmuxStatus::Unavailable(reason),
            },
            TmuxOutcome::Failed { code, message } => TmuxDiscoveryRows {
                rows: Vec::new(),
                status: TmuxStatus::Failed { code, message },
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxDiscoveryRows {
    pub rows: Vec<TmuxSessionRow>,
    pub status: TmuxStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxStatus {
    Available,
    Unavailable(UnavailableReason),
    Failed { code: Option<i32>, message: String },
}

impl<R: TmuxRunner + 'static> DiscoveryProvider for TmuxDiscovery<R> {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        let outcome = self.rows()?;
        let mut nodes = Vec::with_capacity(outcome.rows.len());

        for row in &outcome.rows {
            nodes.push(GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new(format!("{TMUX_BACKEND}:{}", row.name)),
                backend: TMUX_BACKEND.to_string(),
                native_id: row.name.clone(),
                cwd: row.path.clone(),
                activity_epoch: row.activity_epoch,
                created_epoch: row.created_epoch,
            }));
        }

        Ok(GraphFragment {
            nodes,
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
        })
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

    #[test]
    fn parser_yields_empty_rows_for_empty_output() {
        assert!(parse_list_sessions("").is_empty());
        assert!(parse_list_sessions("\n\n   \n").is_empty());
    }

    #[test]
    fn parser_extracts_name_path_activity_and_created_epoch() {
        let rows = parse_list_sessions("alpha\t/work/alpha\t1700000500\t1700000000\n");

        assert_eq!(
            rows,
            vec![TmuxSessionRow {
                name: "alpha".to_string(),
                path: Some("/work/alpha".to_string()),
                activity_epoch: Some(1700000500),
                created_epoch: Some(1700000000),
            }]
        );
    }

    #[test]
    fn parser_handles_paths_with_spaces() {
        let rows = parse_list_sessions("with-space\t/work/has spaces/here\t\t\n");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path.as_deref(), Some("/work/has spaces/here"));
        assert!(rows[0].activity_epoch.is_none());
        assert!(rows[0].created_epoch.is_none());
    }

    #[test]
    fn parser_handles_missing_optional_fields() {
        let rows = parse_list_sessions("only-name\n");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "only-name");
        assert!(rows[0].path.is_none());
    }

    #[test]
    fn parser_skips_rows_without_a_name() {
        let rows = parse_list_sessions("\t/work\t\t\n");

        assert!(rows.is_empty());
    }

    #[test]
    fn parser_drops_malformed_epoch_fields() {
        let rows = parse_list_sessions("alpha\t/work\tNaN\tunknown\n");

        assert_eq!(rows.len(), 1);
        assert!(rows[0].activity_epoch.is_none());
        assert!(rows[0].created_epoch.is_none());
    }

    #[test]
    fn discovery_returns_zero_sessions_for_blank_runner_output() {
        let discovery = TmuxDiscovery::with_runner(FakeTmux::with_sessions(""));

        let fragment = discovery
            .discover(&DiscoveryContext::default())
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn discovery_emits_mux_session_per_row() {
        let stdout = "alpha\t/work/alpha\t1\t0\nbeta\t/work/has space\t\t\n";
        let discovery = TmuxDiscovery::with_runner(FakeTmux::with_sessions(stdout));

        let fragment = discovery
            .discover(&DiscoveryContext::default())
            .expect("discover");

        let sessions: Vec<_> = fragment
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::MuxSession(session) => Some(session.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(sessions.len(), 2);
        let alpha = sessions
            .iter()
            .find(|s| s.native_id == "alpha")
            .expect("alpha");
        assert_eq!(alpha.id.native_id, "tmux:alpha");
        assert_eq!(alpha.cwd.as_deref(), Some("/work/alpha"));
        let beta = sessions
            .iter()
            .find(|s| s.native_id == "beta")
            .expect("beta");
        assert_eq!(beta.cwd.as_deref(), Some("/work/has space"));
    }

    #[test]
    fn discovery_yields_empty_fragment_when_tmux_unavailable() {
        let discovery =
            TmuxDiscovery::with_runner(FakeTmux::unavailable(UnavailableReason::NoServer));

        let fragment = discovery
            .discover(&DiscoveryContext::default())
            .expect("discover");

        assert!(fragment.nodes.is_empty());
    }

    #[test]
    fn discovery_rows_preserve_status_for_unavailable_tmux() {
        let discovery =
            TmuxDiscovery::with_runner(FakeTmux::unavailable(UnavailableReason::BinaryNotFound));

        let rows = discovery.rows().expect("rows");

        assert_eq!(
            rows.status,
            TmuxStatus::Unavailable(UnavailableReason::BinaryNotFound)
        );
        assert!(rows.rows.is_empty());
    }

    #[test]
    fn discovery_rows_surface_failed_status() {
        let discovery = TmuxDiscovery::with_runner(FakeTmux::failed(Some(2), "permission denied"));

        let rows = discovery.rows().expect("rows");

        assert_eq!(
            rows.status,
            TmuxStatus::Failed {
                code: Some(2),
                message: "permission denied".to_string(),
            }
        );
    }

    #[test]
    fn missing_binary_capture_pane_reports_unavailable_binary_not_found() {
        let runner = SystemTmux::with_binary("/definitely/not/here/tmux");
        let outcome = runner.capture_pane("editor").expect("non-fatal");
        assert_eq!(
            outcome,
            TmuxCaptureOutcome::Unavailable(UnavailableReason::BinaryNotFound)
        );
    }

    #[test]
    fn fake_runner_default_capture_pane_returns_unsupported() {
        let runner = FakeTmux::with_sessions("");
        assert_eq!(
            runner.capture_pane("anything").unwrap(),
            TmuxCaptureOutcome::Unsupported
        );
    }

    #[test]
    fn fake_runner_returns_registered_capture_outcomes_by_target() {
        let runner = FakeTmux::with_sessions("")
            .with_capture(
                "editor",
                TmuxCaptureOutcome::Captured("pane content".to_string()),
            )
            .with_capture("missing", TmuxCaptureOutcome::NoTarget)
            .with_capture(
                "broken",
                TmuxCaptureOutcome::Failed {
                    code: Some(1),
                    message: "boom".to_string(),
                },
            );
        assert_eq!(
            runner.capture_pane("editor").unwrap(),
            TmuxCaptureOutcome::Captured("pane content".to_string())
        );
        assert_eq!(
            runner.capture_pane("missing").unwrap(),
            TmuxCaptureOutcome::NoTarget
        );
        assert_eq!(
            runner.capture_pane("broken").unwrap(),
            TmuxCaptureOutcome::Failed {
                code: Some(1),
                message: "boom".to_string(),
            }
        );
        // Unregistered targets keep the default Unsupported.
        assert_eq!(
            runner.capture_pane("other").unwrap(),
            TmuxCaptureOutcome::Unsupported
        );
    }
}
