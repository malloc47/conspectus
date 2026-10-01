//! `conspectus dev` subcommand tree.
//!
//! Debug-only scenario materialization + rendering used by
//! `dev scenario list / graph / table / node / tui`. Gated
//! at `#[cfg(debug_assertions)]` per pre-H-REF-006 shape;
//! release builds compile without this module.
//!
//! Shared surface reached back through `super`:
//! `ColorFlag`, `LayoutFlag`, `OutputFormat`, `InclusionFlag`,
//! `ViewFlag`, `SortFlag`, `FilterArgs`,
//! `resolve_color_from_env`, `view_from_flag`,
//! `apply_grouping_to_tui_config`.

#![cfg(debug_assertions)]

use std::io::{self, IsTerminal};

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};

use conspectus::config;

use super::{
    ColorFlag, FilterArgs, InclusionFlag, LayoutFlag, OutputFormat, SortFlag, ViewFlag,
    apply_grouping_to_tui_config, resolve_color_from_env, view_from_flag,
};

#[derive(Debug, Args)]
pub(super) struct DevArgs {
    #[command(subcommand)]
    command: DevCommand,
}

impl DevArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            DevCommand::Scenario(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Materialize and inspect named replay scenarios.
    Scenario(DevScenarioArgs),
}

#[derive(Debug, Args)]
struct DevScenarioArgs {
    #[command(subcommand)]
    command: DevScenarioCommand,
}

impl DevScenarioArgs {
    fn run(self) -> Result<()> {
        match self.command {
            DevScenarioCommand::List => {
                for scenario in conspectus::dev_scenarios::SCENARIOS {
                    println!("{}\t{}", scenario.name, scenario.description);
                }
                Ok(())
            }
            DevScenarioCommand::Graph(args) => args.run(),
            DevScenarioCommand::Table(args) => args.run(),
            DevScenarioCommand::Node(args) => args.run(),
            DevScenarioCommand::Tui(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum DevScenarioCommand {
    /// List available scenario names.
    List,
    /// Render resolved graph JSON for a scenario.
    Graph(DevScenarioGraphArgs),
    /// Render a table row-type for a scenario.
    Table(DevScenarioTableArgs),
    /// Show one node from a scenario.
    Node(DevScenarioNodeArgs),
    /// Open the interactive TUI on a static scenario graph.
    Tui(DevScenarioTuiArgs),
}

#[derive(Debug, Args)]
struct DevScenarioGraphArgs {
    name: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long, value_enum, default_value_t = InclusionFlag::Include)]
    candidates: InclusionFlag,
    #[arg(long = "diagnostic-nodes", value_enum, default_value_t = InclusionFlag::Include)]
    diagnostic_nodes: InclusionFlag,
}

impl DevScenarioGraphArgs {
    fn run(self) -> Result<()> {
        let world = conspectus::dev_scenarios::materialize(&self.name)?;
        match self.format {
            OutputFormat::Json => println!("{}", world.render_graph_json()?),
            OutputFormat::Dot => {
                let opts = conspectus::output::DotOptions {
                    candidates: self.candidates.into(),
                    diagnostic_nodes: self.diagnostic_nodes.into(),
                };
                println!(
                    "{}",
                    conspectus::output::render_graph_dot(&world.snapshot()?, opts)?
                );
            }
            OutputFormat::Html => {
                let opts = conspectus::output::HtmlOptions {
                    candidates: self.candidates.into(),
                    diagnostic_nodes: self.diagnostic_nodes.into(),
                };
                print!(
                    "{}",
                    conspectus::output::render_graph_html(&world.snapshot()?, opts)?
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DevScenarioTableArgs {
    name: String,
    /// Row-type to render: sessions, mux, union, prs, or forks.
    rows: String,
    /// Force untruncated output.
    #[arg(long)]
    wide: bool,
    /// Render at exactly this many columns.
    #[arg(long, value_name = "N")]
    width: Option<usize>,
    /// Row layout.
    #[arg(long, value_enum, default_value_t = LayoutFlag::Columnar)]
    layout: LayoutFlag,
}

impl DevScenarioTableArgs {
    fn run(self) -> Result<()> {
        let projection = config::Projection::parse(&self.rows).map_err(|err| anyhow!(err))?;
        let world = conspectus::dev_scenarios::materialize(&self.name)?;
        let options = match (self.layout, self.width, self.wide) {
            (LayoutFlag::Columnar, Some(width), _) => {
                conspectus::output::render::RenderOptions::columnar_width(width)
            }
            (LayoutFlag::Columnar, None, _) => conspectus::output::render::RenderOptions::wide(),
            (LayoutFlag::Card, Some(width), _) => {
                conspectus::output::render::RenderOptions::card_width(width)
            }
            (LayoutFlag::Card, None, _) => conspectus::output::render::RenderOptions::card(),
        };
        let table = world.render_table(projection, options)?;
        print!("{table}");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DevScenarioNodeArgs {
    name: String,
    id: String,
    /// When to colorize the output.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl DevScenarioNodeArgs {
    fn run(self) -> Result<()> {
        let world = conspectus::dev_scenarios::materialize(&self.name)?;
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        print!("{}", world.render_node_show(&self.id, color)?);
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DevScenarioTuiArgs {
    name: String,
    /// Initial left-panel organization.
    #[arg(long, value_enum, default_value_t = ViewFlag::Sessions)]
    view: ViewFlag,
    /// Row sort within each group.
    #[arg(long, value_enum, default_value_t = SortFlag::Hierarchy)]
    sort: SortFlag,
    /// Filter / grouping flags. Accepted grouping values depend on
    /// `--view`, matching normal `conspectus tui`.
    #[command(flatten)]
    filter_args: FilterArgs,
    /// When to colorize the output.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl DevScenarioTuiArgs {
    fn run(self) -> Result<()> {
        let world = conspectus::dev_scenarios::materialize(&self.name)?;
        let view = view_from_flag(self.view);
        let filter = self.filter_args.to_row_filter()?;
        let grouping = self
            .filter_args
            .to_grouping(view)?
            .unwrap_or_else(|| conspectus::tui::Grouping::default_for(view));
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let snapshot = world.snapshot()?;
        let mut config = world.tui_config(view, color);
        config.default_sort = match self.sort {
            SortFlag::Hierarchy => conspectus::tui::Sort::Hierarchy,
            SortFlag::Recency => conspectus::tui::Sort::Recency,
        };
        config.initial_filter = filter;
        apply_grouping_to_tui_config(&mut config, grouping);
        conspectus::tui::run_static(config, snapshot)
    }
}
