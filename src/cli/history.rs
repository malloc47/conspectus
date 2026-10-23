//! `conspectus history` (ADR 0140): list, inspect, and prune the
//! entities Conspectus has observed, gone ones included.
//!
//! Everything here reads the history store through a discovery run, so
//! presence is the same targeted check the views use. Only `forget` and
//! `reap` write, and they remove records; they never touch tmux, git, or
//! the filesystem.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand, ValueEnum};

use crate::history::HistoryStore;
use crate::history::reap::{self, Candidate};
use crate::model::{GraphSnapshot, HistoryKind, NodePresence};
use crate::output::render::format_relative_age;

use super::pin::discover;

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[command(subcommand)]
    command: HistoryCommand,
}

impl HistoryArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            HistoryCommand::List(args) => args.run(),
            HistoryCommand::Show(args) => args.run(),
            HistoryCommand::Reap(args) => args.run(),
            HistoryCommand::Forget(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum HistoryCommand {
    /// List recorded entities with their presence.
    List(HistoryListArgs),
    /// Show one record: its recipe and what points at it.
    Show(HistoryShowArgs),
    /// Remove gone records past their `[history.reap_after]` threshold.
    Reap(HistoryReapArgs),
    /// Remove one record.
    Forget(HistoryForgetArgs),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum KindFlag {
    Mux,
    Checkout,
    Repo,
    Pr,
}

impl From<KindFlag> for HistoryKind {
    fn from(flag: KindFlag) -> Self {
        match flag {
            KindFlag::Mux => Self::Mux,
            KindFlag::Checkout => Self::Checkout,
            KindFlag::Repo => Self::Repo,
            KindFlag::Pr => Self::Pr,
        }
    }
}

fn store() -> Result<HistoryStore> {
    HistoryStore::from_env().ok_or_else(|| anyhow!("no state home: set $XDG_STATE_HOME or $HOME"))
}

fn presence_label(snapshot: &GraphSnapshot, id: &str) -> &'static str {
    let Some((kind, _)) = snapshot.history.meta(id) else {
        return "live";
    };
    snapshot
        .history
        .presence_of(id)
        .map_or("live", |presence| presence.label_for(kind.node_kind()))
}

/// The slots whose target is this record, by name.
fn pinned_by(snapshot: &GraphSnapshot, id: &str, key: &str) -> Vec<String> {
    let history_target = crate::model::PinSlot::history_target(id);
    snapshot
        .pins
        .iter()
        .filter(|slot| {
            slot.target
                .as_deref()
                .is_some_and(|target| target == history_target || target == key)
        })
        .map(|slot| slot.id.clone())
        .collect()
}

#[derive(Debug, Args)]
struct HistoryListArgs {
    /// Only this kind.
    #[arg(long, value_enum)]
    kind: Option<KindFlag>,
    /// Only records whose entity is gone.
    #[arg(long)]
    gone: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl HistoryListArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        let now = crate::discovery::current_epoch();
        let wanted: Option<HistoryKind> = self.kind.map(Into::into);
        let mut rows: Vec<_> = snapshot
            .history
            .metas()
            .filter(|(kind, meta)| {
                wanted.is_none_or(|w| w == *kind)
                    && (!self.gone
                        || snapshot
                            .history
                            .presence_of(&meta.id)
                            .is_some_and(NodePresence::is_gone))
            })
            .collect();
        rows.sort_by_key(|(_, meta)| std::cmp::Reverse(meta.last_seen));
        if rows.is_empty() {
            println!("no history records");
            return Ok(());
        }
        for (kind, meta) in rows {
            let pins = pinned_by(&snapshot, &meta.id, &meta.key);
            println!(
                "{}  {:<8}  {:<7}  {:>4}  {}{}{}",
                meta.id,
                kind.as_str(),
                presence_label(&snapshot, &meta.id),
                format_relative_age(meta.last_seen, now),
                meta.display.label,
                meta.display
                    .context
                    .as_deref()
                    .map_or_else(String::new, |c| format!("  ({c})")),
                if pins.is_empty() {
                    String::new()
                } else {
                    // The filled pin marks a running entity, the hollow
                    // one a stopped or removed one, as in the TUI.
                    let marker = if snapshot.history.presence_of(&meta.id).is_none() {
                        crate::tui::icons::state::PIN_RUNNING
                    } else {
                        crate::tui::icons::state::PIN_STOPPED
                    };
                    format!("  {marker} {}", pins.join(", "))
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct HistoryShowArgs {
    /// Record id (`h_…`).
    id: String,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl HistoryShowArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        let history = &snapshot.history;
        let (kind, meta) = history
            .meta(&self.id)
            .ok_or_else(|| anyhow!("no history record `{}`", self.id))?;
        let record = match kind {
            HistoryKind::Mux => serde_json::to_value(history.mux(&self.id)),
            HistoryKind::Checkout => serde_json::to_value(history.checkout(&self.id)),
            HistoryKind::Repo => serde_json::to_value(history.repo(&self.id)),
            HistoryKind::Pr => serde_json::to_value(history.pr(&self.id)),
        }?;
        println!("{} {}", kind.as_str(), presence_label(&snapshot, &self.id));
        println!("{}", serde_json::to_string_pretty(&record)?);
        let pins = pinned_by(&snapshot, &meta.id, &meta.key);
        if !pins.is_empty() {
            println!("pinned by: {}", pins.join(", "));
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct HistoryReapArgs {
    /// Only this kind.
    #[arg(long, value_enum)]
    kind: Option<KindFlag>,
    /// Use this age instead of `[history.reap_after]` (`30d`, `12h`).
    #[arg(long = "older-than", value_name = "DURATION")]
    older_than: Option<String>,
    /// List what would go and stop.
    #[arg(long = "dry-run")]
    dry_run: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl HistoryReapArgs {
    fn run(self) -> Result<()> {
        let older_than = self
            .older_than
            .as_deref()
            .map(crate::config::parse_duration_short)
            .transpose()
            .map_err(|err| anyhow!("--older-than: {err}"))?;
        let snapshot = discover(&self.scan_roots)?;
        let config = crate::config::load_from_cwd().config.history;
        let now = crate::discovery::current_epoch();
        let found = reap::candidates(
            &snapshot,
            &config,
            now,
            self.kind.map(Into::into),
            older_than,
        );
        if found.is_empty() {
            println!("nothing to reap");
            return Ok(());
        }
        for Candidate {
            kind,
            id,
            label,
            last_seen,
        } in &found
        {
            println!(
                "{id}  {:<8}  last seen {} ago  {label}",
                kind.as_str(),
                format_relative_age(*last_seen, now)
            );
        }
        if self.dry_run {
            println!("{} record(s) would be reaped", found.len());
            return Ok(());
        }
        let ids: BTreeSet<String> = found.into_iter().map(|c| c.id).collect();
        let removed = reap::reap(&store()?, &ids)?;
        println!("reaped {removed} record(s)");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct HistoryForgetArgs {
    /// Record id (`h_…`).
    id: String,
    /// Forget it even though a pin slot targets it.
    #[arg(long)]
    force: bool,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl HistoryForgetArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover(&self.scan_roots)?;
        let (_, meta) = snapshot
            .history
            .meta(&self.id)
            .ok_or_else(|| anyhow!("no history record `{}`", self.id))?;
        let pins = pinned_by(&snapshot, &meta.id, &meta.key);
        if !pins.is_empty() && !self.force {
            bail!(
                "pin(s) {} target this record; `pin rm` them first or pass --force",
                pins.join(", ")
            );
        }
        let ids = BTreeSet::from([self.id.clone()]);
        let removed = reap::reap(&store()?, &ids)?;
        if removed == 0 {
            bail!("`{}` was already gone from the history store", self.id);
        }
        println!("forgot {}", self.id);
        Ok(())
    }
}
