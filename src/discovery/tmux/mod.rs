//! Terminal multiplexer (tmux) discovery boundaries.
//!
//! Production discovery shells out to `tmux list-sessions -F <format>` via
//! [`SystemTmux`]. Tests inject [`FakeTmux`] (or any other [`MuxBackend`]) so
//! they never need a real tmux server. [`TmuxDiscovery`] is the
//! [`DiscoveryProvider`] that asks a runner for sessions, parses the rows, and
//! emits provider-neutral `MuxSession` nodes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::discovery::{DiscoveryContext, DiscoveryProvider, GraphFragment};
use crate::model::{GraphNode, MuxSessionId, MuxSessionNode};

/// Mux-backend identifier stamped on `MuxSessionId` /
/// `MuxSessionNode.backend` AND on the discovery provenance for
/// tmux-derived nodes/links. The two concerns (model identity
/// vs provider stamp) happen to share the same string — the
/// alias to [`crate::discovery::providers::TMUX`] makes that
/// equivalence explicit and keeps the literal in one place.
pub const TMUX_BACKEND: &str = crate::discovery::providers::TMUX;

/// Format string used with `tmux list-sessions -F`. Fields are tab-separated so
/// session roots can safely contain spaces.
pub const TMUX_LIST_FORMAT: &str = "#{session_name}\t#{session_path}\t#{session_activity}\t#{session_created}\t#{pane_current_command}\t#{pane_pid}\t#{pane_current_path}\t#{pane_start_command}\t#{session_attached}\t#{session_attached_list}";

/// Backend-neutral mux abstraction (H-EXT-008, ADR 0089).
///
/// Every mux backend Conspectus supports (tmux today, zellij next
/// per H-EXT-010) implements this trait. The trait's capability
/// methods default to `Unsupported` outcomes so a new backend can
/// implement only the operations it actually supports; callers
/// gate on the outcome (H-EXT-009) instead of naming a specific
/// backend.
///
/// `backend_key` is the string every consumer keys off. Pin
/// entries carry `mux.backend = "<backend_key>"` per ADR 0057;
/// discovery stamps `MuxSessionNode.backend` with the same
/// string. Two backends must never share a key.
///
/// The outcome enums (`TmuxOutcome`, `TmuxCaptureOutcome`, …)
/// keep their `Tmux`-prefixed names in this Phase-C step —
/// their variant shapes (`Sessions`, `NoTarget`, `NameCollision`,
/// `Unavailable`, `Failed`, `Unsupported`) are backend-neutral,
/// so the rename is a mechanical follow-up and orthogonal to
/// the trait-shape work here.
pub trait MuxBackend: Send + Sync {
    /// Backend identity. Stamped on pin entries
    /// (`mux.backend`) and `MuxSessionNode.backend`. Every
    /// implementor returns a `&'static` literal.
    fn backend_key(&self) -> &'static str;

    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome>;

    /// Capture the visible content of pane `target` (e.g. a session
    /// name like `editor`, or a fuller `session:window.pane`
    /// selector). `socket_name` selects the tmux server when set
    /// (`tmux -L <socket_name>`); `None` and `Some("default")` both
    /// use the default socket. Default returns
    /// [`TmuxCaptureOutcome::Unsupported`] so test runners that only
    /// model `list_sessions` don't need to change.
    fn capture_pane(
        &self,
        _socket_name: Option<&str>,
        _target: &str,
    ) -> Result<TmuxCaptureOutcome> {
        Ok(TmuxCaptureOutcome::Unsupported)
    }

    /// Rename tmux session `target` to `new_name`. `socket_name`
    /// selects the tmux server when set. Default returns
    /// [`TmuxRenameOutcome::Unsupported`] so runners that only model
    /// read paths keep compiling (e.g. zellij backends per
    /// `H-FUTURE-001`).
    fn rename_session(
        &self,
        _socket_name: Option<&str>,
        _target: &str,
        _new_name: &str,
    ) -> Result<TmuxRenameOutcome> {
        Ok(TmuxRenameOutcome::Unsupported)
    }

    /// Spawn a detached tmux session named `name` rooted at `cwd`
    /// running `argv`. Used by `conspectus pin launch` (ADR 0057)
    /// when no matching mux exists yet. Default returns
    /// [`TmuxNewSessionOutcome::Unsupported`].
    fn new_session(
        &self,
        _socket_name: Option<&str>,
        _name: &str,
        _cwd: &Path,
        _argv: &[OsString],
    ) -> Result<TmuxNewSessionOutcome> {
        Ok(TmuxNewSessionOutcome::Unsupported)
    }

