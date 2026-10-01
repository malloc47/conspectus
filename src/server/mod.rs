//! `conspectus serve` daemon (ADR 0082).
//!
//! Long-running process that keeps a resolved `GraphSnapshot`
//! warm in the background so concurrent one-shot CLI invocations
//! (and the TUI) can pull pre-resolved data over the socket
//! without paying the discovery + resolve cost. The daemon
//! spawns one worker thread per provider class (harness / mux /
//! git / forge per ADR 0079) and each thread re-runs *only its
//! class's* discovery providers on its own `[server.intervals]`
//! cadence.
//!
//! State model:
//!
//! * The daemon's working state lives in two in-memory caches:
//!   [`SnapshotState`] (the live `GraphSnapshot` used as the
//!   per-class refresh prior) and [`SnapshotBytes`] (the
//!   serialized form served verbatim over the socket
//!   `snapshot` command). Both refresh in lockstep via
//!   `publish_snapshot` after every successful cycle.
//! * Persistence is the single `graph.bin` zero-copy artifact
//!   per ADR 0083. Daemonless one-shot CLIs read it via
//!   `snapshot::open_mmap`; the daemon's own warm-restart path
//!   reads it once on startup to seed
//!   [`SnapshotState`] so the first cycle isn't a cold rebuild.
//! * Per-thread cycles are atomic across the full load + evict +
//!   run + publish sequence: every class thread takes a
//!   process-local [`std::sync::Mutex`] before its cycle so two
//!   class threads cannot race on the cache update. The mutex
//!   is brief; discovery runs outside it.
//!
//! Lifecycle expectations:
//!
//! * The process is user-managed (systemd user unit, launchd
//!   agent, or `conspectus serve &`) per ADR 0038. The CLI
//!   must not auto-spawn it.
//! * SIGINT and SIGTERM flip a shared shutdown flag via
//!   `signal-hook` (ADR 0080). Every scheduler thread polls the
//!   flag between sleeps so a shutdown that arrives mid-tick
//!   still completes the cycle in progress before exiting. The
//!   200ms poll cadence trades a negligible CPU floor for
//!   snappy Ctrl-C response.
//! * Daemon startup cleans up any legacy `graph.sqlite*` files
//!   left behind by pre-ADR-0082 builds. The cleanup is
//!   best-effort and one-shot; the daemon has no consumer for
//!   those files.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

pub mod watcher;

use crate::config::ServerIntervals;
use crate::discovery::cache::ProviderClass;
use crate::discovery::{
    DiscoveryCaches, LocalDiscoveryConfig, discover_local_warm_with, hook_sidecar,
};
use crate::hook::HookRecord;
use crate::model::GraphSnapshot;
use crate::resolve::resolve_snapshot;
use crate::server::watcher::{NotifyWatcher, NullWatcher, Watcher, WatcherEvent};
use crate::snapshot;

/// Observable per-class scheduler state. Each class
/// thread updates its entry on every cycle; the `status` socket
/// command reads under a [`Mutex`]. Serialized verbatim into the
/// status response so a future operator-facing diff or
/// monitoring tool can consume the same shape as `conspectus
/// status --format json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ClassState {
    /// Wall-clock epoch (seconds) at which the most recent cycle
    /// for this class started. `None` before the first tick.
    pub last_started_epoch: Option<i64>,
    /// Wall-clock epoch at which the most recent cycle for this
    /// class finished. `< last_started_epoch` means a cycle is
    /// currently in flight; equal means idle between ticks.
    pub last_completed_epoch: Option<i64>,
    /// `"ok"` when the most recent completed cycle succeeded;
    /// `"error"` when it failed. `None` before the first
    /// completion.
    pub last_outcome: Option<String>,
    /// Error detail string when `last_outcome == "error"`.
    /// Cleared on the next successful cycle.
    pub last_error: Option<String>,
}

/// Shared scheduler state — one entry per provider class. The
/// daemon owns the `Arc<Mutex<_>>`; class threads briefly take
/// the lock to write their cycle outcome, the status handler
/// briefly takes it to snapshot for serialization. Held only for
/// the duration of a single read or write, never across
/// discovery work.
#[derive(Debug, Default)]
pub struct SchedulerState {
    classes: BTreeMap<&'static str, ClassState>,
}

impl SchedulerState {
    /// Clone the current per-class map. The result owns its
    /// strings so the caller can drop the lock immediately.
    pub fn snapshot(&self) -> BTreeMap<String, ClassState> {
        self.classes
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    fn record_started(&mut self, class: ProviderClass, epoch: i64) {
        self.classes
            .entry(class.name())
            .or_default()
            .last_started_epoch = Some(epoch);
    }

    fn record_completed(&mut self, class: ProviderClass, epoch: i64, outcome: Result<(), String>) {
        let entry = self.classes.entry(class.name()).or_default();
        entry.last_completed_epoch = Some(epoch);
        match outcome {
            Ok(()) => {
                entry.last_outcome = Some("ok".to_string());
                entry.last_error = None;
            }
            Err(message) => {
                entry.last_outcome = Some("error".to_string());
                entry.last_error = Some(message);
            }
        }
    }
}

/// Inputs to the daemon main loop. Built by the CLI shell from
/// `[server.intervals]` + `--scan-root` + the discovered cwd.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Discovery scan roots. Empty means "use the process cwd"
    /// just like the one-shot CLI's default.
    pub scan_roots: Vec<PathBuf>,
    /// Per-class warm-start TTL intervals. Each class gets its
    /// own scheduler thread that ticks on its interval.
    pub intervals: ServerIntervals,
}

/// Resolve the canonical Unix-domain socket path per ADR 0038.
///
/// Resolution order:
///
/// 1. `$XDG_RUNTIME_DIR/conspectus/server.sock` (the documented
///    canonical location on Linux + freedesktop-spec setups).
/// 2. `$TMPDIR/conspectus-$UID/server.sock` (fallback when
///    `XDG_RUNTIME_DIR` is unset — common on stock macOS).
/// 3. `/tmp/conspectus-$UID/server.sock` (final fallback when
///    `$TMPDIR` is also unset).
///
/// The `-$UID` segregation in the TMPDIR fallback prevents
/// collisions on multi-user systems where the runtime dir is
/// shared. The socket file itself is mode 0600 either way.
pub fn socket_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("conspectus").join("server.sock");
    }
    // SAFETY: getuid() is async-signal-safe and never fails.
    let uid = unsafe { libc::getuid() };
    let base = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    base.join(format!("conspectus-{uid}")).join("server.sock")
}

