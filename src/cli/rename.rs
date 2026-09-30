//! `conspectus rename` subcommand tree (H-REF-006 wave 2).
//!
//! Two subcommands: `session` (agent-session alias write +
//! lockstep tmux rename per ADR 0029) and `mux` (tmux native
//! rename only). Extracted from `cli/mod.rs` alongside the
//! rename-specific helpers `execute_rename_plan`,
//! `run_mux_rename`, and `node_kind_label`.
//!
//! Shared surface reached back through `super`:
//! - `super::discover_for_store_selection` — snapshot for
//!   store selection.
//! - `super::resolve_alias_store` — pick the alias store
//!   scope from operator flags and endpoint context.
//! - `super::candidate_store_paths` — walk the
//!   project+user store list for removal fallback.
//! - `super::DeclaredStoreFlag` — CLI flag enum.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use conspectus::aliases::{AliasEntry, remove_alias_entry, upsert_alias_entry};
use conspectus::declared::declared_endpoint_from_node_id;
use conspectus::discovery::tmux::{MuxBackend, SystemTmux, TmuxRenameOutcome};
use conspectus::model::NodeId;
use conspectus::rename::{
    MuxNativeRename, MuxRenamePlan, RenamePlan, plan_mux_rename, plan_session_rename,
};

use super::{
    DeclaredStoreFlag, WriteStoreFlag, candidate_store_paths, discover_for_store_selection,
    resolve_alias_store,
};

#[derive(Debug, Args)]
pub(super) struct RenameArgs {
    #[command(subcommand)]
    command: RenameCommand,
}