    /// Attach the calling process's terminal to tmux session `name`.
    /// SystemTmux switches to `tmux switch-client` when `$TMUX` is
    /// set so nested clients don't fail (ADR 0057 §Launch
    /// Semantics). The call blocks until the operator detaches or
    /// tmux exits. Default returns
    /// [`TmuxAttachOutcome::Unsupported`].
    fn attach_session(&self, _socket_name: Option<&str>, _name: &str) -> Result<TmuxAttachOutcome> {
        Ok(TmuxAttachOutcome::Unsupported)
    }

    /// Send `literal` to pane `target`, optionally followed by an
    /// `Enter` key (ADR 0057 §Launch §`PinStaleMux`). Used by the
    /// stale-mux relaunch path so the harness command lands inside
    /// the existing pane without recreating the mux. Default returns
    /// [`TmuxSendKeysOutcome::Unsupported`].
    fn send_keys(
        &self,
        _socket_name: Option<&str>,
        _target: &str,
        _literal: &str,
        _press_enter: bool,
    ) -> Result<TmuxSendKeysOutcome> {
        Ok(TmuxSendKeysOutcome::Unsupported)
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

/// Outcome of a `tmux rename-session` call. Mirrors the shape of
/// [`TmuxOutcome`] / [`TmuxCaptureOutcome`] but for the mutation path
/// introduced by ADR 0029's session-naming workstream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxRenameOutcome {
    /// `tmux rename-session -t <target> <new_name>` succeeded.
    Renamed,
    /// tmux returned successfully but the target session doesn't
    /// exist on this server.
    NoTarget,
    /// tmux refused the rename because `new_name` is already taken
    /// by another session on the same server.
    NameCollision,
    /// tmux itself isn't usable on this host.
    Unavailable(UnavailableReason),
    /// tmux returned a non-zero status for an unexpected reason.
    Failed { code: Option<i32>, message: String },
    /// The runner doesn't implement rename (default for runners that
    /// only model read paths).
    Unsupported,
}

/// Outcome of [`MuxBackend::new_session`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxNewSessionOutcome {
    /// `tmux new-session -d -s <name> -c <cwd> <argv...>` succeeded.
    Created,
    /// tmux refused because a session with `name` already exists on
    /// the same server. The launch primitive treats this as the
    /// "name collision" branch from ADR 0057 §Launch.
    NameTaken,
    /// tmux itself isn't usable on this host.
    Unavailable(UnavailableReason),
    /// tmux returned a non-zero status for an unexpected reason.
    Failed { code: Option<i32>, message: String },
    /// The runner doesn't implement new-session (default for runners
    /// that only model read paths).
    Unsupported,
}

/// Outcome of [`MuxBackend::attach_session`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxAttachOutcome {
    /// Tmux ran and exited normally (operator detached, or the
    /// session ended).
    Detached,
    /// Target session does not exist on the chosen server.
    NoTarget,
    /// tmux itself isn't usable on this host.
    Unavailable(UnavailableReason),
    /// tmux exited non-zero for some other reason.
    Failed { code: Option<i32>, message: String },
    /// Runner doesn't implement attach (test runners that don't
    /// model side effects).
    Unsupported,
}

/// Outcome of [`MuxBackend::send_keys`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TmuxSendKeysOutcome {
    /// `tmux send-keys -t <target> <literal> [Enter]` succeeded.
    Sent,
    /// Target pane / window doesn't exist.
    NoTarget,
    /// tmux isn't usable on this host.
    Unavailable(UnavailableReason),
    /// tmux returned non-zero for some other reason.
    Failed { code: Option<i32>, message: String },
    /// Runner doesn't implement send-keys (test runners).
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

    /// Build a `Command` for the tmux binary, prepending `-L
    /// <socket>` when the caller passes a non-default socket per
    /// ADR 0057. `None` and `Some("default")` both yield a bare
    /// `tmux <subcommand>` invocation so the default-socket path is
    /// byte-for-byte identical to today.
    fn cmd(&self, socket_name: Option<&str>) -> Command {
        let mut cmd = Command::new(&self.binary);
        if let Some(socket) = effective_socket(socket_name) {
            cmd.args(["-L", socket]);
        }
        cmd
    }
}

/// Collapse `None` and `Some("default")` to `None` so the caller's
/// `pin.mux.socket_name` slot maps directly to the `-L` switch
/// without sentinel handling everywhere.
fn effective_socket(socket_name: Option<&str>) -> Option<&str> {
    match socket_name {
        None | Some("default") => None,
        Some(name) => Some(name),
    }
}

impl MuxBackend for SystemTmux {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

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

