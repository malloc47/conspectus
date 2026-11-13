//! `conspectus hook write`: record one harness hook payload.
//!
//! A harness runs this once per hook event, with the event's JSON on
//! stdin, and waits for it to finish. The payload is parsed against the
//! fields the harness's adapter reads, and every other value is skipped
//! without being built ([`crate::hook::payload`]). The adapter says what
//! those fields mean, and they go to up to two places:
//!
//! - **The live-state store** (ADR 0204), for every event the adapter
//!   maps to a live event or snapshot. The write waits at most
//!   [`crate::live_store::LOCK_WAIT`] for the store's lock, opens no
//!   socket, and starts no process once the store has an entry for the
//!   session: the pane comes from `TMUX_PANE` and the server from
//!   `$TMUX`, and tmux is asked about that pane, for its session's name,
//!   only the first time.
//! - **The attribution record** (ADR 0028, ADR 0146), only for the
//!   events the adapter says name the session a process runs, such as a
//!   session start. It goes to the daemon's `hook_ingest` command,
//!   bounded by [`crate::server::HOOK_INGEST_TIMEOUT`], or to the hook
//!   spool when no daemon takes it. The live state is written first.
//!
//! The command never disturbs the harness (ADR 0204 decision 6, ADR 0206
//! decision 13): whatever happens once it runs, an unreadable payload,
//! an unknown harness, a store that is corrupt, locked, or unwritable,
//! a missing tmux, a stuck daemon, even a panic, it prints nothing on
//! stdout or stderr and exits 0. Claude Code feeds some hook stdout to
//! the model and treats exit 2 as a block. A malformed command line is
//! the one exception: clap rejects it before the command runs, with a
//! usage message and exit status 2, and `hook init` never writes one.

use std::fs;
use std::io::{self, Read};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

use crate::discovery::harness::HarnessAdapter;
use crate::hook::{HookRecord, HookStore, HookTmuxRecord};
use crate::live_store::{LiveContext, LiveStore, LiveTmux};

#[derive(Debug, Args)]
pub(super) struct HookWriteArgs {
    /// Registered harness key, as in
    /// `conspectus hook write claude-code`.
    harness: String,
    /// Write the hook record and the live state under PATH, without
    /// asking a daemon. Primarily useful for tests and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl HookWriteArgs {
    /// Record the payload on stdin. Never fails and never prints: see
    /// the module documentation.
    pub(super) fn run(self) {
        // A panic would print to stderr and exit with status 101; the
        // harness must see neither.
        panic::set_hook(Box::new(|_| {}));
        let _ = panic::catch_unwind(AssertUnwindSafe(|| self.write()));
    }

    fn write(&self) {
        let observed_ms = crate::live_store::now_ms();
        // Read the whole payload before anything else, so the harness
        // never meets a closed pipe.
        let mut input = Vec::new();
        if io::stdin().read_to_end(&mut input).is_err() {
            return;
        }
        let Some(adapter) = crate::discovery::harness::registered_adapters()
            .find(|adapter| adapter.harness_key() == self.harness)
        else {
            return;
        };
        // Only the fields the adapter reads are built; the rest of the
        // payload, a large tool response included, is skipped unread.
        let Some(payload) = crate::hook::payload::parse(&input, adapter.hook_payload_fields())
        else {
            return;
        };
        let env = WriterEnv::from_env(&self.harness, observed_ms);
        let store = match &self.state_root {
            Some(root) => Some(LiveStore::at(root)),
            None => LiveStore::from_env(),
        };
        let mut effects = SystemEffects {
            state_root: self.state_root.clone(),
        };
        write_payload(adapter, &payload, &env, store.as_ref(), &mut effects);
    }
}