/// Bind the Unix-domain socket at [`socket_path`], creating the
/// parent directory if needed and unlinking any stale socket
/// file from a previous run that did not clean up. The listener
/// is set non-blocking so the listener thread can poll the
/// shutdown flag between accepts.
fn bind_socket(path: &Path) -> Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create socket parent dir {}", parent.display()))?;
    }
    // A previous daemon that crashed without cleaning up its
    // socket file would otherwise make `bind` return EADDRINUSE.
    // The unlink is unconditional because the socket file is
    // useless once the owning process is gone — stale sockets do
    // not serve.
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).with_context(|| format!("bind {}", path.display()))?;
    listener
        .set_nonblocking(true)
        .context("set socket non-blocking")?;
    // Mode 0600: only the owner can read/write. Defends against
    // any reader on a shared host poking at the protocol or
    // sniffing mutation traffic.
    let mut perms = std::fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms).with_context(|| format!("chmod {}", path.display()))?;
    Ok(listener)
}

/// Wire-shape request frame per ADR 0038. `id` is opaque to the
/// server — it round-trips into the response so a multiplexing
/// client (none today, but the spec leaves the door open) can
/// correlate. `args` is provider-specific JSON; commands that
/// take no arguments leave it as the default `null`.
#[derive(Debug, Deserialize)]
struct Request {
    command: String,
    #[serde(default)]
    args: serde_json::Value,
    #[serde(default)]
    id: String,
}

/// Wire-shape response frame per ADR 0038. `result` is the
/// two-state `"ok"` / `"error"` discriminator the client matches
/// on first; `data` and `error` are populated mutually
/// exclusively based on `result`.
#[derive(Debug, Serialize)]
struct Response {
    id: String,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

/// Structured error body inside a `result = "error"` response.
/// `code` is a stable machine-readable identifier the CLI can
/// pattern-match on; `message` is the human-readable detail.
#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

/// Read one length-prefixed frame from the stream. The 4-byte
/// big-endian prefix matches the wire format ADR 0038
/// specifies; an oversized length is rejected to avoid a
/// hostile client allocating gigabytes by sending a forged
/// header.
fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>> {
    const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .context("read length prefix")?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(anyhow!(
            "request frame size {len} exceeds {MAX_FRAME_BYTES}-byte cap"
        ));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).context("read frame body")?;
    Ok(buf)
}

/// Write one length-prefixed frame to the stream.
fn write_frame(stream: &mut UnixStream, payload: &[u8]) -> Result<()> {
    let len =
        u32::try_from(payload.len()).context("response too large for 32-bit length prefix")?;
    stream
        .write_all(&len.to_be_bytes())
        .context("write length prefix")?;
    stream.write_all(payload).context("write frame body")?;
    Ok(())
}

/// Shared cache of the most recently serialized snapshot bytes
/// (header + rkyv archive) per ADR 0083. The daemon updates it
/// after every successful cycle so the socket
/// `snapshot` command can serve verbatim bytes without
/// re-serializing per connection. `None` before the first cycle
/// completes. The `Arc` around `Vec<u8>` lets the socket handler
/// clone-and-return without copying the payload.
///
/// A future optimization can replace the `Mutex` with `ArcSwap`
/// once the socket command's profile shows readers contending
/// with writers (the writer Mutex around each cycle already
/// serializes the producer side, so contention is only on the
/// socket path).
pub type SnapshotBytes = Arc<Mutex<Option<Arc<Vec<u8>>>>>;

/// Live in-memory snapshot the per-class scheduler treats as the
/// prior for the next cycle. Refreshed alongside
/// [`SnapshotBytes`] after every successful cycle. `None` before
/// the first cycle completes or before a warm-start reload
/// populates it from `graph.bin` at daemon startup.
///
/// The daemon never reads its own on-disk artifact during normal
/// operation. The on-disk `graph.bin` exists for
/// daemonless consumers and for daemon warm-restart;
/// it is not the daemon's working state.
pub type SnapshotState = Arc<Mutex<Option<GraphSnapshot>>>;

/// Daemon state shared by the scheduler threads and every
/// per-connection worker, so command handlers that rebuild the graph
/// use the same writer lock, discovery caches, and published
/// snapshot as the scheduler. Cheaply cloneable (every field is an
/// `Arc`).
#[derive(Clone)]
struct DispatchCtx {
    scan_roots: Arc<Vec<PathBuf>>,
    intervals: Arc<ServerIntervals>,
    writer_lock: Arc<Mutex<()>>,
    state: Arc<Mutex<SchedulerState>>,
    snapshot_bytes: SnapshotBytes,
    snapshot_state: SnapshotState,
    snapshot_path: Arc<PathBuf>,
    /// Discovery results reused across cycles (ADR 0098).
    discovery_caches: Arc<DiscoveryCaches>,
}

/// Outcome of a client-side socket call.
#[derive(Debug)]
pub enum ClientOutcome<T> {
    /// The daemon responded successfully with `T` as the parsed
    /// response payload.
    Ok(T),
    /// The daemon responded with a structured error (`result: "error"`).
    DaemonError { code: String, message: String },
    /// No daemon is listening on the socket. Callers fall back
    /// to local execution.
    NoDaemon,
    /// The connection or framing layer hit an unrecoverable
    /// error (e.g. partial read, malformed frame). Callers
    /// typically surface this as a CLI error.
    Transport(anyhow::Error),
}

/// Send a `ping` request to the daemon. Returns the echoed
/// `args` payload on success. Mostly useful as a liveness
/// probe + a smoke test for the wire shape; the CLI uses it to
/// detect whether to route a follow-up command through the
/// socket or fall back to one-shot mode.
pub fn client_ping() -> ClientOutcome<serde_json::Value> {
    call_command("ping", serde_json::Value::Null, "ping")
}

