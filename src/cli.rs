use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeSet;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, Stdio};
use std::str::FromStr;
use std::time::Duration;
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use conspectus::aliases::{
    AliasEntry, AliasesDocument, parse_aliases_document, remove_alias_entry, upsert_alias_entry,
};
use conspectus::config::{self, ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{
    DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredStoreKind, DeclaredStoreSelection,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, select_store_for_declaration, upsert_declared_link,
};
use conspectus::discovery::harness::launch_argv_for;
use conspectus::discovery::tmux::{
    MuxBackend, SystemTmux, TmuxAttachOutcome, TmuxNewSessionOutcome, TmuxRenameOutcome,
    TmuxSendKeysOutcome,
};
use conspectus::hook::{HookStore, HookTmuxRecord};
use conspectus::model::{
    GraphLink, GraphSnapshot, LinkEndpoint, NodeId, PinBinding, Provenance, RelationKind,
};
use conspectus::pins::{
    PinEntry, PinLaunch, PinMux, PinStoreKind, PinStoreSelection, TMUX_MUX_BACKEND,
    load_pin_entry_by_id, remove_pin_entry, select_store_for_pin, upsert_pin_entry, user_pin_store,
};
use conspectus::rename::{MuxNativeRename, RenamePlan, plan_session_rename};

#[derive(Debug, Parser)]
#[command(name = "conspectus", version, about = "AI work graph status tool")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command.unwrap_or_else(default_command) {
            Command::Graph(args) => args.run(),
            Command::Table(args) => args.run(),
            Command::Declared(args) => args.run(),
            Command::Node(args) => args.run(),
            Command::Columns(args) => args.run(),
            Command::Tui(args) => args.run(),
            Command::Hook(args) => args.run(),
            Command::Rename(args) => args.run(),
            Command::Alias(args) => args.run(),
            Command::Pin(args) => args.run(),
            Command::Serve(args) => args.run(),
            Command::Refresh(args) => args.run(),
            Command::Status(args) => args.run(),
            #[cfg(debug_assertions)]
            Command::Dev(args) => args.run(),
        }
    }
}

fn default_command() -> Command {
    Command::Tui(TuiArgs::default())
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
    /// Write or install harness hook integrations.
    Hook(HookArgs),
    /// Rename an agent session or a tmux session.
    Rename(RenameArgs),
    /// Inspect operator-authored session aliases.
    Alias(AliasArgs),
    /// Author or inspect session pins (ADR 0057).
    Pin(Box<PinArgs>),
    /// Run the long-lived background daemon that keeps the
    /// resolved graph snapshot warm between one-shot CLI
    /// invocations (ADR 0038 / P7-006).
    Serve(ServeArgs),
    /// Force a full cold rebuild of the graph cache. Talks to a
    /// running `conspectus serve` daemon over the mutation
    /// socket when present; falls back to an in-process cold
    /// rebuild when no daemon is running.
    Refresh(RefreshArgs),
    /// Print the daemon's per-class scheduler state — last tick
    /// epoch, success / error outcome, error detail. Returns a
    /// one-line "no daemon running" message and exits 0 when
    /// no `conspectus serve` is up.
    Status(StatusArgs),
    /// Debug-only developer commands.
    #[cfg(debug_assertions)]
    #[command(hide = true)]
    Dev(DevArgs),
}

#[cfg(debug_assertions)]
#[derive(Debug, Args)]
struct DevArgs {
    #[command(subcommand)]
    command: DevCommand,
}

#[cfg(debug_assertions)]
impl DevArgs {
    fn run(self) -> Result<()> {
        match self.command {
            DevCommand::Scenario(args) => args.run(),
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Materialize and inspect named replay scenarios.
    Scenario(DevScenarioArgs),
}

#[cfg(debug_assertions)]
#[derive(Debug, Args)]
struct DevScenarioArgs {
    #[command(subcommand)]
    command: DevScenarioCommand,
}

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
#[derive(Debug, Args)]
struct DevScenarioNodeArgs {
    name: String,
    id: String,
    /// When to colorize the output.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,
}

#[cfg(debug_assertions)]
impl DevScenarioNodeArgs {
    fn run(self) -> Result<()> {
        let world = conspectus::dev_scenarios::materialize(&self.name)?;
        let color = resolve_color_from_env(self.color, io::stdout().is_terminal());
        print!("{}", world.render_node_show(&self.id, color)?);
        Ok(())
    }
}

#[cfg(debug_assertions)]
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

#[cfg(debug_assertions)]
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
        let listing = conspectus::output::render::render_columns_listing(projection, color);
        print_paged(
            &listing,
            PagerOptions::from_flags(self.pager, self.no_pager),
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct ServeArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted, mirroring the one-shot
    /// CLI's discovery surface.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl ServeArgs {
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
        let scan_roots: Vec<PathBuf> = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };
        conspectus::server::run(conspectus::server::ServeConfig {
            scan_roots,
            intervals: outcome.config.server.intervals,
        })
    }
}

#[derive(Debug, Args, Default)]
struct RefreshArgs {
    /// Discovery scan root for the in-process fallback path
    /// (used only when no daemon is running). When omitted the
    /// fallback uses the process cwd.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Refresh only one provider class (`git`, `mux`,
    /// `harness`, or `forge`) instead of the full graph. With
    /// the daemon running, the per-class refresh evicts only
    /// that class's slice and re-runs only its providers. In
    /// the in-process fallback the same constraint applies, so
    /// the wall-clock cost matches a single class's discovery.
    #[arg(long)]
    class: Option<String>,
}

impl RefreshArgs {
    fn run(self) -> Result<()> {
        // Validate the --class value up front so we surface a
        // useful error regardless of whether we route through
        // the daemon or the fallback path.
        if let Some(name) = self.class.as_deref()
            && conspectus::discovery::cache::ProviderClass::parse(name).is_none()
        {
            bail!("unknown --class `{name}`; expected one of git, mux, harness, forge");
        }

        // Try the daemon socket first. If a `conspectus serve`
        // process is running it owns the freshest writer
        // discipline and is also the canonical place to
        // coordinate a refresh.
        match conspectus::server::client_refresh(self.class.as_deref()) {
            conspectus::server::ClientOutcome::Ok(epoch) => {
                match self.class.as_deref() {
                    Some(class) => println!("refreshed {class} via daemon (epoch={epoch})"),
                    None => println!("refreshed via daemon (epoch={epoch})"),
                }
                return Ok(());
            }
            conspectus::server::ClientOutcome::DaemonError { code, message } => {
                bail!("daemon refused refresh ({code}): {message}");
            }
            conspectus::server::ClientOutcome::Transport(err) => {
                eprintln!(
                    "conspectus: warning: daemon socket error, falling back to local refresh: {err:#}"
                );
            }
            conspectus::server::ClientOutcome::NoDaemon => {
                // Expected when no daemon is running. Silent
                // fall-through to the local path; an operator
                // who started `conspectus serve` and didn't see
                // a `refreshed via daemon` line will recognize
                // the absence themselves.
            }
        }

        // Fallback: in-process refresh. For a full refresh, do
        // the same cold-rebuild path the table
        // command runs with `--refresh`. For a per-class refresh
        // we load the prior, evict the class, re-run discovery,
        // resolve, and persist, mirroring the daemon's
        // try_class_cycle.
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
            self.scan_roots
        };
        match self.class.as_deref() {
            None => {
                warm_start_discover_and_resolve(
                    roots,
                    true,
                    false,
                    &outcome.config.server.intervals,
                )?;
                println!("refreshed via in-process cold rebuild");
            }
            Some(name) => {
                // `parse` was validated above.
                let class = conspectus::discovery::cache::ProviderClass::parse(name)
                    .expect("class validated above");
                in_process_class_refresh(class, roots, &outcome.config.server.intervals)?;
                println!("refreshed {name} via in-process per-class refresh");
            }
        }
        Ok(())
    }
}

/// In-process per-class refresh used by `conspectus refresh
/// --class <name>` when no daemon is available. P11-011a: the
/// prior is mmap'd from `graph.bin` when the file exists (so a
/// peer daemon's recent snapshot still seeds the per-class
/// evict-and-rerun); otherwise empty. Resolved snapshot lands
/// in `graph.bin` via `cache_resolved_snapshot` so a subsequent
/// invocation can warm-start the same way.
fn in_process_class_refresh(
    class: conspectus::discovery::cache::ProviderClass,
    roots: Vec<PathBuf>,
    intervals: &conspectus::config::ServerIntervals,
) -> Result<()> {
    let mut prior = load_prior_from_graph_bin();
    for provider in class.providers() {
        prior.evict_provider(provider);
    }
    let discovery_config = conspectus::discovery::LocalDiscoveryConfig::from_env();
    let snapshot =
        conspectus::discovery::discover_local_warm_with(roots, discovery_config, prior, intervals)?;
    let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
    cache_resolved_snapshot(&snapshot, false);
    Ok(())
}

/// Try to mmap the on-disk `graph.bin` snapshot as the
/// warm-start prior for an in-process refresh. Returns an empty
/// snapshot on missing-file, version-mismatch, validation
/// failure, or any other unhappy path — the caller falls
/// through to cold rebuild semantics.
fn load_prior_from_graph_bin() -> conspectus::model::GraphSnapshot {
    let path = conspectus::snapshot::graph_bin_path();
    match conspectus::snapshot::open_mmap(&path) {
        Ok(handle) => match conspectus::snapshot::deserialize_owned(&handle) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                eprintln!(
                    "conspectus: warning: failed to deserialize {}: {err:#}",
                    path.display()
                );
                conspectus::model::GraphSnapshot::empty()
            }
        },
        Err(conspectus::snapshot::SnapshotError::Io(err))
            if err.kind() == std::io::ErrorKind::NotFound =>
        {
            conspectus::model::GraphSnapshot::empty()
        }
        Err(err) => {
            eprintln!(
                "conspectus: warning: failed to read {}: {err:#}",
                path.display()
            );
            conspectus::model::GraphSnapshot::empty()
        }
    }
}

