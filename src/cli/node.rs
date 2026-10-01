//! `conspectus node` subcommand tree.
//!
//! Currently just `node show <id>` — resolves a node id
//! against a discovered snapshot and prints its detail
//! panel.

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::config;

use super::{
    ColorFlag, PagerOptions, print_paged, resolve_color_from_env, warm_start_discover_and_resolve,
};

#[derive(Debug, Args)]
pub(super) struct NodeArgs {
    #[command(subcommand)]
    command: NodeCommand,
}

impl NodeArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            NodeCommand::Show(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum NodeCommand {
    /// Print a single node, its candidate links, resolved relationships,
    /// source metadata, and any diagnostics touching it.
    Show(NodeShowArgs),
}

#[derive(Debug, Args)]
struct NodeShowArgs {
    /// Node id. Accepts the short content-addressed prefix from the
    /// session table's `ID` column, the full `NodeId` display form
    /// (e.g. `agent_session:codex:/state:session-x`), or the harness/mux
    /// label (e.g. `codex:session-x`, `tmux:editor`).
    id: String,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. See `conspectus table --help` for
    /// the resolution rules.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
    /// Don't write the rebuilt graph to the `graph.bin` cache.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// Ignore a running `conspectus serve` daemon and rebuild the
    /// graph in-process from live providers.
    #[arg(long = "refresh")]
    refresh: bool,
}

impl NodeShowArgs {
    fn run(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let loader = config::ConfigLoader::from_env();
        let outcome = loader.load_from(&cwd);
        for diagnostic in &outcome.diagnostics {
            eprintln!(
                "conspectus: warning: {}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }
        let roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots.clone()
        };
        let mut snapshot = warm_start_discover_and_resolve(
            roots,
            self.refresh,
            self.no_cache,
            &outcome.config.server.intervals,
        )?;
        crate::resolve::explain_resolved_relationships(&mut snapshot);
        let id = match crate::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let rendered = crate::output::node_show::render_node_show(&snapshot, &id, color);
        print_paged(
            &rendered,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}