/// Send a `status` request to the daemon. Returns the per-class
/// state map on success. The map keys are the
/// [`ProviderClass::name`] values (`"git"`, `"mux"`,
/// `"harness"`, `"forge"`); a class that has not yet completed
/// its first tick may be absent from the map.
pub fn client_status() -> ClientOutcome<BTreeMap<String, ClassState>> {
    match call_command("status", serde_json::Value::Null, "cli-status") {
        ClientOutcome::Ok(value) => {
            let classes = value
                .get("classes")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            match serde_json::from_value::<BTreeMap<String, ClassState>>(classes) {
                Ok(map) => ClientOutcome::Ok(map),
                Err(err) => ClientOutcome::Transport(anyhow!(err).context("parse status payload")),
            }
        }
        ClientOutcome::DaemonError { code, message } => {
            ClientOutcome::DaemonError { code, message }
        }
        ClientOutcome::NoDaemon => ClientOutcome::NoDaemon,
        ClientOutcome::Transport(err) => ClientOutcome::Transport(err),
    }
}

/// Send a `refresh` request to the daemon. When `class` is
/// `Some`, only that class's slice is re-run; when `None`, the
/// daemon performs a full cold rebuild. Returns the
/// `refreshed_epoch` field on success — the wall-clock second
/// at which the daemon took the writer lock to begin the
/// rebuild.
pub fn client_refresh(class: Option<&str>) -> ClientOutcome<u64> {
    let args = match class {
        Some(c) => serde_json::json!({"class": c}),
        None => serde_json::Value::Null,
    };
    match call_command("refresh", args, "cli-refresh") {
        ClientOutcome::Ok(value) => match value
            .get("refreshed_epoch")
            .and_then(serde_json::Value::as_u64)
        {
            Some(epoch) => ClientOutcome::Ok(epoch),
            None => ClientOutcome::Transport(anyhow!(
                "refresh response missing refreshed_epoch: {value}"
            )),
        },
        other => match other {
            ClientOutcome::DaemonError { code, message } => {
                ClientOutcome::DaemonError { code, message }
            }
            ClientOutcome::NoDaemon => ClientOutcome::NoDaemon,
            ClientOutcome::Transport(err) => ClientOutcome::Transport(err),
            ClientOutcome::Ok(_) => unreachable!(),
        },
    }
}

/// Send a hook observation to the daemon so it can update the
/// live in-memory graph and persist the result via `graph.bin`.
/// If the daemon has not completed its first snapshot cycle, it
/// returns `snapshot_unavailable`; callers should fall back to
/// the daemonless latest-hook spool.
pub fn client_hook_ingest(record: &HookRecord) -> ClientOutcome<()> {
    let args = match serde_json::to_value(record) {
        Ok(record) => serde_json::json!({ "record": record }),
        Err(err) => return ClientOutcome::Transport(anyhow!(err).context("serialize hook record")),
    };
    match call_command("hook_ingest", args, "hook-ingest") {
        ClientOutcome::Ok(_) => ClientOutcome::Ok(()),
        ClientOutcome::DaemonError { code, message } => {
            ClientOutcome::DaemonError { code, message }
        }
        ClientOutcome::NoDaemon => ClientOutcome::NoDaemon,
        ClientOutcome::Transport(err) => ClientOutcome::Transport(err),
    }
}

/// Send a `snapshot` request to the daemon and return the
/// decoded snapshot bytes (header + rkyv archive per ADR 0083).
/// Pre-first-cycle responses surface as
/// `DaemonError { code: "snapshot_unavailable" }`; callers
/// typically translate that into a cold-rebuild fallback path
/// or a "waiting for first cycle" UI hint.
///
/// The returned bytes can be passed to
/// `snapshot::open_mmap_unvalidated` after writing to a tmp
/// file (or, eventually, fed to a future `from_bytes` reader
/// that skips the mmap detour). Validation is skipped because
/// the bytes come from the daemon's own
/// `dual_write_artifact` cycle — already structurally sound.
pub fn client_snapshot() -> ClientOutcome<Vec<u8>> {
    use base64::Engine;

    match call_command("snapshot", serde_json::Value::Null, "cli-snapshot") {
        ClientOutcome::Ok(value) => {
            let Some(encoded) = value.get("bytes").and_then(|v| v.as_str()) else {
                return ClientOutcome::Transport(anyhow!(
                    "snapshot response missing data.bytes string: {value}"
                ));
            };
            match base64::engine::general_purpose::STANDARD.decode(encoded) {
                Ok(bytes) => ClientOutcome::Ok(bytes),
                Err(err) => ClientOutcome::Transport(
                    anyhow!(err).context("decode snapshot response bytes (base64)"),
                ),
            }
        }
        ClientOutcome::DaemonError { code, message } => {
            ClientOutcome::DaemonError { code, message }
        }
        ClientOutcome::NoDaemon => ClientOutcome::NoDaemon,
        ClientOutcome::Transport(err) => ClientOutcome::Transport(err),
    }
}

/// Common framing for client-side calls. Connects to the
/// canonical socket path, sends the framed request, reads the
/// framed response, parses the JSON envelope.
fn call_command(
    command: &str,
    args: serde_json::Value,
    id: &str,
) -> ClientOutcome<serde_json::Value> {
    let path = socket_path();
    let mut stream = match UnixStream::connect(&path) {
        Ok(stream) => stream,
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound
                || err.kind() == std::io::ErrorKind::ConnectionRefused =>
        {
            return ClientOutcome::NoDaemon;
        }
        Err(err) => {
            return ClientOutcome::Transport(
                anyhow!(err).context(format!("connect to {}", path.display())),
            );
        }
    };
    let request = serde_json::json!({
        "command": command,
        "args": args,
        "id": id,
    });
    let payload = match serde_json::to_vec(&request) {
        Ok(v) => v,
        Err(err) => return ClientOutcome::Transport(anyhow!(err)),
    };
    if let Err(err) = write_frame(&mut stream, &payload) {
        return ClientOutcome::Transport(err);
    }
    let frame = match read_frame(&mut stream) {
        Ok(v) => v,
        Err(err) => return ClientOutcome::Transport(err),
    };
    let envelope: serde_json::Value = match serde_json::from_slice(&frame) {
        Ok(v) => v,
        Err(err) => return ClientOutcome::Transport(anyhow!(err).context("parse response JSON")),
    };
    match envelope.get("result").and_then(|v| v.as_str()) {
        Some("ok") => ClientOutcome::Ok(
            envelope
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        ),
        Some("error") => {
            let code = envelope
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let message = envelope
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("(no message)")
                .to_string();
            ClientOutcome::DaemonError { code, message }
        }
        _ => ClientOutcome::Transport(anyhow!("response envelope missing result: {envelope}")),
    }
}

