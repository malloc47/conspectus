//! `conspectus rebuild <ref>` (ADR 0142): bring a mux, session,
//! worktree, or repo back from a pin slot, a history record, or a node,
//! as a plan that `--dry-run` prints and a run executes step by step.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

use crate::discovery::system_mux_backend;
use crate::discovery::worktree::{resolve_mutation_backend, worktrunk_available};
use crate::pin_slots::SlotStore;
use crate::rebuild::launch::{RESUME_WATCH, format_attach_command};
use crate::rebuild::{Env, Options, RebuildError, SystemCloner, execute, plan, resolve_subject};

use super::discover_for_store_selection_with;

#[derive(Debug, Args)]
pub(super) struct RebuildArgs {
    /// What to rebuild: a pin slot name, a history record id (`h_…`),
    /// a node id, or a mux name.
    #[arg(value_name = "REF")]
    reference: String,
    /// Print the plan and stop.
    #[arg(long = "dry-run")]
    dry_run: bool,
    /// Skip the terminal hand-off; print the attach command instead.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Resume this session instead of the one the mux hosted last.
    #[arg(long, value_name = "ID")]
    session: Option<String>,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_root: Vec<PathBuf>,
}

impl RebuildArgs {
    pub(super) fn run(self) -> Result<()> {
        // Rebuild reads harness, mux, git, and process state and never
        // PRs, so it skips the forges (ADR 0135).
        let discovered = discover_for_store_selection_with(
            &self.scan_root,
            crate::discovery::LocalDiscoveryConfig::from_env().without_forge(),
        )?;
        let snapshot = crate::resolve::resolve_snapshot(discovered);
        let slots = SlotStore::from_env()
            .map(|store| store.load().slots)
            .unwrap_or_default();
        let subject = resolve_subject(&snapshot, &slots, &self.reference).map_err(describe)?;

        let config = crate::config::load_from_cwd().config;
        let history = crate::history::HistoryWriter::from_env(config.history.clone());
        let worktree = resolve_mutation_backend(config.worktree.backend, worktrunk_available())?;
        let slot_store = SlotStore::from_env();
        let backend_for = |backend: &str| system_mux_backend(backend);
        let env = Env {
            backend_for: &backend_for,
            history: history.as_ref(),
            resume_watch: RESUME_WATCH,
            worktree: worktree.as_deref(),
            slots: slot_store.as_ref(),
            cloner: &SystemCloner,
        };
        let options = Options {
            session: self.session,
            attach: !self.no_attach && !self.dry_run,
        };
        let plan = plan(
            &snapshot,
            &subject,
            &options,
            &env,
            crate::discovery::current_epoch(),
        )
        .map_err(describe)?;

        if self.dry_run {
            print!("{}", plan.render());
            return Ok(());
        }
        for warning in &plan.warnings {
            eprintln!("conspectus: {warning}");
        }
        let executed = execute(&plan, &env).map_err(|failure| anyhow!("{failure}"))?;
        for note in &executed.notes {
            eprintln!("conspectus: {note}");
        }
        for line in &executed.done {
            println!("{line}");
        }
        if self.no_attach
            && let Some(target) = &plan.target
            && plan.creates_something()
        {
            println!(
                "attach with: {}",
                format_attach_command(target.socket.as_deref(), &target.name)
            );
        }
        Ok(())
    }
}

fn describe(err: RebuildError) -> anyhow::Error {
    anyhow!("{err}")
}
