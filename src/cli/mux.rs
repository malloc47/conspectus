//! `conspectus mux` subcommand tree (ADR 0095, ADR 0096).
//!
//! Two verbs:
//! - `mux new` — bare tmux session, no agent, no worktree.
//! - `mux launch` — harness in a fresh tmux.
//!
//! Both are sanctioned under ADR 0143 category 3 (operator-initiated
//! mux lifecycle) plus category 4 (Conspectus-constructed harness
//! argv for `mux launch`). They record the new mux in history, and
//! `--pin` also adds a pin slot for it (ADR 0141): with `--no-launch`
//! the slot is planned and nothing starts until `conspectus rebuild`.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use crate::discovery::harness::launch_argv_for;
use crate::discovery::tmux::{MuxBackend, SystemTmux};

use crate::rebuild::launch::{attach_and_report, format_attach_command, report_new_session};

#[derive(Debug, Args)]
pub struct MuxArgs {
    #[command(subcommand)]
    command: MuxCommand,
}

impl MuxArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            MuxCommand::New(args) => args.run(),
            MuxCommand::Launch(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum MuxCommand {
    /// Create a bare tmux session (no agent, no worktree). The next
    /// discovery cycle picks the mux up; `--pin` also adds a pin slot
    /// for it, and `conspectus pin add <name>` does so afterwards.
    New(MuxNewArgs),
    /// Launch a harness in a fresh tmux session. Distinct from `mux
    /// new`, which spawns a bare shell. The mux and its attributed
    /// harness session appear in discovery on the next refresh;
    /// `--pin` also adds a pin slot, and `--pin --no-launch` only
    /// plans one.
    Launch(MuxLaunchArgs),
}

#[derive(Debug, Args)]
struct MuxNewArgs {
    /// tmux session name.
    name: String,
    /// Working directory tmux passes as `-c`. Defaults to `$PWD`.
    /// Must exist on disk at invocation time.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Non-default tmux socket (`tmux -L <name>`). Absent ⇒ default
    /// socket.
    #[arg(long, value_name = "NAME")]
    socket: Option<String>,
    /// Skip the terminal hand-off. The new session is spawned
    /// detached and the attach command is printed for the operator
    /// to run by hand. Mirrors `mux launch --no-attach`.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Also pin the mux as a slot. Takes an optional slot name,
    /// defaulting to the mux name.
    #[arg(long, value_name = "SLOT", num_args = 0..=1, default_missing_value = "")]
    pin: Option<String>,
    /// With `--pin`: write the slot without starting the mux. The first
    /// `conspectus rebuild <slot>` builds it.
    #[arg(long = "no-launch", requires = "pin")]
    no_launch: bool,
}

impl MuxNewArgs {
    fn run(self) -> Result<()> {
        let runner = SystemTmux::new();
        let history = launch_history_writer();
        self.run_recording(&runner, history.as_ref())
    }

    #[cfg(test)]
    fn run_with_runner(self, runner: &dyn MuxBackend) -> Result<()> {
        self.run_recording(runner, None)
    }

    fn run_recording(
        self,
        runner: &dyn MuxBackend,
        history: Option<&crate::history::HistoryWriter>,
    ) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("mux new requires a non-empty <NAME>");
        }
        let cwd = match self.cwd {
            Some(cwd) => cwd,
            None => crate::cwd::for_default_target("--cwd <path>")?,
        };
        if !cwd.is_dir() {
            bail!(
                "mux new --cwd `{}` is not an existing directory",
                cwd.display()
            );
        }
        let socket = self.socket.as_deref();
        let name = self.name.as_str();
        let pin = pin_request(self.pin.as_deref(), self.no_launch, name, history)?;
        if let Some(pin) = pin.as_ref().filter(|pin| pin.no_launch) {
            return write_pin(pin, None, recipe_fields(name, socket, &cwd, &[], None));
        }
        // ADR 0095: empty argv — tmux launches the operator's login
        // shell. No `send-keys` is used to seed the pane (ADR 0028).
        let outcome = runner
            .new_session(socket, name, &cwd, &[])
            .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
        report_new_session(outcome, name)?;
        let record = record_launch(history, runner.backend_key(), socket, name, &cwd, &[], None);
        if let Some(pin) = &pin {
            write_pin(
                pin,
                record.as_deref(),
                recipe_fields(name, socket, &cwd, &[], None),
            )
            .unwrap_or_else(|err| eprintln!("conspectus: warning: {err}"));
        }
        if self.no_attach {
            let attach_cmd = format_attach_command(socket, name);
            println!("spawned `{name}` (detached); attach with: {attach_cmd}");
            return Ok(());
        }
        attach_and_report(runner, socket, name)
    }
}