/// Per-connection handler. Reads one request frame, dispatches
/// it, writes the response. ADR 0038's framing is one
/// request/response per connection (the daemon does not
/// multiplex). Errors during the read/write are logged + the
/// connection is dropped; a misbehaving client cannot crash the
/// daemon.
fn handle_connection(mut stream: UnixStream, ctx: DispatchCtx) {
    if let Err(err) = try_handle_connection(&mut stream, &ctx) {
        eprintln!("conspectus serve: socket handler error: {err:#}");
    }
}

fn try_handle_connection(stream: &mut UnixStream, ctx: &DispatchCtx) -> Result<()> {
    let frame = read_frame(stream)?;
    let request: Request = serde_json::from_slice(&frame).context("parse request JSON")?;
    let response = dispatch(&request, ctx);
    let payload = serde_json::to_vec(&response).context("serialize response JSON")?;
    write_frame(stream, &payload)?;
    Ok(())
}

/// Command-dispatch table. v1 ships:
///
/// * `ping` — wire-shape sanity check; echoes `args` back under
///   `data.echo`. The CLI client uses this for liveness probes.
/// * `refresh` — forces a full cold rebuild on the daemon side
///   (or, with `args.class`, one class's cycle) and publishes the
///   result to `graph.bin` and the in-memory caches. Useful when an
///   operator
///   knows the on-disk world changed in a way the TTL gate would
///   not pick up for a while (e.g. they just `gh pr create`d
///   and want forge state refreshed now without waiting 5
///   minutes for the next forge tick).
/// * `status` — returns the per-class `SchedulerState` map.
/// * `snapshot` — returns the cached serialized snapshot bytes
///   (header + rkyv archive per ADR 0083) base64-encoded under
///   `data.bytes`. The TUI and the one-shot CLI consume this
///   command. Returns
///   `snapshot_unavailable` when the daemon has not yet
///   completed its first cycle.
/// * `hook_ingest` — applies one harness hook observation to the
///   live in-memory graph, re-resolves, and republishes through
///   the normal `graph.bin` path. Hook writers use their compact
///   on-disk spool only when this command is unavailable.
///
/// The rename / declare-link / ignore-link mutation commands
/// remain operator-callable through one-shot CLI; they bypass
/// the daemon's writer lock and mutate their own config/state
/// files directly. Routing those through the socket is a future
/// cleanup once the daemon owns the full mutation surface.
fn dispatch(request: &Request, ctx: &DispatchCtx) -> Response {
    match request.command.as_str() {
        "ping" => Response {
            id: request.id.clone(),
            result: "ok",
            data: Some(serde_json::json!({"echo": request.args.clone()})),
            error: None,
        },
        "refresh" => handle_refresh(request, ctx),
        "status" => handle_status(request, ctx),
        "snapshot" => handle_snapshot(request, ctx),
        "hook_ingest" => handle_hook_ingest(request, ctx),
        unknown => Response {
            id: request.id.clone(),
            result: "error",
            data: None,
            error: Some(ErrorBody {
                code: "unknown_command".to_string(),
                message: format!("unknown command `{unknown}`"),
            }),
        },
    }
}

/// `hook_ingest` command handler. This is the daemon-owned fast path for
/// hook observations: mutate the live graph, re-resolve, and persist through
/// `publish_snapshot` so `graph.bin` remains the only Conspectus-owned graph
/// persistence artifact.
fn handle_hook_ingest(request: &Request, ctx: &DispatchCtx) -> Response {
    let record = match request.args.get("record").cloned() {
        Some(value) => match serde_json::from_value::<HookRecord>(value) {
            Ok(record) => record,
            Err(err) => {
                return Response {
                    id: request.id.clone(),
                    result: "error",
                    data: None,
                    error: Some(ErrorBody {
                        code: "invalid_hook_record".to_string(),
                        message: format!("{err:#}"),
                    }),
                };
            }
        },
        None => {
            return Response {
                id: request.id.clone(),
                result: "error",
                data: None,
                error: Some(ErrorBody {
                    code: "missing_hook_record".to_string(),
                    message: "hook_ingest requires args.record".to_string(),
                }),
            };
        }
    };

    let _guard = ctx
        .writer_lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(mut snapshot) = load_snapshot_state(&ctx.snapshot_state) else {
        return Response {
            id: request.id.clone(),
            result: "error",
            data: None,
            error: Some(ErrorBody {
                code: "snapshot_unavailable".to_string(),
                message: "daemon has not completed its first cycle".to_string(),
            }),
        };
    };
    hook_sidecar::apply_hook_records(
        &mut snapshot,
        vec![record],
        crate::discovery::current_epoch(),
    );
    let snapshot = resolve_snapshot(snapshot);
    publish_snapshot_to_path(
        snapshot,
        &ctx.snapshot_path,
        &ctx.snapshot_bytes,
        &ctx.snapshot_state,
    );
    Response {
        id: request.id.clone(),
        result: "ok",
        data: Some(serde_json::json!({ "ingested": true })),
        error: None,
    }
}

/// `snapshot` command handler. Reads the cached
/// serialized snapshot bytes the daemon populates after every
/// successful cycle (`dual_write_artifact`), base64-encodes
/// them into `data.bytes`, and returns. Pre-first-cycle calls
/// (the cache is `None`) get a `snapshot_unavailable` error so
/// the client can decide whether to wait, retry, or fall back
/// to cold rebuild.
///
/// The handler does no I/O beyond the response write — it just
/// clones the `Arc<Vec<u8>>` out of the cache under the Mutex
/// (held only long enough to copy the Arc handle), drops the
/// guard, then encodes. A long base64 pass therefore does not
/// block class threads from updating the cache.
fn handle_snapshot(request: &Request, ctx: &DispatchCtx) -> Response {
    use base64::Engine;

    let bytes = {
        let guard = ctx
            .snapshot_bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.clone()
    };
    let Some(bytes) = bytes else {
        return Response {
            id: request.id.clone(),
            result: "error",
            data: None,
            error: Some(ErrorBody {
                code: "snapshot_unavailable".to_string(),
                message: "daemon has not completed its first cycle".to_string(),
            }),
        };
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.as_slice());
    Response {
        id: request.id.clone(),
        result: "ok",
        data: Some(serde_json::json!({"bytes": encoded})),
        error: None,
    }
}

