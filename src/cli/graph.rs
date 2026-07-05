//! `conspectus graph` command (H-REF-006 wave 6).
//!
//! Emits the resolved graph as JSON, DOT, or HTML.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use conspectus::config;

use super::{InclusionFlag, OutputFormat, warm_start_discover_and_resolve};

#[derive(Debug, Args)]
pub(super) struct GraphArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// DOT/HTML only: include or exclude non-resolved candidate
    /// links (and their unresolved-endpoint stubs). Defaults to
    /// include (ADR 0050).
    #[arg(long, value_enum, default_value_t = InclusionFlag::Include)]
    candidates: InclusionFlag,
    /// DOT/HTML only: include or exclude RuntimeProcess diagnostic
    /// nodes. Defaults to include (ADR 0050).
    #[arg(long = "diagnostic-nodes", value_enum, default_value_t = InclusionFlag::Include)]
    diagnostic_nodes: InclusionFlag,
    /// Include resolver score breakdowns on resolved relationships.
    #[arg(long)]
    explain: bool,
    /// P7-003 phase 4: suppress the writer for this invocation.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// P7-003 phase 4: skip the warm-start read so this run scans
    /// every provider cold.
    #[arg(long = "refresh")]
    refresh: bool,
}

impl Default for GraphArgs {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            scan_roots: Vec::new(),
            candidates: InclusionFlag::Include,
            diagnostic_nodes: InclusionFlag::Include,
            explain: false,
            no_cache: false,
            refresh: false,
        }
    }
}

impl GraphArgs {
    pub(super) fn run(self) -> Result<()> {
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
        if self.explain {
            conspectus::resolve::explain_resolved_relationships(&mut snapshot);
        }

        match self.format {
            OutputFormat::Json => {
                println!("{}", conspectus::output::render_graph_json(&snapshot)?);
            }
            OutputFormat::Dot => {
                let opts = conspectus::output::DotOptions {
                    candidates: self.candidates.into(),
                    diagnostic_nodes: self.diagnostic_nodes.into(),
                };
                println!("{}", conspectus::output::render_graph_dot(&snapshot, opts)?);
            }
            OutputFormat::Html => {
                let opts = conspectus::output::HtmlOptions {
                    candidates: self.candidates.into(),
                    diagnostic_nodes: self.diagnostic_nodes.into(),
                };
                print!(
                    "{}",
                    conspectus::output::render_graph_html(&snapshot, opts)?
                );
            }
        }

        Ok(())
    }
}
