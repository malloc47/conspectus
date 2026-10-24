//! `conspectus rebuild <ref>` (ADR 0142): bring a mux, session,
//! worktree, or repo back from a pin slot, a history record, or a node,
//! as a plan that `--dry-run` prints and a run executes step by step.

use std::io::IsTerminal;
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
    /// With `--dry-run`: plan a running mux, worktree, repo, or session
    /// as if it had stopped, from what is known now. Shows the commands
    /// that would recreate it.
    #[arg(long = "assume-stopped")]
    assume_stopped: bool,
    /// With `--dry-run`: print only the commands, as a script you can
    /// paste into a shell (`#` lines are comments).
    #[arg(long)]
    script: bool,
    /// Skip the terminal hand-off; print the attach command instead.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// For a mux: resume this session instead of the one it hosted last.
    #[arg(long, value_name = "ID")]
    session: Option<String>,
    /// Discovery scan root. Defaults to the working directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_root: Vec<PathBuf>,
}

impl RebuildArgs {
    pub(super) fn run(self) -> Result<()> {
        if (self.assume_stopped || self.script) && !self.dry_run {
            return Err(anyhow!(
                "`--assume-stopped` and `--script` only describe a plan; add `--dry-run`. \
                 A real rebuild never touches an entity that is running"
            ));
        }
        // Rebuild reads harness, mux, git, and process state and never
        // PRs, so it skips the forges (ADR 0135).
        let discovered = discover_for_store_selection_with(
            &self.scan_root,
            crate::discovery::LocalDiscoveryConfig::from_env().without_forge(),
        )?;
        let snapshot = crate::resolve::resolve_snapshot(discovered);
        super::pin::report_store_problems(&snapshot);
        // The graph's slot list includes unconverted legacy pins, so a
        // rebuild of one can say to migrate instead of "nothing matches".
        let slots = snapshot.pins.clone();
        let subject = resolve_subject(&snapshot, &slots, &self.reference).map_err(describe)?;

        let config = crate::config::load_from_cwd().config;
        let history = crate::history::HistoryWriter::from_env(config.history.clone());
        let worktree = resolve_mutation_backend(config.worktree.backend, worktrunk_available())?;
        let slot_store = SlotStore::from_env();
        let launch = crate::launch_default::LaunchDefaults::new(config.launch);
        let backend_for = |backend: &str| system_mux_backend(backend);
        let env = Env {
            backend_for: &backend_for,
            history: history.as_ref(),
            resume_watch: RESUME_WATCH,
            worktree: worktree.as_deref(),
            slots: slot_store.as_ref(),
            cloner: &SystemCloner,
            launch: &launch,
        };
        // Without a terminal there is nothing to hand over to; say how
        // to attach instead of failing after the session is made.
        let attach_here = !self.no_attach && std::io::stdin().is_terminal();
        let options = Options {
            session: self.session,
            // A plan shows the attach it would end with; a run only
            // attaches when there is a terminal to hand over.
            attach: !self.no_attach && (self.dry_run || attach_here),
            assume_stopped: self.assume_stopped,
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
            if self.script {
                print!("{}", plan.script());
            } else {
                print!("{}", plan.render());
            }
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
        if !attach_here
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