#[derive(Debug, Args)]
struct MuxLaunchArgs {
    /// Harness key (e.g. `codex`, `claude-code`, `opencode`).
    /// Matched against `HarnessAdapter::harness_key`.
    harness: String,
    /// tmux session name for the new mux. Required.
    #[arg(long)]
    name: String,
    /// Working directory tmux passes as `-c`. Defaults to `$PWD`.
    /// Must exist on disk at invocation time. Ignored when
    /// `--worktree-branch` is set and the realized worktree path is
    /// used instead.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Non-default tmux socket (`tmux -L <name>`). Absent ⇒ default
    /// socket.
    #[arg(long, value_name = "NAME")]
    socket: Option<String>,
    /// Override the harness adapter's default launch argv. Every
    /// value after `--argv` up to the next flag is treated as a
    /// launch-argv token.
    #[arg(long, value_name = "ARG", num_args = 1..)]
    argv: Option<Vec<String>>,
    /// Realize the worktree for `<BRANCH>` under `--worktree-repo`
    /// at launch, and use the worktree path as the mux cwd
    /// (reuses the worktree if it already exists). Requires
    /// `--worktree-repo`.
    #[arg(long = "worktree-branch", value_name = "BRANCH")]
    worktree_branch: Option<String>,
    /// Repo anchor for the worktree realization. Required with
    /// `--worktree-branch`, ignored otherwise.
    #[arg(long = "worktree-repo", value_name = "PATH")]
    worktree_repo: Option<PathBuf>,
    /// Skip the terminal hand-off. Mirrors `mux new --no-attach`.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Also pin the mux as a slot. Takes an optional slot name,
    /// defaulting to the mux name.
    #[arg(long, value_name = "SLOT", num_args = 0..=1, default_missing_value = "")]
    pin: Option<String>,
    /// With `--pin`: write the slot without starting the mux. The first
    /// `conspectus rebuild <slot>` builds it.
    #[arg(long = "no-launch", requires = "pin")]
    no_launch: bool,
    /// Accepted for symmetry with other verbs; `mux launch` has no
    /// discovery-driven state, so it's hidden from help.
    #[arg(long = "scan-root", value_name = "PATH", hide = true)]
    #[allow(dead_code)]
    scan_root: Vec<PathBuf>,
}

impl MuxLaunchArgs {
    fn run(self) -> Result<()> {
        let runner = SystemTmux::new();
        let history = launch_history_writer();
        self.run_recording(&runner, history.as_ref())
    }

    #[cfg(test)]
    fn run_with_runner(self, runner: &dyn MuxBackend) -> Result<()> {
        self.run_recording(runner, None)
    }