/// `status` command handler. Snapshots the
/// [`SchedulerState`] under the Mutex (held only long enough to
/// clone the map), serializes it as JSON, returns. The handler
/// does no I/O beyond the response write so it stays responsive
/// even while a class thread is mid-cycle.
fn handle_status(request: &Request, ctx: &DispatchCtx) -> Response {
    let guard = ctx.state.lock().unwrap_or_else(PoisonError::into_inner);
    let snapshot = guard.snapshot();
    drop(guard);
    let data = match serde_json::to_value(&snapshot) {
        Ok(v) => v,
        Err(err) => {
            return Response {
                id: request.id.clone(),
                result: "error",
                data: None,
                error: Some(ErrorBody {
                    code: "status_serialize_failed".to_string(),
                    message: format!("{err:#}"),
                }),
            };
        }
    };
    Response {
        id: request.id.clone(),
        result: "ok",
        data: Some(serde_json::json!({"classes": data})),
        error: None,
    }
}

/// `refresh` command handler. Acquires the writer lock and runs
/// either a full cold rebuild (default) or — when the caller
/// passes `args.class = "<name>"` — only that class's slice via
/// the same `try_class_cycle` the scheduler uses. Synchronous
/// from the client's perspective; the response lands only after
/// the rebuild commits.
fn handle_refresh(request: &Request, ctx: &DispatchCtx) -> Response {
    let class_arg = request.args.get("class").and_then(|v| v.as_str());
    let class = match class_arg {
        None => None,
        Some(name) => match ProviderClass::parse(name) {
            Some(c) => Some(c),
            None => {
                return Response {
                    id: request.id.clone(),
                    result: "error",
                    data: None,
                    error: Some(ErrorBody {
                        code: "unknown_class".to_string(),
                        message: format!(
                            "unknown class `{name}`; expected one of git, mux, harness, forge"
                        ),
                    }),
                };
            }
        },
    };

    let _guard = ctx
        .writer_lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let started = crate::discovery::current_epoch() as u64;
    let outcome = match class {
        None => run_full_rebuild(ctx),
        Some(c) => try_class_cycle(c, ctx),
    };
    match outcome {
        Ok(()) => Response {
            id: request.id.clone(),
            result: "ok",
            data: Some(serde_json::json!({
                "refreshed_epoch": started,
                "class": class.map(super::discovery::providers::ProviderClass::name),
            })),
            error: None,
        },
        Err(err) => Response {
            id: request.id.clone(),
            result: "error",
            data: None,
            error: Some(ErrorBody {
                code: "refresh_failed".to_string(),
                message: format!("{err:#}"),
            }),
        },
    }
}

/// Force a cold rebuild: empty prior so the freshness gate
/// trips for every class, run discovery, write `graph.bin`.
/// Counterpart to `--refresh` on the one-shot CLI.
fn run_full_rebuild(ctx: &DispatchCtx) -> Result<()> {
    let discovery_config =
        LocalDiscoveryConfig::from_env().with_caches(Arc::clone(&ctx.discovery_caches));
    let snapshot = discover_local_warm_with(
        ctx.scan_roots.to_vec(),
        discovery_config,
        GraphSnapshot::empty(),
        &ctx.intervals,
    )?;
    let snapshot = resolve_snapshot(snapshot);
    publish_snapshot(snapshot, &ctx.snapshot_bytes, &ctx.snapshot_state);
    Ok(())
}

/// Promote a freshly-resolved snapshot to the daemon's two
/// canonical caches per ADR 0082:
///
/// * The on-disk `graph.bin` artifact (atomic rename) so
///   daemonless one-shot CLIs and a future daemon warm-restart
///   can see it.
/// * The in-memory [`SnapshotBytes`] cache so the socket
///   `snapshot` command serves verbatim bytes without
///   re-serializing per connection.
/// * The in-memory [`SnapshotState`] cache so the next
///   per-class cycle's prior comes from RAM rather than disk.
///
/// **Best-effort on disk, durable in memory.** Serialize / write
/// failures log a single warning and continue so a transient
/// disk problem does not stall the daemon's in-memory state —
/// socket readers still see the latest snapshot, and the next
/// cycle's prior is still warm.
fn publish_snapshot(
    snapshot: GraphSnapshot,
    snapshot_bytes: &SnapshotBytes,
    snapshot_state: &SnapshotState,
) {
    publish_snapshot_to_path(
        snapshot,
        &snapshot::graph_bin_path(),
        snapshot_bytes,
        snapshot_state,
    );
}

fn publish_snapshot_to_path(
    snapshot: GraphSnapshot,
    path: &Path,
    snapshot_bytes: &SnapshotBytes,
    snapshot_state: &SnapshotState,
) {
    let bytes = match snapshot::serialize_to_bytes(&snapshot) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("conspectus serve: snapshot serialize failed (graph.bin skipped): {err:#}");
            store_snapshot_state(snapshot_state, snapshot);
            return;
        }
    };
    if let Err(err) = snapshot::write_atomic_bytes(path, &bytes) {
        eprintln!(
            "conspectus serve: snapshot write to {} failed (cache still updated): {err:#}",
            path.display()
        );
        // Fall through to the cache update — readers connected
        // over the socket should still see the new snapshot even
        // if the on-disk file write failed.
    }
    let arc = Arc::new(bytes);
    *snapshot_bytes
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(arc);
    store_snapshot_state(snapshot_state, snapshot);
}

fn store_snapshot_state(snapshot_state: &SnapshotState, snapshot: GraphSnapshot) {
    *snapshot_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(snapshot);
}

