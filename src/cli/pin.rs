//! `conspectus pin` subcommand tree (H-REF-006 wave 10).
//!
//! Pin CRUD, bind / rebind / adopt, launch / attach commands
//! per ADR 0057. This is the largest single-command tree in
//! the CLI; the extraction folds every pin-only helper into
//! this module alongside the args + impls.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand, ValueEnum};

use conspectus::config::ConfigLoader;
use conspectus::declared::{
    DeclaredEndpoint, DeclaredLink, DeclaredLinkState, upsert_declared_link,
};
use conspectus::discovery::harness::launch_argv_for;
use conspectus::discovery::tmux::{
    MuxBackend, SystemTmux, TmuxAttachOutcome, TmuxNewSessionOutcome, TmuxSendKeysOutcome,
};
use conspectus::model::{GraphSnapshot, NodeId, PinBinding, Provenance, RelationKind};
use conspectus::pins::{
    PinEntry, PinLaunch, PinMux, PinStoreKind, PinStoreSelection, TMUX_MUX_BACKEND,
    load_pin_entry_by_id, remove_pin_entry, select_store_for_pin, upsert_pin_entry, user_pin_store,
};

use super::declared::resolve_write_store;
use super::{
    DeclaredStoreFlag, discover_for_store_selection, effective_scan_roots, provenance_label,
    store_label,
};

// =====================================================================
// Pin command tree (ADR 0057 / H-PIN-007/008/009).
// =====================================================================

#[derive(Debug, Args)]
pub struct PinArgs {
    #[command(subcommand)]
    command: PinCommand,
}

