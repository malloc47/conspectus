use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

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
}

impl Default for GraphArgs {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
        }
    }
}

impl GraphArgs {
    fn run(self) -> Result<()> {
        match self.format {
            OutputFormat::Json => {
                let snapshot =
                    conspectus::resolve::resolve_snapshot(conspectus::discovery::empty_graph());
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
