//! `conspectus table <ROWS>` (H-REF-006 wave 8).
//!
//! Five projection subcommands (sessions, mux, union, prs,
//! forks) rendered through `output::table::render_with`, with
//! layout / width / column / color / pager knobs. Includes
//! table-specific helpers `resolve_columns_selection`,
//! `row_config`, and `resolve_table_width`.

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use conspectus::config;

use super::{
    ColorFlag, FilterArgs, LayoutFlag, PagerOptions, current_unix_epoch_for_table, print_paged,
    resolve_color_from_env, warm_start_discover_and_resolve,
};

#[derive(Debug, Args)]
pub(super) struct TableArgs {
    #[command(subcommand)]
    command: TableCommand,
}

impl TableArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            TableCommand::Sessions(args) => args.run(config::Projection::Agent),
            TableCommand::Mux(args) => args.run(config::Projection::Mux),
            TableCommand::Union(args) => args.run(config::Projection::Union),
            TableCommand::Prs(args) => args.run(config::Projection::Pr),
            TableCommand::Forks(args) => args.run(config::Projection::Fork),
        }
    }
}

#[derive(Debug, Subcommand)]
enum TableCommand {
    /// Agent sessions, one per row.
    Sessions(TableRowsArgs),
    /// Mux (terminal multiplexer) sessions, one per row.
    Mux(TableRowsArgs),
    /// Mixed projection: one row per node, preserving relationship status.
    Union(TableRowsArgs),
    /// Forge pull requests, one per row.
    Prs(TableRowsArgs),
    /// Forks recorded by Atelier or other fork-tracking providers.
    Forks(TableRowsArgs),
}

#[derive(Debug, Args, Default)]
struct TableRowsArgs {
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Force untruncated output even when stdout is a TTY. Conflicts
    /// with `--width`.
    #[arg(long, conflicts_with = "width")]
    wide: bool,
    /// Render at exactly this many columns. Useful for reproducible
    /// captures and snapshot tests.
    #[arg(long, value_name = "N")]
    width: Option<usize>,
    /// Row layout. `columnar` (default) renders one row per line;
    /// `card` renders one column per line with blank lines between
    /// rows, similar to `git log` default formatting.
    #[arg(long, value_enum, default_value_t = LayoutFlag::Columnar)]
    layout: LayoutFlag,
    /// Comma-separated column selection. Tokens: `default` / `all`
    /// reset the running set; `+name` adds; `-name` removes; bare
    /// names switch to explicit-list mode. Unknown names error with
    /// the registered list for the row-type. Overrides the
    /// `[table.<rows>].columns` config when both are present.
    #[arg(long, value_name = "LIST")]
    columns: Option<String>,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    /// Conflicts with `--no-pager`.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. `auto` (default) emits ANSI only
    /// when stdout is a TTY (and respects `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces color on;
    /// `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
    /// Filter / grouping flags. Applied by the
    /// `output::*` projection layer in `TableRowsArgs::run` so the
    /// static table narrows the same rows the TUI does for the same
    /// flag set.
    #[command(flatten)]
    filter_args: FilterArgs,
    /// Don't write the rebuilt graph to the `graph.bin` cache. Useful
    /// when `$HOME` isn't writable; doesn't affect the output.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// Ignore a running `conspectus serve` daemon and rebuild the
    /// graph in-process from live providers.
    #[arg(long = "refresh")]
    refresh: bool,
}

impl TableRowsArgs {
    fn run(self, projection: config::Projection) -> Result<()> {
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

        let columns = resolve_columns_selection(
            projection,
            self.columns.as_deref(),
            row_config(projection, &outcome.config).columns.as_deref(),
        );
        let columns = match columns {
            Ok(value) => value,
            Err(err) => {
                eprintln!("conspectus: {err}");
                std::process::exit(2);
            }
        };

        // Resolve the active filter from CLI flags (F8-010). Config
        // parity (load from `[table.<rows>].filters` or merge with
        // `[tui.views.<name>]`) is a follow-up; for now the CLI
        // flags are the only source so the static table narrows the
        // exact set the operator typed.
        let cli_filter = self.filter_args.to_row_filter()?;
        let now_epoch = current_unix_epoch_for_table();

        let roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots.clone()
        };
        let snapshot = warm_start_discover_and_resolve(
            roots,
            self.refresh,
            self.no_cache,
            &outcome.config.server.intervals,
        )?;
        let render_width = resolve_table_width(self.wide, self.width, &io::stdout());
        let mut options = match (self.layout, render_width) {
            (LayoutFlag::Columnar, Some(w)) => {
                conspectus::output::render::RenderOptions::columnar_width(w)
            }
            (LayoutFlag::Columnar, None) => conspectus::output::render::RenderOptions::wide(),
            (LayoutFlag::Card, Some(w)) => conspectus::output::render::RenderOptions::card_width(w),
            (LayoutFlag::Card, None) => conspectus::output::render::RenderOptions::card(),
        };
        if let Some(columns) = columns {
            options = options.with_columns(columns);
        }
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        options = options
            .with_color(color)
            .with_filter(cli_filter)
            .with_now_epoch(now_epoch);
        let table = conspectus::output::table::render_with(&snapshot, projection, &options);
        print_paged(&table, PagerOptions::from_flags(self.pager, self.no_pager));
        Ok(())
    }
}

/// Resolve which column set to render, with CLI overriding config.
/// Returns `Ok(None)` when neither source is set, signalling "use the
/// row-type's registered default set".
fn resolve_columns_selection(
    projection: config::Projection,
    cli_spec: Option<&str>,
    config_names: Option<&[String]>,
) -> Result<Option<Vec<&'static str>>, conspectus::output::render::ColumnsError> {
    if let Some(spec) = cli_spec {
        return conspectus::output::render::parse_columns_spec(projection, spec).map(Some);
    }
    if let Some(names) = config_names {
        return conspectus::output::render::resolve_explicit_columns(projection, names).map(Some);
    }
    Ok(None)
}

fn row_config(projection: config::Projection, config: &config::Config) -> &config::TableRowConfig {
    match projection {
        config::Projection::Agent => &config.table.sessions,
        config::Projection::Mux => &config.table.mux,
        config::Projection::Union => &config.table.union,
        config::Projection::Pr => &config.table.prs,
        config::Projection::Fork => &config.table.forks,
    }
}

/// Decide the render width for `conspectus table <ROWS>`.
///
/// - `--wide` forces `None` (untruncated).
/// - `--width N` forces `Some(N)`.
/// - Otherwise: detect the terminal width when stdout is a TTY, else
///   leave untruncated so piped output stays grep/awk-friendly.
fn resolve_table_width(
    wide: bool,
    width: Option<usize>,
    stdout: &impl IsTerminal,
) -> Option<usize> {
    if wide {
        return None;
    }
    if let Some(w) = width {
        return Some(w);
    }
    if !stdout.is_terminal() {
        return None;
    }
    terminal_size::terminal_size().map(|(w, _)| usize::from(w.0))
}
