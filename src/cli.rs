use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "conspectus", version, about = "AI work graph status tool")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command.unwrap_or(Command::Graph(GraphArgs::default())) {
            Command::Graph(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the current work graph.
    Graph(GraphArgs),
}

#[derive(Debug, Args)]
struct GraphArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl Default for GraphArgs {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            scan_roots: Vec::new(),
        }
    }
}

impl GraphArgs {
    fn run(self) -> Result<()> {
        match self.format {
            OutputFormat::Json => {
                let snapshot = if self.scan_roots.is_empty() {
                    conspectus::discovery::discover_local_at_roots([std::env::current_dir()?])?
                } else {
                    conspectus::discovery::discover_local_at_roots(self.scan_roots)?
                };
                let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
                println!("{}", conspectus::output::render_graph_json(&snapshot)?);
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}
