use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use conspectus::config;

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
            Command::Session(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the current work graph.
    Graph(GraphArgs),
    /// Render the resolved session table.
    Session(SessionArgs),
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

#[derive(Debug, Args, Default)]
struct SessionArgs {
    /// Table projection to render. Defaults to the value loaded from
    /// `.conspectus.toml` / user config, falling back to `agent`.
    #[arg(long, value_enum)]
    projection: Option<ProjectionFlag>,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl SessionArgs {
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

        let projection = self
            .projection
            .map(ProjectionFlag::into_config)
            .unwrap_or(outcome.config.session.projection);

        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let table = conspectus::output::table::render(&snapshot, projection);
        print!("{table}");
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ProjectionFlag {
    Agent,
    Mux,
    Union,
}

impl ProjectionFlag {
    fn into_config(self) -> config::Projection {
        match self {
            Self::Agent => config::Projection::Agent,
            Self::Mux => config::Projection::Mux,
            Self::Union => config::Projection::Union,
        }
    }
}
