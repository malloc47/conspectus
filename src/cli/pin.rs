//! `conspectus pin` subcommand tree (ADR 0141).
//!
//! A pin is a named slot in `$XDG_DATA_HOME/conspectus/pins.toml` that
//! points at a mux, session, checkout, repo, or PR and can carry
//! overrides. Pinning writes only that file (plus a history record when
//! the entity has none yet); bringing a gone entity back is
//! `conspectus rebuild`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use crate::config::ConfigLoader;
use crate::discovery::{DiscoveryContext, LocalDiscoveryConfig};
use crate::history::HistoryWriter;
use crate::legacy_bindings::PinBindingsCache;
use crate::legacy_registry::PinStoreRegistry;
use crate::model::{
    GraphNode, GraphSnapshot, NodeId, PinId, PinKind, PinNode, SlotFields, SlotStatus,
};
use crate::pin_ops;
use crate::pin_slots::{Placement, SlotStore};

use super::{discover_for_store_selection_with, effective_scan_roots};

/// Discovery for the pin commands: no forge calls, which pinning,
/// listing, and migrating never need (ADR 0135).
pub(super) fn pin_discovery_config() -> LocalDiscoveryConfig {
    LocalDiscoveryConfig::from_env().without_forge()
}

#[derive(Debug, Args)]
pub struct PinArgs {
    #[command(subcommand)]
    command: PinCommand,
}

impl PinArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            PinCommand::Add(args) => args.run(),
            PinCommand::Edit(args) => args.run(),
            PinCommand::Rename(args) => args.run(),
            PinCommand::Retarget(args) => args.run(),
            PinCommand::Refresh(args) => args.run(),
            PinCommand::Move(args) => args.run(),
            PinCommand::Rm(args) => args.run(),
            PinCommand::List(args) => args.run(),
            PinCommand::Show(args) => args.run(),
            PinCommand::Migrate(args) => args.run(),
            PinCommand::Create(args) => args.run("create"),
            PinCommand::Launch(args) => args.run("launch"),
            PinCommand::Attach(args) => args.run("attach"),
            PinCommand::Bind(args) => args.run("bind"),
            PinCommand::Rebind(args) => args.run("rebind"),
            PinCommand::Adopt(args) => args.run("adopt"),
        }
    }
}

#[derive(Debug, Subcommand)]
enum PinCommand {
    /// Pin a mux, session, checkout, repo, branch, or PR as a named slot.
    Add(PinAddArgs),
    /// Set or clear a slot's overrides.
    Edit(PinEditArgs),
    /// Rename a slot.
    Rename(PinRenameArgs),
    /// Point a slot at another entity of the same kind.
    Retarget(PinRetargetArgs),
    /// Copy a slot's current recipe into its snapshot.
    Refresh(PinRefreshArgs),
    /// Reorder a slot in the Pinned view.
    Move(PinMoveArgs),
    /// Remove a slot. The entity and its history are left alone.
    Rm(PinRmArgs),
    /// List the slots.
    List(PinListArgs),
    /// Show one slot: target, status, and where each field comes from.
    Show(PinShowArgs),
    /// Convert legacy `[[pins.entries]]` into slots.
    Migrate(PinMigrateArgs),

    /// Retired: use `pin add`.
    #[command(hide = true)]
    Create(Retired),
    /// Retired: use `conspectus rebuild <pin>`.
    #[command(hide = true)]
    Launch(Retired),
    /// Retired: use `conspectus rebuild <pin>`.
    #[command(hide = true)]
    Attach(Retired),
    /// Retired: slots bind by incarnation; use `pin retarget`.
    #[command(hide = true)]
    Bind(Retired),
    /// Retired: renames follow automatically; use `pin retarget`.
    #[command(hide = true)]
    Rebind(Retired),
    /// Retired: use `pin add <mux>`.
    #[command(hide = true)]
    Adopt(Retired),
}

/// A verb from the old pin model. It still parses, so the operator
/// gets the replacement instead of a usage error.
#[derive(Debug, Args)]
struct Retired {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _rest: Vec<String>,
}