    fn run_recording(
        self,
        runner: &dyn MuxBackend,
        history: Option<&crate::history::HistoryWriter>,
    ) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("mux launch requires a non-empty --name");
        }
        if self.harness.trim().is_empty() {
            bail!("mux launch requires a non-empty <HARNESS>");
        }
        if self.no_launch && self.worktree_branch.is_some() {
            bail!(
                "mux launch: --no-launch can't realize a worktree; rebuild the slot to create it"
            );
        }
        // Resolve cwd: prefer worktree realization when --worktree-branch
        // is set; otherwise use --cwd or $PWD.
        let cwd = match (&self.worktree_branch, &self.worktree_repo) {
            (Some(branch), Some(repo)) => realize_worktree(repo, branch)?,
            (Some(_), None) => bail!("mux launch: --worktree-branch requires --worktree-repo"),
            (None, Some(_)) => {
                bail!("mux launch: --worktree-repo is only meaningful with --worktree-branch")
            }
            (None, None) => match self.cwd {
                Some(cwd) => cwd,
                None => crate::cwd::for_default_target("--cwd <path>")?,
            },
        };
        if !cwd.is_dir() {
            bail!(
                "mux launch --cwd `{}` is not an existing directory",
                cwd.display()
            );
        }
        // Resolve argv: --argv override, else the harness adapter's
        // default. Empty for both is a hard error since tmux would
        // start an unattributed process.
        let argv: Vec<std::ffi::OsString> = match self.argv {
            Some(argv) if !argv.is_empty() => {
                argv.into_iter().map(std::ffi::OsString::from).collect()
            }
            _ => launch_argv_for(self.harness.trim()),
        };
        if argv.is_empty() {
            bail!(
                "mux launch: no launch argv for harness `{}`; pass `--argv` explicitly",
                self.harness
            );
        }
        let socket = self.socket.as_deref();
        let name = self.name.as_str();
        let recorded: Vec<String> = argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let harness = self.harness.trim();
        let pin = pin_request(self.pin.as_deref(), self.no_launch, name, history)?;
        if let Some(pin) = pin.as_ref().filter(|pin| pin.no_launch) {
            return write_pin(
                pin,
                None,
                recipe_fields(name, socket, &cwd, &recorded, Some(harness)),
            );
        }
        // Not watched (ADR 0133): attaching right away shows the
        // harness's first output, and a harness that dies non-zero
        // stays as a dead pane with its output (ADR 0103).
        crate::rebuild::launch::spawn(runner, socket, name, &cwd, &argv)?;
        let record = record_launch(
            history,
            runner.backend_key(),
            socket,
            name,
            &cwd,
            &recorded,
            Some(harness),
        );
        if let Some(pin) = &pin {
            write_pin(
                pin,
                record.as_deref(),
                recipe_fields(name, socket, &cwd, &recorded, Some(harness)),
            )
            .unwrap_or_else(|err| eprintln!("conspectus: warning: {err}"));
        }
        if self.no_attach {
            let attach_cmd = format_attach_command(socket, name);
            println!("launched `{name}` (detached); attach with: {attach_cmd}");
            return Ok(());
        }
        attach_and_report(runner, socket, name)
    }
}

/// The history writer launch paths record into (ADR 0140), with the
/// `[history]` settings from config.
pub(super) fn launch_history_writer() -> Option<crate::history::HistoryWriter> {
    crate::history::HistoryWriter::from_env(crate::config::load_from_cwd().config.history)
}

/// Record a mux a launch path just created, with the argv it used, so
/// it is rebuildable before discovery sees it. A failure warns; the
/// mux is already running.
pub(super) fn record_launch(
    history: Option<&crate::history::HistoryWriter>,
    backend: &str,
    socket: Option<&str>,
    name: &str,
    cwd: &std::path::Path,
    argv: &[String],
    harness: Option<&str>,
) -> Option<String> {
    let writer = history?;
    let cwd = cwd.to_string_lossy();
    let launch = crate::history::LaunchedMux {
        backend,
        socket,
        name,
        cwd: &cwd,
        argv,
        harness,
    };
    match writer.record_launched_mux(&launch) {
        Ok(id) => id,
        Err(err) => {
            eprintln!(
                "conspectus: warning: failed to record `{name}` in history at {}: {err}",
                writer.store.dir().display()
            );
            None
        }
    }
}

/// What `--pin` asked for.
struct PinRequest {
    store: crate::pin_slots::SlotStore,
    slot: String,
    no_launch: bool,
}

/// Validate `--pin` before anything starts: a usable slot name, not
/// already taken, a slot store to write, and (unless only planning) a
/// history store to point the slot at.
fn pin_request(
    pin: Option<&str>,
    no_launch: bool,
    default_name: &str,
    history: Option<&crate::history::HistoryWriter>,
) -> Result<Option<PinRequest>> {
    let Some(pin) = pin else {
        return Ok(None);
    };
    let slot = if pin.trim().is_empty() {
        default_name
    } else {
        pin
    };
    crate::pin_slots::validate_id(slot).map_err(|err| anyhow!("--pin: {err}"))?;
    let store = crate::pin_slots::SlotStore::from_env()
        .ok_or_else(|| anyhow!("--pin: no data home: set $XDG_DATA_HOME or $HOME"))?;
    if store
        .load()
        .slots
        .iter()
        .any(|existing| existing.id == slot)
    {
        bail!("--pin: a pin slot named `{slot}` already exists");
    }
    if !no_launch && history.is_none() {
        bail!("--pin needs the history store to point the slot at; none is available");
    }
    Ok(Some(PinRequest {
        store,
        slot: slot.to_string(),
        no_launch,
    }))
}