#[derive(Debug, Args, Default)]
struct StatusArgs {
    /// Output format. `human` (default) prints one line per
    /// class with last-tick freshness; `json` emits the
    /// machine-readable shape the daemon returns over the
    /// socket.
    #[arg(long, value_enum, default_value_t = StatusFormatFlag::Human)]
    format: StatusFormatFlag,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum StatusFormatFlag {
    #[default]
    Human,
    Json,
}

impl StatusArgs {
    fn run(self) -> Result<()> {
        match conspectus::server::client_status() {
            conspectus::server::ClientOutcome::Ok(classes) => {
                match self.format {
                    StatusFormatFlag::Json => {
                        let value = serde_json::to_value(&classes)?;
                        println!("{}", serde_json::to_string_pretty(&value)?);
                    }
                    StatusFormatFlag::Human => render_status_human(&classes),
                }
                Ok(())
            }
            conspectus::server::ClientOutcome::DaemonError { code, message } => {
                bail!("daemon refused status ({code}): {message}");
            }
            conspectus::server::ClientOutcome::Transport(err) => Err(err),
            conspectus::server::ClientOutcome::NoDaemon => {
                println!("no daemon running");
                Ok(())
            }
        }
    }
}

/// Render the daemon-side ClassState map as a one-line-per-class
/// human-readable block. Empty map prints "no class state yet —
/// daemon may have just started." The "yet" framing avoids
/// surprising the operator who started the daemon a beat ago.
fn render_status_human(
    classes: &std::collections::BTreeMap<String, conspectus::server::ClassState>,
) {
    if classes.is_empty() {
        println!("no class state yet — daemon may have just started");
        return;
    }
    let now = current_unix_epoch_for_table().unwrap_or(0);
    for (class, state) in classes {
        let outcome = state.last_outcome.as_deref().unwrap_or("pending");
        let age = match state.last_completed_epoch {
            Some(epoch) => format!("{}s ago", (now - epoch).max(0)),
            None => "—".to_string(),
        };
        let error = state
            .last_error
            .as_deref()
            .map(|m| format!("; error: {m}"))
            .unwrap_or_default();
        println!("{class:8} {outcome:6} last={age}{error}");
    }
}

#[derive(Debug, Args)]
struct HookArgs {
    #[command(subcommand)]
    command: HookCommand,
}

impl HookArgs {
    fn run(self) -> Result<()> {
        match self.command {
            HookCommand::Write(args) => args.run(),
            HookCommand::Init(args) => args.run(),
            HookCommand::Status(args) => args.run(),
            HookCommand::Remove(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum HookCommand {
    /// Write one hook observation from a harness payload on stdin.
    Write(HookWriteArgs),
    /// Install a Conspectus hook into harness configuration.
    Init(HookInitArgs),
    /// Report whether a Conspectus hook is installed.
    Status(HookStatusArgs),
    /// Remove a Conspectus hook from harness configuration.
    Remove(HookRemoveArgs),
}

#[derive(Debug, Args)]
struct HookWriteArgs {
    /// Registered harness key
    /// (e.g. `claude-code`, `codex`, `opencode`).
    /// H-EXT-005: the pre-H-EXT-005 three subcommands
    /// (`hook write claude-code` / `codex` / `opencode`) are
    /// now positional dispatches through the adapter registry.
    /// Existing operator configs that invoke
    /// `conspectus hook write <harness>` continue to work
    /// unchanged.
    harness: String,
    /// Override hook state root. Primarily useful for tests
    /// and experiments.
    #[arg(long = "state-root", value_name = "PATH")]
    state_root: Option<PathBuf>,
}

impl HookWriteArgs {
    fn run(self) -> Result<()> {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            bail!("{} hook payload was empty", self.harness);
        }
        let payload: serde_json::Value = serde_json::from_str(&input)
            .with_context(|| format!("failed to parse {} hook JSON", self.harness))?;
        let (pid, ppid) = harness_pid_pair(&self.harness);
        let harness_version = harness_version_env(&self.harness);
        let record = conspectus::hook::hook_record_from_payload(
            &self.harness,
            &payload,
            pid,
            ppid,
            tmux_context(),
            harness_version,
            conspectus::hook::current_epoch(),
        )?;
        write_or_ingest_hook_record(&record, self.state_root)?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum HookScopeFlag {
    User,
    Project,
}

#[derive(Debug, Args)]
struct HookInitArgs {
    /// Harness whose hook config should be managed.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to inspect or mutate.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
    /// Print the action without writing files.
    #[arg(long)]
    dry_run: bool,
    /// Command installed into the harness config.
    #[arg(long = "command", value_name = "COMMAND")]
    command: Option<String>,
}

#[derive(Debug, Args)]
struct HookStatusArgs {
    /// Harness whose hook config should be inspected.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to inspect.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
}

#[derive(Debug, Args)]
struct HookRemoveArgs {
    /// Harness whose hook config should be managed.
    #[arg(value_enum)]
    harness: HookHarnessFlag,
    /// Configuration scope to mutate.
    #[arg(long, value_enum, default_value_t = HookScopeFlag::User)]
    scope: HookScopeFlag,
    /// Print the action without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum HookHarnessFlag {
    ClaudeCode,
    Codex,
}

impl HookInitArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_install(),
            HookHarnessFlag::Codex => self.run_codex_install(),
        }
    }

    fn run_claude_install(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let command = self
            .command
            .unwrap_or_else(|| default_hook_command("claude-code"));
        let mut document = read_json_document(&path)?;
        let changed = ensure_claude_hook(&mut document, &command);

        if self.dry_run {
            let verb = if changed {
                "would install"
            } else {
                "already installed"
            };
            println!("{verb} Claude Code hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_json_document(&path, &document)?;
            println!("installed Claude Code hook in {}", path.display());
        } else {
            println!("Claude Code hook already installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_install(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let command = self
            .command
            .unwrap_or_else(|| default_hook_command("codex"));
        let mut document = read_toml_document(&path)?;
        let changed = ensure_codex_hook(&mut document, &command);

        if self.dry_run {
            let verb = if changed {
                "would install"
            } else {
                "already installed"
            };
            println!("{verb} Codex hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_toml_document(&path, &document)?;
            println!("installed Codex hook in {}", path.display());
        } else {
            println!("Codex hook already installed in {}", path.display());
        }
        Ok(())
    }
}

impl HookStatusArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_status(),
            HookHarnessFlag::Codex => self.run_codex_status(),
        }
    }

    fn run_claude_status(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let document = read_json_document(&path)?;
        if has_claude_hook(&document) {
            println!("installed\tclaude-code\t{}", path.display());
        } else {
            println!("not-installed\tclaude-code\t{}", path.display());
        }
        Ok(())
    }

    fn run_codex_status(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let document = read_toml_document(&path)?;
        if has_codex_hook(&document) {
            println!("installed\tcodex\t{}", path.display());
        } else {
            println!("not-installed\tcodex\t{}", path.display());
        }
        Ok(())
    }
}

impl HookRemoveArgs {
    fn run(self) -> Result<()> {
        match self.harness {
            HookHarnessFlag::ClaudeCode => self.run_claude_remove(),
            HookHarnessFlag::Codex => self.run_codex_remove(),
        }
    }

    fn run_claude_remove(self) -> Result<()> {
        let path = claude_settings_path(self.scope)?;
        let mut document = read_json_document(&path)?;
        let changed = remove_claude_hook(&mut document);

        if self.dry_run {
            let verb = if changed {
                "would remove"
            } else {
                "not installed"
            };
            println!("{verb} Claude Code hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_json_document(&path, &document)?;
            println!("removed Claude Code hook from {}", path.display());
        } else {
            println!("Claude Code hook not installed in {}", path.display());
        }
        Ok(())
    }

    fn run_codex_remove(self) -> Result<()> {
        let path = codex_config_path(self.scope)?;
        let mut document = read_toml_document(&path)?;
        let changed = remove_codex_hook(&mut document);

        if self.dry_run {
            let verb = if changed {
                "would remove"
            } else {
                "not installed"
            };
            println!("{verb} Codex hook in {}", path.display());
            return Ok(());
        }
        if changed {
            write_toml_document(&path, &document)?;
            println!("removed Codex hook from {}", path.display());
        } else {
            println!("Codex hook not installed in {}", path.display());
        }
        Ok(())
    }
}

fn resolve_hook_state_root(override_root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = override_root {
        return Ok(root);
    }
    HookStore::from_env()
        .map(|store| store.root().to_path_buf())
        .ok_or_else(|| {
            anyhow!("no hook state root available; set HOME or CONSPECTUS_HOOK_SIDECAR_STATE")
        })
}

fn write_or_ingest_hook_record(
    record: &conspectus::hook::HookRecord,
    override_root: Option<PathBuf>,
) -> Result<()> {
    if let Some(root) = override_root {
        HookStore::new(root).write_record(record)?;
        return Ok(());
    }

    match conspectus::server::client_hook_ingest(record) {
        conspectus::server::ClientOutcome::Ok(()) => Ok(()),
        conspectus::server::ClientOutcome::NoDaemon => {
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        conspectus::server::ClientOutcome::DaemonError { code, message } => {
            if code != "snapshot_unavailable" {
                eprintln!("conspectus: warning: daemon refused hook ingest ({code}): {message}");
            }
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
        conspectus::server::ClientOutcome::Transport(err) => {
            eprintln!(
                "conspectus: warning: daemon hook ingest failed, writing local hook spool: {err:#}"
            );
            let root = resolve_hook_state_root(None)?;
            HookStore::new(root).write_record(record)?;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn read_linux_parent_pid() -> Option<u32> {
    read_linux_ppid_of(std::process::id())
}

#[cfg(target_os = "linux")]
fn read_linux_ppid_of(pid: u32) -> Option<u32> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    let mut fields = after_name.split_whitespace();
    fields.next()?;
    fields.next()?.parse().ok()
}

#[cfg(target_os = "linux")]
fn read_linux_comm_of(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Resolve the (pid, ppid) of the harness process (e.g. `claude`,
/// `codex`, `opencode`) that triggered this hook invocation.
///
/// Walks the parent-pid chain from the current process upward, looking
/// for the first ancestor whose `/proc/<pid>/comm` matches one of
/// `expected_binaries`. This is necessary because claude / codex /
/// opencode launch the hook command via a short-lived shell wrapper
/// (e.g. `sh -c 'conspectus hook write …'`), so `std::process::id()`
/// returns the writer's pid — a process that exits within
/// milliseconds. The hook-sidecar discovery layer's liveness check
/// (`src/discovery/hook_sidecar.rs`) treats records with a dead pid
/// as ignored evidence, which would silently disable hook evidence
/// entirely (see H-MUXPROC-020).
///
/// Returns `None` when running on a non-Linux host, when the walk
/// exhausts its depth budget, or when no ancestor matches. Callers
/// must treat `None` as "harness pid unknown" and persist it as a
/// `None` pid in the hook record so the discovery liveness check
/// is skipped instead of failing.
fn resolve_harness_pid(expected_binaries: &[&str]) -> Option<(u32, u32)> {
    #[cfg(target_os = "linux")]
    {
        let start = read_linux_parent_pid()?;
        resolve_harness_pid_with(start, expected_binaries, |pid| {
            Some((read_linux_comm_of(pid)?, read_linux_ppid_of(pid)?))
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = expected_binaries;
        None
    }
}

/// Pure walker used by `resolve_harness_pid`. Factored out so unit
/// tests can mock the `/proc` reader. Caps the walk at 8 hops to
/// prevent runaway recursion on a corrupted process table.
fn resolve_harness_pid_with<F>(
    start_pid: u32,
    expected_binaries: &[&str],
    mut read: F,
) -> Option<(u32, u32)>
where
    F: FnMut(u32) -> Option<(String, u32)>,
{
    let mut pid = start_pid;
    for _ in 0..8 {
        if pid <= 1 {
            return None;
        }
        let (comm, ppid) = read(pid)?;
        if expected_binaries.iter().any(|name| comm == *name) {
            return Some((pid, ppid));
        }
        pid = ppid;
    }
    None
}

/// Process-name set Conspectus expects to see for each harness when
/// walking the parent-pid chain from a hook writer up to the live
/// agent process. Mirrors the harness keys recognized elsewhere in
/// the cross-link and process-tree code.
///
/// H-EXT-005: reads from the adapter registry's
/// `RuntimeSignature::process_command_basenames` so a new
/// harness gets pid-pair resolution for free — no cli.rs
/// match-table edit required.
fn harness_binaries(harness: &str) -> Vec<&'static str> {
    conspectus::discovery::harness::registered_adapters()
        .find(|a| a.harness_key() == harness)
        .map(|a| a.runtime_signature().process_command_basenames.to_vec())
        .unwrap_or_default()
}

/// Environment variable Conspectus consults for a harness's
/// version string when writing a hook sidecar record. Read
/// through a helper (rather than inline in `HookWriteArgs::run`)
/// so the per-harness mapping stays in one place.
fn harness_version_env(harness: &str) -> Option<String> {
    match harness {
        "claude-code" => std::env::var("CLAUDE_CODE_VERSION").ok(),
        "opencode" => std::env::var("CONSPECTUS_OPENCODE_HOOK_VERSION").ok(),
        _ => None,
    }
}

/// Resolve the `(pid, ppid)` pair to record on a hook sidecar entry
/// for `harness`. Returns `(None, None)` when the harness pid cannot
/// be identified — the discovery liveness check then skips the pid
/// branch entirely so the record stays Active rather than being
/// marked Ignored against a stillborn writer pid (H-MUXPROC-020).
fn harness_pid_pair(harness: &str) -> (Option<i64>, Option<i64>) {
    match resolve_harness_pid(&harness_binaries(harness)) {
        Some((pid, ppid)) => (Some(i64::from(pid)), Some(i64::from(ppid))),
        None => (None, None),
    }
}

fn tmux_context() -> Option<HookTmuxRecord> {
    // H-EXT-011: iterate the registered mux backends and ask
    // each for its current-session context. The first backend
    // to answer wins. The pre-H-EXT-011 direct `$TMUX` + `tmux
    // display-message` probe moved onto the SystemTmux
    // implementation of `MuxBackend::current_session_context`.
    // Additional backends (zellij, screen, …) supply their own
    // env-var contract via the same trait method.
    let backends: Vec<Box<dyn conspectus::discovery::tmux::MuxBackend>> = vec![
        Box::new(conspectus::discovery::tmux::SystemTmux::new()),
        Box::new(conspectus::discovery::zellij::SystemZellij::new()),
    ];
    for backend in backends {
        if let Some(ctx) = backend.current_session_context() {
            let record = HookTmuxRecord {
                session_name: ctx.session_name,
                native_id: None,
                pane_id: ctx.pane_id,
                socket_path: ctx.namespace,
            };
            if !record.is_empty() {
                return Some(record);
            }
        }
    }
    None
}

fn claude_settings_path(scope: HookScopeFlag) -> Result<PathBuf> {
    match scope {
        HookScopeFlag::User => {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("HOME is required for --scope user"))?;
            Ok(home.join(".claude").join("settings.json"))
        }
        HookScopeFlag::Project => {
            let cwd = std::env::current_dir()?;
            Ok(cwd.join(".claude").join("settings.json"))
        }
    }
}

fn codex_config_path(scope: HookScopeFlag) -> Result<PathBuf> {
    match scope {
        HookScopeFlag::User => {
            let home = if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
                PathBuf::from(codex_home)
            } else {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .ok_or_else(|| anyhow!("HOME or CODEX_HOME is required for --scope user"))?
                    .join(".codex")
            };
            Ok(home.join("config.toml"))
        }
        HookScopeFlag::Project => {
            let cwd = std::env::current_dir()?;
            Ok(cwd.join(".codex").join("config.toml"))
        }
    }
}

fn default_hook_command(harness: &str) -> String {
    let program = std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "conspectus".to_string());
    format!("{} hook write {harness}", shell_quote(&program))
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn read_json_document(path: &Path) -> Result<serde_json::Value> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let value: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse JSON {}", path.display()))?;
            if value.is_object() {
                Ok(value)
            } else {
                bail!("{} must contain a JSON object", path.display());
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_json_document(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value)? + "\n";
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn read_toml_document(path: &Path) -> Result<DocumentMut> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .parse::<DocumentMut>()
            .with_context(|| format!("failed to parse TOML {}", path.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_toml_document(path: &Path, value: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = value.to_string();
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

fn ensure_claude_hook(document: &mut serde_json::Value, command: &str) -> bool {
    if has_claude_hook(document) {
        return false;
    }

    let object = document
        .as_object_mut()
        .expect("settings document is object");
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    if !hooks.is_object() {
        *hooks = serde_json::json!({});
    }
    let hooks_object = hooks.as_object_mut().expect("hooks is object");
    let session_start = hooks_object
        .entry("SessionStart")
        .or_insert_with(|| serde_json::json!([]));
    if !session_start.is_array() {
        *session_start = serde_json::json!([]);
    }
    session_start
        .as_array_mut()
        .expect("SessionStart is array")
        .push(serde_json::json!({
            "matcher": "resume|startup|clear|compact",
            "hooks": [
                {
                    "type": "command",
                    "command": command
                }
            ]
        }));
    true
}

fn has_claude_hook(document: &serde_json::Value) -> bool {
    document
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|entries| entries.iter().any(entry_contains_conspectus_hook))
}

fn remove_claude_hook(document: &mut serde_json::Value) -> bool {
    let Some(entries) = document
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("SessionStart"))
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };

    let mut changed = false;
    for entry in entries.iter_mut() {
        let Some(hooks) = entry
            .get_mut("hooks")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let original_len = hooks.len();
        hooks.retain(|hook| !hook_is_conspectus_command(hook));
        changed |= hooks.len() != original_len;
    }
    entries.retain(|entry| {
        entry
            .get("hooks")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|hooks| !hooks.is_empty())
    });
    changed
}

fn entry_contains_conspectus_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|hooks| hooks.iter().any(hook_is_conspectus_command))
}

fn hook_is_conspectus_command(hook: &serde_json::Value) -> bool {
    hook.get("type").and_then(serde_json::Value::as_str) == Some("command")
        && hook
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_some_and(is_conspectus_hook_command)
}

fn is_conspectus_hook_command(command: &str) -> bool {
    command.contains("hook write claude-code")
}

fn ensure_codex_hook(document: &mut DocumentMut, command: &str) -> bool {
    if has_codex_hook(document) {
        return false;
    }

    let hooks = document
        .entry("hooks")
        .or_insert_with(|| Item::Table(Table::new()));
    if !hooks.is_table() {
        *hooks = Item::Table(Table::new());
    }
    let hooks_table = hooks.as_table_mut().expect("hooks is table");
    let session_start = hooks_table
        .entry("SessionStart")
        .or_insert_with(|| Item::Value(Value::Array(Array::new())));
    if !session_start.is_array() {
        *session_start = Item::Value(Value::Array(Array::new()));
    }
    session_start
        .as_array_mut()
        .expect("SessionStart is array")
        .push(codex_hook_entry_value(command));
    true
}

fn has_codex_hook(document: &DocumentMut) -> bool {
    document
        .get("hooks")
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(Item::as_array)
        .is_some_and(|entries| entries.iter().any(codex_entry_contains_conspectus_hook))
}

fn remove_codex_hook(document: &mut DocumentMut) -> bool {
    let Some(entries) = document
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut("SessionStart"))
        .and_then(Item::as_array_mut)
    else {
        return false;
    };

    let mut changed = false;
    let retained: Vec<Value> = entries
        .iter()
        .filter_map(|entry| {
            let mut entry = entry.clone();
            if remove_codex_hooks_from_entry(&mut entry) {
                changed = true;
            }
            (!codex_entry_hooks_empty(&entry)).then_some(entry)
        })
        .collect();
    if retained.len() != entries.len() {
        changed = true;
    }
    if changed {
        entries.clear();
        for entry in retained {
            entries.push(entry);
        }
    }
    changed
}

fn codex_hook_entry_value(command: &str) -> Value {
    let mut hook = toml_edit::InlineTable::new();
    hook.insert("type", Value::from("command"));
    hook.insert("command", Value::from(command));
    hook.insert("async", Value::from(false));

    let mut hooks = Array::new();
    hooks.push(Value::InlineTable(hook));

    let mut entry = toml_edit::InlineTable::new();
    entry.insert("hooks", Value::Array(hooks));
    Value::InlineTable(entry)
}

fn codex_entry_contains_conspectus_hook(entry: &Value) -> bool {
    entry
        .as_inline_table()
        .and_then(|table| table.get("hooks"))
        .and_then(Value::as_array)
        .is_some_and(|hooks| hooks.iter().any(codex_hook_is_conspectus_command))
}

fn codex_hook_is_conspectus_command(hook: &Value) -> bool {
    hook.as_inline_table().is_some_and(|table| {
        table.get("type").and_then(Value::as_str) == Some("command")
            && table
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.contains("hook write codex"))
    })
}

fn remove_codex_hooks_from_entry(entry: &mut Value) -> bool {
    let Some(hooks) = entry
        .as_inline_table_mut()
        .and_then(|table| table.get_mut("hooks"))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let original_len = hooks.len();
    let retained: Vec<Value> = hooks
        .iter()
        .filter(|hook| !codex_hook_is_conspectus_command(hook))
        .cloned()
        .collect();
    if retained.len() == original_len {
        return false;
    }
    hooks.clear();
    for hook in retained {
        hooks.push(hook);
    }
    true
}

fn codex_entry_hooks_empty(entry: &Value) -> bool {
    entry
        .as_inline_table()
        .and_then(|table| table.get("hooks"))
        .and_then(Value::as_array)
        .is_none_or(Array::is_empty)
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
    /// P7-003 phase 4: suppress the writer for this invocation.
    /// The render still uses the warm-start cache; the post-render
    /// write is skipped.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// P7-003 phase 4: skip the warm-start read so this run scans
    /// every provider cold. Writer still runs unless `--no-cache`
    /// is also set.
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
        conspectus::resolve::explain_resolved_relationships(&mut snapshot);
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
    /// Filter / grouping flags (ADR 0031). Applied by the
    /// `output::*` projection layer in `TableRowsArgs::run` so the
    /// static table narrows the same rows the TUI does for the same
    /// flag set.
    #[command(flatten)]
    filter_args: FilterArgs,
    /// P7-003: skip writing the resolved snapshot to the canonical
    /// `graph.bin` cache after this invocation. Useful for
    /// debugging or when running against a non-writable `$HOME`.
    /// Does not affect the rendered output.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// P7-003: skip the warm-start read so this invocation rebuilds
    /// from the live providers alone, ignoring the persisted cache.
    /// The writer side still runs unless `--no-cache` is also set,
    /// so the next invocation can warm-start off this run's output.
    /// Phase 3 will graduate the warm-start path from a backstop
    /// merge into per-provider TTL comparison + selective re-run.
    #[arg(long = "refresh")]
    refresh: bool,
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

fn current_unix_epoch_for_table() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
}

/// P11-011a resolution chain for every one-shot CLI command
/// that renders a resolved graph (`table`, `node show`,
/// `graph`). The order is:
///
/// 1. **Daemon snapshot**: when `conspectus serve` is reachable
///    on the socket, request the freshly-resolved snapshot via
///    `client_snapshot()`, decode it, and return. Skips
///    discovery + resolve + write entirely.
/// 2. **Cold rebuild**: run `discover_local_warm_with` with an
///    empty prior (no on-disk warm-start anymore — see ADR
///    0082's "daemonless cold rebuild is fine at seconds")
///    and write the resulting snapshot to `graph.bin` so the
///    next daemon cycle (or a future mmap-fresh revival) has
///    the file ready.
///
/// `refresh` (operator-forced cold scan via `--refresh`)
/// bypasses (1) so the flag semantic — "ignore the daemon and
/// rebuild from disk" — is preserved. `no_cache` skips the
/// `graph.bin` write at the end of (2).
fn warm_start_discover_and_resolve(
    roots: Vec<PathBuf>,
    refresh: bool,
    no_cache: bool,
    intervals: &conspectus::config::ServerIntervals,
) -> Result<conspectus::model::GraphSnapshot> {
    if !refresh && let Some(snapshot) = try_daemon_snapshot() {
        return Ok(snapshot);
    }
    let discovery_config = conspectus::discovery::LocalDiscoveryConfig::from_env();
    let snapshot = conspectus::discovery::discover_local_warm_with(
        roots,
        discovery_config,
        conspectus::model::GraphSnapshot::empty(),
        intervals,
    )?;
    let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
    cache_resolved_snapshot(&snapshot, no_cache);
    Ok(snapshot)
}

/// Best-effort daemon-snapshot fetch. Returns `Some` only when
/// the daemon responds with a structurally valid snapshot;
/// every other outcome (no daemon, snapshot_unavailable error
/// during first cycle, transport error, malformed bytes)
/// returns `None` so the caller falls through. Silent: a noisy
/// stderr per CLI invocation would clobber whatever the
/// operator was reading.
fn try_daemon_snapshot() -> Option<conspectus::model::GraphSnapshot> {
    use conspectus::server::{ClientOutcome, client_snapshot};
    let bytes = match client_snapshot() {
        ClientOutcome::Ok(bytes) => bytes,
        _ => return None,
    };
    conspectus::snapshot::from_bytes(&bytes).ok()
}

/// P11-011a: write the resolved snapshot to `graph.bin` (the
/// canonical persistence artifact post-ADR-0082). Best-effort:
/// a write failure prints a `conspectus: warning:` line to
/// stderr but never aborts the command — the rendered output
/// the operator just saw is the primary product. `no_cache`
/// lets the operator opt out for a single invocation (e.g.
/// when running against a non-writable `$HOME` or wanting an
/// in-memory-only render).
fn cache_resolved_snapshot(snapshot: &conspectus::model::GraphSnapshot, no_cache: bool) {
    if no_cache {
        return;
    }
    let bin_path = conspectus::snapshot::graph_bin_path();
    if let Err(err) = conspectus::snapshot::write_atomic(&bin_path, snapshot) {
        eprintln!(
            "conspectus: warning: failed to write {}: {err:#}",
            bin_path.display()
        );
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

#[derive(Debug, Args)]
struct TuiArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Initial left-panel organization. When omitted, F8-013's
    /// persisted-last-view sidecar wins; if absent, falls back to
    /// `[tui].default_view` and finally `sessions`.
    #[arg(long, value_enum)]
    view: Option<ViewFlag>,
    /// Disable the F8-013 persisted-last-view sidecar for this
    /// run. The session still uses the standard precedence
    /// (`--view` > config default > `sessions`) for its starting
    /// view but does not write the sidecar on view switches.
    /// `--snapshot` implies this automatically.
    #[arg(long = "no-resume-view")]
    no_resume_view: bool,
    /// Deprecated alias for `--grouping` when `--view sessions` is
    /// active (ADR 0031). Continues to work but emits a one-line
    /// deprecation warning to stderr; `--grouping` overrides on
    /// conflict.
    #[arg(long = "sessions-grouping", value_enum)]
    sessions_grouping: Option<SessionsGroupingFlag>,
    /// Row sort within each group.
    #[arg(long, value_enum)]
    sort: Option<SortFlag>,
    #[command(flatten)]
    filter_args: FilterArgs,
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
    /// P7-003 phase 4: suppress the writer for this TUI invocation.
    /// The discovery loop still reads from the cache on each
    /// refresh; only the post-refresh write is skipped.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// P7-003 phase 4: force a cold scan on every refresh. The
    /// writer still runs unless `--no-cache` is also set so
    /// concurrent one-shot CLI invocations in other shells still
    /// benefit from this session's discovery output.
    #[arg(long = "refresh")]
    refresh: bool,
    /// When to colorize the output. `auto` (default) emits ANSI
    /// only when stdout is a TTY (and respects `NO_COLOR`,
    /// `CLICOLOR`, `CLICOLOR_FORCE`, `TERM=dumb`); `always` forces
    /// color on; `never` forces it off.
    #[arg(long, value_enum, default_value_t = ColorFlag::Auto)]
    color: ColorFlag,

    /// Dev-only: render one TUI frame to stdout with ANSI styling
    /// preserved and exit, instead of starting the interactive event
    /// loop. Lets agents iterate on renderer changes without a
    /// manual screenshot loop (ADR 0067). Combine with
    /// `--snapshot-width/--snapshot-height` to size the frame,
    /// `--snapshot-keys` to drive the UI into a non-default state
    /// before the snapshot, and `--snapshot-pane` to slice the
    /// output.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot")]
    snapshot: bool,

    /// Frame width for `--snapshot`. Defaults to 160 columns —
    /// roughly a wide terminal — so the right pane has room to
    /// render.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-width", value_name = "COLS", default_value_t = 160)]
    snapshot_width: u16,

    /// Frame height for `--snapshot`. Defaults to 40 rows.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-height", value_name = "ROWS", default_value_t = 40)]
    snapshot_height: u16,

    /// Vim-style key script to dispatch before the snapshot.
    /// Literal characters pass through; `<Name>` brackets map to
    /// non-printable keys (`<Enter> <Tab> <Down> <C-r>` …). See
    /// `src/tui/snapshot.rs` for the full name set.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-keys", value_name = "SCRIPT", default_value = "")]
    snapshot_keys: String,

    /// Region of the rendered frame to emit. `all` (default) emits
    /// the entire buffer; `header`, `left`, `right`, and `status`
    /// slice to the matching pane using the same layout the renderer
    /// applies.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-pane", value_enum, default_value_t = SnapshotPaneFlag::All)]
    snapshot_pane: SnapshotPaneFlag,

    /// Read the input `GraphSnapshot` from this JSON file instead
    /// of running live discovery (ADR 0068). Useful when iterating
    /// on a renderer fix against a stable world.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-fixture", value_name = "PATH")]
    snapshot_fixture: Option<PathBuf>,

    /// After the input snapshot is produced (live or fixture-
    /// loaded) and resolved, write it to this JSON file (ADR
    /// 0068). Pair with `--snapshot-fixture` for a round-trip, or
    /// use alone to capture the current operator's world into a
    /// reusable fixture.
    #[cfg(feature = "snapshot")]
    #[arg(long = "snapshot-export-fixture", value_name = "PATH")]
    snapshot_export_fixture: Option<PathBuf>,

    /// Launch the interactive TUI against a fixture JSON instead
    /// of running live discovery (ADR 0069). Bypasses scan-root
    /// discovery entirely; the `r` accelerator re-reads the
    /// fixture from disk so the operator can edit the file and
    /// cycle in the new state. Mutually exclusive with
    /// `--snapshot`.
    #[cfg(feature = "snapshot")]
    #[arg(long = "fixture", value_name = "PATH", conflicts_with = "snapshot")]
    fixture: Option<PathBuf>,
}

#[cfg(feature = "snapshot")]
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum SnapshotPaneFlag {
    #[default]
    All,
    Header,
    Left,
    Right,
    Status,
}

#[cfg(feature = "snapshot")]
impl SnapshotPaneFlag {
    fn to_pane(self) -> conspectus::tui::snapshot::SnapshotPane {
        use conspectus::tui::snapshot::SnapshotPane;
        match self {
            SnapshotPaneFlag::All => SnapshotPane::All,
            SnapshotPaneFlag::Header => SnapshotPane::Header,
            SnapshotPaneFlag::Left => SnapshotPane::Left,
            SnapshotPaneFlag::Right => SnapshotPane::Right,
            SnapshotPaneFlag::Status => SnapshotPane::Status,
        }
    }
}

impl Default for TuiArgs {
    fn default() -> Self {
        Self {
            scan_roots: Vec::new(),
            view: None,
            no_resume_view: false,
            sessions_grouping: None,
            sort: None,
            filter_args: FilterArgs::default(),
            refresh_interval: "30s".to_string(),
            mux_preview_interval: "2s".to_string(),
            no_live_preview: false,
            no_cache: false,
            refresh: false,
            color: ColorFlag::Auto,
            #[cfg(feature = "snapshot")]
            snapshot: false,
            #[cfg(feature = "snapshot")]
            snapshot_width: 160,
            #[cfg(feature = "snapshot")]
            snapshot_height: 40,
            #[cfg(feature = "snapshot")]
            snapshot_keys: String::new(),
            #[cfg(feature = "snapshot")]
            snapshot_pane: SnapshotPaneFlag::All,
            #[cfg(feature = "snapshot")]
            snapshot_fixture: None,
            #[cfg(feature = "snapshot")]
            snapshot_export_fixture: None,
            #[cfg(feature = "snapshot")]
            fixture: None,
        }
    }
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
    Workspace,
    Repo,
    Checkout,
    ScanRoot,
    None,
}

impl SessionsGroupingFlag {
    fn to_grouping(self) -> conspectus::tui::Grouping {
        use conspectus::tui::{Grouping, SessionsGrouping};
        match self {
            SessionsGroupingFlag::Graph => Grouping::Sessions(SessionsGrouping::Graph),
            SessionsGroupingFlag::Workspace => Grouping::Sessions(SessionsGrouping::Workspace),
            SessionsGroupingFlag::Repo => Grouping::Sessions(SessionsGrouping::Repo),
            SessionsGroupingFlag::Checkout => Grouping::Sessions(SessionsGrouping::Checkout),
            SessionsGroupingFlag::ScanRoot => Grouping::Sessions(SessionsGrouping::ScanRoot),
            SessionsGroupingFlag::None => Grouping::Sessions(SessionsGrouping::None),
        }
    }
}

/// Filter / grouping flag surface shared by `conspectus tui` and
/// `conspectus table <ROWS>` (ADR 0031, F8-009). Mount with
/// `#[command(flatten)]` so the host struct picks up every flag
/// without re-declaring them.
///
/// Resolution helpers ([`FilterArgs::to_row_filter`] and
/// [`FilterArgs::to_grouping`]) take the active view so per-view
/// grouping validation can produce actionable errors against the
/// view's enum.
#[derive(Debug, Args, Default, Clone)]
struct FilterArgs {
    /// Narrow to one or more harness keys. Repeatable; values
    /// accumulate into a set. Comparison is case-insensitive and
    /// trim-aware.
    #[arg(long = "harness", value_name = "HARNESS")]
    harness: Vec<String>,
    /// Drop rows whose `last_active_epoch` is older than this
    /// window (e.g. `7d`, `24h`, `30m`).
    #[arg(long = "max-age", value_name = "DURATION")]
    max_age: Option<String>,
    /// Narrow by derived mux state. Comma-separated or repeatable.
    /// Legal values: `attached`, `ambiguous`, `unmuxed`.
    #[arg(long = "mux-state", value_name = "STATE", value_delimiter = ',')]
    mux_state: Vec<String>,
    /// Per-view grouping. Accepted values depend on `--view`; see
    /// `conspectus tui --help` for the per-view list (ADR 0031).
    #[arg(long = "grouping", value_name = "VALUE")]
    grouping: Option<String>,
}

impl FilterArgs {
    /// Convert the raw flag values into a [`conspectus::filter::RowFilter`].
    /// Returns an error when a value fails to parse (max-age
    /// duration, mux-state spelling).
    fn to_row_filter(&self) -> Result<conspectus::filter::RowFilter> {
        use conspectus::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
        let harness = if self.harness.is_empty() {
            None
        } else {
            Some(HarnessFilter::from_values(self.harness.iter()))
        };
        let max_age = match self.max_age.as_deref() {
            Some(raw) => Some(
                parse_filter_duration(raw)
                    .map_err(|err| anyhow!("invalid --max-age `{raw}`: {err}"))?,
            ),
            None => None,
        };
        let mux_state = if self.mux_state.is_empty() {
            None
        } else {
            let mut keys = Vec::with_capacity(self.mux_state.len());
            for raw in &self.mux_state {
                let key = MuxStateKey::from_str_ci(raw).ok_or_else(|| {
                    anyhow!(
                        "invalid --mux-state `{raw}`; expected one of attached, ambiguous, unmuxed"
                    )
                })?;
                keys.push(key);
            }
            Some(MuxStateFilter::from_values(keys))
        };
        Ok(RowFilter {
            harness,
            max_age,
            mux_state,
            ..RowFilter::default()
        })
    }

    /// Convert the `--grouping` flag value into a typed
    /// [`conspectus::tui::Grouping`] for the active view. Returns
    /// `Ok(None)` when the flag wasn't provided; returns an error
    /// when the value isn't valid for `view` so the caller can list
    /// the legal values in the message.
    fn to_grouping(
        &self,
        view: conspectus::tui::View,
    ) -> Result<Option<conspectus::tui::Grouping>> {
        let Some(raw) = self.grouping.as_deref() else {
            return Ok(None);
        };
        conspectus::tui::Grouping::parse_for(view, raw)
            .map(Some)
            .ok_or_else(|| {
                let choices = conspectus::tui::Grouping::values_for(view)
                    .iter()
                    .map(|g| g.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow!(
                    "invalid --grouping `{raw}` for --view {}; expected one of {choices}",
                    view_flag_label(view)
                )
            })
    }
}

fn view_flag_label(view: conspectus::tui::View) -> &'static str {
    match view {
        conspectus::tui::View::Sessions => "sessions",
        conspectus::tui::View::Mux => "mux",
        conspectus::tui::View::Union => "union",
        conspectus::tui::View::Prs => "prs",
        conspectus::tui::View::Forks => "forks",
    }
}

fn view_from_flag(flag: ViewFlag) -> conspectus::tui::View {
    match flag {
        ViewFlag::Sessions => conspectus::tui::View::Sessions,
        ViewFlag::Mux => conspectus::tui::View::Mux,
        ViewFlag::Union => conspectus::tui::View::Union,
        ViewFlag::Prs => conspectus::tui::View::Prs,
        ViewFlag::Forks => conspectus::tui::View::Forks,
    }
}

#[cfg(debug_assertions)]
fn apply_grouping_to_tui_config(
    config: &mut conspectus::tui::RunConfig,
    grouping: conspectus::tui::Grouping,
) {
    match grouping {
        conspectus::tui::Grouping::Sessions(grouping) => {
            config.sessions_grouping = grouping;
        }
        conspectus::tui::Grouping::Mux(grouping) => {
            config.mux_grouping = grouping;
        }
        conspectus::tui::Grouping::Union(_)
        | conspectus::tui::Grouping::Prs(_)
        | conspectus::tui::Grouping::Forks(_) => {}
    }
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

        // Resolve scan roots: CLI flags win, then config, then a
        // single-element fallback to the current working directory
        // (the original cwd-scoped v1 behavior).
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
        let scan_roots = if !self.scan_roots.is_empty() {
            self.scan_roots
        } else if !outcome.config.tui.scan_roots.is_empty() {
            outcome.config.tui.scan_roots.clone()
        } else {
            vec![cwd.clone()]
        };

        // F8-013 view precedence:
        //   1. explicit `--view` flag wins.
        //   2. otherwise read the persisted last-active view from
        //      `$XDG_STATE_HOME/conspectus/tui-state.json`.
        //   3. otherwise fall back to `[tui].default_view` /
        //      `View::Sessions`.
        // `--no-resume-view` and `--snapshot` both skip step 2 so the
        // start view stays deterministic for scripts and the
        // snapshot fixture path.
        #[cfg(feature = "snapshot")]
        let suppress_resume = self.no_resume_view || self.snapshot;
        #[cfg(not(feature = "snapshot"))]
        let suppress_resume = self.no_resume_view;
        let view = if let Some(flag) = self.view {
            view_from_flag(flag)
        } else if !suppress_resume {
            let cache = conspectus::tui_state::TuiStateCache::from_env();
            conspectus::tui_state::read_last_view(&cache).unwrap_or(conspectus::tui::View::Sessions)
        } else {
            conspectus::tui::View::Sessions
        };

        // Resolve initial filter: CLI flags win over config.
        let cli_filter = self.filter_args.to_row_filter()?;
        let initial_filter = if cli_filter.is_empty() {
            outcome.config.tui.views.for_view(view).filter.clone()
        } else {
            cli_filter
        };

        // Resolve initial grouping with precedence:
        //  1. --grouping (new, per-view, validated)
        //  2. --sessions-grouping (legacy alias; warns; only valid when view=sessions)
        //  3. config `[tui.views.<name>].grouping`
        //  4. Grouping::default_for(view)
        let mut initial_grouping = self.filter_args.to_grouping(view)?;
        if let Some(legacy) = self.sessions_grouping {
            eprintln!(
                "conspectus: warning: --sessions-grouping is deprecated; \
                 use --grouping instead (ADR 0031)"
            );
            if view != conspectus::tui::View::Sessions {
                eprintln!(
                    "conspectus: warning: --sessions-grouping ignored because \
                     --view is not `sessions`"
                );
            } else if initial_grouping.is_none() {
                initial_grouping = Some(legacy.to_grouping());
            }
        }
        let initial_grouping = match initial_grouping {
            Some(g) => g,
            None => outcome
                .config
                .tui
                .views
                .for_view(view)
                .grouping
                .unwrap_or_else(|| conspectus::tui::Grouping::default_for(view)),
        };
        let sessions_grouping = match initial_grouping {
            conspectus::tui::Grouping::Sessions(g) => g,
            // For non-sessions views, the runtime still needs a
            // SessionsGrouping for build_tree_for_view's sessions
            // branch; fall back to the default so a `--view mux
            // --grouping host` launch doesn't accidentally drag a
            // sessions grouping along. F8-003 generalizes this.
            _ => conspectus::tui::SessionsGrouping::Graph,
        };
        let mux_grouping = match initial_grouping {
            conspectus::tui::Grouping::Mux(g) => g,
            _ => conspectus::tui::MuxGrouping::Session,
        };

        let explicit_sort = self.sort.is_some();
        let default_sort = match self.sort.unwrap_or(SortFlag::Hierarchy) {
            SortFlag::Hierarchy => conspectus::tui::Sort::Hierarchy,
            SortFlag::Recency => conspectus::tui::Sort::Recency,
        };
        let default_sort = if sessions_grouping == conspectus::tui::SessionsGrouping::None {
            conspectus::tui::Sort::Recency
        } else {
            default_sort
        };

        let config = conspectus::tui::RunConfig {
            scan_roots,
            cwd: Some(cwd),
            default_view: view,
            default_sort,
            sessions_grouping,
            mux_grouping,
            initial_filter,
            explicit_filter: !self.filter_args.harness.is_empty()
                || self.filter_args.max_age.is_some()
                || !self.filter_args.mux_state.is_empty(),
            explicit_sort,
            explicit_grouping: self.filter_args.grouping.is_some()
                || self.sessions_grouping.is_some(),
            refresh_interval,
            mux_preview_interval,
            live_preview_enabled: !self.no_live_preview,
            color,
            current_tmux_session: current_tmux_session_name(),
            theme: outcome.config.tui.theme.clone(),
            show_edge_meta: outcome.config.tui.detail.show_edge_meta,
            show_harness_chips: outcome.config.tui.show_harness_chips,
            intervals: outcome.config.server.intervals,
            no_cache: self.no_cache,
            refresh: self.refresh,
        };

        #[cfg(feature = "snapshot")]
        if self.snapshot {
            return conspectus::tui::snapshot::run(
                config,
                conspectus::tui::snapshot::SnapshotConfig {
                    width: self.snapshot_width,
                    height: self.snapshot_height,
                    keys: self.snapshot_keys,
                    pane: self.snapshot_pane.to_pane(),
                    fixture: self.snapshot_fixture,
                    export_fixture: self.snapshot_export_fixture,
                },
            );
        }

        #[cfg(feature = "snapshot")]
        if let Some(path) = self.fixture {
            return conspectus::tui::run_from_fixture(config, path);
        }

        conspectus::tui::run(config)
    }
}

fn current_tmux_session_name() -> Option<String> {
    // H-EXT-011: reuse the trait-based probe. Kept as a
    // separate helper because the TUI runtime consumes the
    // `Option<String>` shape directly (for the self-attach
    // guard); constructing the full HookTmuxRecord here would
    // be wasted work.
    let backend = conspectus::discovery::tmux::SystemTmux::new();
    backend
        .current_session_context()
        .and_then(|ctx| ctx.session_name)
}

/// Parse a duration like [`parse_tui_duration`] but accept a `d`
/// (days) suffix. Used by `--max-age` where day-scale windows are
/// common; `parse_tui_duration` deliberately rejects `d` because
/// day-scale refresh intervals don't make sense.
fn parse_filter_duration(input: &str) -> Result<Duration, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h/d)".to_string())?;
    let (num_str, suffix) = trimmed.split_at(split);
    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{num_str}`"))?;
    let dur = match suffix {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value.saturating_mul(60)),
        "h" => Duration::from_secs(value.saturating_mul(3600)),
        "d" => Duration::from_secs(value.saturating_mul(86_400)),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h/d)")),
    };
    Ok(dur)
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
#[path = "cli_tests.rs"]
mod tests;

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

#[derive(Debug, Args)]
struct RenameArgs {
    #[command(subcommand)]
    command: RenameCommand,
}

impl RenameArgs {
    fn run(self) -> Result<()> {
        match self.command {
            RenameCommand::Session(args) => args.run(),
            RenameCommand::Mux(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum RenameCommand {
    /// Set, change, or clear an agent session's display-name alias.
    /// The linked tmux session is renamed in lockstep by default;
    /// pass `--no-mux` to skip the tmux side.
    Session(RenameSessionArgs),
    /// Rename a tmux session. No alias is written; only the tmux
    /// native name changes (per ADR 0029 mux-id stability rule).
    Mux(RenameMuxArgs),
}

#[derive(Debug, Args)]
struct RenameSessionArgs {
    /// Agent session id. Accepts the short row id, the full
    /// `NodeId` display form, or the `harness:session_key` label
    /// (same forms `conspectus node show` understands).
    id: String,
    /// New display name. Mutually exclusive with `--clear`.
    name: Option<String>,
    /// Skip the lockstep tmux rename. The alias is still written.
    #[arg(long = "no-mux")]
    no_mux: bool,
    /// Remove any existing alias for this session instead of setting one.
    /// Mutually exclusive with `<NAME>`.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Restrict the alias write to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameSessionArgs {
    fn run(self) -> Result<()> {
        let new_display_name = match (self.name, self.clear) {
            (Some(_), true) => bail!("--clear and <NAME> are mutually exclusive"),
            (None, false) => bail!("specify either a new <NAME> or --clear"),
            (Some(name), false) => Some(name),
            (None, true) => None,
        };

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let session_id = match resolved {
            NodeId::AgentSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename session only operates on agent sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        let plan = plan_session_rename(&snapshot, &session_id, new_display_name, self.no_mux)
            .map_err(|err| anyhow!(err.to_string()))?;

        execute_rename_plan(&plan, self.store, &self.scan_roots, &SystemTmux::new())
    }
}

#[derive(Debug, Args)]
struct RenameMuxArgs {
    /// Mux session id. Accepts the short row id, the full `NodeId`
    /// display form, or the `tmux:<native>` label.
    id: String,
    /// New tmux session name. Required because mux aliases are not
    /// stored (per ADR 0029) — only the native tmux name changes.
    name: Option<String>,
    /// Rejected: mux sessions have no Conspectus-owned alias to
    /// clear. Surfaced so the help text documents the constraint.
    #[arg(long, conflicts_with = "name")]
    clear: bool,
    /// Root used to discover the running tmux server, if any.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl RenameMuxArgs {
    fn run(self) -> Result<()> {
        if self.clear {
            bail!(
                "mux sessions have no Conspectus-owned alias to clear; \
                 supply a new <NAME> instead"
            );
        }
        let new_name = self
            .name
            .ok_or_else(|| anyhow!("rename mux requires a new <NAME>"))?;
        if new_name.trim().is_empty() {
            bail!("mux rename requires a non-empty <NAME>");
        }

        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);

        let resolved = match conspectus::output::node_show::resolve_node_id(&self.id, &snapshot) {
            Ok(id) => id,
            Err(err) => {
                eprint!("conspectus: {err}");
                std::process::exit(2);
            }
        };
        let mux_id = match resolved {
            NodeId::MuxSession(id) => id,
            other => bail!(
                "`{}` resolves to a {} node; rename mux only operates on mux sessions",
                self.id,
                node_kind_label(&other)
            ),
        };

        run_mux_rename(
            &MuxNativeRename {
                mux: mux_id,
                new_name,
            },
            &SystemTmux::new(),
        )
    }
}

/// Execute the alias-write side of `plan`, then (when present) the
/// linked tmux rename. Either step can leave the other in a partial
/// state — we surface the error and let the operator decide whether
/// to re-run.
fn execute_rename_plan(
    plan: &RenamePlan,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
    tmux: &dyn MuxBackend,
) -> Result<()> {
    let endpoint = declared_endpoint_from_node_id(&NodeId::AgentSession(
        plan.agent_alias_write.session.clone(),
    ));
    match &plan.agent_alias_write.display_name {
        Some(display_name) => {
            let path = resolve_alias_store(store, &endpoint, scan_roots)?;
            let entry = AliasEntry {
                node: endpoint,
                display_name: display_name.clone(),
                reason: None,
            };
            let outcome =
                upsert_alias_entry(&path, entry).map_err(|err| anyhow!(err.to_string()))?;
            let verb = if outcome.changed {
                "wrote"
            } else {
                "unchanged"
            };
            println!("{verb} alias `{}` in {}", display_name, path.display());
        }
        None => {
            let stores = alias_candidate_store_paths(store, scan_roots)?;
            let mut removed_from = None;
            for path in &stores {
                if !path.is_file() {
                    continue;
                }
                let outcome =
                    remove_alias_entry(path, &endpoint).map_err(|err| anyhow!(err.to_string()))?;
                if outcome.changed {
                    removed_from = Some(path.clone());
                    break;
                }
            }
            match removed_from {
                Some(path) => println!("removed alias from {}", path.display()),
                None => println!("no alias found for session"),
            }
        }
    }

    if let Some(mux_rename) = &plan.mux_native_rename {
        run_mux_rename(mux_rename, tmux)?;
    }
    Ok(())
}

fn run_mux_rename(rename: &MuxNativeRename, tmux: &dyn MuxBackend) -> Result<()> {
    let outcome = tmux
        // Default-socket rename — `conspectus rename` is the alias
        // overlay surface (ADR 0029) that runs on whatever socket
        // owned the discovered mux. Pin-driven non-default-socket
        // renames will run from `pin rename` via H-PIN-014 instead.
        .rename_session(None, &rename.mux.native_id, &rename.new_name)
        .map_err(|err| anyhow!("tmux rename-session failed: {err}"))?;
    match outcome {
        TmuxRenameOutcome::Renamed => {
            println!(
                "renamed tmux session `{}` to `{}`",
                rename.mux.native_id, rename.new_name
            );
            Ok(())
        }
        TmuxRenameOutcome::NoTarget => bail!(
            "tmux session `{}` not found on this server",
            rename.mux.native_id
        ),
        TmuxRenameOutcome::NameCollision => bail!(
            "tmux refused to rename `{}` to `{}`: name already in use",
            rename.mux.native_id,
            rename.new_name
        ),
        TmuxRenameOutcome::Unavailable(reason) => bail!("tmux unavailable: {}", reason.as_str()),
        TmuxRenameOutcome::Failed { code, message } => {
            bail!("tmux rename-session failed (exit code {code:?}): {message}")
        }
        TmuxRenameOutcome::Unsupported => bail!("tmux runner does not support rename_session"),
    }
}

fn node_kind_label(id: &NodeId) -> &'static str {
    match id {
        NodeId::Repo(_) => "repo",
        NodeId::Checkout(_) => "checkout",
        NodeId::Workspace(_) => "workspace",
        NodeId::AgentSession(_) => "agent_session",
        NodeId::MuxSession(_) => "mux_session",
        NodeId::Pin(_) => "pin",
        NodeId::RuntimeProcess(_) => "runtime_process",
        NodeId::Branch(_) => "branch",
        NodeId::Fork(_) => "fork",
        NodeId::ForgePr(_) => "forge_pr",
    }
}

fn resolve_alias_store(
    store: Option<DeclaredStoreFlag>,
    endpoint: &DeclaredEndpoint,
    scan_roots: &[PathBuf],
) -> Result<PathBuf> {
    match store {
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for alias writes; pick `project` or `user`")
        }
        Some(DeclaredStoreFlag::User) => {
            let loader = ConfigLoader::from_env();
            loader.user_config_path().ok_or_else(|| {
                anyhow!("no user config path available; set $HOME or $XDG_CONFIG_HOME")
            })
        }
        Some(DeclaredStoreFlag::Project) => project_store_path(scan_roots),
        None => {
            let snapshot = discover_for_store_selection(scan_roots)?;
            let loader = ConfigLoader::from_env();
            let selection = select_store_for_declaration(endpoint, endpoint, &snapshot, &loader)
                .ok_or_else(|| {
                    anyhow!("could not pick an alias store; pass --store user or --store project")
                })?;
            Ok(selection.path)
        }
    }
}

fn alias_candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    // Same shape as `candidate_store_paths`: project stores first,
    // then user, so a project alias is removed before the global
    // entry takes over the rendering precedence.
    candidate_store_paths(store, scan_roots)
}

#[derive(Debug, Args)]
struct AliasArgs {
    #[command(subcommand)]
    command: AliasCommand,
}

impl AliasArgs {
    fn run(self) -> Result<()> {
        match self.command {
            AliasCommand::List(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum AliasCommand {
    /// List session aliases from the discovered config stores.
    List(AliasListArgs),
}

#[derive(Debug, Args)]
struct AliasListArgs {
    /// Limit the list to a store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl AliasListArgs {
    fn run(self) -> Result<()> {
        let loader = ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records: Vec<AliasListRecord> = Vec::new();
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in &scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_alias_records(&mut records, DeclaredStoreFlag::Project, path);
            }
        }
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_alias_records(&mut records, DeclaredStoreFlag::User, path);
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                alias_record_key(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    alias_record_key(right),
                ))
        });

        for record in records {
            match record.entry {
                Ok(entry) => println!(
                    "{}\t{}\t{}\t{}",
                    store_label(record.store),
                    record.path.display(),
                    format_alias_endpoint(&entry.node),
                    entry.display_name
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

#[derive(Debug)]
struct AliasListRecord {
    store: DeclaredStoreFlag,
    path: PathBuf,
    entry: std::result::Result<AliasEntry, String>,
}

fn alias_record_key(record: &AliasListRecord) -> String {
    match &record.entry {
        Ok(entry) => format_alias_endpoint(&entry.node),
        Err(_) => String::new(),
    }
}

fn append_alias_records(
    records: &mut Vec<AliasListRecord>,
    store: DeclaredStoreFlag,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to read aliases: {err}")),
            });
            return;
        }
    };
    let parsed: AliasesDocument = match parse_aliases_document(&text) {
        Ok(document) => document,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to parse aliases: {err}")),
            });
            return;
        }
    };
    for entry in parsed.entries() {
        records.push(AliasListRecord {
            store,
            path: path.clone(),
            entry: Ok(entry.clone()),
        });
    }
}

fn format_alias_endpoint(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => format!("agent_session:{harness_key}:{state_scope}:{session_key}"),
        DeclaredEndpoint::MuxSession { native_id } => format!("mux_session:{native_id}"),
        DeclaredEndpoint::Pin { id } => format!("pin:{id}"),
        DeclaredEndpoint::RuntimeProcess { observation_key } => {
            format!("runtime_process:{observation_key}")
        }
        DeclaredEndpoint::Repo { common_dir } => format!("repo:{common_dir}"),
        DeclaredEndpoint::Checkout { root, .. } => format!("checkout:{root}"),
        DeclaredEndpoint::Workspace { root } => format!("workspace:{root}"),
        DeclaredEndpoint::Branch { refname, .. } => format!("branch:{refname}"),
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => format!("fork:{provider_source_key}"),
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => format!("forge_pr:{provider}:{host}/{owner}/{repo}#{number}"),
    }
}

// =====================================================================
// Pin command tree (ADR 0057 / H-PIN-007/008/009).
// =====================================================================

#[derive(Debug, Args)]
pub struct PinArgs {
    #[command(subcommand)]
    command: PinCommand,
}

impl PinArgs {
    fn run(self) -> Result<()> {
        match self.command {
            PinCommand::Create(args) => args.run(),
            PinCommand::List(args) => args.run(),
            PinCommand::Show(args) => args.run(),
            PinCommand::Rename(args) => args.run(),
            PinCommand::Rm(args) => args.run(),
            PinCommand::Launch(args) => args.run(PinLaunchIntent::Launch),
            PinCommand::Attach(args) => args.run(PinLaunchIntent::Attach),
            PinCommand::Bind(args) => args.run(),
            PinCommand::Rebind(args) => args.run(),
            PinCommand::Adopt(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum PinCommand {
    /// Declare a session pin.
    Create(Box<PinCreateArgs>),
    /// List session pins from the discovered config stores.
    List(PinListArgs),
    /// Show a single pin by id, including resolver binding state.
    Show(PinShowArgs),
    /// Rename a pin's `id` or `display_name`.
    Rename(PinRenameArgs),
    /// Remove a pin by id.
    Rm(PinRmArgs),
    /// Launch a pin's session (creating the tmux session if needed)
    /// and attach the terminal.
    Launch(PinLaunchArgs),
    /// Attach to a pin's already-bound session, or fall through to
    /// launch when no live session exists yet.
    Attach(PinLaunchArgs),
    /// Resolve `PinAmbiguous` by binding the pin to a specific
    /// agent-session id. Writes a `LocalDeclared linked_to_mux`
    /// link the resolver treats as authoritative.
    Bind(PinBindArgs),
    /// Update the pin's `mux.name` (and optionally
    /// `mux.socket_name`) after an external tmux rename. Pure TOML
    /// mutation — does not touch tmux.
    Rebind(PinRebindArgs),
    /// Convert an existing live tmux session into a pin without
    /// creating a new mux. Harness and cwd are inferred from the
    /// running session unless overridden.
    Adopt(PinAdoptArgs),
}

/// Filter the binding states `pin list` includes (ADR 0057).
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum PinStateFilter {
    All,
    Bound,
    Unbound,
    Stale,
}

#[derive(Debug, Args)]
struct PinCreateArgs {
    /// Stable pin id (unique within the chosen store).
    id: String,
    /// Harness key (`codex`, `claude-code`, `opencode`, `aider`).
    #[arg(long)]
    harness: String,
    /// Absolute path the pin anchors on (also passed as `tmux -c` at
    /// launch). Required to exist on disk at write time.
    #[arg(long, value_name = "PATH")]
    cwd: PathBuf,
    /// Operator-chosen display name. Defaults to `<id>`.
    #[arg(long)]
    display: Option<String>,
    /// Mux session name. Defaults to the chosen `--display` (and
    /// therefore to `<id>` when neither is set).
    #[arg(long = "mux-name", value_name = "NAME")]
    mux_name: Option<String>,
    /// Optional tmux socket name (the equivalent of `tmux -L
    /// <name>`). Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Override the per-harness default `launch.argv`. Repeatable —
    /// each value is one argv token.
    #[arg(long = "launch-arg", value_name = "ARG")]
    launch_argv: Vec<String>,
    /// Free-form explanatory text written under the pin's `reason`
    /// field.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl PinCreateArgs {
    fn run(self) -> Result<()> {
        let display = self.display.clone().unwrap_or_else(|| self.id.clone());
        let mux_name = self.mux_name.clone().unwrap_or_else(|| display.clone());

        let entry = PinEntry {
            id: self.id.clone(),
            display_name: display,
            harness: self.harness,
            cwd: self.cwd.display().to_string(),
            mux: PinMux {
                backend: TMUX_MUX_BACKEND.to_string(),
                name: mux_name,
                socket_name: self.mux_socket,
            },
            launch: if self.launch_argv.is_empty() {
                None
            } else {
                Some(PinLaunch {
                    argv: self.launch_argv,
                })
            },
            reason: self.reason,
        };

        let selection = resolve_pin_write_store(self.store, &self.cwd)?;

        let outcome = upsert_pin_entry(&selection.path, entry.clone())
            .map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            if outcome.entry_count == 1 {
                "wrote"
            } else {
                "updated"
            }
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` in {} ({})",
            entry.id,
            selection.path.display(),
            pin_store_label(selection.kind)
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinListArgs {
    /// Limit the list to a store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Filter by binding state.
    #[arg(long, value_enum, default_value_t = PinStateFilter::All)]
    state: PinStateFilter,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinListArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let include_project = matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        );
        let include_user = matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User);

        let mut rows: Vec<String> = Vec::new();
        for pin in &snapshot.pins {
            let store_flag = match pin.provenance {
                Provenance::LocalPin => DeclaredStoreFlag::Project,
                Provenance::GlobalPin => DeclaredStoreFlag::User,
                _ => continue,
            };
            if matches!(store_flag, DeclaredStoreFlag::Project) && !include_project {
                continue;
            }
            if matches!(store_flag, DeclaredStoreFlag::User) && !include_user {
                continue;
            }
            if !pin_matches_filter(pin.binding.as_ref(), self.state) {
                continue;
            }
            rows.push(render_pin_row(
                store_flag,
                pin.provenance,
                &pin.id,
                &pin.display_name,
                &pin.harness,
                &pin.cwd,
                pin.mux.native_id(),
                bound_session_label(pin.binding.as_ref()).unwrap_or_default(),
                pin_state_label(pin.binding.as_ref()),
                &pin.store_path,
            ));
        }

        rows.sort();
        for row in rows {
            println!("{row}");
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinShowArgs {
    /// Pin id.
    id: String,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinShowArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };
        println!("id           {}", pin.id);
        println!("display_name {}", pin.display_name);
        println!("harness      {}", pin.harness);
        println!("cwd          {}", pin.cwd);
        println!("mux          {}", pin.mux.native_id());
        if let Some(socket) = pin.mux.socket_name.as_deref() {
            println!("socket_name  {socket}");
        }
        if let Some(argv) = pin.launch_argv.as_ref() {
            println!("launch_argv  {}", argv.join(" "));
        }
        if let Some(reason) = pin.reason.as_deref() {
            println!("reason       {reason}");
        }
        println!("provenance   {}", provenance_label(pin.provenance));
        println!("store        {}", pin.store_path);
        println!("state        {}", pin_state_label(pin.binding.as_ref()));
        match pin.binding.as_ref() {
            Some(PinBinding::Bound { session, mux }) => {
                println!("bound_mux    {}", mux.native_id);
                println!("bound_session {}", session.session_key);
            }
            Some(PinBinding::StaleMux { mux }) => {
                println!("bound_mux    {} (no live harness session)", mux.native_id);
            }
            _ => {}
        }
        // ADR 0058 H-PIN-RESUME-005: when the pin is unbound and the
        // sidecar has a recorded last-bound session, surface it so
        // the operator can see what `pin launch` would resume into.
        if let Some(last) = pin_last_session_for(&snapshot, &self.id) {
            println!(
                "last_session {} (observed {})",
                last.session_id,
                format_epoch_iso8601(last.observed_epoch),
            );
        }
        // Surface pin-specific diagnostics for this pin (PinAmbiguous,
        // PinDrift, etc.). Each is rendered on its own line so the
        // operator can pipe / grep the output.
        for diagnostic in snapshot
            .diagnostics
            .iter()
            .filter(|d| pin_diagnostic_matches(d, &self.id))
        {
            println!("diagnostic   {}", format_pin_diagnostic(diagnostic));
        }
        Ok(())
    }
}

fn pin_last_session_for<'a>(
    snapshot: &'a GraphSnapshot,
    pin_id: &str,
) -> Option<&'a conspectus::model::PinLastSession> {
    use conspectus::model::Diagnostic;
    snapshot.diagnostics.iter().find_map(|d| match d {
        Diagnostic::PinUnbound {
            pin_id: id,
            last_session: Some(last),
            ..
        } if id == pin_id => Some(last),
        _ => None,
    })
}

fn format_epoch_iso8601(epoch: i64) -> String {
    // Minimal UTC ISO 8601 formatter using the standard library's
    // civil-time algorithm. Mirrors RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`)
    // without pulling in chrono/humantime for a single output line.
    let secs = epoch.max(0) as u64;
    let (year, month, day, hour, minute, second) = civil_from_unix_seconds(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert a Unix epoch (seconds, UTC) into a civil
/// `(year, month, day, hour, minute, second)` tuple. Uses the
/// Hinnant algorithm (Howard Hinnant's `days_from_civil` inverse)
/// so we don't need a date library for one CLI line.
fn civil_from_unix_seconds(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let time_of_day = secs % 86_400;
    let hour = (time_of_day / 3_600) as u32;
    let minute = ((time_of_day % 3_600) / 60) as u32;
    let second = (time_of_day % 60) as u32;

    // Hinnant: days since 1970-01-01 → civil date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (y + if month <= 2 { 1 } else { 0 }) as i32;

    (year, month, day, hour, minute, second)
}

#[derive(Debug, Args)]
struct PinRenameArgs {
    /// Existing pin id.
    id: String,
    /// New pin id. Omit to keep the current id and only change
    /// `--display`.
    new_id: Option<String>,
    /// New display name.
    #[arg(long)]
    display: Option<String>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRenameArgs {
    fn run(self) -> Result<()> {
        if self.new_id.is_none() && self.display.is_none() {
            bail!("`pin rename` requires either a new id, `--display <name>`, or both");
        }
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        let Some((path, mut entry)) =
            load_pin_entry_by_id(&paths, &self.id).map_err(|err| anyhow!(err.to_string()))?
        else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        let original_id = entry.id.clone();
        if let Some(new_display) = self.display {
            entry.display_name = new_display;
        }
        let new_id_value = self.new_id.clone().unwrap_or_else(|| entry.id.clone());

        if new_id_value != original_id {
            // Storage rename: remove the old entry, then upsert the
            // entry under the new id. Both writes target the same
            // store so the operation is a single
            // remove-then-upsert flow.
            entry.id = new_id_value.clone();
            remove_pin_entry(&path, &original_id).map_err(|err| anyhow!(err.to_string()))?;
        }
        let outcome = upsert_pin_entry(&path, entry).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "renamed"
        } else {
            "unchanged"
        };
        if new_id_value != original_id {
            println!(
                "{verb} pin `{}` → `{}` in {}",
                original_id,
                new_id_value,
                path.display()
            );
        } else {
            println!("{verb} pin `{}` in {}", original_id, path.display());
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PinRmArgs {
    /// Pin id to remove.
    id: String,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRmArgs {
    fn run(self) -> Result<()> {
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        for path in &paths {
            let outcome =
                remove_pin_entry(path, &self.id).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                println!("removed pin `{}` from {}", self.id, path.display());
                return Ok(());
            }
        }
        bail!("no pin `{}` in any discovered store", self.id);
    }
}

#[derive(Debug, Args)]
pub struct PinBindArgs {
    /// Pin id to bind.
    id: String,
    /// Harness-native session key of the agent session the pin
    /// should bind to. Must already be visible in discovery.
    #[arg(long = "to", value_name = "SESSION_KEY")]
    to: String,
    /// Optional reason recorded on the declared link.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection for the declared
    /// link write.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinBindArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        // Pin and target must share a harness so the resolver's
        // post-bind attribution sees the LocalDeclared link as the
        // authoritative `LinkedToMux` for the pin's harness.
        let target = snapshot
            .nodes
            .iter()
            .find_map(|node| match node {
                conspectus::model::GraphNode::AgentSession(session)
                    if session.id.harness_key == pin.harness
                        && session.id.session_key == self.to =>
                {
                    Some(session.id.clone())
                }
                _ => None,
            })
            .ok_or_else(|| {
                anyhow!(
                    "no `{}` agent session with session_key `{}` in discovery",
                    pin.harness,
                    self.to
                )
            })?;

        let source_endpoint = DeclaredEndpoint::AgentSession {
            harness_key: target.harness_key.clone(),
            state_scope: target.state_scope.clone(),
            session_key: target.session_key,
        };
        let target_endpoint = DeclaredEndpoint::MuxSession {
            native_id: pin.mux.native_id(),
        };
        let link = DeclaredLink {
            id: format!("pin:{}:bound", pin.id),
            relation: RelationKind::LinkedToMux,
            state: DeclaredLinkState::Active,
            source: source_endpoint,
            target: target_endpoint,
            reason: self.reason,
            overridden_by: None,
            // Operator-facing breadcrumb tying the declared link to
            // its originating pin. Read by `declared list` so
            // operators see WHY this override exists.
            label: Some(format!("pin:{}", pin.id)),
        };

        let path = resolve_write_store(
            self.store,
            Some(&link.source),
            Some(&link.target),
            &self.scan_roots,
        )?;
        let outcome = upsert_declared_link(&path, link).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "wrote"
        } else {
            "unchanged"
        };
        println!(
            "{verb} declared override for pin `{}` → session `{}` in {}",
            pin.id,
            self.to,
            path.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct PinRebindArgs {
    /// Pin id to rebind.
    id: String,
    /// New tmux session name to bind the pin to.
    #[arg(long = "mux", value_name = "NAME")]
    mux: String,
    /// Optional non-default tmux socket. Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinRebindArgs {
    fn run(self) -> Result<()> {
        let paths = candidate_pin_store_paths(&self.scan_roots)?;
        let Some((path, mut entry)) =
            load_pin_entry_by_id(&paths, &self.id).map_err(|err| anyhow!(err.to_string()))?
        else {
            bail!("no pin `{}` in any discovered store", self.id);
        };
        let previous = entry.mux.clone();
        entry.mux = PinMux {
            backend: previous.backend,
            name: self.mux,
            socket_name: self.mux_socket,
        };
        let outcome =
            upsert_pin_entry(&path, entry.clone()).map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "rebound"
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` → mux `{}` in {}",
            entry.id,
            entry.mux.native_id(),
            path.display()
        );
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct PinAdoptArgs {
    /// New pin id.
    id: String,
    /// Existing tmux session name to adopt as this pin's bound mux.
    mux_name: String,
    /// Override the inferred harness when discovery can't or
    /// shouldn't attribute one.
    #[arg(long)]
    harness: Option<String>,
    /// Display name for the new pin. Defaults to `<id>`.
    #[arg(long)]
    display: Option<String>,
    /// Optional non-default tmux socket. Absent ⇒ default socket.
    #[arg(long = "mux-socket", value_name = "NAME")]
    mux_socket: Option<String>,
    /// Override the inferred cwd. By default, adopt uses the mux's
    /// observed cwd; pass `--cwd` to set a different anchor.
    #[arg(long, value_name = "PATH")]
    cwd: Option<PathBuf>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinAdoptArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;

        let backend = TMUX_MUX_BACKEND.to_string();
        let native_id = match self.mux_socket.as_deref() {
            None | Some("default") => format!("{}:{}", backend, self.mux_name),
            Some(socket) => format!("{}:{}:{}", backend, socket, self.mux_name),
        };

        let mux_node = snapshot
            .nodes
            .iter()
            .find_map(|node| match node {
                conspectus::model::GraphNode::MuxSession(mux) if mux.native_id == native_id => {
                    Some(mux)
                }
                _ => None,
            })
            .ok_or_else(|| {
                anyhow!(
                    "no live mux with native_id `{native_id}` — start the tmux session first or rebind to an existing pin"
                )
            })?;

        // Harness inference: walk active LinkedToMux candidates with
        // this mux as the target; the first AgentSession source wins.
        let inferred_harness = snapshot
            .candidate_links
            .iter()
            .filter(|link| {
                link.relation == RelationKind::LinkedToMux
                    && matches!(link.state, conspectus::model::LinkState::Active)
                    && link.target_node_id() == Some(&NodeId::MuxSession(mux_node.id.clone()))
            })
            .find_map(|link| match &link.source {
                NodeId::AgentSession(session) => Some(session.harness_key.clone()),
                _ => None,
            });

        let harness = match (self.harness, inferred_harness) {
            (Some(explicit), _) => explicit,
            (None, Some(inferred)) => inferred,
            (None, None) => bail!(
                "could not infer a harness for mux `{native_id}`; pass `--harness <key>` explicitly"
            ),
        };

        let cwd = match self.cwd {
            Some(explicit) => explicit,
            None => {
                let observed = mux_node.cwd.as_deref().ok_or_else(|| {
                    anyhow!("mux `{native_id}` has no observed cwd; pass `--cwd <PATH>` explicitly")
                })?;
                PathBuf::from(observed)
            }
        };

        let display = self.display.clone().unwrap_or_else(|| self.id.clone());
        let entry = PinEntry {
            id: self.id.clone(),
            display_name: display,
            harness,
            cwd: cwd.display().to_string(),
            mux: PinMux {
                backend,
                name: self.mux_name.clone(),
                socket_name: self.mux_socket.clone(),
            },
            launch: None,
            reason: None,
        };

        let selection = resolve_pin_write_store(self.store, &cwd)?;
        let outcome = upsert_pin_entry(&selection.path, entry.clone())
            .map_err(|err| anyhow!(err.to_string()))?;
        let verb = if outcome.changed {
            "adopted"
        } else {
            "unchanged"
        };
        println!(
            "{verb} pin `{}` from mux `{}` (harness: {}) in {}",
            entry.id,
            entry.mux.native_id(),
            entry.harness,
            selection.path.display()
        );
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PinLaunchIntent {
    /// `pin launch` — operator wants the pin running. Spawn if
    /// needed, then attach.
    Launch,
    /// `pin attach` — operator expects the pin already running but
    /// will accept a fresh spawn if not.
    Attach,
}

#[derive(Debug, Args)]
pub struct PinLaunchArgs {
    /// Pin id to launch / attach.
    id: String,
    /// Skip the terminal hand-off. The new session (if any) is
    /// spawned detached and the attach command is printed for the
    /// operator to run by hand. Useful for scripts and CI dry runs.
    #[arg(long = "no-attach")]
    no_attach: bool,
    /// Root used to discover project-local pin stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl PinLaunchArgs {
    fn run(self, intent: PinLaunchIntent) -> Result<()> {
        let runner = SystemTmux::new();
        self.run_with_runner(intent, &runner)
    }

    fn run_with_runner(self, intent: PinLaunchIntent, runner: &dyn MuxBackend) -> Result<()> {
        let snapshot = discover_and_resolve(&self.scan_roots)?;
        let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == self.id) else {
            bail!("no pin `{}` in any discovered store", self.id);
        };

        let socket = pin.mux.effective_socket();
        let mux_name = pin.mux.name.as_str();
        let argv: Vec<std::ffi::OsString> = pin
            .launch_argv
            .as_ref()
            .map(|argv| argv.iter().map(std::ffi::OsString::from).collect())
            .filter(|argv: &Vec<_>| !argv.is_empty())
            .unwrap_or_else(|| launch_argv_for(&pin.harness));

        if argv.is_empty() {
            bail!(
                "no launch argv configured for harness `{}`; set `launch.argv` on the pin",
                pin.harness
            );
        }

        match pin.binding.as_ref() {
            Some(PinBinding::Bound { mux, session }) => {
                println!(
                    "pin `{}` already bound to session `{}` in mux `{}`",
                    pin.id, session.session_key, mux.native_id
                );
                if !self.no_attach {
                    attach_and_report(runner, socket, mux_name)?;
                }
                Ok(())
            }
            Some(PinBinding::StaleMux { mux }) => {
                println!(
                    "pin `{}` mux `{}` is live but has no `{}` session; relaunching via send-keys",
                    pin.id, mux.native_id, pin.harness
                );
                let literal = format_argv_for_send_keys(&argv);
                let outcome = runner
                    .send_keys(socket, mux_name, &literal, true)
                    .map_err(|err| anyhow!("tmux send-keys failed: {err}"))?;
                report_send_keys(outcome, mux_name)?;
                if !self.no_attach {
                    attach_and_report(runner, socket, mux_name)?;
                }
                Ok(())
            }
            Some(PinBinding::Unbound) | None => {
                if matches!(intent, PinLaunchIntent::Attach) {
                    eprintln!(
                        "conspectus: note: pin `{}` is unbound; falling through to launch",
                        pin.id
                    );
                }
                let cwd = std::path::PathBuf::from(&pin.cwd);
                // ADR 0058 / H-PIN-RESUME-004: consult the
                // per-pin sidecar to splice in resume_argv when a
                // prior session is known and still reachable.
                // Falls back to the default argv on every honest
                // failure path (no sidecar, session missing, fork,
                // harness without resume CLI).
                let effective_argv =
                    resolve_resume_argv(&snapshot, pin, &cwd).unwrap_or_else(|| argv.clone());
                let outcome = runner
                    .new_session(socket, mux_name, &cwd, &effective_argv)
                    .map_err(|err| anyhow!("tmux new-session failed: {err}"))?;
                report_new_session(outcome, mux_name)?;
                if self.no_attach {
                    let attach_cmd = format_attach_command(socket, mux_name);
                    println!("spawned `{mux_name}` (detached); attach with: {attach_cmd}");
                    return Ok(());
                }
                attach_and_report(runner, socket, mux_name)?;
                Ok(())
            }
        }
    }
}

/// Consult the per-pin sidecar (ADR 0058) for a prior session and, when
/// reachable, build the harness's `resume_argv` for it. Returns `None`
/// (so the caller falls back to default argv) on every honest failure
/// mode: no sidecar, recorded session no longer on disk (deletes the
/// sidecar), fork in the lineage chain, or the harness has no
/// resume CLI.
fn resolve_resume_argv(
    snapshot: &GraphSnapshot,
    pin: &conspectus::model::PinCandidate,
    cwd: &std::path::Path,
) -> Option<Vec<std::ffi::OsString>> {
    let cache = conspectus::pin_bindings::PinBindingsCache::from_env();
    cache.directory()?;
    resolve_resume_argv_with_cache(snapshot, pin, cwd, &cache)
}

fn resolve_resume_argv_with_cache(
    snapshot: &GraphSnapshot,
    pin: &conspectus::model::PinCandidate,
    cwd: &std::path::Path,
    cache: &conspectus::pin_bindings::PinBindingsCache,
) -> Option<Vec<std::ffi::OsString>> {
    use conspectus::discovery::harness::resume_argv_for;
    use conspectus::pin_bindings::{LineageOutcome, delete as delete_sidecar, lineage_head, read};

    let record = match read(cache, &pin.id) {
        Ok(Some(record)) => record,
        Ok(None) => return None,
        Err(err) => {
            eprintln!(
                "conspectus: pin `{}`: ignoring unreadable sidecar ({err}); launching fresh",
                pin.id
            );
            return None;
        }
    };

    let head = match lineage_head(snapshot, &record.harness, &record.session_id) {
        LineageOutcome::Head(head) => head,
        LineageOutcome::SessionMissing => {
            // ADR 0058 Q7: stale sidecar — recorded session can't
            // be found anywhere in the current snapshot. Delete it
            // so it doesn't keep producing this hint on subsequent
            // launches.
            match delete_sidecar(cache, &pin.id) {
                Ok(_) => eprintln!(
                    "conspectus: pin `{}`: previous session `{}` no longer exists; \
                     cleared sidecar, launching fresh",
                    pin.id, record.session_id
                ),
                Err(err) => eprintln!(
                    "conspectus: pin `{}`: previous session `{}` no longer exists; \
                     could not clear sidecar ({err}); launching fresh",
                    pin.id, record.session_id
                ),
            }
            return None;
        }
        LineageOutcome::Fork { at, successors } => {
            eprintln!(
                "conspectus: pin `{}`: session `{}` has {} compacted successors; \
                 launching fresh — pick one with `conspectus session continue <id>` or \
                 resume manually",
                pin.id,
                at.session_key,
                successors.len(),
            );
            return None;
        }
    };

    match resume_argv_for(&pin.harness, &head.session_key, cwd) {
        Some(argv) => {
            println!(
                "pin `{}`: resuming recorded session `{}`",
                pin.id, head.session_key
            );
            Some(argv)
        }
        None => {
            eprintln!(
                "conspectus: pin `{}`: harness `{}` does not expose a resume command; \
                 launching fresh",
                pin.id, pin.harness
            );
            None
        }
    }
}

fn attach_and_report(runner: &dyn MuxBackend, socket: Option<&str>, name: &str) -> Result<()> {
    let outcome = runner
        .attach_session(socket, name)
        .map_err(|err| anyhow!("tmux attach failed: {err}"))?;
    match outcome {
        TmuxAttachOutcome::Detached => Ok(()),
        TmuxAttachOutcome::NoTarget => {
            bail!("tmux session `{name}` vanished between launch and attach (race) — try again")
        }
        TmuxAttachOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxAttachOutcome::Failed { code, message } => {
            bail!("tmux attach failed (exit {code:?}): {message}")
        }
        TmuxAttachOutcome::Unsupported => {
            bail!("this tmux runner does not support attach; run `tmux attach -t {name}` manually")
        }
    }
}

fn report_send_keys(outcome: TmuxSendKeysOutcome, name: &str) -> Result<()> {
    match outcome {
        TmuxSendKeysOutcome::Sent => Ok(()),
        TmuxSendKeysOutcome::NoTarget => {
            bail!("tmux session `{name}` disappeared before send-keys reached it")
        }
        TmuxSendKeysOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxSendKeysOutcome::Failed { code, message } => {
            bail!("tmux send-keys failed (exit {code:?}): {message}")
        }
        TmuxSendKeysOutcome::Unsupported => bail!("this tmux runner does not support send-keys"),
    }
}

fn report_new_session(outcome: TmuxNewSessionOutcome, name: &str) -> Result<()> {
    match outcome {
        TmuxNewSessionOutcome::Created => Ok(()),
        TmuxNewSessionOutcome::NameTaken => bail!(
            "a tmux session named `{name}` already exists; rename the existing tmux or pick a different `mux.name`"
        ),
        TmuxNewSessionOutcome::Unavailable(reason) => bail!(
            "tmux is unavailable on this host: {reason}",
            reason = reason.as_str()
        ),
        TmuxNewSessionOutcome::Failed { code, message } => {
            bail!("tmux new-session failed (exit {code:?}): {message}")
        }
        TmuxNewSessionOutcome::Unsupported => {
            bail!("this tmux runner does not support new-session")
        }
    }
}

/// Build the `tmux [-L <socket>] attach-session -t <name>` command
/// string used in `--no-attach` output. Single-arg quoting is
/// intentionally minimal — `mux.name` is operator-typed and the
/// schema rejects shell metacharacters by virtue of tmux's own
/// session-name rules (alphanumeric + a small set of punctuation).
fn format_attach_command(socket: Option<&str>, name: &str) -> String {
    match socket {
        None => format!("tmux attach-session -t {name}"),
        Some(socket) => format!("tmux -L {socket} attach-session -t {name}"),
    }
}

/// Render an argv slice as a tmux `send-keys` literal so the
/// harness command lands in the existing pane. We quote each
/// arg with double-quotes when it contains spaces; this matches
/// the way operators would type the command interactively.
fn format_argv_for_send_keys(argv: &[std::ffi::OsString]) -> String {
    argv.iter()
        .map(|token| {
            let token = token.to_string_lossy();
            if token.is_empty() || token.contains(char::is_whitespace) {
                format!("\"{token}\"")
            } else {
                token.into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_pin_write_store(
    flag: Option<DeclaredStoreFlag>,
    cwd: &Path,
) -> Result<PinStoreSelection> {
    let loader = ConfigLoader::from_env();
    match flag {
        Some(DeclaredStoreFlag::User) => {
            user_pin_store(&loader).map_err(|err| anyhow!(err.to_string()))
        }
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for `pin create` (pick `project` or `user`)");
        }
        Some(DeclaredStoreFlag::Project) | None => {
            select_store_for_pin(cwd, &loader).map_err(|err| anyhow!(err.to_string()))
        }
    }
}

/// Candidate stores `pin rename` / `pin rm` should look in. Order
/// matters: project before user so a project-local pin shadows a
/// same-id user pin, matching the resolver's local-over-global rule.
fn candidate_pin_store_paths(scan_roots: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if let Some(path) = loader.locate_project_config(&root)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }
    if let Some(path) = loader.user_config_path()
        && seen.insert(path.clone())
    {
        paths.push(path);
    }
    Ok(paths)
}

fn discover_and_resolve(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let snapshot = discover_for_store_selection(scan_roots)?;
    let mut resolved = conspectus::resolve::resolve_snapshot(snapshot);
    record_pin_bindings_best_effort(&resolved);
    decorate_unbound_pins_best_effort(&mut resolved);
    Ok(resolved)
}

/// Enrich `PinUnbound` diagnostics with the sidecar's recorded
/// last-bound session so the launch path's UX surfaces
/// (`pin show`, TUI hint, right detail pane) can advertise resume
/// affordances. Mirrors `record_pin_bindings_best_effort`: silent
/// no-op when the cache root is absent.
fn decorate_unbound_pins_best_effort(snapshot: &mut GraphSnapshot) {
    use conspectus::pin_bindings::{PinBindingsCache, decorate_unbound_diagnostics};
    let cache = PinBindingsCache::from_env();
    if cache.directory().is_none() {
        return;
    }
    decorate_unbound_diagnostics(snapshot, &cache);
}

/// Record the resolver's pin bindings to per-pin sidecar files
/// (ADR 0058 / H-PIN-RESUME-003). Best-effort — sidecar I/O failures
/// log to stderr and never propagate up through discovery, so a
/// missing cache directory or read-only mount degrades pin launch's
/// continuity story without breaking the cycle.
fn record_pin_bindings_best_effort(snapshot: &GraphSnapshot) {
    use conspectus::pin_bindings::{PinBindingsCache, record_bindings};
    let cache = PinBindingsCache::from_env();
    if cache.directory().is_none() {
        // No $XDG_CACHE_HOME, no $HOME — silently skip rather than
        // log on every cycle. Operators without a cache directory
        // opt out of the continuity feature implicitly.
        return;
    }
    let now = conspectus::hook::current_epoch();
    for (pin_id, result) in record_bindings(snapshot, &cache, now) {
        if let Err(err) = result {
            eprintln!("conspectus: pin-binding sidecar write failed for `{pin_id}`: {err}");
        }
    }
}

fn pin_state_label(binding: Option<&PinBinding>) -> &'static str {
    match binding {
        Some(PinBinding::Bound { .. }) => "bound",
        Some(PinBinding::StaleMux { .. }) => "stale",
        Some(PinBinding::Unbound) => "unbound",
        None => "unresolved",
    }
}

fn pin_matches_filter(binding: Option<&PinBinding>, filter: PinStateFilter) -> bool {
    match filter {
        PinStateFilter::All => true,
        PinStateFilter::Bound => matches!(binding, Some(PinBinding::Bound { .. })),
        PinStateFilter::Stale => matches!(binding, Some(PinBinding::StaleMux { .. })),
        PinStateFilter::Unbound => matches!(binding, Some(PinBinding::Unbound) | None),
    }
}

fn pin_store_label(kind: PinStoreKind) -> &'static str {
    match kind {
        PinStoreKind::Project => "project",
        PinStoreKind::User => "user",
    }
}

fn bound_session_label(binding: Option<&PinBinding>) -> Option<String> {
    match binding {
        Some(PinBinding::Bound { session, .. }) => Some(session.session_key.clone()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pin_row(
    store: DeclaredStoreFlag,
    provenance: Provenance,
    id: &str,
    display_name: &str,
    harness: &str,
    cwd: &str,
    mux_native_id: String,
    bound_session: String,
    state_label: &str,
    store_path: &str,
) -> String {
    [
        store_label(store).to_string(),
        provenance_label(provenance).to_string(),
        state_label.to_string(),
        id.to_string(),
        display_name.to_string(),
        harness.to_string(),
        cwd.to_string(),
        mux_native_id,
        bound_session,
        store_path.to_string(),
    ]
    .join("\t")
}

fn pin_diagnostic_matches(diagnostic: &conspectus::model::Diagnostic, pin_id: &str) -> bool {
    use conspectus::model::Diagnostic;
    match diagnostic {
        Diagnostic::PinUnbound { pin_id: id, .. }
        | Diagnostic::PinStaleMux { pin_id: id, .. }
        | Diagnostic::PinAmbiguous { pin_id: id, .. }
        | Diagnostic::PinDrift { pin_id: id, .. } => id == pin_id,
        _ => false,
    }
}

fn format_pin_diagnostic(diagnostic: &conspectus::model::Diagnostic) -> String {
    use conspectus::model::Diagnostic;
    match diagnostic {
        Diagnostic::PinUnbound {
            expected_mux_native_id,
            ..
        } => format!("unbound (no live mux matching `{expected_mux_native_id}`)"),
        Diagnostic::PinStaleMux { mux, .. } => {
            format!("stale_mux (mux `{}` has no live harness)", mux.native_id)
        }
        Diagnostic::PinAmbiguous {
            chosen, competing, ..
        } => format!(
            "ambiguous (chose `{}`; {} competing: [{}])",
            chosen.session_key,
            competing.len(),
            competing
                .iter()
                .map(|s| s.session_key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Diagnostic::PinDrift {
            declared_cwd,
            observed_cwd,
            ..
        } => format!("drift (declared `{declared_cwd}`, observed `{observed_cwd}`)"),
        _ => format!("{diagnostic:?}"),
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Dot,
    Html,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum InclusionFlag {
    Include,
    Exclude,
}

impl From<InclusionFlag> for conspectus::output::Inclusion {
    fn from(flag: InclusionFlag) -> Self {
        match flag {
            InclusionFlag::Include => Self::Include,
            InclusionFlag::Exclude => Self::Exclude,
        }
    }
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
        link.source.compact_label(),
        link.target.compact_label(),
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
        Provenance::LocalPin => "local_pin",
        Provenance::GlobalDeclared => "global_declared",
        Provenance::GlobalPin => "global_pin",
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
        DeclaredEndpoint::parse_compact(raw).map(Self)
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
        "created_checkout" => Ok(RelationKind::CreatedCheckout),
        "referenced_checkout" => Ok(RelationKind::ReferencedCheckout),
        "parent_session" => Ok(RelationKind::ParentSession),
        "child_session" => Ok(RelationKind::ChildSession),
        "created_branch" => Ok(RelationKind::CreatedBranch),
        "associated_branch" => Ok(RelationKind::AssociatedBranch),
        "parent_fork" => Ok(RelationKind::ParentFork),
        "rooted_at_path" => Ok(RelationKind::RootedAtPath),
        "mux_contains_process" => Ok(RelationKind::MuxContainsProcess),
        "process_identifies_session" => Ok(RelationKind::ProcessIdentifiesSession),
        "process_candidates_session" => Ok(RelationKind::ProcessCandidatesSession),
        _ => Err(format!(
            "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
        )),
    }
}

fn relation_label(relation: &RelationKind) -> &'static str {
    relation.snake_case()
}

// H-REF-001: `parse_endpoint` + `endpoint_label` moved to
// `DeclaredEndpoint::parse_compact` / `compact_label` in
// `crate::declared`. Callers in this module use the methods
// directly.

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

/// Read-only variant of discovery used by the declared/pin
/// store-selection helpers. Pre-P11-011a this loaded the
/// previous graph.sqlite as the warm-start prior; with
/// graph.sqlite retired it falls through to a cold rebuild.
/// The helper deliberately skips the writer side regardless —
/// this is a transient pre-write probe, not the user's primary
/// artifact, and rewriting the cache from a CRUD-adjacent code
/// path would surprise operators who expected the cache to
/// track their last render.
fn discover_for_store_selection(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    let loader = ConfigLoader::from_env();
    let outcome = loader.load_from(&cwd);
    let discovery_config = conspectus::discovery::LocalDiscoveryConfig::from_env();
    conspectus::discovery::discover_local_warm_with(
        roots,
        discovery_config,
        GraphSnapshot::empty(),
        &outcome.config.server.intervals,
    )
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