impl PinArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            PinCommand::Create(args) => args.run(),
            PinCommand::List(args) => args.run(),
            PinCommand::Show(args) => args.run(),
            PinCommand::Rename(args) => args.run(),
            PinCommand::Rm(args) => args.run(),
            PinCommand::Launch(args) => args.run(PinLaunchIntent::Launch),
            PinCommand::Attach(args) => args.run(PinLaunchIntent::Attach),
            PinCommand::Bind(args) => args.run(),
            PinCommand::Rebind(args) => args.run(),
            PinCommand::Adopt(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum PinCommand {
    /// Declare a session pin.
    Create(Box<PinCreateArgs>),
    /// List session pins from the discovered config stores.
    List(PinListArgs),
    /// Show a single pin by id, including resolver binding state.
    Show(PinShowArgs),
    /// Rename a pin's `id` or `display_name`.
    Rename(PinRenameArgs),
    /// Remove a pin by id.
    Rm(PinRmArgs),
    /// Launch a pin's session (creating the tmux session if needed)
    /// and attach the terminal.
    Launch(PinLaunchArgs),
    /// Attach to a pin's already-bound session, or fall through to
    /// launch when no live session exists yet.
    Attach(PinLaunchArgs),
    /// Resolve `PinAmbiguous` by binding the pin to a specific
    /// agent-session id. Writes a `LocalDeclared linked_to_mux`
    /// link the resolver treats as authoritative.
    Bind(PinBindArgs),
    /// Update the pin's `mux.name` (and optionally
    /// `mux.socket_name`) after an external tmux rename. Pure TOML
    /// mutation — does not touch tmux.
    Rebind(PinRebindArgs),
    /// Convert an existing live tmux session into a pin without
    /// creating a new mux. Harness and cwd are inferred from the
    /// running session unless overridden.
    Adopt(PinAdoptArgs),
}

/// Filter the binding states `pin list` includes (ADR 0057).
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum PinStateFilter {
    All,
    Bound,
    Unbound,
    Stale,
}

#[derive(Debug, Args)]
struct PinCreateArgs {
    /// Stable pin id (unique within the chosen store).
    id: String,
    /// Harness key (`codex`, `claude-code`, `opencode`, `aider`).
    #[arg(long)]
    harness: String,
    /// Absolute path the pin anchors on (also passed as `tmux -c` at
    /// launch). Required to exist on disk at write time.
    #[arg(long, value_name = "PATH")]
    cwd: PathBuf,
    /// Operator-chosen display name. Defaults to `<id>`.
    #[arg(long)]
    display: Option<String>,
    /// Mux session name. Defaults to the chosen `--display` (and
    /// therefore to `<id>` when neither is set).
    #[arg(long = "mux-name", value_name = "NAME")]
    mux_name: Option<String>,
    /// Optional tmux socket name (the equivalent of `tmux -L
    /// <name>`). Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Override the per-harness default `launch.argv`. Repeatable —
    /// each value is one argv token.
    #[arg(long = "launch-arg", value_name = "ARG")]
    launch_argv: Vec<String>,
    /// Free-form explanatory text written under the pin's `reason`
    /// field.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl PinCreateArgs {
    fn run(self) -> Result<()> {
        let display = self.display.clone().unwrap_or_else(|| self.id.clone());
        let mux_name = self.mux_name.clone().unwrap_or_else(|| display.clone());

        let entry = PinEntry {
            id: self.id.clone(),
            display_name: display,
            harness: self.harness,
            cwd: self.cwd.display().to_string(),
            mux: PinMux {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: mux_name,
                socket_name: self.mux_socket,
            },
            launch: if self.launch_argv.is_empty() {
                None
            } else {
                Some(PinLaunch {
                    argv: self.launch_argv,
                })
            },
            reason: self.reason,
        };

        let selection = resolve_pin_write_store(self.store, &self.cwd)?;

        let outcome = upsert_pin_entry(&selection.path, entry.clone())
            .map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            if outcome.entry_count == 1 {
                "wrote"
            } else {
                "updated"
            }
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` in {} ({})",
            entry.id,
            selection.path.display(),
            pin_store_label(selection.kind)
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinListArgs {
    /// Limit the list to a store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Filter by binding state.
    #[arg(long, value_enum, default_value_t = PinStateFilter::All)]
    state: PinStateFilter,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinListArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let include_project = matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        );
        let include_user = matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User);

        let mut rows: Vec<String> = Vec::new();
        for pin in &snapshot.pins {
            let store_flag = match pin.provenance {
                Provenance::LocalPin => DeclaredStoreFlag::Project,
                Provenance::GlobalPin => DeclaredStoreFlag::User,
                _ => continue,
            };
            if matches!(store_flag, DeclaredStoreFlag::Project) && !include_project {
                continue;
            }
            if matches!(store_flag, DeclaredStoreFlag::User) && !include_user {
                continue;
            }
            if !pin_matches_filter(pin.binding.as_ref(), self.state) {
                continue;
            }
            rows.push(render_pin_row(
                store_flag,
                pin.provenance,
                &pin.id,
                &pin.display_name,
                &pin.harness,
                &pin.cwd,
                pin.mux.native_id(),
                bound_session_label(pin.binding.as_ref()).unwrap_or_default(),
                pin_state_label(pin.binding.as_ref()),
                &pin.store_path,
            ));
        }

        rows.sort();
        for row in rows {
            println!("{row}");
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinShowArgs {
    /// Pin id.
    id: String,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinShowArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };
        println!("id           {}", pin.id);
        println!("display_name {}", pin.display_name);
        println!("harness      {}", pin.harness);
        println!("cwd          {}", pin.cwd);
        println!("mux          {}", pin.mux.native_id());
        if let Some(socket) = pin.mux.socket_name.as_deref() {
            println!("socket_name  {socket}");
        }
        if let Some(argv) = pin.launch_argv.as_ref() {
            println!("launch_argv  {}", argv.join(" "));
        }
        if let Some(reason) = pin.reason.as_deref() {
            println!("reason       {reason}");
        }
        println!("provenance   {}", provenance_label(pin.provenance));
        println!("store        {}", pin.store_path);
        println!("state        {}", pin_state_label(pin.binding.as_ref()));
        match pin.binding.as_ref() {
            Some(PinBinding::Bound { session, mux }) => {
                println!("bound_mux    {}", mux.native_id);
                println!("bound_session {}", session.session_key);
            }
            Some(PinBinding::StaleMux { mux }) => {
                println!("bound_mux    {} (no live harness session)", mux.native_id);
            }
            _ => {}
        }
        // ADR 0058 H-PIN-RESUME-005: when the pin is unbound and the
        // sidecar has a recorded last-bound session, surface it so
        // the operator can see what `pin launch` would resume into.
        if let Some(last) = pin_last_session_for(&snapshot, &self.id) {
            println!(
                "last_session {} (observed {})",
                last.session_id,
                format_epoch_iso8601(last.observed_epoch),
            );
        }
        // Surface pin-specific diagnostics for this pin (PinAmbiguous,
        // PinDrift, etc.). Each is rendered on its own line so the
        // operator can pipe / grep the output.
        for diagnostic in snapshot
            .diagnostics
            .iter()
            .filter(|d| pin_diagnostic_matches(d, &self.id))
        {
            println!("diagnostic   {}", format_pin_diagnostic(diagnostic));
        }
        Ok(())
    }
}

fn pin_last_session_for<'a>(
    snapshot: &'a GraphSnapshot,
    pin_id: &str,
) -> Option<&'a conspectus::model::PinLastSession> {
    use conspectus::model::Diagnostic;
    snapshot.diagnostics.iter().find_map(|d| match d {
        Diagnostic::PinUnbound {
            pin_id: id,
            last_session: Some(last),
            ..
        } if id == pin_id => Some(last),
        _ => None,
    })
}

pub(super) fn format_epoch_iso8601(epoch: i64) -> String {
    // Minimal UTC ISO 8601 formatter using the standard library's
    // civil-time algorithm. Mirrors RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`)
    // without pulling in chrono/humantime for a single output line.
    let secs = epoch.max(0) as u64;
    let (year, month, day, hour, minute, second) = civil_from_unix_seconds(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a Unix epoch (seconds, UTC) into a civil
/// `(year, month, day, hour, minute, second)` tuple. Uses the
/// Hinnant algorithm (Howard Hinnant's `days_from_civil` inverse)
/// so we don't need a date library for one CLI line.
fn civil_from_unix_seconds(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let hour = (time_of_day / 3_600) as u32;
    let minute = ((time_of_day % 3_600) / 60) as u32;
    let second = (time_of_day % 60) as u32;

    // Hinnant: days since 1970-01-01 → civil date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (y + if month <= 2 { 1 } else { 0 }) as i32;

    (year, month, day, hour, minute, second)
}

#[derive(Debug, Args)]
struct PinRenameArgs {
    /// Existing pin id.
    id: String,
    /// New pin id. Omit to keep the current id and only change
    /// `--display`.
    new_id: Option<String>,
    /// New display name.
    #[arg(long)]
    display: Option<String>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRenameArgs {
    fn run(self) -> Result<()> {
        if self.new_id.is_none() && self.display.is_none() {
            bail!("`pin rename` requires either a new id, `--display <name>`, or both");
        }
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        let Some((path, mut entry)) =
            load_pin_entry_by_id(&paths, &self.id).map_err(|err| anyhow!(err.to_string()))?
        else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        let original_id = entry.id.clone();
        if let Some(new_display) = self.display {
            entry.display_name = new_display;
        }
        let new_id_value = self.new_id.clone().unwrap_or_else(|| entry.id.clone());

        if new_id_value != original_id {
            // Storage rename: remove the old entry, then upsert the
            // entry under the new id. Both writes target the same
            // store so the operation is a single
            // remove-then-upsert flow.
            entry.id = new_id_value.clone();
            remove_pin_entry(&path, &original_id).map_err(|err| anyhow!(err.to_string()))?;
        }
        let outcome = upsert_pin_entry(&path, entry).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "renamed"
        } else {
            "unchanged"
        };
        if new_id_value != original_id {
            println!(
                "{verb} pin `{}` → `{}` in {}",
                original_id,
                new_id_value,
                path.display()
            );
        } else {
            println!("{verb} pin `{}` in {}", original_id, path.display());
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRmArgs {
    /// Pin id to remove.
    id: String,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRmArgs {
    fn run(self) -> Result<()> {
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        for path in &paths {
            let outcome =
                remove_pin_entry(path, &self.id).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                println!("removed pin `{}` from {}", self.id, path.display());
                return Ok(());
            }
        }
        bail!("no pin `{}` in any discovered store", self.id);
    }
}

#[derive(Debug, Args)]
pub struct PinBindArgs {
    /// Pin id to bind.
    id: String,
    /// Harness-native session key of the agent session the pin
    /// should bind to. Must already be visible in discovery.
    #[arg(long = "to", value_name = "SESSION_KEY")]
    to: String,
    /// Optional reason recorded on the declared link.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection for the declared
    /// link write.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinBindArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        // Pin and target must share a harness so the resolver's
        // post-bind attribution sees the LocalDeclared link as the
        // authoritative `LinkedToMux` for the pin's harness.
        let target = snapshot
            .nodes
            .iter()
            .find_map(|node| match node {
                conspectus::model::GraphNode::AgentSession(session)
                    if session.id.harness_key == pin.harness
                        && session.id.session_key == self.to =>
                {
                    Some(session.id.clone())
                }
                _ => None,
            })
            .ok_or_else(|| {
                anyhow!(
                    "no `{}` agent session with session_key `{}` in discovery",
                    pin.harness,
                    self.to
                )
            })?;

        let source_endpoint = DeclaredEndpoint::AgentSession {
            harness_key: target.harness_key.clone(),
            state_scope: target.state_scope.clone(),
            session_key: target.session_key,
        };
        let target_endpoint = DeclaredEndpoint::MuxSession {
            native_id: pin.mux.native_id(),
        };
        let link = DeclaredLink {
            id: format!("pin:{}:bound", pin.id),
            relation: RelationKind::LinkedToMux,
            state: DeclaredLinkState::Active,
            source: source_endpoint,
            target: target_endpoint,
            reason: self.reason,
            overridden_by: None,
            // Operator-facing breadcrumb tying the declared link to
            // its originating pin. Read by `declared list` so
            // operators see WHY this override exists.
            label: Some(format!("pin:{}", pin.id)),
        };

        let path = resolve_write_store(
            self.store,
            Some(&link.source),
            Some(&link.target),
            &self.scan_roots,
        )?;
        let outcome = upsert_declared_link(&path, link).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "wrote"
        } else {
            "unchanged"
        };
        println!(
            "{verb} declared override for pin `{}` → session `{}` in {}",
            pin.id,
            self.to,
            path.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct PinRebindArgs {
    /// Pin id to rebind.
    id: String,
    /// New tmux session name to bind the pin to.
    #[arg(long = "mux", value_name = "NAME")]
    mux: String,
    /// Optional non-default tmux socket. Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRebindArgs {
    fn run(self) -> Result<()> {
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        let Some((path, mut entry)) =
            load_pin_entry_by_id(&paths, &self.id).map_err(|err| anyhow!(err.to_string()))?
        else {
            bail!("no pin `{}` in any discovered store", self.id);
        };
        let previous = entry.mux.clone();
        entry.mux = PinMux {
            backend: previous.backend,
            name: self.mux,
            socket_name: self.mux_socket,
        };
        let outcome =
            upsert_pin_entry(&path, entry.clone()).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "rebound"
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` → mux `{}` in {}",
            entry.id,
            entry.mux.native_id(),
            path.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct PinAdoptArgs {
    /// New pin id.
    id: String,
    /// Existing tmux session name to adopt as this pin's bound mux.
    mux_name: String,
    /// Override the inferred harness when discovery can't or
    /// shouldn't attribute one.
    #[arg(long)]
    harness: Option<String>,
    /// Display name for the new pin. Defaults to `<id>`.
    #[arg(long)]
    display: Option<String>,
    /// Optional non-default tmux socket. Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Override the inferred cwd. By default, adopt uses the mux's
    /// observed cwd; pass `--cwd` to set a different anchor.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinAdoptArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;

        let backend = TMUX_MUX_BACKEND.to_string();
        let native_id = match self.mux_socket.as_deref() {
            None | Some("default") => format!("{}:{}", backend, self.mux_name),
            Some(socket) => format!("{}:{}:{}", backend, socket, self.mux_name),
        };

        let mux_node = snapshot
            .nodes
            .iter()
            .find_map(|node| match node {
                conspectus::model::GraphNode::MuxSession(mux) if mux.native_id == native_id => {
                    Some(mux)
                }
                _ => None,
            })
            .ok_or_else(|| {
                anyhow!(
                    "no live mux with native_id `{native_id}` — start the tmux session first or rebind to an existing pin"
                )
            })?;

        // Harness inference: walk active LinkedToMux candidates with
        // this mux as the target; the first AgentSession source wins.
        let inferred_harness = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, conspectus::model::LinkState::Active)
                    && link.target_node_id() == Some(&NodeId::MuxSession(mux_node.id.clone()))
            })
            .find_map(|link| match &link.source {
                NodeId::AgentSession(session) => Some(session.harness_key.clone()),
                _ => None,
            });

        let harness = match (self.harness, inferred_harness) {
            (Some(explicit), _) => explicit,
            (None, Some(inferred)) => inferred,
            (None, None) => bail!(
                "could not infer a harness for mux `{native_id}`; pass `--harness <key>` explicitly"
            ),
        };

        let cwd = match self.cwd {
            Some(explicit) => explicit,
            None => {
                let observed = mux_node.cwd.as_deref().ok_or_else(|| {
                    anyhow!("mux `{native_id}` has no observed cwd; pass `--cwd <PATH>` explicitly")
                })?;
                PathBuf::from(observed)
            }
        };

        let display = self.display.clone().unwrap_or_else(|| self.id.clone());
        let entry = PinEntry {
            id: self.id.clone(),
            display_name: display,
            harness,
            cwd: cwd.display().to_string(),
            mux: PinMux {
                backend,
                name: self.mux_name.clone(),
                socket_name: self.mux_socket.clone(),
            },
            launch: None,
            reason: None,
        };

        let selection = resolve_pin_write_store(self.store, &cwd)?;
        let outcome = upsert_pin_entry(&selection.path, entry.clone())
            .map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "adopted"
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` from mux `{}` (harness: {}) in {}",
            entry.id,
            entry.mux.native_id(),
            entry.harness,
            selection.path.display()
        );
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PinLaunchIntent {
    /// `pin launch` — operator wants the pin running. Spawn if
    /// needed, then attach.
    Launch,
    /// `pin attach` — operator expects the pin already running but
    /// will accept a fresh spawn if not.
    Attach,
}

#[derive(Debug, Args)]
pub struct PinLaunchArgs {
    /// Pin id to launch / attach.
    id: String,
    /// Skip the terminal hand-off. The new session (if any) is
    /// spawned detached and the attach command is printed for the
    /// operator to run by hand. Useful for scripts and CI dry runs.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinLaunchArgs {
    fn run(self, intent: PinLaunchIntent) -> Result<()> {
        let runner = SystemTmux::new();
        self.run_with_runner(intent, &runner)
    }

    fn run_with_runner(self, intent: PinLaunchIntent, runner: &dyn MuxBackend) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        let socket = pin.mux.effective_socket();
        let mux_name = pin.mux.name.as_str();
        let argv: Vec<std::ffi::OsString> = pin
            .launch_argv
            .as_ref()
            .map(|argv| argv.iter().map(std::ffi::OsString::from).collect())
            .filter(|argv: &Vec<_>| !argv.is_empty())
            .unwrap_or_else(|| launch_argv_for(&pin.harness));

        if argv.is_empty() {
            bail!(
                "no launch argv configured for harness `{}`; set `launch.argv` on the pin",
                pin.harness
            );
        }

        match pin.binding.as_ref() {
            Some(PinBinding::Bound { mux, session }) => {
                println!(
                    "pin `{}` already bound to session `{}` in mux `{}`",
                    pin.id, session.session_key, mux.native_id
                );
                if !self.no_attach {
                    attach_and_report(runner, socket, mux_name)?;
                }
                Ok(())
            }
            Some(PinBinding::StaleMux { mux }) => {
                println!(
                    "pin `{}` mux `{}` is live but has no `{}` session; relaunching via send-keys",
                    pin.id, mux.native_id, pin.harness
                );
                let literal = format_argv_for_send_keys(&argv);
                let outcome = runner
                    .send_keys(socket, mux_name, &literal, true)
                    .map_err(|err| anyhow!("tmux send-keys failed: {err}"))?;
                report_send_keys(outcome, mux_name)?;
                if !self.no_attach {
                    attach_and_report(runner, socket, mux_name)?;
                }
                Ok(())
            }
            Some(PinBinding::Unbound) | None => {
                if matches!(intent, PinLaunchIntent::Attach) {
                    eprintln!(
                        "conspectus: note: pin `{}` is unbound; falling through to launch",
                        pin.id
                    );
                }
                let cwd = std::path::PathBuf::from(&pin.cwd);
                // ADR 0058 / H-PIN-RESUME-004: consult the
                // per-pin sidecar to splice in resume_argv when a
                // prior session is known and still reachable.
                // Falls back to the default argv on every honest
                // failure path (no sidecar, session missing, fork,
                // harness without resume CLI).
                let effective_argv =
                    resolve_resume_argv(&snapshot, pin, &cwd).unwrap_or_else(|| argv.clone());
                let outcome = runner
                    .new_session(socket, mux_name, &cwd, &effective_argv)
                    .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
                report_new_session(outcome, mux_name)?;
                if self.no_attach {
                    let attach_cmd = format_attach_command(socket, mux_name);
                    println!("spawned `{mux_name}` (detached); attach with: {attach_cmd}");
                    return Ok(());
                }
                attach_and_report(runner, socket, mux_name)?;
                Ok(())
            }
        }
    }
}

/// Consult the per-pin sidecar (ADR 0058) for a prior session and, when
/// reachable, build the harness's `resume_argv` for it. Returns `None`
/// (so the caller falls back to default argv) on every honest failure
/// mode: no sidecar, recorded session no longer on disk (deletes the
/// sidecar), fork in the lineage chain, or the harness has no
/// resume CLI.
fn resolve_resume_argv(
    snapshot: &GraphSnapshot,
    pin: &conspectus::model::PinCandidate,
    cwd: &std::path::Path,
) -> Option<Vec<std::ffi::OsString>> {
    let cache = conspectus::pin_bindings::PinBindingsCache::from_env();
    cache.directory()?;
    resolve_resume_argv_with_cache(snapshot, pin, cwd, &cache)
}

pub(super) fn resolve_resume_argv_with_cache(
    snapshot: &GraphSnapshot,
    pin: &conspectus::model::PinCandidate,
    cwd: &std::path::Path,
    cache: &conspectus::pin_bindings::PinBindingsCache,
) -> Option<Vec<std::ffi::OsString>> {
    use conspectus::discovery::harness::resume_argv_for;
    use conspectus::pin_bindings::{LineageOutcome, delete as delete_sidecar, lineage_head, read};

    let record = match read(cache, &pin.id) {
        Ok(Some(record)) => record,
        Ok(None) => return None,
        Err(err) => {
            eprintln!(
                "conspectus: pin `{}`: ignoring unreadable sidecar ({err}); launching fresh",
                pin.id
            );
            return None;
        }
    };

    let head = match lineage_head(snapshot, &record.harness, &record.session_id) {
        LineageOutcome::Head(head) => head,
        LineageOutcome::SessionMissing => {
            // ADR 0058 Q7: stale sidecar — recorded session can't
            // be found anywhere in the current snapshot. Delete it
            // so it doesn't keep producing this hint on subsequent
            // launches.
            match delete_sidecar(cache, &pin.id) {
                Ok(_) => eprintln!(
                    "conspectus: pin `{}`: previous session `{}` no longer exists; \
                     cleared sidecar, launching fresh",
                    pin.id, record.session_id
                ),
                Err(err) => eprintln!(
                    "conspectus: pin `{}`: previous session `{}` no longer exists; \
                     could not clear sidecar ({err}); launching fresh",
                    pin.id, record.session_id
                ),
            }
            return None;
        }
        LineageOutcome::Fork { at, successors } => {
            eprintln!(
                "conspectus: pin `{}`: session `{}` has {} compacted successors; \
                 launching fresh — pick one with `conspectus session continue <id>` or \
                 resume manually",
                pin.id,
                at.session_key,
                successors.len(),
            );
            return None;
        }
    };

    match resume_argv_for(&pin.harness, &head.session_key, cwd) {
        Some(argv) => {
            println!(
                "pin `{}`: resuming recorded session `{}`",
                pin.id, head.session_key
            );
            Some(argv)
        }
        None => {
            eprintln!(
                "conspectus: pin `{}`: harness `{}` does not expose a resume command; \
                 launching fresh",
                pin.id, pin.harness
            );
            None
        }
    }
}

fn attach_and_report(runner: &dyn MuxBackend, socket: Option<&str>, name: &str) -> Result<()> {
    let outcome = runner
        .attach_session(socket, name)
        .map_err(|err| anyhow!("tmux attach failed: {err}"))?;
    match outcome {
        TmuxAttachOutcome::Detached => Ok(()),
        TmuxAttachOutcome::NoTarget => {
            bail!("tmux session `{name}` vanished between launch and attach (race) — try again")
        }
        TmuxAttachOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxAttachOutcome::Failed { code, message } => {
            bail!("tmux attach failed (exit {code:?}): {message}")
        }
        TmuxAttachOutcome::Unsupported => {
            bail!("this tmux runner does not support attach; run `tmux attach -t {name}` manually")
        }
    }
}

fn report_send_keys(outcome: TmuxSendKeysOutcome, name: &str) -> Result<()> {
    match outcome {
        TmuxSendKeysOutcome::Sent => Ok(()),
        TmuxSendKeysOutcome::NoTarget => {
            bail!("tmux session `{name}` disappeared before send-keys reached it")
        }
        TmuxSendKeysOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxSendKeysOutcome::Failed { code, message } => {
            bail!("tmux send-keys failed (exit {code:?}): {message}")
        }
        TmuxSendKeysOutcome::Unsupported => bail!("this tmux runner does not support send-keys"),
    }
}

fn report_new_session(outcome: TmuxNewSessionOutcome, name: &str) -> Result<()> {
    match outcome {
        TmuxNewSessionOutcome::Created => Ok(()),
        TmuxNewSessionOutcome::NameTaken => bail!(
            "a tmux session named `{name}` already exists; rename the existing tmux or pick a different `mux.name`"
        ),
        TmuxNewSessionOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxNewSessionOutcome::Failed { code, message } => {
            bail!("tmux new-session failed (exit {code:?}): {message}")
        }
        TmuxNewSessionOutcome::Unsupported => {
            bail!("this tmux runner does not support new-session")
        }
    }
}

/// Build the `tmux [-L <socket>] attach-session -t <name>` command
/// string used in `--no-attach` output. Single-arg quoting is
/// intentionally minimal — `mux.name` is operator-typed and the
/// schema rejects shell metacharacters by virtue of tmux's own
/// session-name rules (alphanumeric + a small set of punctuation).
pub(super) fn format_attach_command(socket: Option<&str>, name: &str) -> String {
    match socket {
        None => format!("tmux attach-session -t {name}"),
        Some(socket) => format!("tmux -L {socket} attach-session -t {name}"),
    }
}

/// Render an argv slice as a tmux `send-keys` literal so the
/// harness command lands in the existing pane. We quote each
/// arg with double-quotes when it contains spaces; this matches
/// the way operators would type the command interactively.
pub(super) fn format_argv_for_send_keys(argv: &[std::ffi::OsString]) -> String {
    argv.iter()
        .map(|token| {
            let token = token.to_string_lossy();
            if token.is_empty() || token.contains(char::is_whitespace) {
                format!("\"{token}\"")
            } else {
                token.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_pin_write_store(
    flag: Option<DeclaredStoreFlag>,
    cwd: &Path,
) -> Result<PinStoreSelection> {
    let loader = ConfigLoader::from_env();
    match flag {
        Some(DeclaredStoreFlag::User) => {
            user_pin_store(&loader).map_err(|err| anyhow!(err.to_string()))
        }
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for `pin create` (pick `project` or `user`)");
        }
        Some(DeclaredStoreFlag::Project) | None => {
            select_store_for_pin(cwd, &loader).map_err(|err| anyhow!(err.to_string()))
        }
    }
}

/// Candidate stores `pin rename` / `pin rm` should look in. Order
/// matters: project before user so a project-local pin shadows a
/// same-id user pin, matching the resolver's local-over-global rule.
fn candidate_pin_store_paths(scan_roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if let Some(path) = loader.locate_project_config(&root)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }
    if let Some(path) = loader.user_config_path()
        && seen.insert(path.clone())
    {
        paths.push(path);
    }
    Ok(paths)
}

fn discover_and_resolve(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let snapshot = discover_for_store_selection(scan_roots)?;
    let mut resolved = conspectus::resolve::resolve_snapshot(snapshot);
    record_pin_bindings_best_effort(&resolved);
    decorate_unbound_pins_best_effort(&mut resolved);
    Ok(resolved)
}

/// Enrich `PinUnbound` diagnostics with the sidecar's recorded
/// last-bound session so the launch path's UX surfaces
/// (`pin show`, TUI hint, right detail pane) can advertise resume
/// affordances. Mirrors `record_pin_bindings_best_effort`: silent
/// no-op when the cache root is absent.
fn decorate_unbound_pins_best_effort(snapshot: &mut GraphSnapshot) {
    use conspectus::pin_bindings::{PinBindingsCache, decorate_unbound_diagnostics};
    let cache = PinBindingsCache::from_env();
    if cache.directory().is_none() {
        return;
    }
    decorate_unbound_diagnostics(snapshot, &cache);
}

/// Record the resolver's pin bindings to per-pin sidecar files
/// (ADR 0058 / H-PIN-RESUME-003). Best-effort — sidecar I/O failures
/// log to stderr and never propagate up through discovery, so a
/// missing cache directory or read-only mount degrades pin launch's
/// continuity story without breaking the cycle.
fn record_pin_bindings_best_effort(snapshot: &GraphSnapshot) {
    use conspectus::pin_bindings::{PinBindingsCache, record_bindings};
    let cache = PinBindingsCache::from_env();
    if cache.directory().is_none() {
        // No $XDG_CACHE_HOME, no $HOME — silently skip rather than
        // log on every cycle. Operators without a cache directory
        // opt out of the continuity feature implicitly.
        return;
    }
    let now = conspectus::hook::current_epoch();
    for (pin_id, result) in record_bindings(snapshot, &cache, now) {
        if let Err(err) = result {
            eprintln!("conspectus: pin-binding sidecar write failed for `{pin_id}`: {err}");
        }
    }
}

fn pin_state_label(binding: Option<&PinBinding>) -> &'static str {
    match binding {
        Some(PinBinding::Bound { .. }) => "bound",
        Some(PinBinding::StaleMux { .. }) => "stale",
        Some(PinBinding::Unbound) => "unbound",
        None => "unresolved",
    }
}

fn pin_matches_filter(binding: Option<&PinBinding>, filter: PinStateFilter) -> bool {
    match filter {
        PinStateFilter::All => true,
        PinStateFilter::Bound => matches!(binding, Some(PinBinding::Bound { .. })),
        PinStateFilter::Stale => matches!(binding, Some(PinBinding::StaleMux { .. })),
        PinStateFilter::Unbound => matches!(binding, Some(PinBinding::Unbound) | None),
    }
}

fn pin_store_label(kind: PinStoreKind) -> &'static str {
    match kind {
        PinStoreKind::Project => "project",
        PinStoreKind::User => "user",
    }
}

fn bound_session_label(binding: Option<&PinBinding>) -> Option<String> {
    match binding {
        Some(PinBinding::Bound { session, .. }) => Some(session.session_key.clone()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pin_row(
    store: DeclaredStoreFlag,
    provenance: Provenance,
    id: &str,
    display_name: &str,
    harness: &str,
    cwd: &str,
    mux_native_id: String,
    bound_session: String,
    state_label: &str,
    store_path: &str,
) -> String {
    [
        store_label(store).to_string(),
        provenance_label(provenance).to_string(),
        state_label.to_string(),
        id.to_string(),
        display_name.to_string(),
        harness.to_string(),
        cwd.to_string(),
        mux_native_id,
        bound_session,
        store_path.to_string(),
    ]
    .join("\t")
}

fn pin_diagnostic_matches(diagnostic: &conspectus::model::Diagnostic, pin_id: &str) -> bool {
    use conspectus::model::Diagnostic;
    match diagnostic {
        Diagnostic::PinUnbound { pin_id: id, .. }
        | Diagnostic::PinStaleMux { pin_id: id, .. }
        | Diagnostic::PinAmbiguous { pin_id: id, .. }
        | Diagnostic::PinDrift { pin_id: id, .. } => id == pin_id,
        _ => false,
    }
}

fn format_pin_diagnostic(diagnostic: &conspectus::model::Diagnostic) -> String {
    use conspectus::model::Diagnostic;
    match diagnostic {
        Diagnostic::PinUnbound {
            expected_mux_native_id,
            ..
        } => format!("unbound (no live mux matching `{expected_mux_native_id}`)"),
        Diagnostic::PinStaleMux { mux, .. } => {
            format!("stale_mux (mux `{}` has no live harness)", mux.native_id)
        }
        Diagnostic::PinAmbiguous {
            chosen, competing, ..
        } => format!(
            "ambiguous (chose `{}`; {} competing: [{}])",
            chosen.session_key,
            competing.len(),
            competing
                .iter()
                .map(|s| s.session_key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Diagnostic::PinDrift {
            declared_cwd,
            observed_cwd,
            ..
        } => format!("drift (declared `{declared_cwd}`, observed `{observed_cwd}`)"),
        _ => format!("{diagnostic:?}"),
    }
}