impl Retired {
    fn run(self, verb: &str) -> Result<()> {
        let instead = match verb {
            "create" => "`conspectus pin add <mux|node id|h_…>`",
            "launch" => "`conspectus rebuild <pin>` (or `conspectus mux launch`)",
            "attach" => "`conspectus rebuild <pin>`",
            "bind" => "`conspectus pin retarget <pin> <session or history id>`",
            "rebind" => {
                "`conspectus pin retarget <pin> <mux>` (a pin already follows its mux through renames)"
            }
            _ => "`conspectus pin add <mux>`",
        };
        bail!(
            "`conspectus pin {verb}` was retired; use {instead}. \
             Convert existing pins with `conspectus pin migrate`."
        )
    }
}

fn slot_store() -> Result<SlotStore> {
    SlotStore::from_env()
        .ok_or_else(|| anyhow!("no data home: set $XDG_DATA_HOME or $HOME to store pins"))
}

pub(super) fn discover(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let discovered = discover_for_store_selection_with(scan_roots, pin_discovery_config())?;
    let snapshot = crate::resolve::resolve_snapshot(discovered);
    report_store_problems(&snapshot);
    Ok(snapshot)
}

/// Say on stderr why `pins.toml` or the history files couldn't be read
/// (a newer schema, bad TOML or JSON), once each, so an empty list isn't
/// mistaken for no data.
pub(super) fn report_store_problems(snapshot: &GraphSnapshot) {
    let mut seen = BTreeSet::new();
    for diagnostic in &snapshot.diagnostics {
        if let crate::model::Diagnostic::Config { path, message } = diagnostic
            && (path == "history" || path.ends_with("pins.toml"))
            && seen.insert(message.clone())
        {
            eprintln!("conspectus: {message}");
        }
    }
}

fn history_writer() -> Option<HistoryWriter> {
    let config = crate::config::load_from_cwd().config;
    HistoryWriter::from_env(config.history)
}

fn parse_assignments(
    kind: PinKind,
    assignments: &[String],
) -> Result<Vec<(String, crate::model::FieldValue)>> {
    assignments
        .iter()
        .map(|assignment| pin_ops::parse_override(kind, assignment).map_err(|e| anyhow!("{e}")))
        .collect()
}

fn pinned_node<'a>(snapshot: &'a GraphSnapshot, id: &str) -> Result<&'a PinNode> {
    snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            GraphNode::Pin(pin) if pin.id.id == id => Some(pin),
            _ => None,
        })
        .ok_or_else(|| anyhow!("no pin named `{id}`"))
}

/// Refuse to write a slot that only exists as an unconverted legacy
/// entry.
fn refuse_legacy(snapshot: &GraphSnapshot, id: &str) -> Result<()> {
    if pinned_node(snapshot, id)?.legacy {
        bail!(legacy_message(id));
    }
    Ok(())
}

fn legacy_message(id: &str) -> String {
    format!(
        "`{id}` is a legacy pin that hasn't been converted yet; run `conspectus pin migrate` first"
    )
}

/// The error for a slot command that found no slot called `name`: a
/// legacy pin of that name gets the migrate hint instead.
fn missing_slot(name: &str, err: anyhow::Error) -> anyhow::Error {
    if let Ok(snapshot) = discover(&[])
        && pinned_node(&snapshot, name).is_ok_and(|pin| pin.legacy)
    {
        return anyhow!(legacy_message(name));
    }
    err
}

