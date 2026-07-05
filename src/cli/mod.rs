use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, Stdio};
use std::time::Duration;

use conspectus::config::{self, ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{
    DeclaredEndpoint, DeclaredStoreKind, DeclaredStoreSelection, select_store_for_declaration,
};
use conspectus::discovery::tmux::MuxBackend;
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

// H-REF-006 wave 1: `ColumnsArgs` moved to `cli/columns.rs`.
mod columns;
use columns::ColumnsArgs;

// H-REF-006 wave 2: `RenameArgs` + subtree moved to `cli/rename.rs`.
mod rename;
use rename::RenameArgs;

// H-REF-006 wave 3: `DevArgs` subtree moved to `cli/dev.rs`.
// Gated at the module level via `#![cfg(debug_assertions)]`
// inside `dev.rs`; release builds don't compile it.
#[cfg(debug_assertions)]
mod dev;
#[cfg(debug_assertions)]
use dev::DevArgs;

// H-REF-006 wave 7: `ServeArgs`, `RefreshArgs`, `StatusArgs`
// subtrees moved to `cli/lifecycle.rs`.
mod lifecycle;
use lifecycle::{RefreshArgs, ServeArgs, StatusArgs};

// H-REF-006 wave 4: `HookArgs` subtree moved to `cli/hook.rs`.
mod hook;
use hook::HookArgs;

// H-REF-006 wave 5: `NodeArgs` subtree moved to `cli/node.rs`.
mod node;
use node::NodeArgs;

// H-REF-006 wave 6: `GraphArgs` moved to `cli/graph.rs`.
mod graph;
use graph::GraphArgs;

// H-REF-006 wave 8: `TableArgs` subtree moved to `cli/table.rs`.
mod table;
use table::TableArgs;

// H-REF-006 wave 9: `AliasArgs` subtree moved to `cli/alias.rs`.
mod alias;
use alias::AliasArgs;

// H-REF-006 wave 11: declared subtree moved to `cli/declared.rs`.
mod declared;
use declared::DeclaredArgs;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum LayoutFlag {
    #[default]
    Columnar,
    Card,
}

pub(super) fn current_unix_epoch_for_table() -> Option<i64> {
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
pub(super) enum ViewFlag {
    #[default]
    Sessions,
    Mux,
    Union,
    Prs,
    Forks,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(super) enum SortFlag {
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
    /// Per-view grouping. Accepted values depend on `--view`; see
    /// `conspectus tui --help` for the per-view list (ADR 0031).
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
mod tests;
// H-REF-006 wave 2 continued: rename subtree moved out;
// `resolve_alias_store` promoted to `pub(super)` for the
// rename module below.

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

// H-REF-006 wave 10: pin subtree moved to `cli/pin.rs`.
mod pin;
use pin::PinArgs;

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

/// Borrow checker convenience: lets us reuse the existing
/// [`DeclaredStoreSelection`] type for emitted CLI messages.
fn _selection_display(selection: &DeclaredStoreSelection) -> String {
    let kind = match selection.kind {
        DeclaredStoreKind::Project => "project",
        DeclaredStoreKind::User => "user",
    };
    format!("{kind} {}", selection.path.display())
}