/// Best-effort daemon warm-restart: on startup, try
/// to load `graph.bin` so the first class cycle's prior is the
/// snapshot the previous daemon process left behind. The first
/// cycle then runs as a normal per-class refresh (evict its
/// slice, re-discover) instead of paying the full cold-rebuild
/// cost. Returns `None` for missing-file, version-mismatch,
/// validation-failure, or any other unhappy path — the daemon
/// falls through to first-cycle cold-rebuild semantics.
fn warm_start_from_disk() -> Option<GraphSnapshot> {
    let path = snapshot::graph_bin_path();
    let handle = match snapshot::open_mmap(&path) {
        Ok(handle) => handle,
        Err(snapshot::SnapshotError::Io(err)) if err.kind() == std::io::ErrorKind::NotFound => {
            // First run on this machine, or the operator wiped the
            // cache; not an error worth surfacing.
            return None;
        }
        Err(err) => {
            eprintln!("conspectus serve: warm-start skipped (graph.bin unreadable): {err:#}");
            return None;
        }
    };
    match snapshot::deserialize_owned(&handle) {
        Ok(snapshot) => {
            eprintln!(
                "conspectus serve: warm-start loaded prior snapshot from {}",
                path.display()
            );
            Some(snapshot)
        }
        Err(err) => {
            eprintln!("conspectus serve: warm-start skipped (deserialize failed): {err:#}");
            None
        }
    }
}

/// One-shot cleanup of legacy `graph.sqlite*` artifacts left
/// behind by daemons built before ADR 0082. The daemon no longer
/// reads or writes them — `graph.bin` is the canonical
/// persistence artifact. The cleanup is best-effort: filesystem
/// errors log and continue; the files are harmless if left
/// behind (they just consume disk).
fn cleanup_legacy_sqlite_artifacts() {
    let bin_path = snapshot::graph_bin_path();
    let Some(dir) = bin_path.parent() else {
        return;
    };
    for name in ["graph.sqlite", "graph.sqlite-wal", "graph.sqlite-shm"] {
        let candidate = dir.join(name);
        if candidate.exists() {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {
                    eprintln!(
                        "conspectus serve: removed legacy artifact {}",
                        candidate.display()
                    );
                }
                Err(err) => {
                    eprintln!(
                        "conspectus serve: failed to remove legacy artifact {}: {err:#}",
                        candidate.display()
                    );
                }
            }
        }
    }
    let backups = dir.join("backups");
    if backups.exists() {
        match std::fs::remove_dir_all(&backups) {
            Ok(()) => {
                eprintln!(
                    "conspectus serve: removed legacy backups dir {}",
                    backups.display()
                );
            }
            Err(err) => {
                eprintln!(
                    "conspectus serve: failed to remove legacy backups dir {}: {err:#}",
                    backups.display()
                );
            }
        }
    }
}

