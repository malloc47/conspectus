//! `conspectus tui` command.
//!
//! Interactive TUI entry point plus its associated
//! `SnapshotPaneFlag` / `SessionsGroupingFlag` value enums,
//! duration parsers, and tmux-context helpers.

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::{Args, ValueEnum};

use conspectus::config;
use conspectus::discovery::tmux::MuxBackend;

use super::{ColorFlag, FilterArgs, SortFlag, ViewFlag, resolve_color_from_env, view_from_flag};

#[derive(Debug, Args)]
pub(super) struct TuiArgs {
    /// Discovery scan root. Repeatable. Defaults to the current
    /// working directory when omitted.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
    /// Initial view. When omitted, the view you last used wins, then
    /// `[tui].default_view`, then `sessions`.
    #[arg(long, value_enum)]
    view: Option<ViewFlag>,
    /// Don't restore or remember the last-used view for this run.
    /// The session still uses the standard precedence
    /// (`--view` > config default > `sessions`) for its starting
    /// view but does not write the sidecar on view switches.
    /// `--snapshot` implies this automatically.
    #[arg(long = "no-resume-view")]
    no_resume_view: bool,
    /// Deprecated alias for `--grouping` when `--view sessions` is
    /// active. Continues to work but emits a one-line
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
    pub(super) refresh_interval: String,
    /// Selected mux pane capture cadence.
    #[arg(
        long = "mux-preview-interval",
        value_name = "DURATION",
        default_value = "2s"
    )]
    pub(super) mux_preview_interval: String,
    /// Suppress live extras: mux pane capture and transcript-tail
    /// reads. Graph-resident previews continue to render.
    #[arg(long = "no-live-preview")]
    no_live_preview: bool,
    /// Don't write each refreshed graph to the `graph.bin` cache.
    #[arg(long = "no-cache")]
    no_cache: bool,
    /// Ignore a running `conspectus serve` daemon and rebuild the
    /// graph in-process on every refresh.
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

pub(super) fn view_flag_label(view: conspectus::tui::View) -> &'static str {
    match view {
        conspectus::tui::View::Sessions => "sessions",
        conspectus::tui::View::Mux => "mux",
        conspectus::tui::View::Union => "union",
        conspectus::tui::View::Prs => "prs",
        conspectus::tui::View::Forks => "forks",
    }
}

impl TuiArgs {
    pub(super) fn run(self) -> Result<()> {
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

        // View precedence:
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
        let configured_view = outcome
            .config
            .tui
            .default_view
            .unwrap_or(conspectus::tui::View::Sessions);
        let view = if let Some(flag) = self.view {
            view_from_flag(flag)
        } else if !suppress_resume {
            let cache = conspectus::tui_state::TuiStateCache::from_env();
            conspectus::tui_state::read_last_view(&cache).unwrap_or(configured_view)
        } else {
            configured_view
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
            // sessions grouping along.
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
            // Startup default; the operator's last
            // choice is restored from persisted TUI state on top of
            // this, mirroring how `sort` flows.
            default_mux_recency: conspectus::tui::MuxRecency::default(),
            narrow_layout_threshold: outcome.config.tui.narrow_layout_threshold,
            intervals: outcome.config.server.intervals,
            no_cache: self.no_cache,
            refresh: self.refresh,
            discovery_caches: std::sync::Arc::default(),
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
    // Reuse the trait-based probe. Kept as a
    // separate helper because the TUI runtime consumes the
    // `Option<String>` shape directly (for the self-attach
    // guard); constructing the full HookTmuxRecord here would
    // be wasted work.
    let backend = conspectus::discovery::tmux::SystemTmux::new();
    backend
        .current_session_context()
        .and_then(|ctx| ctx.session_name)
}

pub(super) fn parse_filter_duration(input: &str) -> Result<Duration, String> {
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

pub(super) fn parse_tui_duration(input: &str) -> Result<Duration, String> {
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