    fn capture_pane(&self, socket_name: Option<&str>, target: &str) -> Result<TmuxCaptureOutcome> {
        // `-p` prints to stdout instead of leaving the capture in
        // the buffer; `-J` joins wrapped lines so the result reads
        // naturally in a fixed-width preview pane; `-e` emits the
        // pane's ANSI escape sequences so the TUI preview can
        // render with the same colours the operator sees in the
        // source pane (ADR 0025).
        let output = self
            .cmd(socket_name)
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

    fn rename_session(
        &self,
        socket_name: Option<&str>,
        target: &str,
        new_name: &str,
    ) -> Result<TmuxRenameOutcome> {
        let output = self
            .cmd(socket_name)
            .args(["rename-session", "-t", target, new_name])
            .output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxRenameOutcome::Unavailable(
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
            return Ok(TmuxRenameOutcome::Renamed);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_no_server(&stderr) {
            return Ok(TmuxRenameOutcome::Unavailable(UnavailableReason::NoServer));
        }
        if looks_like_no_target(&stderr) {
            return Ok(TmuxRenameOutcome::NoTarget);
        }
        if looks_like_name_collision(&stderr) {
            return Ok(TmuxRenameOutcome::NameCollision);
        }

        Ok(TmuxRenameOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }

    fn new_session(
        &self,
        socket_name: Option<&str>,
        name: &str,
        cwd: &Path,
        argv: &[OsString],
    ) -> Result<TmuxNewSessionOutcome> {
        // `-d` so tmux returns immediately rather than attaching;
        // the caller invokes `attach_session` separately so the
        // create-then-attach flow stays observable across both
        // SystemTmux (real) and FakeTmux (test) runners.
        let mut command = self.cmd(socket_name);
        command
            .arg("new-session")
            .arg("-d")
            .arg("-s")
            .arg(name)
            .arg("-c")
            .arg(cwd);
        for token in argv {
            command.arg(token);
        }
        let output = command.output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxNewSessionOutcome::Unavailable(
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
            return Ok(TmuxNewSessionOutcome::Created);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if looks_like_no_server(&stderr) {
            // `new-session` on an offline server is fine — tmux
            // starts the server transparently. If we still see
            // "no server" here it's the legitimate
            // unavailable-host case.
            return Ok(TmuxNewSessionOutcome::Unavailable(
                UnavailableReason::NoServer,
            ));
        }
        if looks_like_name_collision(&stderr) {
            return Ok(TmuxNewSessionOutcome::NameTaken);
        }

        Ok(TmuxNewSessionOutcome::Failed {
            code: output.status.code(),
            message: stderr,
        })
    }

    fn attach_session(&self, socket_name: Option<&str>, name: &str) -> Result<TmuxAttachOutcome> {
        // Inside an existing tmux client (`$TMUX` set), nested
        // attaches refuse. `switch-client -t <name>` is the safe
        // analogue — it shifts the current client to the target
        // session without starting a nested client.
        let nested = std::env::var_os("TMUX").is_some();
        let subcommand = if nested {
            "switch-client"
        } else {
            "attach-session"
        };

        // `attach-session` blocks until the operator detaches; we
        // spawn + wait via `status()` so the parent terminal is
        // owned by tmux for the lifetime of the call. Callers that
        // need to bracket this (e.g. TUI runtime restore + reinit)
        // wrap the call site, not the runner.
        let status = self
            .cmd(socket_name)
            .args([subcommand, "-t", name])
            .status();

        let status = match status {
            Ok(status) => status,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxAttachOutcome::Unavailable(
                    UnavailableReason::BinaryNotFound,
                ));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn tmux binary at {}", self.binary.display())
                });
            }
        };

        if status.success() {
            return Ok(TmuxAttachOutcome::Detached);
        }

        // We don't capture stderr for `status()`, so we rely on
        // exit-code conventions: tmux exits non-zero with a brief
        // error printed to its own stderr (already on the
        // operator's screen). Code 1 typically means "no session
        // found"; other codes are reported verbatim.
        let code = status.code();
        if matches!(code, Some(1)) {
            return Ok(TmuxAttachOutcome::NoTarget);
        }
        Ok(TmuxAttachOutcome::Failed {
            code,
            message: format!("tmux exited with status {status}"),
        })
    }