/// The recipe fields a slot for this launch carries.
fn recipe_fields(
    name: &str,
    socket: Option<&str>,
    cwd: &std::path::Path,
    argv: &[String],
    harness: Option<&str>,
) -> crate::model::SlotFields {
    use crate::model::FieldValue::{List, Text};
    let mut fields = crate::model::SlotFields::new();
    fields.insert("name".into(), Text(name.to_string()));
    if let Some(socket) = crate::model::effective_socket(socket) {
        fields.insert("socket".into(), Text(socket.to_string()));
    }
    fields.insert("cwd".into(), Text(cwd.to_string_lossy().into_owned()));
    if !argv.is_empty() {
        fields.insert("argv".into(), List(argv.to_vec()));
    }
    if let Some(harness) = harness {
        fields.insert("harness".into(), Text(harness.to_string()));
    }
    fields
}

/// Write the slot for a launch (or, with `--no-launch`, for a mux that
/// doesn't exist yet). A failure after launching warns: the mux is up.
fn write_pin(
    request: &PinRequest,
    record: Option<&str>,
    fields: crate::model::SlotFields,
) -> Result<()> {
    use crate::model::PinKind;
    let slot = match record {
        Some(record) => {
            crate::pin_ops::pin_record(&request.store, &request.slot, PinKind::Mux, record, fields)
        }
        None => crate::pin_ops::pin_planned(&request.store, &request.slot, PinKind::Mux, fields),
    }
    .map_err(|err| anyhow!("{err}"))?;
    println!(
        "pinned `{}` ({})",
        slot.id,
        if record.is_some() { "live" } else { "planned" }
    );
    Ok(())
}

/// Realize the worktree for `branch` under `repo` and return the
/// worktree's absolute path. Idempotent — if the worktree already
/// exists, its path is returned unchanged. Mirrors ADR 0094 pin-side
/// realize-at-launch minus the pin-store read.
fn realize_worktree(repo: &std::path::Path, branch: &str) -> Result<PathBuf> {
    use crate::discovery::worktree::{
        WorktreeBackendSelection, WorktreeCreateRequest, WorktreeMutationOutcome,
        resolve_mutation_backend, worktrunk_available,
    };
    // If a worktree already exists for this branch, prefer its path.
    let backend_selection = WorktreeBackendSelection::default();
    let backend = resolve_mutation_backend(backend_selection, worktrunk_available())?
        .ok_or_else(|| anyhow!("mux launch: no worktree mutation backend configured"))?;
    if let Some(existing) = existing_worktree_for_branch(&*backend, repo, branch)? {
        return Ok(existing);
    }
    let request = WorktreeCreateRequest {
        repo_root: repo.to_path_buf(),
        branch: branch.to_string(),
        base: None,
    };
    let outcome = backend
        .create(&request)
        .map_err(|err| anyhow!("worktree create failed: {err}"))?;
    match outcome {
        WorktreeMutationOutcome::Succeeded { path: Some(path) } => Ok(PathBuf::from(path)),
        WorktreeMutationOutcome::Succeeded { path: None } => {
            // Re-list to locate the freshly created worktree.
            existing_worktree_for_branch(&*backend, repo, branch)?
                .ok_or_else(|| anyhow!("worktree created but not found in listing"))
        }
        WorktreeMutationOutcome::Unsupported => {
            bail!("mux launch: configured worktree backend does not support create")
        }
        WorktreeMutationOutcome::Failed { code, message } => {
            bail!(
                "mux launch: worktree create failed (exit {}): {message}",
                code.map_or_else(|| "?".to_string(), |c| c.to_string())
            )
        }
    }
}

fn existing_worktree_for_branch(
    backend: &dyn crate::discovery::worktree::WorktreeBackend,
    repo: &std::path::Path,
    branch: &str,
) -> Result<Option<PathBuf>> {
    let records = backend
        .list(repo)
        .map_err(|err| anyhow!("worktree list failed: {err}"))?;
    let short = branch.trim_start_matches("refs/heads/");
    Ok(records
        .into_iter()
        .find(|rec| {
            rec.branch
                .as_deref()
                .is_some_and(|b| b.trim_start_matches("refs/heads/") == short)
        })
        .map(|rec| PathBuf::from(rec.path)))
}

#[cfg(test)]
#[path = "mux_tests.rs"]
mod tests;
