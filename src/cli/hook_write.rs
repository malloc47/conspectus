//! `conspectus hook write`: record one harness hook payload.
//!
//! A harness runs this once per hook event with the event's JSON on
//! stdin. The record goes to the daemon's `hook_ingest` command, or to
//! the hook spool when no daemon answers (ADR 0028, ADR 0146).

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;

use crate::hook::{HookStore, HookTmuxRecord};

#[derive(Debug, Args)]
pub(super) struct HookWriteArgs {
    /// Registered harness key, as in
    /// `conspectus hook write claude-code`.
    harness: String,
    /// Override hook state root. Primarily useful for tests
    /// and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl HookWriteArgs {
    pub(super) fn run(self) -> Result<()> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            bail!("{} hook payload was empty", self.harness);
        }
        let payload: serde_json::Value = serde_json::from_str(&input)
            .with_context(|| format!("failed to parse {} hook JSON", self.harness))?;
        let harness = harness_process(&self.harness);
        let background = harness.as_ref().is_some_and(|process| process.background);
        // A background harness's environment came from whichever client
        // started it, so its tmux context names someone else's pane.
        let tmux = if background { None } else { tmux_context() };
        let harness_version = harness_version_env(&self.harness);
        let mut record = crate::hook::hook_record_from_payload(
            &self.harness,
            &payload,
            harness.as_ref().map(|process| process.pid),
            harness.as_ref().map(|process| process.ppid),
            tmux,
            harness_version,
            crate::discovery::current_epoch(),
        )?;
        record.background_harness = background;
        write_or_ingest_hook_record(&record, self.state_root)?;
        Ok(())
    }
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

fn write_or_ingest_hook_record(
    record: &crate::hook::HookRecord,
    override_root: Option<PathBuf>,
) -> Result<()> {
    if let Some(root) = override_root {
        HookStore::new(root).write_record(record)?;
        return Ok(());
    }

    match crate::server::client_hook_ingest(record) {
        crate::server::ClientOutcome::Ok(()) => Ok(()),
        crate::server::ClientOutcome::NoDaemon => {
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        crate::server::ClientOutcome::DaemonError { code, message } => {
            if code != "snapshot_unavailable" {
                eprintln!("conspectus: warning: daemon refused hook ingest ({code}): {message}");
            }
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        crate::server::ClientOutcome::Transport(err) => {
            eprintln!(
                "conspectus: warning: daemon hook ingest failed, writing local hook spool: {err:#}"
            );
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
    }
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

/// Environment variable Conspectus consults for a harness's
/// version string when writing a hook sidecar record. Read
/// through a helper (rather than inline in `HookWriteArgs::run`)
/// so the per-harness mapping stays in one place.
fn harness_version_env(harness: &str) -> Option<String> {
    match harness {
        "claude-code" => std::env::var("CLAUDE_CODE_VERSION").ok(),
        "opencode" => std::env::var("CONSPECTUS_OPENCODE_HOOK_VERSION").ok(),
        "pi" => std::env::var("CONSPECTUS_PI_HOOK_VERSION").ok(),
        _ => None,
    }
}

/// The harness process that triggered a hook invocation.
struct HarnessProcess {
    pid: i64,
    ppid: i64,
    /// The harness's runtime signature classifies this process's
    /// command line as a background process, such as opencode 2.x's
    /// shared `opencode serve --service`.
    background: bool,
}

/// Find the harness process behind this hook invocation. `None` when it
/// cannot be identified; the record then carries no pid, so the
/// discovery liveness check skips the pid branch entirely and the
/// record stays Active rather than being marked Ignored against a
/// stillborn writer pid.
fn harness_process(harness: &str) -> Option<HarnessProcess> {
    let (pid, ppid) = resolve_harness_pid(&harness_binaries(harness))?;
    Some(HarnessProcess {
        pid: i64::from(pid),
        ppid: i64::from(ppid),
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
    // display-message`; other backends supply their own env-var
    // contract via the same trait method.
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
