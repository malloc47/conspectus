use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, Stdio};
use std::str::FromStr;
use std::time::Duration;

use conspectus::config::{self, ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{
    DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredStoreKind, DeclaredStoreSelection,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, select_store_for_declaration, upsert_declared_link,
};
use conspectus::model::{GraphLink, GraphSnapshot, LinkEndpoint, Provenance, RelationKind};

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
            Command::Table(args) => args.run(),
            Command::Declared(args) => args.run(),
            Command::Node(args) => args.run(),
            Command::Columns(args) => args.run(),
            Command::Tui(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the current work graph.
    Graph(GraphArgs),
    /// Render a tabular projection of the resolved graph.
    Table(TableArgs),
    /// Inspect or author declared graph links.
    Declared(Box<DeclaredArgs>),
    /// Inspect a single node and its surrounding links.
    Node(NodeArgs),
    /// List registered columns for a `conspectus table <ROWS>` row-type.
    Columns(ColumnsArgs),
    /// Open the interactive terminal UI.
    Tui(TuiArgs),
}

#[derive(Debug, Args)]
struct ColumnsArgs {
    /// Row-type whose registered columns to list. Accepts the same
    /// tokens as `conspectus table <ROWS>` (sessions, mux, union,
    /// prs, forks).
    row_type: String,
    /// Skip the pager even when stdout is a TTY.
    #[arg(long)]
    no_pager: bool,
    /// Force output through a pager even when stdout is not a TTY.
    #[arg(long, conflicts_with = "no_pager")]
    pager: bool,
    /// When to colorize the output. `auto` (default) emits ANSI only
    /// when stdout is a TTY (and respects `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces color on;
    /// `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

impl ColumnsArgs {
    fn run(self) -> Result<()> {
        let projection = match config::Projection::parse(&self.row_type) {
            Ok(value) => value,
            Err(err) => {
                eprintln!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let listing = conspectus::output::table::render_columns_listing(projection, color);
        print_paged(
            &listing,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct NodeArgs {
    #[command(subcommand)]
    command: NodeCommand,
}

impl NodeArgs {
    fn run(self) -> Result<()> {
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
}

impl NodeShowArgs {
    fn run(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let id = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        let rendered = conspectus::output::node_show::render_node_show(&snapshot, &id, color);
        print_paged(
            &rendered,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
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

#[derive(Debug, Args)]
struct TableArgs {
    #[command(subcommand)]
    command: TableCommand,
}

impl TableArgs {
    fn run(self) -> Result<()> {
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
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum LayoutFlag {
    #[default]
    Columnar,
    Card,
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

        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let render_width = resolve_table_width(self.wide, self.width, &io::stdout());
        let mut options = match (self.layout, render_width) {
            (LayoutFlag::Columnar, Some(w)) => {
                conspectus::output::table::RenderOptions::columnar_width(w)
            }
            (LayoutFlag::Columnar, None) => conspectus::output::table::RenderOptions::wide(),
            (LayoutFlag::Card, Some(w)) => conspectus::output::table::RenderOptions::card_width(w),
            (LayoutFlag::Card, None) => conspectus::output::table::RenderOptions::card(),
        };
        if let Some(columns) = columns {
            options = options.with_columns(columns);
        }
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        options = options.with_color(color);
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
) -> Result<Option<Vec<&'static str>>, conspectus::output::table::ColumnsError> {
    if let Some(spec) = cli_spec {
        return conspectus::output::table::parse_columns_spec(projection, spec).map(Some);
    }
    if let Some(names) = config_names {
        return conspectus::output::table::resolve_explicit_columns(projection, names).map(Some);
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

/// `--color` flag value. The renderer ultimately consumes a `bool`;
/// the value enum exists to give clap a stable parse surface and so
/// we can document the per-token semantics in `--help`.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ColorFlag {
    /// Auto-detect: color when stdout is a TTY and no env opt-out
    /// is set. See [`resolve_color`] for the full precedence table.
    #[default]
    Auto,
    /// Force color on, even when stdout is not a TTY. Overrides
    /// `NO_COLOR`, matching the cargo/git/ripgrep convention that an
    /// explicit user flag wins over passive env signals.
    Always,
    /// Force color off, regardless of TTY / env.
    Never,
}

/// Pull the env vars [`resolve_color`] cares about from the process
/// environment and dispatch. `stdout_is_tty` lets callers pass an
/// explicit boolean (typically `io::stdout().is_terminal()`) so this
/// function stays trivially testable.
fn resolve_color_from_env(flag: ColorFlag, stdout_is_tty: bool) -> bool {
    resolve_color(
        flag,
        std::env::var("NO_COLOR").ok(),
        std::env::var("CLICOLOR_FORCE").ok(),
        std::env::var("CLICOLOR").ok(),
        std::env::var("TERM").ok(),
        stdout_is_tty,
    )
}

/// Resolve `--color` to a boolean per ADR 0022:
///
/// 1. `--color=never`  ⇒ `false`.
/// 2. `--color=always` ⇒ `true`.
/// 3. `NO_COLOR` set to any non-empty value ⇒ `false`
///    (<https://no-color.org>).
/// 4. `CLICOLOR_FORCE` set to a non-zero value ⇒ `true` (BSD-style
///    force-on; matches the role of `--color=always` for env
///    signals).
/// 5. `TERM=dumb` ⇒ `false`.
/// 6. `CLICOLOR=0` ⇒ `false` (BSD-style opt-out).
/// 7. Otherwise: color iff stdout is a TTY.
///
/// Pure function over its arguments so unit tests can pin every
/// permutation without mutating process-wide env.
fn resolve_color(
    flag: ColorFlag,
    no_color: Option<String>,
    cli_color_force: Option<String>,
    cli_color: Option<String>,
    term: Option<String>,
    stdout_is_tty: bool,
) -> bool {
    match flag {
        ColorFlag::Never => return false,
        ColorFlag::Always => return true,
        ColorFlag::Auto => {}
    }
    if no_color.as_deref().is_some_and(|s| !s.is_empty()) {
        return false;
    }
    if cli_color_force
        .as_deref()
        .is_some_and(|s| !s.is_empty() && s != "0")
    {
        return true;
    }
    if term.as_deref() == Some("dumb") {
        return false;
    }
    if cli_color.as_deref() == Some("0") {
        return false;
    }
    stdout_is_tty
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

/// Whether and how to page rendered output (H-TBL-013).
#[derive(Debug, Clone, Copy)]
struct PagerOptions {
    /// `--pager` forces pager even when stdout is not a TTY (useful
    /// for `PAGER=cat` integration tests).
    force_on: bool,
    /// `--no-pager` skips pager even on a TTY.
    force_off: bool,
}

impl PagerOptions {
    fn from_flags(pager: bool, no_pager: bool) -> Self {
        Self {
            force_on: pager,
            force_off: no_pager,
        }
    }

    fn should_page(self, stdout: &impl IsTerminal) -> bool {
        if self.force_off {
            return false;
        }
        if self.force_on {
            return true;
        }
        stdout.is_terminal()
    }
}

/// Print `content` to stdout, optionally through a pager. Falls back
/// to direct print when no pager is configured/available or when
/// `options` disables paging. Git-style behavior: `$PAGER` (when set
/// and non-empty) wins; otherwise `less` (with `LESS=FRX` defaults
/// when the env var is not already set so a single-screen output
/// prints inline and ANSI passes through); otherwise `more`;
/// otherwise direct.
fn print_paged(content: &str, options: PagerOptions) {
    if !options.should_page(&io::stdout()) {
        print!("{content}");
        return;
    }
    for mut cmd in pager_candidates() {
        match cmd.stdin(Stdio::piped()).spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(content.as_bytes());
                }
                let _ = child.wait();
                return;
            }
            Err(_) => continue,
        }
    }
    print!("{content}");
}

/// Resolve the ordered list of pager commands to try.
///
/// For bare `less` invocations (whether from `$PAGER=less` or the
/// internal fallback) Conspectus passes `-F -R -X` explicitly as
/// command-line arguments. The flags need to apply even when the
/// user has a `$LESS` env value of their own, so setting `LESS=FRX`
/// only when `$LESS` is unset (the original implementation) silently
/// fell back to "no quit-if-one-screen" for users with any `$LESS`
/// set. Command-line args merge cleanly with `$LESS`, so existing
/// `LESS=-R` setups keep their `R` and gain the `F` they need.
fn pager_candidates() -> Vec<ProcCommand> {
    pager_candidates_with_env(std::env::var("PAGER").ok())
}

/// Pure version of [`pager_candidates`] for unit testing — takes the
/// `$PAGER` value explicitly so tests don't need to mutate
/// process-wide environment.
fn pager_candidates_with_env(pager_env: Option<String>) -> Vec<ProcCommand> {
    let mut candidates = Vec::new();

    if let Some(pager) = pager_env.filter(|s| !s.trim().is_empty()) {
        // Crude tokenization on whitespace (no shell-quoting support).
        // Git itself runs PAGER through `sh -c`, but that pulls in a
        // shell dependency we'd rather avoid. Users with quoted args
        // can set $PAGER to a wrapper script.
        let parts: Vec<&str> = pager.split_whitespace().collect();
        if let Some((prog, args)) = parts.split_first() {
            let mut cmd = ProcCommand::new(prog);
            if *prog == "less" && args.is_empty() {
                cmd.args(LESS_DEFAULT_ARGS);
            } else {
                cmd.args(args);
            }
            candidates.push(cmd);
        }
    }

    let mut less = ProcCommand::new("less");
    less.args(LESS_DEFAULT_ARGS);
    candidates.push(less);

    candidates.push(ProcCommand::new("more"));

    candidates
}

/// Default args Conspectus passes to `less` when the user has not
/// supplied any of their own via `$PAGER`.
///
/// - `-F` quit if the entire output fits on one screen.
/// - `-R` pass raw ANSI control sequences through (forward-compatible
///   with any future color story; harmless on plain text).
/// - `-X` skip the terminal init/deinit so the rendered output stays
///   on the user's scrollback instead of being cleared on exit.
const LESS_DEFAULT_ARGS: &[&str] = &["-F", "-R", "-X"];

#[derive(Debug, Args, Default)]
struct TuiArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Initial left-panel organization.
    #[arg(long, value_enum, default_value_t = ViewFlag::Sessions)]
    view: ViewFlag,
    /// Top-level grouping in the sessions tree. See
    /// `docs/implementation/phase-08-interactive-tui.md` for
    /// semantics of each value.
    #[arg(long = "sessions-grouping", value_enum, default_value_t = SessionsGroupingFlag::Graph)]
    sessions_grouping: SessionsGroupingFlag,
    /// Row sort within each group.
    #[arg(long, value_enum, default_value_t = SortFlag::Hierarchy)]
    sort: SortFlag,
    /// Background graph refresh cadence (e.g. `30s`, `1m`, `500ms`).
    #[arg(
        long = "refresh-interval",
        value_name = "DURATION",
        default_value = "30s"
    )]
    refresh_interval: String,
    /// Selected mux pane capture cadence.
    #[arg(
        long = "mux-preview-interval",
        value_name = "DURATION",
        default_value = "2s"
    )]
    mux_preview_interval: String,
    /// Suppress live extras: mux pane capture and transcript-tail
    /// reads. Graph-resident previews continue to render.
    #[arg(long = "no-live-preview")]
    no_live_preview: bool,
    /// When to colorize the output. `auto` (default) emits ANSI
    /// only when stdout is a TTY (and respects `NO_COLOR`,
    /// `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces
    /// color on; `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum ViewFlag {
    #[default]
    Sessions,
    Mux,
    Union,
    Prs,
    Forks,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum SortFlag {
    #[default]
    Hierarchy,
    Recency,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum SessionsGroupingFlag {
    #[default]
    Graph,
    Repo,
    Worktree,
    ScanRoot,
}

impl TuiArgs {
    fn run(self) -> Result<()> {
        let refresh_interval = parse_tui_duration(&self.refresh_interval).map_err(|err| {
            anyhow!(
                "invalid --refresh-interval `{}`: {err}",
                self.refresh_interval
            )
        })?;
        let mux_preview_interval =
            parse_tui_duration(&self.mux_preview_interval).map_err(|err| {
                anyhow!(
                    "invalid --mux-preview-interval `{}`: {err}",
                    self.mux_preview_interval
                )
            })?;
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());

        let config = conspectus::tui::RunConfig {
            scan_roots: self.scan_roots,
            default_view: match self.view {
                ViewFlag::Sessions => conspectus::tui::View::Sessions,
                ViewFlag::Mux => conspectus::tui::View::Mux,
                ViewFlag::Union => conspectus::tui::View::Union,
                ViewFlag::Prs => conspectus::tui::View::Prs,
                ViewFlag::Forks => conspectus::tui::View::Forks,
            },
            default_sort: match self.sort {
                SortFlag::Hierarchy => conspectus::tui::Sort::Hierarchy,
                SortFlag::Recency => conspectus::tui::Sort::Recency,
            },
            sessions_grouping: match self.sessions_grouping {
                SessionsGroupingFlag::Graph => conspectus::tui::SessionsGrouping::Graph,
                SessionsGroupingFlag::Repo => conspectus::tui::SessionsGrouping::Repo,
                SessionsGroupingFlag::Worktree => conspectus::tui::SessionsGrouping::Worktree,
                SessionsGroupingFlag::ScanRoot => conspectus::tui::SessionsGrouping::ScanRoot,
            },
            refresh_interval,
            mux_preview_interval,
            live_preview_enabled: !self.no_live_preview,
            color,
        };

        conspectus::tui::run(config)
    }
}

/// Parse a small subset of duration strings: `<integer><ms|s|m|h>`.
/// Kept in-tree to avoid pulling in `humantime` for the TUI flag
/// surface; revisit if more formats are needed.
fn parse_tui_duration(input: &str) -> Result<Duration, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h)".to_string())?;
    let (num_str, suffix) = trimmed.split_at(split);
    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{num_str}`"))?;
    let dur = match suffix {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value.saturating_mul(60)),
        "h" => Duration::from_secs(value.saturating_mul(3600)),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h)")),
    };
    Ok(dur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(cmd: &ProcCommand) -> String {
        cmd.get_program().to_string_lossy().into_owned()
    }

    fn args(cmd: &ProcCommand) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn parse_tui_duration_accepts_each_supported_unit() {
        assert_eq!(parse_tui_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_tui_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_tui_duration("2m"), Ok(Duration::from_secs(120)));
        assert_eq!(parse_tui_duration("1h"), Ok(Duration::from_secs(3600)));
    }

    #[test]
    fn parse_tui_duration_trims_surrounding_whitespace() {
        assert_eq!(parse_tui_duration("  10s  "), Ok(Duration::from_secs(10)));
    }

    #[test]
    fn parse_tui_duration_rejects_missing_unit() {
        let err = parse_tui_duration("30").unwrap_err();
        assert!(err.contains("missing unit"), "got: {err}");
    }

    #[test]
    fn parse_tui_duration_rejects_unknown_unit() {
        let err = parse_tui_duration("30d").unwrap_err();
        assert!(err.contains("unknown unit"), "got: {err}");
    }

    #[test]
    fn parse_tui_duration_rejects_empty_string() {
        assert!(parse_tui_duration("").is_err());
        assert!(parse_tui_duration("   ").is_err());
    }

    #[test]
    fn parse_tui_duration_rejects_negative_or_non_integer() {
        assert!(parse_tui_duration("-5s").is_err());
        assert!(parse_tui_duration("1.5s").is_err());
    }

    #[test]
    fn pager_candidates_when_pager_env_is_unset_starts_with_less_plus_defaults() {
        let candidates = pager_candidates_with_env(None);
        assert!(candidates.len() >= 2);
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
        assert_eq!(program(&candidates[1]), "more");
    }

    #[test]
    fn pager_candidates_when_pager_is_bare_less_adds_default_flags() {
        // Regression for the original bug: the user had `PAGER=less`
        // (no args) plus `LESS=-R` set, so the original implementation
        // left less without `-F` and dropped into the pager even for
        // short, single-screen tables. The fix passes `-F -R -X` on
        // the command line whenever the resolved pager is plain
        // `less`, so the flags merge with the user's `$LESS` instead
        // of being silently skipped.
        let candidates = pager_candidates_with_env(Some("less".to_string()));
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
    }

    #[test]
    fn pager_candidates_when_pager_is_less_with_explicit_args_respects_user_choice() {
        let candidates = pager_candidates_with_env(Some("less -X".to_string()));
        assert_eq!(program(&candidates[0]), "less");
        // Explicit args are kept verbatim; we do not silently append
        // `-F` because the user opted in to their own less flag set.
        assert_eq!(args(&candidates[0]), vec!["-X"]);
    }

    #[test]
    fn pager_candidates_passes_non_less_pager_through_unchanged() {
        let candidates = pager_candidates_with_env(Some("bat --paging=always".to_string()));
        assert_eq!(program(&candidates[0]), "bat");
        assert_eq!(args(&candidates[0]), vec!["--paging=always"]);
    }

    #[test]
    fn resolve_color_never_wins_against_every_env_signal() {
        assert!(!resolve_color(
            ColorFlag::Never,
            Some("1".into()),
            Some("1".into()),
            Some("1".into()),
            Some("xterm".into()),
            true,
        ));
    }

    #[test]
    fn resolve_color_always_overrides_no_color_and_dumb_term() {
        assert!(resolve_color(
            ColorFlag::Always,
            Some("1".into()),
            None,
            None,
            Some("dumb".into()),
            false,
        ));
    }

    #[test]
    fn resolve_color_auto_honors_no_color() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            Some("1".into()),
            None,
            None,
            None,
            true,
        ));
        // Empty NO_COLOR is treated as unset (per the spec — value
        // matters, not just presence).
        assert!(resolve_color(
            ColorFlag::Auto,
            Some(String::new()),
            None,
            None,
            None,
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_honors_clicolor_force_even_on_non_tty() {
        assert!(resolve_color(
            ColorFlag::Auto,
            None,
            Some("1".into()),
            None,
            None,
            false,
        ));
        // CLICOLOR_FORCE=0 is *not* "force on".
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            Some("0".into()),
            None,
            None,
            false,
        ));
    }

    #[test]
    fn resolve_color_auto_dumb_term_opts_out() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            None,
            Some("dumb".into()),
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_clicolor_zero_opts_out() {
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            Some("0".into()),
            None,
            true,
        ));
    }

    #[test]
    fn resolve_color_auto_falls_back_to_isatty() {
        assert!(resolve_color(ColorFlag::Auto, None, None, None, None, true));
        assert!(!resolve_color(
            ColorFlag::Auto,
            None,
            None,
            None,
            None,
            false
        ));
    }

    #[test]
    fn resolve_color_from_env_uses_supplied_tty_signal() {
        // The wrapper pulls env vars from the real process; we can
        // only safely pin behavior under the flag values that
        // short-circuit before any env lookup.
        assert!(!resolve_color_from_env(ColorFlag::Never, true));
        assert!(resolve_color_from_env(ColorFlag::Always, false));
    }

    #[test]
    fn pager_candidates_empty_pager_env_falls_back_to_internal_defaults() {
        let candidates = pager_candidates_with_env(Some("   ".to_string()));
        // Whitespace-only `$PAGER` falls back to the internal `less`
        // with default flags.
        assert_eq!(program(&candidates[0]), "less");
        assert_eq!(args(&candidates[0]), vec!["-F", "-R", "-X"]);
    }
}

#[derive(Debug, Args)]
struct DeclaredArgs {
    #[command(subcommand)]
    command: DeclaredCommand,
}

impl DeclaredArgs {
    fn run(self) -> Result<()> {
        match self.command {
            DeclaredCommand::List(args) => args.run(),
            DeclaredCommand::Create(args) => args.run(),
            DeclaredCommand::Remove(args) => args.run_remove(),
            DeclaredCommand::Confirm(args) => args.run_confirm(),
            DeclaredCommand::Ignore(args) => args.run(),
            DeclaredCommand::Override(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum DeclaredCommand {
    /// List declared links from discovered config stores.
    List(DeclaredListArgs),
    /// Create a declared link.
    Create(Box<DeclaredCreateArgs>),
    /// Remove a declared link by id.
    Remove(DeclaredIdArgs),
    /// Confirm a discovered relationship as a declared link.
    Confirm(DeclaredIdArgs),
    /// Mark a declared link ignored.
    Ignore(DeclaredIgnoreArgs),
    /// Replace one declared link with another.
    Override(DeclaredOverrideArgs),
}

#[derive(Debug, Args)]
struct DeclaredListArgs {
    /// Limit the list to a declared-link store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local declared-link stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredListArgs {
    fn run(self) -> Result<()> {
        let loader = config::ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records = Vec::new();
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_declared_records(
                &mut records,
                DeclaredStoreFlag::User,
                Provenance::GlobalDeclared,
                path,
            );
        }
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_declared_records(
                    &mut records,
                    DeclaredStoreFlag::Project,
                    Provenance::LocalDeclared,
                    path,
                );
            }
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                declared_record_id(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    declared_record_id(right),
                ))
        });
        for record in records {
            match record.link {
                Ok(link) => println!(
                    "{}",
                    render_declared_record(&record.path, record.store, record.provenance, &link)
                ),
                Err(message) => eprintln!(
                    "conspectus: warning: {}: {}",
                    record.path.display(),
                    message
                ),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredCreateArgs {
    /// Stable id for the declaration.
    #[arg(long)]
    id: String,
    /// Relationship kind, such as linked_to_mux or branch_has_forge_pr.
    #[arg(long, value_parser = parse_relation_kind)]
    relation: RelationKind,
    /// Source endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    source: DeclaredEndpointArg,
    /// Target endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    target: DeclaredEndpointArg,
    /// Human reason stored with the declaration.
    #[arg(long)]
    reason: Option<String>,
    /// Human label stored with the declaration.
    #[arg(long)]
    label: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores for nearest-store selection.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredCreateArgs {
    fn run(self) -> Result<()> {
        let link = DeclaredLink {
            id: self.id,
            relation: self.relation,
            state: DeclaredLinkState::Active,
            source: self.source.0,
            target: self.target.0,
            reason: self.reason,
            overridden_by: None,
            label: self.label,
        };

        let path = resolve_write_store(
            self.store,
            Some(&link.source),
            Some(&link.target),
            &self.scan_roots,
        )?;

        let outcome =
            upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            if outcome.link_count == 1 {
                "wrote"
            } else {
                "updated"
            }
        } else {
            "unchanged"
        };
        println!("{verb} declared link `{}` in {}", link.id, path.display());
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredIdArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIdArgs {
    fn run_remove(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let mut removed_from = None;
        for path in &stores {
            if !path.is_file() {
                continue;
            }
            let outcome =
                remove_declared_link(path, &self.id).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                removed_from = Some(path.clone());
                break;
            }
        }

        match removed_from {
            Some(path) => {
                println!(
                    "removed declared link `{}` from {}",
                    self.id,
                    path.display()
                );
                Ok(())
            }
            None => {
                bail!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                );
            }
        }
    }

    fn run_confirm(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Active,
            None,
            self.store,
            &self.scan_roots,
        )
    }
}

/// Shared implementation for `declared confirm` and `declared ignore`.
///
/// Both commands take a candidate-link id from the current discovered
/// graph and produce a declared link whose source/target/relation
/// mirror the candidate. They only differ in the link state and the
/// optional reason string.
fn run_confirm_or_ignore(
    candidate_id: &str,
    state: DeclaredLinkState,
    reason: Option<String>,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<()> {
    let snapshot = discover_for_store_selection(scan_roots)?;
    let candidate = find_candidate_by_id(&snapshot, candidate_id)?;
    let target_node = match &candidate.target {
        LinkEndpoint::Node { id } => id.clone(),
        LinkEndpoint::Unresolved { .. } => bail!(
            "candidate `{candidate_id}` targets an unresolved endpoint; declare it directly with \
             `conspectus declared create`"
        ),
    };

    let source_endpoint = declared_endpoint_from_node_id(&candidate.source);
    let target_endpoint = declared_endpoint_from_node_id(&target_node);

    let link = DeclaredLink {
        id: candidate_id.to_string(),
        relation: candidate.relation.clone(),
        state,
        source: source_endpoint.clone(),
        target: target_endpoint.clone(),
        reason,
        overridden_by: None,
        label: None,
    };

    let path = resolve_write_store(
        store,
        Some(&source_endpoint),
        Some(&target_endpoint),
        scan_roots,
    )?;

    let outcome =
        upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

    let verb = match (state, outcome.changed) {
        (DeclaredLinkState::Active, true) => "confirmed",
        (DeclaredLinkState::Ignored, true) => "ignored",
        (DeclaredLinkState::Overridden, true) => "overrode",
        (_, false) => "unchanged",
    };
    println!("{verb} declared link `{}` in {}", link.id, path.display());
    Ok(())
}

fn find_candidate_by_id<'a>(snapshot: &'a GraphSnapshot, id: &str) -> Result<&'a GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .find(|link| link.id == id)
        .ok_or_else(|| anyhow!("no candidate link with id `{id}` was discovered"))
}

#[derive(Debug, Args)]
struct DeclaredIgnoreArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Reason the declaration should be ignored.
    #[arg(long)]
    reason: Option<String>,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIgnoreArgs {
    fn run(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Ignored,
            self.reason,
            self.store,
            &self.scan_roots,
        )
    }
}

#[derive(Debug, Args)]
struct DeclaredOverrideArgs {
    /// Declared-link id to replace.
    #[arg(long)]
    id: String,
    /// Replacement declared-link id.
    #[arg(long = "overridden-by")]
    overridden_by: String,
    /// Reason the old declaration was overridden.
    #[arg(long)]
    reason: Option<String>,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredOverrideArgs {
    fn run(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let (path, existing) = load_declared_link_by_id(&stores, &self.id)
            .map_err(|err| anyhow!(err.to_string()))?
            .ok_or_else(|| {
                anyhow!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                )
            })?;

        let mut replacement = existing;
        replacement.state = DeclaredLinkState::Overridden;
        replacement.overridden_by = Some(self.overridden_by);
        if let Some(reason) = self.reason {
            replacement.reason = Some(reason);
        }

        let outcome = upsert_declared_link(&path, replacement.clone())
            .map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            "overrode"
        } else {
            "unchanged"
        };
        println!(
            "{verb} declared link `{}` in {}",
            replacement.id,
            path.display()
        );
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum DeclaredStoreFlag {
    All,
    Project,
    User,
}

#[derive(Debug)]
struct DeclaredListRecord {
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
    link: std::result::Result<DeclaredLink, String>,
}

fn declared_record_id(record: &DeclaredListRecord) -> &str {
    match &record.link {
        Ok(link) => &link.id,
        Err(_) => "",
    }
}

fn append_declared_records(
    records: &mut Vec<DeclaredListRecord>,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(DeclaredListRecord {
                store,
                provenance,
                path,
                link: Err(format!("failed to read declared config: {err}")),
            });
            return;
        }
    };

    match parse_declared_document(&text) {
        Ok(document) => {
            records.extend(
                document
                    .links()
                    .iter()
                    .cloned()
                    .map(|link| DeclaredListRecord {
                        store,
                        provenance,
                        path: path.clone(),
                        link: Ok(link),
                    }),
            )
        }
        Err(err) => records.push(DeclaredListRecord {
            store,
            provenance,
            path,
            link: Err(format!("failed to parse declared config: {err}")),
        }),
    }
}

fn render_declared_record(
    path: &std::path::Path,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    link: &DeclaredLink,
) -> String {
    [
        store_label(store).to_string(),
        provenance_label(provenance).to_string(),
        state_label(link.state).to_string(),
        link.id.clone(),
        relation_label(&link.relation).to_string(),
        endpoint_label(&link.source),
        endpoint_label(&link.target),
        link.reason.clone().unwrap_or_default(),
        link.overridden_by.clone().unwrap_or_default(),
        link.label.clone().unwrap_or_default(),
        path.display().to_string(),
    ]
    .join("\t")
}

fn store_label(store: DeclaredStoreFlag) -> &'static str {
    match store {
        DeclaredStoreFlag::All => "all",
        DeclaredStoreFlag::Project => "project",
        DeclaredStoreFlag::User => "user",
    }
}

fn provenance_label(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "local_declared",
        Provenance::GlobalDeclared => "global_declared",
        Provenance::StrongDiscovered => "strong_discovered",
        Provenance::Discovered => "discovered",
        Provenance::Convention => "convention",
        Provenance::Cached => "cached",
    }
}

fn state_label(state: DeclaredLinkState) -> &'static str {
    match state {
        DeclaredLinkState::Active => "active",
        DeclaredLinkState::Ignored => "ignored",
        DeclaredLinkState::Overridden => "overridden",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeclaredEndpointArg(DeclaredEndpoint);

impl FromStr for DeclaredEndpointArg {
    type Err = String;

    fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
        parse_endpoint(raw).map(Self)
    }
}

fn parse_relation_kind(raw: &str) -> std::result::Result<RelationKind, String> {
    match raw {
        "associated_with" => Ok(RelationKind::AssociatedWith),
        "belongs_to_repo" => Ok(RelationKind::BelongsToRepo),
        "checked_out_branch" => Ok(RelationKind::CheckedOutBranch),
        "workspace_contains_repo" => Ok(RelationKind::WorkspaceContainsRepo),
        "branch_has_forge_pr" => Ok(RelationKind::BranchHasForgePr),
        "linked_to_mux" => Ok(RelationKind::LinkedToMux),
        "rooted_in" => Ok(RelationKind::RootedIn),
        "forks_workspace" => Ok(RelationKind::ForksWorkspace),
        "forks_repo" => Ok(RelationKind::ForksRepo),
        "created_worktree" => Ok(RelationKind::CreatedWorktree),
        "referenced_worktree" => Ok(RelationKind::ReferencedWorktree),
        "parent_session" => Ok(RelationKind::ParentSession),
        "child_session" => Ok(RelationKind::ChildSession),
        "created_branch" => Ok(RelationKind::CreatedBranch),
        "associated_branch" => Ok(RelationKind::AssociatedBranch),
        "parent_fork" => Ok(RelationKind::ParentFork),
        "rooted_at_path" => Ok(RelationKind::RootedAtPath),
        _ => Err(format!(
            "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
        )),
    }
}

fn relation_label(relation: &RelationKind) -> &'static str {
    match relation {
        RelationKind::AssociatedWith => "associated_with",
        RelationKind::BelongsToRepo => "belongs_to_repo",
        RelationKind::CheckedOutBranch => "checked_out_branch",
        RelationKind::WorkspaceContainsRepo => "workspace_contains_repo",
        RelationKind::BranchHasForgePr => "branch_has_forge_pr",
        RelationKind::LinkedToMux => "linked_to_mux",
        RelationKind::RootedIn => "rooted_in",
        RelationKind::ForksWorkspace => "forks_workspace",
        RelationKind::ForksRepo => "forks_repo",
        RelationKind::CreatedWorktree => "created_worktree",
        RelationKind::ReferencedWorktree => "referenced_worktree",
        RelationKind::ParentSession => "parent_session",
        RelationKind::ChildSession => "child_session",
        RelationKind::CreatedBranch => "created_branch",
        RelationKind::AssociatedBranch => "associated_branch",
        RelationKind::ParentFork => "parent_fork",
        RelationKind::RootedAtPath => "rooted_at_path",
    }
}

fn parse_endpoint(raw: &str) -> std::result::Result<DeclaredEndpoint, String> {
    let (kind, fields) = raw.split_once(':').ok_or_else(endpoint_syntax_error)?;
    let fields = parse_endpoint_fields(fields)?;
    match kind {
        "repo" => Ok(DeclaredEndpoint::Repo {
            common_dir: required_field(&fields, "common_dir")?,
        }),
        "worktree" => Ok(DeclaredEndpoint::Worktree {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            root: required_field(&fields, "root")?,
        }),
        "workspace" => Ok(DeclaredEndpoint::Workspace {
            root: required_field(&fields, "root")?,
        }),
        "agent_session" => Ok(DeclaredEndpoint::AgentSession {
            harness_key: required_field(&fields, "harness_key")?,
            state_scope: required_field(&fields, "state_scope")?,
            session_key: required_field(&fields, "session_key")?,
        }),
        "mux_session" => Ok(DeclaredEndpoint::MuxSession {
            native_id: required_field(&fields, "native_id")?,
        }),
        "branch" => Ok(DeclaredEndpoint::Branch {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            refname: required_field(&fields, "refname")?,
        }),
        "fork" => Ok(DeclaredEndpoint::Fork {
            provider_source_key: required_field(&fields, "provider_source_key")?,
        }),
        "forge_pr" => Ok(DeclaredEndpoint::ForgePr {
            provider: required_field(&fields, "provider")?,
            host: required_field(&fields, "host")?,
            owner: required_field(&fields, "owner")?,
            repo: required_field(&fields, "repo")?,
            number: required_field(&fields, "number")?
                .parse()
                .map_err(|_| "endpoint field `number` must be an integer".to_string())?,
        }),
        _ => Err(endpoint_syntax_error()),
    }
}

fn endpoint_label(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => {
            format!("repo:common_dir={common_dir}")
        }
        DeclaredEndpoint::Worktree {
            repo_common_dir,
            root,
        } => {
            format!("worktree:repo_common_dir={repo_common_dir},root={root}")
        }
        DeclaredEndpoint::Workspace { root } => {
            format!("workspace:root={root}")
        }
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => {
            format!(
                "agent_session:harness_key={harness_key},state_scope={state_scope},session_key={session_key}"
            )
        }
        DeclaredEndpoint::MuxSession { native_id } => {
            format!("mux_session:native_id={native_id}")
        }
        DeclaredEndpoint::Branch {
            repo_common_dir,
            refname,
        } => {
            format!("branch:repo_common_dir={repo_common_dir},refname={refname}")
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => {
            format!("fork:provider_source_key={provider_source_key}")
        }
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            format!(
                "forge_pr:provider={provider},host={host},owner={owner},repo={repo},number={number}"
            )
        }
    }
}

fn parse_endpoint_fields(raw: &str) -> std::result::Result<BTreeMap<&str, &str>, String> {
    if raw.is_empty() {
        return Err(endpoint_syntax_error());
    }

    let mut fields = BTreeMap::new();
    for part in raw.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(endpoint_syntax_error)?;
        if key.is_empty() || value.is_empty() {
            return Err(endpoint_syntax_error());
        }
        fields.insert(key, value);
    }
    Ok(fields)
}

fn required_field(fields: &BTreeMap<&str, &str>, key: &str) -> std::result::Result<String, String> {
    fields
        .get(key)
        .map(|value| (*value).to_string())
        .ok_or_else(|| format!("missing endpoint field `{key}`"))
}

fn endpoint_syntax_error() -> String {
    "invalid endpoint syntax; expected type:key=value,... using declared TOML field names"
        .to_string()
}

/// Resolve which config file a write should target.
///
/// `Some(Project)` / `Some(User)` short-circuit the nearest-store walk;
/// `Some(All)` is rejected because writes have to pick exactly one store.
/// When `store` is `None`, run discovery from the scan roots and ask
/// [`select_store_for_declaration`] to pick the nearest project store,
/// falling back to user config.
fn resolve_write_store(
    store: Option<DeclaredStoreFlag>,
    source: Option<&DeclaredEndpoint>,
    target: Option<&DeclaredEndpoint>,
    scan_roots: &[PathBuf],
) -> Result<PathBuf> {
    match store {
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for write commands; pick `project` or `user`")
        }
        Some(DeclaredStoreFlag::User) => {
            let loader = ConfigLoader::from_env();
            loader.user_config_path().ok_or_else(|| {
                anyhow!("no user config path available; set $HOME or $XDG_CONFIG_HOME")
            })
        }
        Some(DeclaredStoreFlag::Project) => project_store_path(scan_roots),
        None => match (source, target) {
            (Some(source), Some(target)) => {
                let snapshot = discover_for_store_selection(scan_roots)?;
                let loader = ConfigLoader::from_env();
                let selection = select_store_for_declaration(source, target, &snapshot, &loader)
                    .ok_or_else(|| {
                        anyhow!(
                            "could not pick a declared-link store; \
                             pass --store user or --store project"
                        )
                    })?;
                Ok(selection.path)
            }
            _ => bail!(
                "automatic store selection requires both source and target endpoints; \
                 use --store user or --store project"
            ),
        },
    }
}

fn project_store_path(scan_roots: &[PathBuf]) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let loader = ConfigLoader::from_env();
    let roots = effective_scan_roots(scan_roots, &cwd);

    for root in &roots {
        if let Some(path) = loader.locate_project_config(root) {
            return Ok(path);
        }
    }
    // No existing project config along any scan root: fall back to the
    // first scan root (or cwd) and create one there.
    let fallback = roots.first().cloned().unwrap_or(cwd);
    Ok(fallback.join(PROJECT_CONFIG_FILENAME))
}

/// Effective list of roots used for nearest-store probing. If the caller
/// did not pass any `--scan-root`, we default to the current working
/// directory.
fn effective_scan_roots(scan_roots: &[PathBuf], cwd: &Path) -> Vec<PathBuf> {
    if scan_roots.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        scan_roots.to_vec()
    }
}

fn discover_for_store_selection(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    conspectus::discovery::discover_local_at_roots(roots)
}

/// Candidate stores the read-modify-write helpers should look in when
/// removing or mutating an existing declaration. Order matters: writes
/// stop at the first store that holds a matching id.
fn candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let mut paths = Vec::new();

    let include_project = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::Project)
    );
    let include_user = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::User)
    );

    if include_project {
        let cwd = std::env::current_dir()?;
        let roots = effective_scan_roots(scan_roots, &cwd);
        let mut seen = BTreeSet::new();
        for root in roots {
            if let Some(path) = loader.locate_project_config(root)
                && seen.insert(path.clone())
            {
                paths.push(path);
            }
        }
    }

    if include_user && let Some(path) = loader.user_config_path() {
        paths.push(path);
    }

    Ok(paths)
}

fn store_search_label(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        "any declared-link store".to_string()
    } else {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Borrow checker convenience: lets us reuse the existing
/// [`DeclaredStoreSelection`] type for emitted CLI messages.
fn _selection_display(selection: &DeclaredStoreSelection) -> String {
    let kind = match selection.kind {
        DeclaredStoreKind::Project => "project",
        DeclaredStoreKind::User => "user",
    };
    format!("{kind} {}", selection.path.display())
}