/// Socket listener loop. Non-blocking accept + 200ms poll cadence
/// so a shutdown signal is observed without the listener needing
/// special wakeup. Per-connection handling spawns a fresh worker
/// thread so a slow client cannot stall other in-flight
/// requests. The accepted connections themselves are blocking
/// — the per-connection handler does one read + one write then
/// closes, so blocking is fine and matches the framing's
/// one-request-per-connection contract.
fn socket_listener_loop(listener: UnixListener, ctx: DispatchCtx, shutdown: &AtomicBool) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if let Err(err) = stream.set_nonblocking(false) {
                    eprintln!("conspectus serve: failed to reset connection blocking: {err:#}");
                    continue;
                }
                let ctx = ctx.clone();
                thread::spawn(move || handle_connection(stream, ctx));
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(err) => {
                eprintln!("conspectus serve: accept failed: {err:#}");
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// Spawn the per-class scheduler threads and block on the
/// shutdown signal (SIGINT or SIGTERM, registered via
/// `signal-hook` per ADR 0080). Returns once every worker
/// thread has observed the shutdown flag, finished its in-flight
/// cycle, and exited.
pub fn run(config: ServeConfig) -> Result<()> {
    eprintln!(
        "conspectus serve: starting; per-class scheduler; scan roots = {:?}",
        config.scan_roots
    );

    let socket_path = socket_path();
    let listener = bind_socket(&socket_path)
        .with_context(|| format!("bind socket at {}", socket_path.display()))?;
    eprintln!("conspectus serve: listening on {}", socket_path.display());

    // Single process-local writer lock. Each class thread takes
    // it before its load-prior + evict + run + persist sequence
    // so two threads cannot race on the merge step. Briefly held
    // for the entire cycle; for v1 this trades throughput
    // (forge's minutes-long discovery blocks harness/mux) for
    // correctness. A future layer splits this into a writer
    // thread + a request channel so the load is back-pressured
    // rather than serialized.
    let writer_lock = Arc::new(Mutex::new(()));

    // Per-class scheduler state observed via the `status` socket
    // command. Threads briefly take the Mutex to write
    // their cycle outcome; the status handler briefly takes it
    // to snapshot.
    let state = Arc::new(Mutex::new(SchedulerState::default()));

    // Cached serialized snapshot bytes for the socket
    // `snapshot` command. The daemon updates this after every
    // successful cycle via [`publish_snapshot`]; `None` before
    // the first cycle.
    let snapshot_bytes: SnapshotBytes = Arc::new(Mutex::new(None));

    // Live in-memory snapshot the per-class scheduler reads as
    // its prior. Optionally seeded from `graph.bin`
    // on startup so a warm-restart skips the cold-rebuild cost.
    let snapshot_state: SnapshotState = Arc::new(Mutex::new(warm_start_from_disk()));

    // Best-effort cleanup of legacy `graph.sqlite*` artifacts.
    // The daemon has no consumer for them anymore.
    // Failures (read-only data dir, missing parent, etc.) log
    // and continue; the files are harmless if left behind.
    cleanup_legacy_sqlite_artifacts();

    // Shutdown latch per ADR 0080. signal-hook flips this atomic
    // on SIGINT/SIGTERM; every scheduler thread polls it between
    // sleeps so a shutdown that arrives mid-tick still completes
    // the cycle in progress before exiting.
    let shutdown = Arc::new(AtomicBool::new(false));
    register_shutdown_signals(&shutdown).context("install signal handlers for SIGINT/SIGTERM")?;

    let ctx = DispatchCtx {
        scan_roots: Arc::new(config.scan_roots),
        intervals: Arc::new(config.intervals),
        writer_lock,
        state,
        snapshot_bytes,
        snapshot_state,
        snapshot_path: Arc::new(snapshot::graph_bin_path()),
        discovery_caches: Arc::default(),
    };

    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for class in ProviderClass::all() {
        let ctx = ctx.clone();
        let shutdown = Arc::clone(&shutdown);
        let class = *class;
        let watcher = build_watcher_for(class);
        handles.push(thread::spawn(move || {
            class_loop(class, &ctx, watcher, &shutdown);
        }));
    }

    let listener_shutdown = Arc::clone(&shutdown);
    handles.push(thread::spawn(move || {
        socket_listener_loop(listener, ctx, &listener_shutdown);
    }));

    // Join every thread. With the shutdown latch in place, each
    // loop exits cleanly when SIGINT/SIGTERM is observed; we
    // wait for all of them so a graceful shutdown surfaces a
    // single "stopped" line on stderr at the end.
    for handle in handles {
        if let Err(panic) = handle.join() {
            eprintln!("conspectus serve: scheduler thread panicked: {panic:?}");
        }
    }

    // Unlink the socket file. A stale file would otherwise make
    // the next `conspectus serve` invocation hit EADDRINUSE on
    // bind (bind_socket also handles this on the next startup,
    // but cleaning up here keeps the runtime dir tidy and
    // matches the "socket file is useless without the owning
    // process" invariant).
    let _ = std::fs::remove_file(&socket_path);
    eprintln!("conspectus serve: stopped");
    Ok(())
}

/// Build the watcher appropriate for `class` (ADR 0081).
///
/// `Harness` watches every configured harness state directory
/// from `LocalDiscoveryConfig::from_env`. If `notify` installation
/// fails (rlimit, unsupported filesystem, etc.) the fallback is
/// a [`NullWatcher`] so the class still ticks on its interval.
/// `Mux` / `Git` / `Forge` use [`NullWatcher`] until their own
/// watcher targets land (git refs, etc.).
fn build_watcher_for(class: ProviderClass) -> Box<dyn Watcher> {
    if matches!(class, ProviderClass::Harness) {
        let discovery_config = LocalDiscoveryConfig::from_env();
        let paths: Vec<PathBuf> = discovery_config
            .harness_state_roots
            .values()
            .cloned()
            .collect();
        match NotifyWatcher::new(paths.iter().map(std::path::PathBuf::as_path)) {
            Ok(watcher) => {
                let path_summary: Vec<String> =
                    paths.iter().map(|p| p.display().to_string()).collect();
                eprintln!(
                    "conspectus serve: harness watcher installed on {} path(s): {}",
                    paths.len(),
                    path_summary.join(", ")
                );
                Box::new(watcher)
            }
            Err(err) => {
                eprintln!(
                    "conspectus serve: harness watcher install failed, falling back to interval polling: {err:#}"
                );
                Box::new(NullWatcher)
            }
        }
    } else {
        Box::new(NullWatcher)
    }
}

/// Register SIGINT and SIGTERM against the shared shutdown
/// flag (ADR 0080). Other signals follow the same pattern when
/// they land (SIGHUP for config reload, SIGUSR1 for status
/// dumps).
fn register_shutdown_signals(shutdown: &Arc<AtomicBool>) -> Result<()> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    use signal_hook::flag;
    flag::register(SIGINT, Arc::clone(shutdown)).context("register SIGINT handler")?;
    flag::register(SIGTERM, Arc::clone(shutdown)).context("register SIGTERM handler")?;
    Ok(())
}

/// Per-class scheduler loop. Runs one cycle immediately on
/// startup (so the cache is populated before the first sleep)
/// then ticks at the class's interval. Errors per cycle log to
/// stderr and the loop continues so a transient blip does not
/// silently retire the class's refresh duty. The shutdown
/// latch is polled between cycles and at every chunk of the
/// inter-tick sleep so a Ctrl-C does not have to wait up to a
/// full forge interval (5 minutes) to be observed.
///
/// A per-class minimum-cycle-gap floor throttles watcher-driven
/// re-runs so a noisy source (e.g. Codex's SQLite `-wal`/`-shm`
/// files churning under `~/.codex/`) can't push the loop into
/// `wait → run_cycle → wait → run_cycle` at kilohertz cadence.
/// The floor is [`min_cycle_gap`] of the class interval; the
/// first `Changed` after a quiet stretch still fires immediately.
fn class_loop(
    class: ProviderClass,
    ctx: &DispatchCtx,
    mut watcher: Box<dyn Watcher>,
    shutdown: &AtomicBool,
) {
    let interval = class.ttl_duration(&ctx.intervals);
    let min_gap = min_cycle_gap(interval);
    eprintln!(
        "conspectus serve: {} scheduler started; interval = {:?}, min cycle gap = {:?}",
        class.name(),
        interval,
        min_gap,
    );
    run_cycle(class, ctx);
    let mut last_cycle_end = Instant::now();
    while !shutdown.load(Ordering::Relaxed) {
        if throttle_since(last_cycle_end, min_gap, shutdown) {
            break;
        }
        match wait_for_class_signal(watcher.as_mut(), interval, shutdown) {
            WatcherEvent::ShuttingDown => break,
            // Both Changed and Timeout trigger a cycle. The
            // distinction matters for observability (a watcher
            // wake fired before the interval) but the work is
            // identical: re-run this class's slice.
            WatcherEvent::Changed | WatcherEvent::Timeout => {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                run_cycle(class, ctx);
                last_cycle_end = Instant::now();
            }
        }
    }
    eprintln!(
        "conspectus serve: {} scheduler stopping after shutdown signal",
        class.name()
    );
}

/// Minimum time to wait between the end of one `run_cycle` and
/// entering the next `wait_for_class_signal`. Prevents a
/// constantly-firing watcher (Codex's SQLite `-wal`/`-shm`,
/// hook drops, anything an active AI agent writes into a watched
/// dir) from turning the class loop into a tight
/// `wait → run_cycle → wait → run_cycle` spin. The clamp keeps
/// short-interval classes (harness = 5s) responsive while long-
/// interval classes (forge = 5m) don't inherit a giant floor.
pub(crate) fn min_cycle_gap(interval: Duration) -> Duration {
    const FLOOR: Duration = Duration::from_millis(250);
    const CEILING: Duration = Duration::from_secs(2);
    let quartered = interval / 4;
    quartered.clamp(FLOOR, CEILING)
}