/// What the writer learns from its own process and environment, without
/// starting a process.
#[derive(Clone, Debug, Default)]
pub(super) struct WriterEnv {
    /// When the writer started, Unix epoch milliseconds.
    pub(super) observed_ms: i64,
    /// The harness process that ran the hook, when it can be found.
    pub(super) process: Option<HarnessProcess>,
    /// The tmux pane the writer runs in; `None` outside tmux.
    pub(super) tmux: Option<TmuxEnv>,
    /// The version of the Conspectus plugin that ran the hook, for a
    /// harness whose plugin exports one.
    pub(super) harness_version: Option<String>,
}

impl WriterEnv {
    fn from_env(harness: &str, observed_ms: i64) -> Self {
        Self {
            observed_ms,
            process: harness_process(harness),
            tmux: tmux_env(
                std::env::var_os("TMUX").map(|value| value.to_string_lossy().into_owned()),
                std::env::var("TMUX_PANE").ok(),
            ),
            harness_version: harness_version_env(harness),
        }
    }
}

/// The tmux pane a process runs in, as tmux exports it to the pane.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct TmuxEnv {
    /// The server's socket path: `$TMUX` up to its first comma.
    pub(super) socket: Option<String>,
    /// `TMUX_PANE`, the pane id (`%3`).
    pub(super) pane: Option<String>,
}

/// The pane from the values of `$TMUX` and `TMUX_PANE`. Without `$TMUX`
/// the process is not in tmux, whatever `TMUX_PANE` says.
pub(super) fn tmux_env(tmux: Option<String>, pane: Option<String>) -> Option<TmuxEnv> {
    let tmux = tmux?;
    Some(TmuxEnv {
        socket: tmux
            .split_once(',')
            .map(|(socket, _)| socket.to_string())
            .filter(|socket| !socket.is_empty()),
        pane: pane.filter(|pane| !pane.is_empty()),
    })
}

/// What the writer does beyond the live-state store. Tests stand in for
/// it to see what a payload triggers.
pub(super) trait WriterEffects {
    /// The mux context of the writer's own pane (`TMUX_PANE`'s, when it
    /// names one), from the mux itself. Starts tmux.
    fn mux_context(&mut self) -> Option<HookTmuxRecord>;
    /// Record `record` for attribution: through the daemon, or in the
    /// spool.
    fn attribute(&mut self, record: &HookRecord);
}

struct SystemEffects {
    state_root: Option<PathBuf>,
}

impl WriterEffects for SystemEffects {
    fn mux_context(&mut self) -> Option<HookTmuxRecord> {
        tmux_context()
    }

    fn attribute(&mut self, record: &HookRecord) {
        let _ = write_or_ingest_hook_record(record, self.state_root.clone());
    }
}

/// Record one parsed payload: the live state when the adapter maps it,
/// then the attribution record when the adapter says the event names
/// the session.
pub(super) fn write_payload(
    adapter: &dyn HarnessAdapter,
    payload: &serde_json::Value,
    env: &WriterEnv,
    store: Option<&LiveStore>,
    effects: &mut dyn WriterEffects,
) {
    let background = env.process.is_some_and(|process| process.background);
    // A background harness's environment came from whichever client
    // started it, so its tmux context names someone else's pane.
    let tmux = env.tmux.as_ref().filter(|_| !background);
    let attribution = adapter.is_attribution_event(payload);
    // The attribution record carries the mux context; one answer serves
    // the live-state entry too.
    let mux = if attribution && !background {
        effects.mux_context()
    } else {
        None
    };
    if let Some(store) = store {
        write_live_state(adapter, payload, env, tmux, mux.as_ref(), store, effects);
    }
    if attribution {
        attribute(adapter, payload, env, mux, effects);
    }
}

