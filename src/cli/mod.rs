use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, Stdio};

use conspectus::config::{ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{DeclaredEndpoint, select_store_for_declaration};
use conspectus::model::{GraphSnapshot, Provenance};

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
            Command::Mux(args) => args.run(),
            Command::Worktree(args) => args.run(),
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
    /// Author, launch, or inspect session pins. See `docs/pins-walkthrough.md`.
    Pin(Box<PinArgs>),
    /// Start a tmux session: a bare shell (`new`) or a harness with no
    /// pin (`launch`).
    Mux(MuxArgs),
    /// List, create, and tear down git worktrees. `list` is read-only;
    /// new / rm / merge / close / prune delegate to the configured
    /// mutation backend (worktrunk). See `docs/worktrees.md`.
    Worktree(WorktreeArgs),
    /// Run the long-lived background daemon that keeps the
    /// resolved graph snapshot warm between one-shot CLI
    /// invocations.
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

mod columns;
use columns::ColumnsArgs;

mod rename;
use rename::RenameArgs;

// Debug builds only: `dev.rs` is gated with `#![cfg(debug_assertions)]`.
#[cfg(debug_assertions)]
mod dev;
#[cfg(debug_assertions)]
use dev::DevArgs;

mod lifecycle;
use lifecycle::{RefreshArgs, ServeArgs, StatusArgs};

mod hook;
use hook::HookArgs;

mod node;
use node::NodeArgs;

mod graph;
use graph::GraphArgs;

mod table;
use table::TableArgs;

mod alias;
use alias::AliasArgs;

mod declared;
use declared::DeclaredArgs;

mod tui;
use tui::TuiArgs;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum LayoutFlag {
    #[default]
    Columnar,
    Card,
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
pub(super) fn warm_start_discover_and_resolve(
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
    let ClientOutcome::Ok(bytes) = client_snapshot() else {
        return None;
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
pub(super) fn cache_resolved_snapshot(snapshot: &conspectus::model::GraphSnapshot, no_cache: bool) {
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
/// `--color` flag value. The renderer ultimately consumes a `bool`;
/// the value enum exists to give clap a stable parse surface and so
/// we can document the per-token semantics in `--help`.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum ColorFlag {
    /// Auto-detect: color when stdout is a TTY, unless `NO_COLOR`,
    /// `CLICOLOR=0`, or `TERM=dumb` opts out (`CLICOLOR_FORCE` opts in).
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
pub(super) fn resolve_color_from_env(flag: ColorFlag, stdout_is_tty: bool) -> bool {
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

/// Whether and how to page rendered output (H-TBL-013).
#[derive(Debug, Clone, Copy)]
pub(super) struct PagerOptions {
    /// `--pager` forces pager even when stdout is not a TTY (useful
    /// for `PAGER=cat` integration tests).
    force_on: bool,
    /// `--no-pager` skips pager even on a TTY.
    force_off: bool,
}

impl PagerOptions {
    pub(super) fn from_flags(pager: bool, no_pager: bool) -> Self {
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
pub(super) fn print_paged(content: &str, options: PagerOptions) {
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

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum ViewFlag {
    #[default]
    Sessions,
    Mux,
    // Hidden from the TUI's view switcher (H-VIEW-001); still accepted
    // so existing scripts and configs keep working.
    #[value(hide = true)]
    Union,
    #[value(hide = true)]
    Prs,
    #[value(hide = true)]
    Forks,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum SortFlag {
    #[default]
    Hierarchy,
    Recency,
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
pub(super) struct FilterArgs {
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
    /// Grouping for the view or row type. Valid values differ per
    /// view; an invalid value errors with the list of valid ones.
    #[arg(long = "grouping", value_name = "VALUE")]
    grouping: Option<String>,
}

impl FilterArgs {
    /// Convert the raw flag values into a [`conspectus::filter::RowFilter`].
    /// Returns an error when a value fails to parse (max-age
    /// duration, mux-state spelling).
    pub(super) fn to_row_filter(&self) -> Result<conspectus::filter::RowFilter> {
        use conspectus::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
        let harness = if self.harness.is_empty() {
            None
        } else {
            Some(HarnessFilter::from_values(self.harness.iter()))
        };
        let max_age = match self.max_age.as_deref() {
            Some(raw) => Some(
                tui::parse_filter_duration(raw)
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
    pub(super) fn to_grouping(
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
                    tui::view_flag_label(view)
                )
            })
    }
}

pub(super) fn view_from_flag(flag: ViewFlag) -> conspectus::tui::View {
    match flag {
        ViewFlag::Sessions => conspectus::tui::View::Sessions,
        ViewFlag::Mux => conspectus::tui::View::Mux,
        ViewFlag::Union => conspectus::tui::View::Union,
        ViewFlag::Prs => conspectus::tui::View::Prs,
        ViewFlag::Forks => conspectus::tui::View::Forks,
    }
}

#[cfg(debug_assertions)]
pub(super) fn apply_grouping_to_tui_config(
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

/// Parse a duration like [`parse_tui_duration`] but accept a `d`
/// (days) suffix. Used by `--max-age` where day-scale windows are
/// common; `parse_tui_duration` deliberately rejects `d` because
/// day-scale refresh intervals don't make sense.
/// Parse a small subset of duration strings: `<integer><ms|s|m|h>`.
/// Kept in-tree to avoid pulling in `humantime` for the TUI flag
/// surface; revisit if more formats are needed.
#[cfg(test)]
mod tests;

pub(super) fn resolve_alias_store(
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

mod pin;
use pin::PinArgs;

// H-MUX-NEW-001 / ADR 0095: `conspectus mux new` subcommand.
mod mux;
use mux::MuxArgs;

// H-WT-002: `conspectus worktree` subcommand.
mod worktree;
use worktree::WorktreeArgs;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    Json,
    Dot,
    Html,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum InclusionFlag {
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
pub(super) enum DeclaredStoreFlag {
    All,
    Project,
    User,
}

/// `--store` for commands that write a new entry. `all` names no single
/// destination, so only `project` and `user` are offered.
#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum WriteStoreFlag {
    Project,
    User,
}

impl From<WriteStoreFlag> for DeclaredStoreFlag {
    fn from(flag: WriteStoreFlag) -> Self {
        match flag {
            WriteStoreFlag::Project => Self::Project,
            WriteStoreFlag::User => Self::User,
        }
    }
}

pub(super) fn store_label(store: DeclaredStoreFlag) -> &'static str {
    match store {
        DeclaredStoreFlag::All => "all",
        DeclaredStoreFlag::Project => "project",
        DeclaredStoreFlag::User => "user",
    }
}

pub(super) fn provenance_label(provenance: Provenance) -> &'static str {
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

pub(super) fn project_store_path(scan_roots: &[PathBuf]) -> Result<PathBuf> {
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
pub(super) fn effective_scan_roots(scan_roots: &[PathBuf], cwd: &Path) -> Vec<PathBuf> {
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
pub(super) fn discover_for_store_selection(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
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
pub(super) fn candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let mut paths = Vec::new();

    let include_project = matches!(
        store,
        None | Some(DeclaredStoreFlag::All | DeclaredStoreFlag::Project)
    );
    let include_user = matches!(
        store,
        None | Some(DeclaredStoreFlag::All | DeclaredStoreFlag::User)
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