/// Sleep until at least `min_gap` has elapsed since
/// `last_cycle_end`, polling `shutdown` on the same 200ms
/// granularity as [`wait_for_class_signal`] so Ctrl-C latency
/// stays predictable regardless of the class's throttle floor.
/// Returns `true` when the shutdown latch flipped mid-throttle
/// so the caller can break out immediately.
fn throttle_since(last_cycle_end: Instant, min_gap: Duration, shutdown: &AtomicBool) -> bool {
    let elapsed = last_cycle_end.elapsed();
    if elapsed >= min_gap {
        return false;
    }
    let poll = Duration::from_millis(200);
    let deadline = last_cycle_end + min_gap;
    while Instant::now() < deadline {
        if shutdown.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(poll));
    }
    false
}

/// Wait up to `interval` for the class's watcher to fire,
/// returning early on `Changed` so the cycle runs as soon as
/// the OS reports a filesystem change. Polls the shutdown flag
/// every 200ms so SIGINT/SIGTERM is observed within that
/// window regardless of how long the class interval is (a
/// 5-minute forge tick would otherwise hold the daemon
/// hostage to shutdown for the full window).
fn wait_for_class_signal(
    watcher: &mut dyn Watcher,
    interval: Duration,
    shutdown: &AtomicBool,
) -> WatcherEvent {
    let poll = Duration::from_millis(200);
    let deadline = Instant::now() + interval;
    while Instant::now() < deadline {
        if shutdown.load(Ordering::Relaxed) {
            return WatcherEvent::ShuttingDown;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let chunk = remaining.min(poll);
        match watcher.wait(chunk) {
            WatcherEvent::Changed => return WatcherEvent::Changed,
            WatcherEvent::ShuttingDown => return WatcherEvent::ShuttingDown,
            WatcherEvent::Timeout => continue,
        }
    }
    WatcherEvent::Timeout
}

/// Acquire the writer lock and run one class cycle. Errors are
/// logged and swallowed so the calling loop keeps going. Writes
/// the cycle outcome (started_epoch, completed_epoch, success
/// / error + message) into the shared [`SchedulerState`] for
/// `conspectus status` to observe.
fn run_cycle(class: ProviderClass, ctx: &DispatchCtx) {
    let state = &ctx.state;
    let started = crate::discovery::current_epoch();
    record_state_started(state, class, started);
    // Poisoned-mutex recovery: a panic in a peer class while it
    // held the lock taints it, but the in-memory snapshot cache
    // is durable across the lock take-over and a stale entry
    // self-heals on the next successful cycle. Carry on rather
    // than aborting the daemon.
    let _guard = ctx
        .writer_lock
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let outcome = try_class_cycle(class, ctx);
    let completed = crate::discovery::current_epoch();
    match outcome {
        Ok(()) => record_state_completed(state, class, completed, Ok(())),
        Err(err) => {
            let msg = format!("{err:#}");
            eprintln!(
                "conspectus serve: {} cycle failed, retrying next tick: {msg}",
                class.name()
            );
            record_state_completed(state, class, completed, Err(msg));
        }
    }
}

/// Write a `started_epoch` to the shared state. Poisoned-mutex
/// recovery follows the same pattern as the writer lock; the
/// in-memory observability is best-effort.
fn record_state_started(state: &Mutex<SchedulerState>, class: ProviderClass, epoch: i64) {
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    guard.record_started(class, epoch);
}

fn record_state_completed(
    state: &Mutex<SchedulerState>,
    class: ProviderClass,
    epoch: i64,
    outcome: Result<(), String>,
) {
    let mut guard = state.lock().unwrap_or_else(PoisonError::into_inner);
    guard.record_completed(class, epoch, outcome);
}

/// Load the prior cache, evict this class's slice so the
/// freshness gate marks it as untested, run discovery (which
/// then runs only this class's providers since every other class
/// is still fresh), merge + resolve + persist. The shared writer
/// lock around `run_cycle` ensures no peer thread reads-old +
/// writes between our load and write.
/// Read the prior cache from the in-memory [`SnapshotState`],
/// evict this class's slice so the freshness gate marks it as
/// untested, run discovery (which then runs only this class's
/// providers since every other class is still fresh), merge +
/// resolve, and publish the result to both caches plus
/// `graph.bin`. The shared writer lock around `run_cycle`
/// ensures no peer thread reads-old + writes between our load
/// and write.
///
/// The prior used to come from
/// `query::load_cached_snapshot(None)` (a read of the on-disk
/// `graph.sqlite`). It now comes from the in-memory
/// [`SnapshotState`] the previous cycle populated. A daemon
/// restart with no warm-start path arrives at the first cycle
/// with `None` — the cycle runs as a cold rebuild (since every
/// provider's "prior slice" is empty) and seeds the cache.
fn try_class_cycle(class: ProviderClass, ctx: &DispatchCtx) -> Result<()> {
    let mut prior = load_snapshot_state(&ctx.snapshot_state).unwrap_or_else(GraphSnapshot::empty);
    for provider in class.providers() {
        prior.evict_provider(provider);
    }
    let discovery_config =
        LocalDiscoveryConfig::from_env().with_caches(Arc::clone(&ctx.discovery_caches));
    let snapshot = discover_local_warm_with(
        ctx.scan_roots.to_vec(),
        discovery_config,
        prior,
        &ctx.intervals,
    )?;
    let snapshot = resolve_snapshot(snapshot);
    publish_snapshot(snapshot, &ctx.snapshot_bytes, &ctx.snapshot_state);
    Ok(())
}

/// Clone the live snapshot out of the in-memory cache so the
/// caller can mutate it (typically by evicting a class's slice).
/// Returns `None` before the first successful cycle or when a
/// daemon warm-start path has not yet seeded the cache from
/// `graph.bin`.
fn load_snapshot_state(snapshot_state: &SnapshotState) -> Option<GraphSnapshot> {
    let guard = snapshot_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    guard.clone()
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