/// Apply what `payload` says to the live-state store. A busy lock, an
/// input the store refuses, or a failed write drops it; the session's
/// next event repairs the entry.
fn write_live_state(
    adapter: &dyn HarnessAdapter,
    payload: &serde_json::Value,
    env: &WriterEnv,
    tmux: Option<&TmuxEnv>,
    mux: Option<&HookTmuxRecord>,
    store: &LiveStore,
    effects: &mut dyn WriterEffects,
) {
    let context = LiveContext {
        pid: env.process.map(|process| process.pid),
        pid_start: env.process.and_then(|process| process.start),
        tmux: tmux.map(|tmux| LiveTmux {
            socket: tmux.socket.clone(),
            session: None,
            pane: tmux.pane.clone(),
        }),
        ..LiveContext::default()
    };
    let mut events = adapter.live_events_from_payload(payload, &context, env.observed_ms);
    let mut snapshot = adapter.live_snapshot_from_payload(payload, &context, env.observed_ms);
    let session = events
        .first()
        .map(|event| (&event.harness, &event.session_id))
        .or_else(|| {
            snapshot
                .as_ref()
                .map(|snapshot| (&snapshot.harness, &snapshot.session_id))
        });
    let Some((harness, session_id)) = session else {
        return;
    };
    if tmux.is_some() {
        // Only tmux knows the session's name. It is asked the first
        // time the store sees the session; later events keep the name
        // the entry holds.
        let learned = match mux {
            Some(mux) => Some(mux.clone()),
            None if store.read().get(harness, session_id).is_none() => effects.mux_context(),
            None => None,
        };
        if let Some(learned) = learned {
            let contexts = events
                .iter_mut()
                .map(|event| &mut event.context)
                .chain(snapshot.iter_mut().map(|snapshot| &mut snapshot.context));
            for context in contexts {
                fill_tmux(context, &learned);
            }
        }
    }
    if !events.is_empty() {
        let _ = store.apply_events(adapter.live_reducer(), &events);
    }
    if let Some(snapshot) = &snapshot {
        let _ = store.apply_snapshot(snapshot);
    }
}

/// Fill what the environment did not say about the pane from what tmux
/// answered.
fn fill_tmux(context: &mut LiveContext, learned: &HookTmuxRecord) {
    if let Some(tmux) = &mut context.tmux {
        tmux.socket = tmux.socket.take().or_else(|| learned.socket_path.clone());
        tmux.session = tmux.session.take().or_else(|| learned.session_name.clone());
        tmux.pane = tmux.pane.take().or_else(|| learned.pane_id.clone());
    }
}

/// Build the attribution record from `payload` and hand it on. A payload
/// the adapter cannot build one from is dropped.
fn attribute(
    adapter: &dyn HarnessAdapter,
    payload: &serde_json::Value,
    env: &WriterEnv,
    mux: Option<HookTmuxRecord>,
    effects: &mut dyn WriterEffects,
) {
    let process = env.process;
    let Ok(mut record) = adapter.hook_record_from_payload(
        payload,
        process.map(|process| process.pid),
        process.and_then(|process| process.ppid),
        mux,
        env.harness_version.clone(),
        env.observed_ms.div_euclid(1000),
    ) else {
        return;
    };
    record.background_harness = process.is_some_and(|process| process.background);
    effects.attribute(&record);
}

fn resolve_hook_state_root(override_root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = override_root {
        return Ok(root);
    }
    HookStore::from_env()
        .map(|store| store.root().to_path_buf())
        .ok_or_else(|| {
            anyhow!("no hook state root available; set HOME or CONSPECTUS_HOOK_SIDECAR_STATE")
        })
}

/// Send `record` to the daemon, which writes the spool itself when it
/// takes the record (ADR 0146). Any other answer, including none within
/// the ingest bound, leaves the spool to this process.
fn write_or_ingest_hook_record(record: &HookRecord, override_root: Option<PathBuf>) -> Result<()> {
    if let Some(root) = override_root {
        HookStore::new(root).write_record(record)?;
        return Ok(());
    }
    if let crate::server::ClientOutcome::Ok(()) = crate::server::client_hook_ingest(record) {
        return Ok(());
    }
    let root = resolve_hook_state_root(None)?;
    HookStore::new(root).write_record(record)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_linux_parent_pid() -> Option<u32> {
    read_linux_ppid_of(std::process::id())
}

#[cfg(target_os = "linux")]
fn read_linux_ppid_of(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    let mut fields = after_name.split_whitespace();
    fields.next()?;
    fields.next()?.parse().ok()
}

#[cfg(target_os = "linux")]
fn read_linux_cmdline_of(pid: u32) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<_> = raw
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .map(String::from_utf8_lossy)
        .collect();
    (!args.is_empty()).then(|| args.join(" "))
}