impl RenameArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            RenameCommand::Session(args) => args.run(),
            RenameCommand::Mux(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum RenameCommand {
    /// Set, change, or clear an agent session's display-name alias.
    /// The linked tmux session is renamed in lockstep by default;
    /// pass `--no-mux` to skip the tmux side.
    Session(RenameSessionArgs),
    /// Rename a tmux session. No alias is written; only the tmux
    /// native name changes.
    Mux(RenameMuxArgs),
}

#[derive(Debug, Args)]
struct RenameSessionArgs {
    /// Agent session id. Accepts the short row id, the full
    /// `NodeId` display form, or the `harness:session_key` label
    /// (same forms `conspectus node show` understands).
    id: String,
    /// New display name. Mutually exclusive with `--clear`.
    name: Option<String>,
    /// Skip the lockstep tmux rename. The alias is still written.
    #[arg(long = "no-mux")]
    no_mux: bool,
    /// Remove any existing alias for this session instead of setting one.
    /// Mutually exclusive with `<NAME>`.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Restrict the alias write to one store.
    #[arg(long, value_enum)]
    store: Option<WriteStoreFlag>,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameSessionArgs {
    fn run(self) -> Result<()> {
        let new_display_name = match (self.name, self.clear) {
            (Some(_), true) => bail!("--clear and <NAME> are mutually exclusive"),
            (None, false) => bail!("specify either a new <NAME> or --clear"),
            (Some(name), false) => Some(name),
            (None, true) => None,
        };

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let session_id = match resolved {
            NodeId::AgentSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename session only operates on agent sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        let plan = plan_session_rename(&snapshot, &session_id, new_display_name, self.no_mux)
            .map_err(|err| anyhow!(err.to_string()))?;

        execute_rename_plan(
            &plan,
            self.store.map(Into::into),
            &self.scan_roots,
            &SystemTmux::new(),
        )
    }
}

#[derive(Debug, Args)]
struct RenameMuxArgs {
    /// Mux session id. Accepts the short row id, the full `NodeId`
    /// display form, or the `tmux:<native>` label.
    id: String,
    /// New tmux session name. Required because mux aliases are not
    /// stored; only the native tmux name changes.
    name: Option<String>,
    /// Rejected: mux sessions have no Conspectus-owned alias to
    /// clear. Surfaced so the help text documents the constraint.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Root used to discover the running tmux server, if any.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameMuxArgs {
    fn run(self) -> Result<()> {
        if self.clear {
            bail!(
                "mux sessions have no Conspectus-owned alias to clear; \
                 supply a new <NAME> instead"
            );
        }
        let new_name = self
            .name
            .ok_or_else(|| anyhow!("rename mux requires a new <NAME>"))?;
        if new_name.trim().is_empty() {
            bail!("mux rename requires a non-empty <NAME>");
        }

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let mux_id = match resolved {
            NodeId::MuxSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename mux only operates on mux sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        // H-RENAME-MUX: graph-aware cascade — rewrite any pin whose
        // `mux.name` matches the current mux native id + socket so
        // pin bindings survive the tmux rename.
        let plan = match plan_mux_rename(&snapshot, &mux_id, new_name) {
            Ok(plan) => plan,
            Err(err) => bail!("mux rename planning failed: {err}"),
        };
        execute_mux_rename_plan(&plan, &SystemTmux::new())
    }
}

/// Execute a [`MuxRenamePlan`]: rewrite each affected pin store,
/// then chain the tmux `rename-session` so the live tmux name
/// tracks the pin's declared intent.
fn execute_mux_rename_plan(plan: &MuxRenamePlan, tmux: &dyn MuxBackend) -> Result<()> {
    for update in &plan.pin_mux_name_updates {
        let store_paths = [PathBuf::from(&update.store_path)];
        let loaded = conspectus::pins::load_pin_entry_by_id(&store_paths, &update.pin_id)
            .map_err(|err| anyhow!("read pin store `{}`: {err}", update.store_path))?
            .ok_or_else(|| {
                anyhow!(
                    "pin `{}` not found in store `{}`",
                    update.pin_id,
                    update.store_path
                )
            })?;
        let (_path, mut entry) = loaded;
        if entry.mux.name != update.new_mux_name {
            entry.mux.name = update.new_mux_name.clone();
            conspectus::pins::upsert_pin_entry(&update.store_path, entry)
                .map_err(|err| anyhow!("write pin store `{}`: {err}", update.store_path))?;
            println!(
                "cascaded to pin `{}` (mux.name in `{}`)",
                update.pin_id, update.store_path
            );
        }
    }
    run_mux_rename(&plan.mux_rename, tmux)
}

/// Execute the alias-write side of `plan`, then (when present) the
/// linked tmux rename. Either step can leave the other in a partial
/// state — we surface the error and let the operator decide whether
/// to re-run.
fn execute_rename_plan(
    plan: &RenamePlan,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
    tmux: &dyn MuxBackend,
) -> Result<()> {
    let endpoint = declared_endpoint_from_node_id(&NodeId::AgentSession(
        plan.agent_alias_write.session.clone(),
    ));
    if let Some(display_name) = &plan.agent_alias_write.display_name {
        let path = resolve_alias_store(store, &endpoint, scan_roots)?;
        let entry = AliasEntry {
            node: endpoint,
            display_name: display_name.clone(),
            reason: None,
        };
        let outcome = upsert_alias_entry(&path, entry).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "wrote"
        } else {
            "unchanged"
        };
        println!("{verb} alias `{}` in {}", display_name, path.display());
    } else {
        // H-REF-006 wave 2: former `alias_candidate_store_paths`
        // trivially delegated to `candidate_store_paths` — the
        // wrapper retired here.
        let stores = candidate_store_paths(store, scan_roots)?;
        let mut removed_from = None;
        for path in &stores {
            if !path.is_file() {
                continue;
            }
            let outcome =
                remove_alias_entry(path, &endpoint).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                removed_from = Some(path.clone());
                break;
            }
        }
        match removed_from {
            Some(path) => println!("removed alias from {}", path.display()),
            None => println!("no alias found for session"),
        }
    }

    if let Some(mux_rename) = &plan.mux_native_rename {
        run_mux_rename(mux_rename, tmux)?;
    }
    Ok(())
}

fn run_mux_rename(rename: &MuxNativeRename, tmux: &dyn MuxBackend) -> Result<()> {
    let outcome = tmux
        // Default-socket rename — `conspectus rename` is the alias
        // overlay surface (ADR 0029) that runs on whatever socket
        // owned the discovered mux. Pin-driven non-default-socket
        // renames will run from `pin rename` via H-PIN-014 instead.
        .rename_session(None, &rename.mux.native_id, &rename.new_name)
        .map_err(|err| anyhow!("tmux rename-session failed: {err}"))?;
    match outcome {
        TmuxRenameOutcome::Renamed => {
            println!(
                "renamed tmux session `{}` to `{}`",
                rename.mux.native_id, rename.new_name
            );
            Ok(())
        }
        TmuxRenameOutcome::NoTarget => bail!(
            "tmux session `{}` not found on this server",
            rename.mux.native_id
        ),
        TmuxRenameOutcome::NameCollision => bail!(
            "tmux refused to rename `{}` to `{}`: name already in use",
            rename.mux.native_id,
            rename.new_name
        ),
        TmuxRenameOutcome::Unavailable(reason) => bail!("tmux unavailable: {}", reason.as_str()),
        TmuxRenameOutcome::Failed { code, message } => {
            bail!("tmux rename-session failed (exit code {code:?}): {message}")
        }
        TmuxRenameOutcome::Unsupported => bail!("tmux runner does not support rename_session"),
    }
}

fn node_kind_label(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::Pin(_) => "pin",
        NodeId::RuntimeProcess(_) => "runtime_process",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}