    fn send_keys(
        &self,
        socket_name: Option<&str>,
        target: &str,
        literal: &str,
        press_enter: bool,
    ) -> Result<TmuxSendKeysOutcome> {
        let mut command = self.cmd(socket_name);
        command.args(["send-keys", "-t", target, literal]);
        if press_enter {
            command.arg("Enter");
        }
        let output = command.output();

        let output = match output {
            Ok(output) => output,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(TmuxSendKeysOutcome::Unavailable(
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
            return Ok(TmuxSendKeysOutcome::Sent);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if looks_like_no_server(&stderr) {
            return Ok(TmuxSendKeysOutcome::Unavailable(
                UnavailableReason::NoServer,
            ));
        }
        if looks_like_no_target(&stderr) {
            return Ok(TmuxSendKeysOutcome::NoTarget);
        }
        Ok(TmuxSendKeysOutcome::Failed {
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

fn looks_like_name_collision(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("duplicate session")
        || lower.contains("session already exists")
        || lower.contains("name already in use")
}

/// `(socket_name, target, new_name)` triple recorded by every
/// `FakeTmux::rename_session` call. Aliased so the clippy
/// `type_complexity` lint stays satisfied where the recorder is
/// referenced.
type FakeTmuxRenameCall = (Option<String>, String, String);
/// `(socket_name, name, cwd, argv)` tuple recorded by every
/// `FakeTmux::new_session` call.
type FakeTmuxNewSessionCall = (Option<String>, String, PathBuf, Vec<OsString>);
/// `(socket_name, name)` pair recorded by every
/// `FakeTmux::attach_session` call.
type FakeTmuxAttachCall = (Option<String>, String);
/// `(socket_name, target, literal, press_enter)` tuple recorded by
/// every `FakeTmux::send_keys` call.
type FakeTmuxSendKeysCall = (Option<String>, String, String, bool);

/// Test runner that returns pre-canned outcomes.
#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct FakeTmux {
    outcome: TmuxOutcome,
    /// Per-target canned captures. A missing target falls through
    /// to [`TmuxCaptureOutcome::Unsupported`].
    captures: std::collections::BTreeMap<String, TmuxCaptureOutcome>,
    /// Per-target canned rename outcomes. A missing target falls
    /// through to [`TmuxRenameOutcome::Renamed`] so existing tests
    /// that only care about the call being recorded don't need to
    /// register a response.
    rename_outcomes: std::collections::BTreeMap<String, TmuxRenameOutcome>,
    /// Recorded triples across every `rename_session` call.
    rename_calls: std::sync::Arc<std::sync::Mutex<Vec<FakeTmuxRenameCall>>>,
    /// Per-name canned `new-session` outcomes. Missing keys fall
    /// through to [`TmuxNewSessionOutcome::Created`] so callers that
    /// only care about the call being recorded don't need to register
    /// a response.
    new_session_outcomes: std::collections::BTreeMap<String, TmuxNewSessionOutcome>,
    /// Recorded tuples across every `new_session` call.
    new_session_calls: std::sync::Arc<std::sync::Mutex<Vec<FakeTmuxNewSessionCall>>>,
    /// Per-name canned `attach_session` outcomes. Default is
    /// [`TmuxAttachOutcome::Detached`].
    attach_outcomes: std::collections::BTreeMap<String, TmuxAttachOutcome>,
    /// Recorded pairs across every `attach_session` call.
    attach_calls: std::sync::Arc<std::sync::Mutex<Vec<FakeTmuxAttachCall>>>,
    /// Per-target canned `send_keys` outcomes. Default is
    /// [`TmuxSendKeysOutcome::Sent`].
    send_keys_outcomes: std::collections::BTreeMap<String, TmuxSendKeysOutcome>,
    /// Recorded tuples across every `send_keys` call.
    send_keys_calls: std::sync::Arc<std::sync::Mutex<Vec<FakeTmuxSendKeysCall>>>,
}

impl PartialEq for FakeTmux {
    fn eq(&self, other: &Self) -> bool {
        self.outcome == other.outcome
            && self.captures == other.captures
            && self.rename_outcomes == other.rename_outcomes
            && *self.rename_calls.lock().unwrap() == *other.rename_calls.lock().unwrap()
            && self.new_session_outcomes == other.new_session_outcomes
            && *self.new_session_calls.lock().unwrap() == *other.new_session_calls.lock().unwrap()
            && self.attach_outcomes == other.attach_outcomes
            && *self.attach_calls.lock().unwrap() == *other.attach_calls.lock().unwrap()
            && self.send_keys_outcomes == other.send_keys_outcomes
            && *self.send_keys_calls.lock().unwrap() == *other.send_keys_calls.lock().unwrap()
    }
}

impl Eq for FakeTmux {}

impl FakeTmux {
    fn empty_state() -> Self {
        Self {
            outcome: TmuxOutcome::Sessions(String::new()),
            captures: std::collections::BTreeMap::new(),
            rename_outcomes: std::collections::BTreeMap::new(),
            rename_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            new_session_outcomes: std::collections::BTreeMap::new(),
            new_session_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            attach_outcomes: std::collections::BTreeMap::new(),
            attach_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            send_keys_outcomes: std::collections::BTreeMap::new(),
            send_keys_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn with_sessions(stdout: impl Into<String>) -> Self {
        let mut tmux = Self::empty_state();
        tmux.outcome = TmuxOutcome::Sessions(stdout.into());
        tmux
    }

    pub fn unavailable(reason: UnavailableReason) -> Self {
        let mut tmux = Self::empty_state();
        tmux.outcome = TmuxOutcome::Unavailable(reason);
        tmux
    }

    pub fn failed(code: Option<i32>, message: impl Into<String>) -> Self {
        let mut tmux = Self::empty_state();
        tmux.outcome = TmuxOutcome::Failed {
            code,
            message: message.into(),
        };
        tmux
    }

    /// Register a canned capture-pane response for `target`.
    pub fn with_capture(mut self, target: impl Into<String>, capture: TmuxCaptureOutcome) -> Self {
        self.captures.insert(target.into(), capture);
        self
    }

    /// Register a canned rename-session response for `target`. Unset
    /// targets default to [`TmuxRenameOutcome::Renamed`].
    pub fn with_rename(mut self, target: impl Into<String>, outcome: TmuxRenameOutcome) -> Self {
        self.rename_outcomes.insert(target.into(), outcome);
        self
    }

    /// Register a canned `new-session` response for `name`. Unset
    /// names default to [`TmuxNewSessionOutcome::Created`].
    pub fn with_new_session(
        mut self,
        name: impl Into<String>,
        outcome: TmuxNewSessionOutcome,
    ) -> Self {
        self.new_session_outcomes.insert(name.into(), outcome);
        self
    }

    /// Register a canned `attach_session` response for `name`. Unset
    /// names default to [`TmuxAttachOutcome::Detached`].
    pub fn with_attach(mut self, name: impl Into<String>, outcome: TmuxAttachOutcome) -> Self {
        self.attach_outcomes.insert(name.into(), outcome);
        self
    }

    /// Register a canned `send_keys` response for `target`. Unset
    /// targets default to [`TmuxSendKeysOutcome::Sent`].
    pub fn with_send_keys(
        mut self,
        target: impl Into<String>,
        outcome: TmuxSendKeysOutcome,
    ) -> Self {
        self.send_keys_outcomes.insert(target.into(), outcome);
        self
    }

    /// Snapshot of `(socket_name, target, new_name)` triples
    /// recorded by [`MuxBackend::rename_session`].
    pub fn rename_calls(&self) -> Vec<FakeTmuxRenameCall> {
        self.rename_calls.lock().unwrap().clone()
    }

    /// Snapshot of `(socket_name, name, cwd, argv)` tuples recorded
    /// by [`MuxBackend::new_session`].
    pub fn new_session_calls(&self) -> Vec<FakeTmuxNewSessionCall> {
        self.new_session_calls.lock().unwrap().clone()
    }

    /// Snapshot of `(socket_name, name)` pairs recorded by
    /// [`MuxBackend::attach_session`].
    pub fn attach_calls(&self) -> Vec<FakeTmuxAttachCall> {
        self.attach_calls.lock().unwrap().clone()
    }

    /// Snapshot of `(socket_name, target, literal, press_enter)`
    /// tuples recorded by [`MuxBackend::send_keys`].
    pub fn send_keys_calls(&self) -> Vec<FakeTmuxSendKeysCall> {
        self.send_keys_calls.lock().unwrap().clone()
    }
}

impl MuxBackend for FakeTmux {
    fn backend_key(&self) -> &'static str {
        TMUX_BACKEND
    }

    fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
        Ok(self.outcome.clone())
    }

    fn capture_pane(&self, _socket_name: Option<&str>, target: &str) -> Result<TmuxCaptureOutcome> {
        Ok(self
            .captures
            .get(target)
            .cloned()
            .unwrap_or(TmuxCaptureOutcome::Unsupported))
    }

    fn rename_session(
        &self,
        socket_name: Option<&str>,
        target: &str,
        new_name: &str,
    ) -> Result<TmuxRenameOutcome> {
        self.rename_calls.lock().unwrap().push((
            socket_name.map(str::to_string),
            target.to_string(),
            new_name.to_string(),
        ));
        Ok(self
            .rename_outcomes
            .get(target)
            .cloned()
            .unwrap_or(TmuxRenameOutcome::Renamed))
    }

    fn new_session(
        &self,
        socket_name: Option<&str>,
        name: &str,
        cwd: &Path,
        argv: &[OsString],
    ) -> Result<TmuxNewSessionOutcome> {
        self.new_session_calls.lock().unwrap().push((
            socket_name.map(str::to_string),
            name.to_string(),
            cwd.to_path_buf(),
            argv.to_vec(),
        ));
        Ok(self
            .new_session_outcomes
            .get(name)
            .cloned()
            .unwrap_or(TmuxNewSessionOutcome::Created))
    }

    fn attach_session(&self, socket_name: Option<&str>, name: &str) -> Result<TmuxAttachOutcome> {
        self.attach_calls
            .lock()
            .unwrap()
            .push((socket_name.map(str::to_string), name.to_string()));
        Ok(self
            .attach_outcomes
            .get(name)
            .cloned()
            .unwrap_or(TmuxAttachOutcome::Detached))
    }

    fn send_keys(
        &self,
        socket_name: Option<&str>,
        target: &str,
        literal: &str,
        press_enter: bool,
    ) -> Result<TmuxSendKeysOutcome> {
        self.send_keys_calls.lock().unwrap().push((
            socket_name.map(str::to_string),
            target.to_string(),
            literal.to_string(),
            press_enter,
        ));
        Ok(self
            .send_keys_outcomes
            .get(target)
            .cloned()
            .unwrap_or(TmuxSendKeysOutcome::Sent))
    }
}

impl MuxBackend for Box<dyn MuxBackend> {
    fn backend_key(&self) -> &'static str {
        (**self).backend_key()
    }

    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome> {
        (**self).list_sessions(format)
    }

    fn capture_pane(&self, socket_name: Option<&str>, target: &str) -> Result<TmuxCaptureOutcome> {
        (**self).capture_pane(socket_name, target)
    }

    fn rename_session(
        &self,
        socket_name: Option<&str>,
        target: &str,
        new_name: &str,
    ) -> Result<TmuxRenameOutcome> {
        (**self).rename_session(socket_name, target, new_name)
    }

    fn new_session(
        &self,
        socket_name: Option<&str>,
        name: &str,
        cwd: &Path,
        argv: &[OsString],
    ) -> Result<TmuxNewSessionOutcome> {
        (**self).new_session(socket_name, name, cwd, argv)
    }

    fn attach_session(&self, socket_name: Option<&str>, name: &str) -> Result<TmuxAttachOutcome> {
        (**self).attach_session(socket_name, name)
    }

    fn send_keys(
        &self,
        socket_name: Option<&str>,
        target: &str,
        literal: &str,
        press_enter: bool,
    ) -> Result<TmuxSendKeysOutcome> {
        (**self).send_keys(socket_name, target, literal, press_enter)
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
    pub active_pane_command: Option<String>,
    pub active_pane_pid: Option<i64>,
    pub active_pane_current_path: Option<String>,
    pub active_pane_start_command: Option<String>,
    pub client_attached: Option<bool>,
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
    let active_pane_command = optional_string(fields.next());
    let active_pane_pid = optional_epoch(fields.next());
    let active_pane_current_path = optional_string(fields.next());
    let active_pane_start_command = optional_string(fields.next());
    let attached_count = optional_usize(fields.next());
    let attached_list = optional_string(fields.next());
    let client_attached = interactive_client_attached(attached_count, attached_list.as_deref());

    Some(TmuxSessionRow {
        name,
        path,
        activity_epoch,
        created_epoch,
        active_pane_command,
        active_pane_pid,
        active_pane_current_path,
        active_pane_start_command,
        client_attached,
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

fn optional_usize(value: Option<&str>) -> Option<usize> {
    optional_string(value)?.parse().ok()
}

fn interactive_client_attached(
    attached_count: Option<usize>,
    attached_list: Option<&str>,
) -> Option<bool> {
    if let Some(attached_list) = attached_list {
        return Some(
            attached_list
                .split(',')
                .map(str::trim)
                .any(looks_like_interactive_client),
        );
    }
    attached_count.map(|count| count > 0)
}

fn looks_like_interactive_client(client_name: &str) -> bool {
    client_name.starts_with('/')
}

#[derive(Clone, Debug)]
pub struct TmuxDiscovery<R: MuxBackend> {
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

impl<R: MuxBackend> TmuxDiscovery<R> {
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

impl<R: MuxBackend + 'static> DiscoveryProvider for TmuxDiscovery<R> {
    fn discover(&self, _context: &DiscoveryContext) -> Result<GraphFragment> {
        let outcome = self.rows()?;
        let mut nodes = Vec::with_capacity(outcome.rows.len());

        for row in &outcome.rows {
            nodes.push(GraphNode::MuxSession(MuxSessionNode {
                id: MuxSessionId::new(format!("{TMUX_BACKEND}:{}", row.name)),
                backend: TMUX_BACKEND.to_string(),
                native_id: row.name.clone(),
                cwd: row.path.clone(),
                active_pane_command: row.active_pane_command.clone(),
                active_pane_pid: row.active_pane_pid,
                active_pane_current_path: row.active_pane_current_path.clone(),
                active_pane_start_command: row.active_pane_start_command.clone(),
                client_attached: row.client_attached,
                activity_epoch: row.activity_epoch,
                created_epoch: row.created_epoch,
            }));
        }

        let mut fragment = GraphFragment {
            nodes,
            candidate_links: Vec::new(),
            diagnostics: Vec::new(),
            node_provenance: BTreeMap::new(),
        };
        crate::discovery::stamp_fragment(
            &mut fragment,
            TMUX_BACKEND,
            crate::discovery::current_epoch(),
        );
        Ok(fragment)
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
                active_pane_command: None,
                active_pane_pid: None,
                active_pane_current_path: None,
                active_pane_start_command: None,
                client_attached: None,
            }]
        );
    }

    #[test]
    fn parser_extracts_active_pane_process_fields() {
        let rows = parse_list_sessions(
            "alpha\t/work\t1700000500\t1700000000\tclaude\t123\t/work\tclaude --resume abc\n",
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].active_pane_command.as_deref(), Some("claude"));
        assert_eq!(rows[0].active_pane_pid, Some(123));
        assert_eq!(rows[0].active_pane_current_path.as_deref(), Some("/work"));
        assert_eq!(
            rows[0].active_pane_start_command.as_deref(),
            Some("claude --resume abc")
        );
    }

    #[test]
    fn parser_extracts_session_attached_flag() {
        let rows = parse_list_sessions("alpha\t/work\t1700000500\t1700000000\t\t\t\t\t1\n");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client_attached, Some(true));

        let rows = parse_list_sessions("beta\t/work\t1700000500\t1700000000\t\t\t\t\t0\n");
        assert_eq!(rows[0].client_attached, Some(false));
    }

    #[test]
    fn parser_treats_attached_list_as_interactive_tty_signal() {
        let rows = parse_list_sessions(
            "alpha\t/work\t1700000500\t1700000000\t\t\t\t\t2\tclient-123,/dev/pts/1\n",
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client_attached, Some(true));

        let rows =
            parse_list_sessions("beta\t/work\t1700000500\t1700000000\t\t\t\t\t1\tclient-123\n");
        assert_eq!(rows[0].client_attached, Some(false));
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
        let outcome = runner.capture_pane(None, "editor").expect("non-fatal");
        assert_eq!(
            outcome,
            TmuxCaptureOutcome::Unavailable(UnavailableReason::BinaryNotFound)
        );
    }

    #[test]
    fn fake_runner_default_capture_pane_returns_unsupported() {
        let runner = FakeTmux::with_sessions("");
        assert_eq!(
            runner.capture_pane(None, "anything").unwrap(),
            TmuxCaptureOutcome::Unsupported
        );
    }

    #[test]
    fn missing_binary_rename_session_reports_unavailable_binary_not_found() {
        let runner = SystemTmux::with_binary("/definitely/not/here/tmux");
        let outcome = runner
            .rename_session(None, "alpha", "new")
            .expect("non-fatal");
        assert_eq!(
            outcome,
            TmuxRenameOutcome::Unavailable(UnavailableReason::BinaryNotFound)
        );
    }

    #[test]
    fn fake_runner_default_rename_session_records_call_and_returns_renamed() {
        let runner = FakeTmux::with_sessions("");
        let outcome = runner.rename_session(None, "alpha", "beta").expect("ok");
        assert_eq!(outcome, TmuxRenameOutcome::Renamed);
        assert_eq!(
            runner.rename_calls(),
            vec![(None, "alpha".to_string(), "beta".to_string())]
        );
    }

    #[test]
    fn fake_runner_returns_registered_rename_outcome_by_target() {
        let runner = FakeTmux::with_sessions("")
            .with_rename("alpha", TmuxRenameOutcome::NoTarget)
            .with_rename("beta", TmuxRenameOutcome::NameCollision)
            .with_rename(
                "broken",
                TmuxRenameOutcome::Failed {
                    code: Some(1),
                    message: "boom".to_string(),
                },
            );
        assert_eq!(
            runner.rename_session(None, "alpha", "alias").unwrap(),
            TmuxRenameOutcome::NoTarget
        );
        assert_eq!(
            runner.rename_session(None, "beta", "alias").unwrap(),
            TmuxRenameOutcome::NameCollision
        );
        assert_eq!(
            runner.rename_session(None, "broken", "alias").unwrap(),
            TmuxRenameOutcome::Failed {
                code: Some(1),
                message: "boom".to_string(),
            }
        );
        assert_eq!(
            runner.rename_session(None, "other", "alias").unwrap(),
            TmuxRenameOutcome::Renamed
        );
        assert_eq!(
            runner.rename_calls(),
            vec![
                (None, "alpha".to_string(), "alias".to_string()),
                (None, "beta".to_string(), "alias".to_string()),
                (None, "broken".to_string(), "alias".to_string()),
                (None, "other".to_string(), "alias".to_string()),
            ]
        );
    }

    #[test]
    fn default_rename_session_impl_returns_unsupported() {
        struct ReadOnlyRunner;
        impl MuxBackend for ReadOnlyRunner {
            fn backend_key(&self) -> &'static str {
                "test-read-only"
            }

            fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
                Ok(TmuxOutcome::Sessions(String::new()))
            }
        }
        let runner = ReadOnlyRunner;
        assert_eq!(
            runner.rename_session(None, "alpha", "beta").unwrap(),
            TmuxRenameOutcome::Unsupported
        );
    }

    #[test]
    fn name_collision_stderr_maps_to_name_collision() {
        assert!(looks_like_name_collision("duplicate session: alpha"));
        assert!(looks_like_name_collision("session already exists"));
        assert!(!looks_like_name_collision("can't find session alpha"));
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
            runner.capture_pane(None, "editor").unwrap(),
            TmuxCaptureOutcome::Captured("pane content".to_string())
        );
        assert_eq!(
            runner.capture_pane(None, "missing").unwrap(),
            TmuxCaptureOutcome::NoTarget
        );
        assert_eq!(
            runner.capture_pane(None, "broken").unwrap(),
            TmuxCaptureOutcome::Failed {
                code: Some(1),
                message: "boom".to_string(),
            }
        );
        // Unregistered targets keep the default Unsupported.
        assert_eq!(
            runner.capture_pane(None, "other").unwrap(),
            TmuxCaptureOutcome::Unsupported
        );
    }

    // ---- H-PIN-010 socket threading + new mutation methods ----

    #[test]
    fn fake_runner_default_new_session_records_call_and_returns_created() {
        let runner = FakeTmux::with_sessions("");
        let outcome = runner
            .new_session(
                None,
                "ingest",
                Path::new("/tmp/repo"),
                &[OsString::from("codex")],
            )
            .expect("ok");
        assert_eq!(outcome, TmuxNewSessionOutcome::Created);
        assert_eq!(
            runner.new_session_calls(),
            vec![(
                None,
                "ingest".to_string(),
                PathBuf::from("/tmp/repo"),
                vec![OsString::from("codex")],
            )]
        );
    }

    #[test]
    fn fake_runner_threads_non_default_socket_into_recorded_calls() {
        let runner = FakeTmux::with_sessions("");
        runner
            .new_session(
                Some("scratch"),
                "ingest",
                Path::new("/tmp/repo"),
                &[OsString::from("codex")],
            )
            .expect("ok");
        runner
            .attach_session(Some("scratch"), "ingest")
            .expect("ok");
        runner
            .send_keys(Some("scratch"), "ingest", "codex --resume", true)
            .expect("ok");
        runner
            .rename_session(Some("scratch"), "ingest", "ingest-refactor")
            .expect("ok");

        assert_eq!(
            runner.new_session_calls().first().unwrap().0.as_deref(),
            Some("scratch")
        );
        assert_eq!(
            runner.attach_calls(),
            vec![(Some("scratch".to_string()), "ingest".to_string())]
        );
        assert_eq!(
            runner.send_keys_calls(),
            vec![(
                Some("scratch".to_string()),
                "ingest".to_string(),
                "codex --resume".to_string(),
                true,
            )]
        );
        assert_eq!(
            runner.rename_calls(),
            vec![(
                Some("scratch".to_string()),
                "ingest".to_string(),
                "ingest-refactor".to_string(),
            )]
        );
    }

    #[test]
    fn fake_runner_returns_registered_outcomes_per_method() {
        let runner = FakeTmux::with_sessions("")
            .with_new_session("taken", TmuxNewSessionOutcome::NameTaken)
            .with_attach("missing", TmuxAttachOutcome::NoTarget)
            .with_send_keys("missing", TmuxSendKeysOutcome::NoTarget);

        assert_eq!(
            runner
                .new_session(None, "taken", Path::new("/tmp"), &[])
                .unwrap(),
            TmuxNewSessionOutcome::NameTaken
        );
        assert_eq!(
            runner.attach_session(None, "missing").unwrap(),
            TmuxAttachOutcome::NoTarget
        );
        assert_eq!(
            runner.send_keys(None, "missing", "echo hi", false).unwrap(),
            TmuxSendKeysOutcome::NoTarget
        );
    }

    #[test]
    fn default_trait_impls_for_new_methods_return_unsupported() {
        struct MinimalRunner;
        impl MuxBackend for MinimalRunner {
            fn backend_key(&self) -> &'static str {
                "test-minimal"
            }

            fn list_sessions(&self, _format: &str) -> Result<TmuxOutcome> {
                Ok(TmuxOutcome::Sessions(String::new()))
            }
        }
        let runner = MinimalRunner;
        assert_eq!(
            runner
                .new_session(None, "x", Path::new("/tmp"), &[])
                .unwrap(),
            TmuxNewSessionOutcome::Unsupported
        );
        assert_eq!(
            runner.attach_session(None, "x").unwrap(),
            TmuxAttachOutcome::Unsupported
        );
        assert_eq!(
            runner.send_keys(None, "x", "y", false).unwrap(),
            TmuxSendKeysOutcome::Unsupported
        );
    }

    #[test]
    fn missing_binary_new_session_reports_unavailable() {
        let runner = SystemTmux::with_binary("/definitely/no/such/binary");
        let outcome = runner
            .new_session(None, "ingest", Path::new("/tmp"), &[])
            .expect("non-fatal");
        assert!(matches!(
            outcome,
            TmuxNewSessionOutcome::Unavailable(UnavailableReason::BinaryNotFound)
        ));
    }
}