#[cfg(target_os = "linux")]
fn read_linux_comm_of(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Resolve the (pid, ppid) of the harness process (e.g. `claude`,
/// `codex`, `opencode`) that triggered this hook invocation.
///
/// Walks the parent-pid chain from the current process upward, looking
/// for the first ancestor whose `/proc/<pid>/comm` matches one of
/// `expected_binaries`. This is necessary because claude / codex /
/// opencode launch the hook command via a short-lived shell wrapper
/// (e.g. `sh -c 'conspectus hook write …'`), so `std::process::id()`
/// returns the writer's pid — a process that exits within
/// milliseconds. The hook-sidecar discovery layer's liveness check
/// (`src/discovery/hook_sidecar.rs`) treats records with a dead pid
/// as ignored evidence, which would silently disable hook evidence
/// entirely.
///
/// Returns `None` when running on a non-Linux host, when the walk
/// exhausts its depth budget, or when no ancestor matches. Callers
/// must treat `None` as "harness pid unknown" and persist it as a
/// `None` pid in the hook record so the discovery liveness check
/// is skipped instead of failing.
fn resolve_harness_pid(expected_binaries: &[&str]) -> Option<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        let start = read_linux_parent_pid()?;
        resolve_harness_pid_with(start, expected_binaries, |pid| {
            Some((read_linux_comm_of(pid)?, read_linux_ppid_of(pid)?))
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = expected_binaries;
        None
    }
}

/// Pure walker used by `resolve_harness_pid`. Factored out so unit
/// tests can mock the `/proc` reader. Caps the walk at 8 hops to
/// prevent runaway recursion on a corrupted process table.
pub(super) fn resolve_harness_pid_with<F>(
    start_pid: u32,
    expected_binaries: &[&str],
    mut read: F,
) -> Option<(u32, u32)>
where
    F: FnMut(u32) -> Option<(String, u32)>,
{
    let mut pid = start_pid;
    for _ in 0..8 {
        if pid <= 1 {
            return None;
        }
        let (comm, ppid) = read(pid)?;
        if expected_binaries.iter().any(|name| comm == *name) {
            return Some((pid, ppid));
        }
        pid = ppid;
    }
    None
}

/// Process-name set Conspectus expects to see for each harness when
/// walking the parent-pid chain from a hook writer up to the live
/// agent process. Mirrors the harness keys recognized elsewhere in
/// the cross-link and process-tree code.
///
/// Reads from the adapter registry's
/// `RuntimeSignature::process_command_basenames` so a new
/// harness gets pid-pair resolution for free — no cli.rs
/// match-table edit required.
pub(super) fn harness_binaries(harness: &str) -> Vec<&'static str> {
    crate::discovery::harness::registered_adapters()
        .find(|a| a.harness_key() == harness)
        .map(|a| a.runtime_signature().process_command_basenames.to_vec())
        .unwrap_or_default()
}

/// The version string of `harness`'s hook record, from the variable
/// `var` reads: the version of the Conspectus plugin that ran the hook,
/// for a harness whose plugin exports one (opencode, Pi). Claude Code
/// documents no version for its hooks, in their input or their
/// environment, so its record carries none rather than one that is
/// never there.
pub(super) fn harness_version_from(
    harness: &str,
    var: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let name = match harness {
        "opencode" => "CONSPECTUS_OPENCODE_HOOK_VERSION",
        "pi" => "CONSPECTUS_PI_HOOK_VERSION",
        _ => return None,
    };
    var(name)
}

fn harness_version_env(harness: &str) -> Option<String> {
    harness_version_from(harness, |name| std::env::var(name).ok())
}

/// The variable a harness sets to its own pid in the environment of the
/// hooks it runs. Claude Code sets `CLAUDE_PID` from version 2.1.214.
fn harness_pid_env(harness: &str) -> Option<&'static str> {
    match harness {
        "claude-code" => Some("CLAUDE_PID"),
        _ => None,
    }
}