#[derive(Debug, Args)]
struct PinAddArgs {
    /// What to pin: a node id, a history id (`h_…`), or a mux name.
    #[arg(value_name = "TARGET", required_unless_present = "planned")]
    target: Option<String>,
    /// Slot name. Defaults to the entity's name.
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
    /// Create a slot with no target, built by its first rebuild. Needs
    /// `--kind`.
    #[arg(long, requires = "kind", conflicts_with = "target")]
    planned: bool,
    /// Kind of a planned slot: mux, session, checkout (alias worktree),
    /// repo, branch, pr, workspace, or fork.
    #[arg(long, value_name = "KIND")]
    kind: Option<String>,
    /// Override a recipe field: `field=value`. Repeatable.
    #[arg(long = "set", value_name = "FIELD=VALUE")]
    set: Vec<String>,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinAddArgs {
    fn run(self) -> Result<()> {
        let store = slot_store()?;
        if self.planned {
            let name = self
                .name
                .as_deref()
                .ok_or_else(|| anyhow!("a planned pin needs `--name`"))?;
            let kind = self.kind.as_deref().unwrap_or_default();
            let kind = PinKind::parse(kind)
                .ok_or_else(|| anyhow!("unknown kind `{kind}` ({})", PinKind::names()))?;
            let mut overrides = SlotFields::new();
            for (field, value) in parse_assignments(kind, &self.set)? {
                overrides.insert(field, value);
            }
            pin_ops::pin_planned(&store, name, kind, overrides).map_err(|e| anyhow!("{e}"))?;
            println!("pinned planned {} `{name}`", kind.as_str());
            return Ok(());
        }

        let target = self.target.as_deref().unwrap_or_default();
        let snapshot = discover(&self.scan_roots)?;
        let what = pin_ops::find_pinnable(&snapshot, target).map_err(|e| {
            if pinned_node(&snapshot, target).is_ok_and(|pin| pin.legacy) {
                anyhow!(legacy_message(target))
            } else {
                anyhow!("{e}")
            }
        })?;
        let history = history_writer();
        let slot = pin_ops::pin(
            &snapshot,
            &store,
            history.as_ref(),
            &what,
            self.name.as_deref(),
        )
        .map_err(|e| anyhow!("{e}"))?;
        if !self.set.is_empty() {
            let set = parse_assignments(slot.kind, &self.set)?;
            pin_ops::edit(&store, &slot.id, &set, &[]).map_err(|e| anyhow!("{e}"))?;
        }
        println!(
            "pinned {} `{}` as `{}`",
            slot.kind.as_str(),
            target,
            slot.id
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinEditArgs {
    /// Slot name.
    name: String,
    /// Override a recipe field: `field=value`. Repeatable.
    #[arg(long = "set", value_name = "FIELD=VALUE")]
    set: Vec<String>,
    /// Clear an override. Repeatable.
    #[arg(long = "unset", value_name = "FIELD")]
    unset: Vec<String>,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinEditArgs {
    fn run(self) -> Result<()> {
        if self.set.is_empty() && self.unset.is_empty() {
            bail!("`pin edit` needs `--set FIELD=VALUE` or `--unset FIELD`");
        }
        let store = slot_store()?;
        let snapshot = discover(&self.scan_roots)?;
        refuse_legacy(&snapshot, &self.name)?;
        let kind = pinned_node(&snapshot, &self.name)?.kind;
        let set = parse_assignments(kind, &self.set)?;
        let changed =
            pin_ops::edit(&store, &self.name, &set, &self.unset).map_err(|e| anyhow!("{e}"))?;
        println!(
            "{} `{}`",
            if changed { "edited" } else { "unchanged" },
            self.name
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRenameArgs {
    /// Current slot name.
    name: String,
    /// New slot name.
    new_name: String,
    /// Rename the slot only; leave a live mux slot's tmux session as it
    /// is.
    #[arg(long = "no-mux")]
    no_mux: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRenameArgs {
    fn run(self) -> Result<()> {
        let store = slot_store()?;
        if !self.no_mux {
            // A live mux slot renames its tmux session too, and the
            // plan sets the slot's `name` override so it keeps following
            // the mux (ADR 0029).
            let snapshot = discover(&self.scan_roots)?;
            refuse_legacy(&snapshot, &self.name)?;
            let pin = pinned_node(&snapshot, &self.name)?;
            if let (PinKind::Mux, SlotStatus::Live, Some(NodeId::MuxSession(mux))) =
                (pin.kind, pin.status, pin.target_node.as_ref())
            {
                let plan = crate::rename::plan_mux_rename(&snapshot, mux, self.new_name.clone())
                    .map_err(|err| anyhow!("mux rename planning failed: {err}"))?;
                super::rename::execute_mux_rename_plan(
                    &plan,
                    &crate::discovery::tmux::SystemTmux::new(),
                )?;
                // The plan only rewrites an override that already
                // exists; a slot renamed on purpose pins the name.
                let name = (
                    "name".to_string(),
                    crate::model::FieldValue::Text(self.new_name.clone()),
                );
                pin_ops::edit(&store, &self.name, &[name], &[]).map_err(|e| anyhow!("{e}"))?;
            }
        }
        let changed = pin_ops::rename(&store, &self.name, &self.new_name)
            .map_err(|e| missing_slot(&self.name, anyhow!("{e}")))?;
        println!(
            "{} `{}` → `{}`",
            if changed { "renamed" } else { "unchanged" },
            self.name,
            self.new_name
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRetargetArgs {
    /// Slot name.
    name: String,
    /// The new target: a node id, a history id (`h_…`), or a mux name.
    target: String,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRetargetArgs {
    fn run(self) -> Result<()> {
        let store = slot_store()?;
        let snapshot = discover(&self.scan_roots)?;
        refuse_legacy(&snapshot, &self.name)?;
        let what = pin_ops::find_pinnable(&snapshot, &self.target).map_err(|e| anyhow!("{e}"))?;
        let history = history_writer();
        pin_ops::retarget(&snapshot, &store, history.as_ref(), &self.name, &what)
            .map_err(|e| anyhow!("{e}"))?;
        println!("`{}` now points at `{}`", self.name, self.target);
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRefreshArgs {
    /// Slot name.
    name: String,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRefreshArgs {
    fn run(self) -> Result<()> {
        let store = slot_store()?;
        let snapshot = discover(&self.scan_roots)?;
        refuse_legacy(&snapshot, &self.name)?;
        let changed =
            pin_ops::refresh_snapshot(&snapshot, &store, &self.name).map_err(|e| anyhow!("{e}"))?;
        println!(
            "{} the snapshot of `{}`",
            if changed { "refreshed" } else { "no change to" },
            self.name
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinMoveArgs {
    /// Slot name.
    name: String,
    /// Move to the top.
    #[arg(long, conflicts_with_all = ["bottom", "before", "after", "up", "down"])]
    top: bool,
    /// Move to the bottom.
    #[arg(long, conflicts_with_all = ["before", "after", "up", "down"])]
    bottom: bool,
    /// Move just before this slot.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["after", "up", "down"])]
    before: Option<String>,
    /// Move just after this slot.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["up", "down"])]
    after: Option<String>,
    /// Move one place up.
    #[arg(long, conflicts_with = "down")]
    up: bool,
    /// Move one place down.
    #[arg(long)]
    down: bool,
}

impl PinMoveArgs {
    fn run(self) -> Result<()> {
        let placement = if self.top {
            Placement::Top
        } else if self.bottom {
            Placement::Bottom
        } else if let Some(other) = self.before {
            Placement::Before(other)
        } else if let Some(other) = self.after {
            Placement::After(other)
        } else if self.up {
            Placement::Step(-1)
        } else if self.down {
            Placement::Step(1)
        } else {
            bail!("`pin move` needs one of --top, --bottom, --before, --after, --up, --down");
        };
        let write = slot_store()?
            .move_slot(&self.name, &placement)
            .map_err(|e| missing_slot(&self.name, anyhow!("{e}")))?;
        println!(
            "{} `{}`",
            if write.changed { "moved" } else { "unchanged" },
            self.name
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRmArgs {
    /// Slot name.
    name: String,
}

impl PinRmArgs {
    fn run(self) -> Result<()> {
        let write = slot_store()?
            .remove(&self.name)
            .map_err(|e| missing_slot(&self.name, anyhow!("{e}")))?;
        if !write.changed {
            return Err(missing_slot(
                &self.name,
                anyhow!("no pin named `{}`", self.name),
            ));
        }
        println!("removed pin `{}`", self.name);
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinListArgs {
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinListArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        let mut pins: Vec<&PinNode> = snapshot
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::Pin(pin) => Some(pin),
                _ => None,
            })
            .collect();
        pins.sort_by_key(|pin| pin.order);
        for advice in snapshot
            .diagnostics
            .iter()
            .filter_map(crate::model::Diagnostic::advice)
        {
            eprintln!("conspectus: {advice}");
        }
        if pins.is_empty() {
            println!("no pins; `conspectus pin add <target>` makes one");
            return Ok(());
        }
        let width = pins.iter().map(|pin| pin.id.id.len()).max().unwrap_or(0);
        for pin in pins {
            println!(
                "{:<width$}  {:<8}  {:<8}  {}{}",
                pin.id.id,
                pin.kind.as_str(),
                pin.status.label_for(pin.kind),
                pin.label,
                if pin.legacy {
                    "  (legacy: run `pin migrate`)"
                } else {
                    ""
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinShowArgs {
    /// Slot name.
    name: String,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinShowArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        pinned_node(&snapshot, &self.name)?;
        let id = NodeId::Pin(PinId::new(&self.name));
        print!(
            "{}",
            crate::output::node_show::render_node_show(&snapshot, &id, false)
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinMigrateArgs {
    /// Print what would change and stop.
    #[arg(long = "dry-run")]
    dry_run: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinMigrateArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        let cwd = crate::cwd::current();
        let roots = effective_scan_roots(&self.scan_roots, cwd.as_deref());
        let context = DiscoveryContext::from_roots(roots)?;
        let loader = ConfigLoader::from_env();
        let registry = PinStoreRegistry::from_env().read();
        let mut diagnostics = Vec::new();
        let entries = crate::discovery::pins::legacy_entries(
            &snapshot,
            &context,
            &loader,
            &registry,
            &mut diagnostics,
        );
        // A config file that can't be read or parsed hides every pin in
        // it; say which, and fail at the end so the operator notices.
        let mut unreadable = 0usize;
        for diagnostic in &diagnostics {
            if let crate::model::Diagnostic::Config { path, message } = diagnostic
                && (message.starts_with("failed to parse pins")
                    || message.starts_with("failed to read pins"))
            {
                unreadable += 1;
                eprintln!("conspectus: {path}: {message}; its pins weren't migrated");
            }
        }
        let store = slot_store()?;
        let existing = store.load().slots;
        let plan =
            crate::pin_migrate::plan(&snapshot, entries, &existing, &PinBindingsCache::from_env());
        for warning in &plan.warnings {
            eprintln!("conspectus: {warning}");
        }
        let unreadable_error = || {
            anyhow!(
                "{unreadable} legacy config file(s) couldn't be read, so their pins weren't migrated"
            )
        };
        if plan.is_empty() {
            if unreadable > 0 {
                return Err(unreadable_error());
            }
            println!("no legacy pins to migrate");
            return Ok(());
        }
        if self.dry_run {
            print!("{}", plan.render());
            return if unreadable > 0 {
                Err(unreadable_error())
            } else {
                Ok(())
            };
        }
        for (id, why) in &plan.skipped {
            eprintln!("conspectus: pin `{id}` skipped: {why}");
        }
        let history = history_writer()
            .ok_or_else(|| anyhow!("no state home: history is needed to migrate pins"))?;
        let user_config = loader.user_config_path();
        let report = crate::pin_migrate::run(
            &plan,
            &crate::pin_migrate::Targets {
                slots: &store,
                history: &history,
                user_config: user_config.as_deref(),
                fallback_epoch: crate::discovery::current_epoch(),
            },
        );
        println!(
            "migrated {} pin(s); removed {} legacy entr{}",
            report.migrated.len(),
            report.removed_entries,
            if report.removed_entries == 1 {
                "y"
            } else {
                "ies"
            },
        );
        for alias in &report.aliases {
            println!("kept alias {alias}");
        }
        for note in &report.notes {
            eprintln!("conspectus: {note}");
        }
        if report.failed.is_empty() {
            if unreadable > 0 {
                return Err(unreadable_error());
            }
            Ok(())
        } else {
            for (id, why) in &report.failed {
                eprintln!("conspectus: pin `{id}` not migrated: {why}");
            }
            bail!("{} pin(s) could not be migrated", report.failed.len())
        }
    }
}