/// A pid as a harness exports it: a positive number above init's.
pub(super) fn parse_exported_pid(value: &str) -> Option<u32> {
    value.trim().parse().ok().filter(|pid| *pid > 1)
}

/// The harness process its pid variable names: (pid, ppid). Where
/// `/proc` exists the pid must name a running process, so a value that
/// outlived its process leaves the decision to the walk.
fn harness_pid_from_env(harness: &str) -> Option<(u32, Option<u32>)> {
    let pid = parse_exported_pid(&std::env::var(harness_pid_env(harness)?).ok()?)?;
    #[cfg(target_os = "linux")]
    {
        let ppid = read_linux_ppid_of(pid)?;
        Some((pid, Some(ppid)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some((pid, None))
    }
}

/// The harness process that triggered a hook invocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HarnessProcess {
    pub(super) pid: i64,
    pub(super) ppid: Option<i64>,
    /// When the process started (field 22 of `/proc/<pid>/stat`), so a
    /// pid the kernel hands to a new process is told apart.
    pub(super) start: Option<u64>,
    /// The harness's runtime signature classifies this process's
    /// command line as a background process, such as opencode 2.x's
    /// shared `opencode serve --service`.
    pub(super) background: bool,
}

/// Find the harness process behind this hook invocation: the pid the
/// harness exported, else the nearest ancestor running the harness.
/// `None` when it cannot be identified; the record then carries no pid,
/// so the discovery liveness check skips the pid branch entirely and the
/// record stays Active rather than being marked Ignored against a
/// stillborn writer pid.
fn harness_process(harness: &str) -> Option<HarnessProcess> {
    let (pid, ppid) = harness_pid_from_env(harness).or_else(|| {
        resolve_harness_pid(&harness_binaries(harness)).map(|(pid, ppid)| (pid, Some(ppid)))
    })?;
    Some(HarnessProcess {
        pid: i64::from(pid),
        ppid: ppid.map(i64::from),
        start: crate::live_store::process_start_time(i64::from(pid)),
        background: harness_command_line(pid)
            .is_some_and(|command| is_background_harness_command(harness, &command)),
    })
}

fn harness_command_line(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        read_linux_cmdline_of(pid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

/// Whether `harness`'s registered runtime signature classifies
/// `command` as a background process.
pub(super) fn is_background_harness_command(harness: &str, command: &str) -> bool {
    crate::discovery::harness::registered_adapters()
        .find(|a| a.harness_key() == harness)
        .is_some_and(|a| (a.runtime_signature().is_background_process)(command))
}

fn tmux_context() -> Option<HookTmuxRecord> {
    // Iterate the registered mux backends and ask
    // each for its current-session context. The first backend
    // to answer wins. `SystemTmux` reads `$TMUX` and runs `tmux
    // display-message` about `TMUX_PANE`'s pane; other backends
    // supply their own env-var contract via the same trait method.
    let backends: Vec<Box<dyn crate::discovery::tmux::MuxBackend>> = vec![
        Box::new(crate::discovery::tmux::SystemTmux::new()),
        Box::new(crate::discovery::zellij::SystemZellij::new()),
    ];
    for backend in backends {
        if let Some(ctx) = backend.current_session_context() {
            let record = HookTmuxRecord {
                session_name: ctx.session_name,
                native_id: None,
                pane_id: ctx.pane_id,
                socket_path: ctx.namespace,
            };
            if !record.is_empty() {
                return Some(record);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "hook_write_tests.rs"]
mod tests;
