//! Conspectus configuration loading.
//!
//! See ADR 0012 for the layout and precedence rules and ADR 0021 for the
//! `[table.<rows>]` schema introduced alongside `conspectus table
//! <ROWS>`. The CLI typically calls [`load_from_cwd`], which walks the
//! current directory upward looking for `.conspectus.toml` and merges
//! that on top of the user-level config. Tests usually construct a
//! [`ConfigLoader`] directly so they can inject paths and a fake
//! `$HOME` boundary.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use std::collections::BTreeMap;

use crate::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
use crate::tui::icons::parse_icon_override;
use crate::tui::theme::{Theme, ThemeKeyKind, parse_color, parse_modifier, parse_style_spec};
use crate::tui::{Grouping, View};

/// Filename Conspectus looks for in project trees.
pub const PROJECT_CONFIG_FILENAME: &str = ".conspectus.toml";

/// Path under `$XDG_CONFIG_HOME` (or the platform equivalent) where
/// the user-level config lives.
pub const USER_CONFIG_RELATIVE: &str = "conspectus/config.toml";

/// Default terminal width (columns) at/above which the TUI keeps its
/// side-by-side left/right panes, and below which it reflows them to a
/// vertical stack (H-LAYOUT-001). Operators override this with
/// `[tui] narrow_layout_threshold`. This is the canonical home for the
/// value; the renderer reads the resolved [`TuiConfig`] field rather
/// than a hard-coded constant.
pub const DEFAULT_NARROW_LAYOUT_THRESHOLD: u16 = 100;

/// Resolved configuration after project + user + defaults are merged.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub table: TableConfig,
    pub tui: TuiConfig,
    pub server: ServerConfig,
    pub worktree: WorktreeConfig,
}

/// Settings under `[worktree]` (H-WT-003). Governs which backend
/// performs worktree mutation (create / remove) in the CLI / TUI;
/// discovery always lists via the read-only git backend.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorktreeConfig {
    /// `[worktree] backend = "auto" | "git" | "worktrunk"`. Defaults to
    /// `auto` (worktrunk when `wt` is on PATH, else read-only).
    pub backend: crate::discovery::worktree::WorktreeBackendSelection,
}

/// Settings under `[server]`. Configures both the `conspectus
/// serve` daemon (P7-006) and the one-shot CLI's warm-start TTL
/// gate (P7-003 phase 3) per ADR 0079. The shared shape is the
/// whole point: a single per-class number controls both
/// "refresh this often" and "stale after this long" — recording
/// the same value twice would drift.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerConfig {
    pub intervals: ServerIntervals,
}

/// Per-provider-class refresh / TTL durations.
///
/// Defaults match ADR 0038 (`harness=5s`, `mux=5s`, `git=30s`,
/// `forge=5m`). The four classes collapse the granular per-emit
/// provider strings (`git`, `git::cwd`, `tmux`, `github`,
/// `claude-code`, …) per the mapping in ADR 0079.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerIntervals {
    pub harness: Duration,
    pub mux: Duration,
    pub git: Duration,
    pub forge: Duration,
}

impl Default for ServerIntervals {
    fn default() -> Self {
        Self {
            harness: Duration::from_secs(5),
            mux: Duration::from_secs(5),
            git: Duration::from_secs(30),
            forge: Duration::from_secs(300),
        }
    }
}

/// Settings under `[tui]` in `.conspectus.toml` / user config.
///
/// `Default` is hand-written (rather than derived) so
/// `narrow_layout_threshold` starts at
/// [`DEFAULT_NARROW_LAYOUT_THRESHOLD`] instead of `0`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TuiConfig {
    /// Scan roots for `conspectus tui` discovery. Empty means "use
    /// the process cwd" (the v1-default behavior). `~` and `~/<rel>`
    /// tokens are expanded against the loader's home directory at
    /// merge time, so the on-disk form stays portable across
    /// machines. CLI `--scan-root` flags override this list when
    /// present.
    pub scan_roots: Vec<PathBuf>,
    /// Per-view grouping and filter defaults (ADR 0031). Configured
    /// under `[tui.views.<name>]` sub-tables. CLI flags override
    /// these when present.
    pub views: TuiViewsConfig,
    /// Detail-pane (right pane) configuration (T8-042). Configured
    /// under `[tui.detail]` in the on-disk config.
    pub detail: TuiDetailConfig,
    /// Resolved color theme (ADR 0032). Built from `[tui.theme]` with
    /// unspecified entries falling back to [`Theme::default`].
    pub theme: Theme,
    /// Opt-in display of per-harness count chips in the top header
    /// (H-UI-004 audit). Default `false`: row badges already carry
    /// per-session identity and group summaries carry per-group
    /// totals. Operators who want the aggregate set
    /// `[tui] show_harness_chips = true` in their config.
    pub show_harness_chips: bool,
    /// Terminal width (columns) below which the body reflows from
    /// side-by-side panes to a vertical stack (H-LAYOUT-001). Sourced
    /// from `[tui] narrow_layout_threshold`; defaults to
    /// [`DEFAULT_NARROW_LAYOUT_THRESHOLD`].
    pub narrow_layout_threshold: u16,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            scan_roots: Vec::new(),
            views: TuiViewsConfig::default(),
            detail: TuiDetailConfig::default(),
            theme: Theme::default(),
            show_harness_chips: false,
            narrow_layout_threshold: DEFAULT_NARROW_LAYOUT_THRESHOLD,
        }
    }
}

/// Settings under `[tui.detail]` in `.conspectus.toml` / user config
/// (T8-042). The detail pane is the right-panel graph explorer; this
/// block controls its visual defaults.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TuiDetailConfig {
    /// When `true`, the link rows in the explorer render the
    /// `provenance · confidence · state` trailing meta line by
    /// default. When `false` (the default), the meta line is
    /// suppressed and the operator can flip it on per-session with
    /// the `E` accelerator. Per T8-042 the right pane is primarily a
    /// graph-navigation surface; the edge-meta detail is opt-in for
    /// operators actively diagnosing resolver decisions.
    pub show_edge_meta: bool,
}

/// Per-view configuration block — one entry per registered view.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TuiViewsConfig {
    pub sessions: TuiViewConfig,
    pub mux: TuiViewConfig,
    pub union: TuiViewConfig,
    pub prs: TuiViewConfig,
    pub forks: TuiViewConfig,
}

impl TuiViewsConfig {
    /// Read-only access to a view's config slice.
    pub fn for_view(&self, view: View) -> &TuiViewConfig {
        match view {
            View::Sessions => &self.sessions,
            View::Mux => &self.mux,
            View::Union => &self.union,
            View::Prs => &self.prs,
            View::Forks => &self.forks,
        }
    }
}

/// Settings for a single TUI view. Both fields are optional so an
/// absent block means "use the defaults baked into the runtime
/// (ADR 0031)" without forcing every config file to enumerate
/// every view.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TuiViewConfig {
    /// Initial grouping for this view. `None` falls back to
    /// [`Grouping::default_for`].
    pub grouping: Option<Grouping>,
    /// Active row filter for this view. Empty filter (the
    /// [`RowFilter::default`] value) means "no constraint".
    pub filter: RowFilter,
}

/// Per-row-type settings under `[table.<rows>]`. Each row-type gets
/// its own [`TableRowConfig`] so H-TBL-007 column registries land in a
/// single, predictable slot.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableConfig {
    pub sessions: TableRowConfig,
    pub mux: TableRowConfig,
    pub union: TableRowConfig,
    pub prs: TableRowConfig,
    pub forks: TableRowConfig,
}

/// Settings for one row-type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableRowConfig {
    /// Explicit column list for this row-type. `None` falls back to
    /// the row-type's registered default columns. The CLI `--columns`
    /// flag overrides this when both are present.
    pub columns: Option<Vec<String>>,
}

/// Table row-type. Matches the `conspectus table <ROWS>` positional
/// (ADR 0021) and the [`crate::output::table`] renderer's row-type
/// discriminator (ADR 0006).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Projection {
    #[default]
    Agent,
    Mux,
    Union,
    Pr,
    Fork,
}

impl Projection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Mux => "mux",
            Self::Union => "union",
            Self::Pr => "prs",
            Self::Fork => "forks",
        }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "agent" | "sessions" => Ok(Self::Agent),
            "mux" => Ok(Self::Mux),
            "union" => Ok(Self::Union),
            "pr" | "prs" => Ok(Self::Pr),
            "fork" | "forks" => Ok(Self::Fork),
            other => Err(anyhow!(
                "invalid table row-type `{other}`; expected one of sessions, mux, union, prs, forks"
            )),
        }
    }
}

/// Disk-shape of `.conspectus.toml` / user config. Kept private so
/// the merged [`Config`] is the only thing the rest of the crate sees.
#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    table: Option<TableFile>,
    #[serde(default)]
    tui: Option<TuiFile>,
    /// `[server]` table per ADR 0038 + ADR 0079. Optional; absence
    /// keeps the [`ServerIntervals::default`] values.
    #[serde(default)]
    server: Option<ServerFile>,
    /// `[worktree]` table (H-WT-003).
    #[serde(default)]
    worktree: Option<WorktreeFile>,
    /// Legacy `[session]` key from before ADR 0021. Its presence
    /// triggers a diagnostic so users discover the schema migrated;
    /// its contents are not read.
    #[serde(default)]
    session: Option<toml::Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct WorktreeFile {
    #[serde(default)]
    backend: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ServerFile {
    #[serde(default)]
    intervals: Option<ServerIntervalsFile>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ServerIntervalsFile {
    #[serde(default)]
    harness: Option<String>,
    #[serde(default)]
    mux: Option<String>,
    #[serde(default)]
    git: Option<String>,
    #[serde(default)]
    forge: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TuiFile {
    #[serde(default)]
    scan_roots: Option<Vec<String>>,
    /// Legacy alias for `[tui.views.sessions].grouping` (ADR 0031).
    /// Presence emits a deprecation diagnostic; value still merges
    /// into the new schema as long as the new key is absent.
    #[serde(default)]
    sessions_grouping: Option<String>,
    #[serde(default)]
    views: Option<TuiViewsFile>,
    /// `[tui.detail]` table (T8-042). Right-panel detail-explorer
    /// visual defaults.
    #[serde(default)]
    detail: Option<TuiDetailFile>,
    /// `[tui.theme]` table (ADR 0032). Flat map of palette overrides
    /// — unspecified keys keep the runtime defaults, unknown keys
    /// produce a diagnostic, malformed values produce a diagnostic
    /// and the field falls back to its default.
    #[serde(default)]
    theme: Option<BTreeMap<String, toml::Value>>,
    /// `[tui] show_harness_chips` opt-in for the per-harness count
    /// chips in the top header (H-UI-004 audit). Default `false`;
    /// see the field of the same name on [`TuiConfig`] for the
    /// motivation.
    #[serde(default)]
    show_harness_chips: Option<bool>,
    /// `[tui] narrow_layout_threshold` — terminal columns below which
    /// the body reflows to a vertical stack (H-LAYOUT-001). Read as a
    /// signed integer so an out-of-range value produces a targeted
    /// diagnostic instead of failing the whole file parse.
    #[serde(default)]
    narrow_layout_threshold: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TuiDetailFile {
    #[serde(default)]
    show_edge_meta: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TuiViewsFile {
    #[serde(default)]
    sessions: Option<TuiViewFile>,
    #[serde(default)]
    mux: Option<TuiViewFile>,
    #[serde(default)]
    union: Option<TuiViewFile>,
    #[serde(default)]
    prs: Option<TuiViewFile>,
    #[serde(default)]
    forks: Option<TuiViewFile>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TuiViewFile {
    #[serde(default)]
    grouping: Option<String>,
    /// One or more filter predicates whose values OR together (set
    /// union per dimension). Single-entry inline tables are the
    /// common case; multi-entry array-of-tables lets operators add
    /// alternate predicates without losing existing ones.
    #[serde(default)]
    filters: Option<TuiViewFilters>,
}

/// Disk shape of `[[tui.views.<name>.filters]]`. Either a single
/// inline table or an array of tables — both deserialize through
/// the same vector internally.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum TuiViewFilters {
    Single(TuiViewFilterEntry),
    Many(Vec<TuiViewFilterEntry>),
}

impl TuiViewFilters {
    fn entries(self) -> Vec<TuiViewFilterEntry> {
        match self {
            Self::Single(entry) => vec![entry],
            Self::Many(entries) => entries,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TuiViewFilterEntry {
    #[serde(default)]
    harness: Option<Vec<String>>,
    #[serde(default)]
    max_age: Option<String>,
    #[serde(default)]
    mux_state: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TableFile {
    #[serde(default)]
    sessions: Option<TableRowFile>,
    #[serde(default)]
    mux: Option<TableRowFile>,
    #[serde(default)]
    union: Option<TableRowFile>,
    #[serde(default)]
    prs: Option<TableRowFile>,
    #[serde(default)]
    forks: Option<TableRowFile>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TableRowFile {
    #[serde(default)]
    columns: Option<Vec<String>>,
}

/// Result of a single load attempt.
#[derive(Clone, Debug, Default)]
pub struct LoadOutcome {
    pub config: Config,
    /// Files that produced parser or schema errors. The values are
    /// printed to stderr by the CLI but do not abort the run, per
    /// ADR 0012.
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub project_path: Option<PathBuf>,
    pub user_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

/// Pluggable loader that takes explicit `cwd`, `home`, and
/// `xdg_config_home` so tests don't have to mutate process state.
#[derive(Clone, Debug, Default)]
pub struct ConfigLoader {
    home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a loader populated from the process environment.
    pub fn from_env() -> Self {
        Self {
            home: env_path("HOME"),
            xdg_config_home: env_path("XDG_CONFIG_HOME"),
        }
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn with_xdg_config_home(mut self, xdg: impl Into<PathBuf>) -> Self {
        self.xdg_config_home = Some(xdg.into());
        self
    }

    /// Walk upward from `start` looking for [`PROJECT_CONFIG_FILENAME`],
    /// stopping at `$HOME` (when known) or the filesystem root.
    pub fn locate_project_config(&self, start: impl AsRef<Path>) -> Option<PathBuf> {
        let start = start.as_ref();
        let home = self.home.as_deref();
        let mut current = Some(start);
        while let Some(dir) = current {
            let candidate = dir.join(PROJECT_CONFIG_FILENAME);
            if candidate.is_file() {
                return Some(candidate);
            }
            if home.is_some_and(|home| dir == home) {
                break;
            }
            current = dir.parent();
        }
        None
    }

    /// Compute the user-level config path. Returns `None` only when no
    /// suitable base directory is known (no `$HOME`, no
    /// `$XDG_CONFIG_HOME`).
    pub fn user_config_path(&self) -> Option<PathBuf> {
        if let Some(xdg) = &self.xdg_config_home {
            return Some(xdg.join(USER_CONFIG_RELATIVE));
        }
        self.home
            .as_ref()
            .map(|home| home.join(".config").join(USER_CONFIG_RELATIVE))
    }

    /// Load and merge config from `cwd`. See ADR 0012 for precedence.
    pub fn load_from(&self, cwd: impl AsRef<Path>) -> LoadOutcome {
        let mut outcome = LoadOutcome::default();
        let mut config = Config::default();

        if let Some(path) = self.user_config_path()
            && path.is_file()
        {
            outcome.user_path = Some(path.clone());
            merge_from_file(
                &mut config,
                &path,
                self.home.as_deref(),
                &mut outcome.diagnostics,
            );
        }

        if let Some(path) = self.locate_project_config(cwd) {
            outcome.project_path = Some(path.clone());
            merge_from_file(
                &mut config,
                &path,
                self.home.as_deref(),
                &mut outcome.diagnostics,
            );
        }

        outcome.config = config;
        outcome
    }
}

/// Convenience wrapper: build a loader from the environment and load
/// from the current working directory.
pub fn load_from_cwd() -> Result<LoadOutcome> {
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    Ok(ConfigLoader::from_env().load_from(cwd))
}

fn merge_from_file(
    config: &mut Config,
    path: &Path,
    home: Option<&Path>,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("failed to read config: {err}"),
            });
            return;
        }
    };

    let parsed: ConfigFile = match toml::from_str(&text) {
        Ok(parsed) => parsed,
        Err(err) => {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            });
            return;
        }
    };

    if parsed.session.is_some() {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_path_buf(),
            message: "unknown section `session`; the schema moved to `[table.<rows>]` per ADR 0021"
                .to_string(),
        });
    }

    if let Some(table) = parsed.table {
        merge_table(&mut config.table, table);
    }

    if let Some(tui) = parsed.tui {
        merge_tui(&mut config.tui, tui, home, path, diagnostics);
    }

    if let Some(server) = parsed.server {
        merge_server(&mut config.server, server, path, diagnostics);
    }

    if let Some(worktree) = parsed.worktree {
        merge_worktree(&mut config.worktree, worktree, path, diagnostics);
    }
}

fn merge_worktree(
    config: &mut WorktreeConfig,
    file: WorktreeFile,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(raw) = file.backend else {
        return;
    };
    match crate::discovery::worktree::WorktreeBackendSelection::parse(&raw) {
        Some(selection) => config.backend = selection,
        None => diagnostics.push(ConfigDiagnostic {
            path: path.to_path_buf(),
            message: format!(
                "invalid `[worktree] backend` value `{raw}`; expected auto, git, or worktrunk"
            ),
        }),
    }
}

fn merge_server(
    config: &mut ServerConfig,
    file: ServerFile,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(intervals) = file.intervals else {
        return;
    };
    merge_server_intervals(&mut config.intervals, intervals, path, diagnostics);
}

fn merge_server_intervals(
    config: &mut ServerIntervals,
    file: ServerIntervalsFile,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    // Per-field merge: each interval is independent, so a malformed
    // value diagnoses and leaves the default in place rather than
    // killing the rest of the table. This matches the ADR 0079
    // "best-effort merge" contract.
    set_server_interval(
        &mut config.harness,
        "harness",
        file.harness,
        path,
        diagnostics,
    );
    set_server_interval(&mut config.mux, "mux", file.mux, path, diagnostics);
    set_server_interval(&mut config.git, "git", file.git, path, diagnostics);
    set_server_interval(&mut config.forge, "forge", file.forge, path, diagnostics);
}

fn set_server_interval(
    target: &mut Duration,
    field: &str,
    raw: Option<String>,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(raw) = raw else { return };
    match parse_duration_short(&raw) {
        Ok(duration) => *target = duration,
        Err(message) => diagnostics.push(ConfigDiagnostic {
            path: path.to_path_buf(),
            message: format!("invalid `[server.intervals]` `{field}` value `{raw}`: {message}"),
        }),
    }
}

/// Parse a short-form duration string of the shape
/// `<non-negative-integer><ms|s|m|h>`. Shared with the CLI
/// `--refresh-interval` flag (which calls through its own
/// historical `parse_tui_duration` wrapper); the format is the
/// same so operators can copy values between flag and config.
fn parse_duration_short(input: &str) -> Result<Duration, String> {
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

fn merge_tui(
    config: &mut TuiConfig,
    file: TuiFile,
    home: Option<&Path>,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    if let Some(raw_roots) = file.scan_roots {
        config.scan_roots = raw_roots
            .into_iter()
            .map(|raw| expand_home(&raw, home))
            .collect();
    }

    // Legacy alias: `[tui].sessions_grouping = "..."`. Per ADR 0031
    // it stays parseable but emits a deprecation diagnostic, and
    // only seeds the new key when no `[tui.views.sessions].grouping`
    // is present. The new key wins on conflict so operators who
    // already migrated don't get their value clobbered.
    let mut legacy_sessions_grouping: Option<Grouping> = None;
    if let Some(raw) = file.sessions_grouping.as_deref() {
        match Grouping::parse_for(View::Sessions, raw) {
            Some(g) => {
                legacy_sessions_grouping = Some(g);
                diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: "`[tui].sessions_grouping` is deprecated; \
                                  set `[tui.views.sessions].grouping` instead (ADR 0031)"
                        .to_string(),
                });
            }
            None => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "invalid `[tui].sessions_grouping` value `{raw}`; expected one of {}",
                    grouping_choices_for(View::Sessions)
                ),
            }),
        }
    }

    if let Some(views) = file.views {
        merge_tui_view(
            &mut config.views.sessions,
            views.sessions,
            View::Sessions,
            path,
            diagnostics,
        );
        merge_tui_view(
            &mut config.views.mux,
            views.mux,
            View::Mux,
            path,
            diagnostics,
        );
        merge_tui_view(
            &mut config.views.union,
            views.union,
            View::Union,
            path,
            diagnostics,
        );
        merge_tui_view(
            &mut config.views.prs,
            views.prs,
            View::Prs,
            path,
            diagnostics,
        );
        merge_tui_view(
            &mut config.views.forks,
            views.forks,
            View::Forks,
            path,
            diagnostics,
        );
    }

    // Seed the sessions grouping from the legacy alias only when the
    // operator hasn't moved over yet.
    if let Some(legacy) = legacy_sessions_grouping
        && config.views.sessions.grouping.is_none()
    {
        config.views.sessions.grouping = Some(legacy);
    }

    if let Some(detail_file) = file.detail
        && let Some(show_edge_meta) = detail_file.show_edge_meta
    {
        config.detail.show_edge_meta = show_edge_meta;
    }

    if let Some(show_harness_chips) = file.show_harness_chips {
        config.show_harness_chips = show_harness_chips;
    }

    if let Some(raw) = file.narrow_layout_threshold {
        // Require a positive value that fits in the u16 width the
        // layout math uses. Zero would stack unconditionally and is
        // almost certainly a mistake, so it diagnoses too.
        match u16::try_from(raw) {
            Ok(value) if value >= 1 => config.narrow_layout_threshold = value,
            _ => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "invalid `[tui] narrow_layout_threshold` value `{raw}`; \
                     expected an integer between 1 and {}",
                    u16::MAX
                ),
            }),
        }
    }

    if let Some(theme_file) = file.theme {
        merge_tui_theme(&mut config.theme, theme_file, path, diagnostics);
    }
}

/// Apply `[tui.theme]` overrides to the in-memory [`Theme`]. Each
/// entry routes by [`ThemeKeyKind`]; unknown keys, non-string values,
/// and unparseable specs each emit a per-key [`ConfigDiagnostic`]
/// and leave the corresponding field at its default value.
fn merge_tui_theme(
    theme: &mut Theme,
    overrides: BTreeMap<String, toml::Value>,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let known: BTreeMap<&'static str, ThemeKeyKind> = Theme::known_keys()
        .iter()
        .map(|entry| (entry.name, entry.kind))
        .collect();

    for (key, value) in overrides {
        if key == "icons" {
            merge_tui_theme_icons(theme, value, path, diagnostics);
            continue;
        }
        if key == "harness" {
            merge_tui_theme_harness(theme, value, path, diagnostics);
            continue;
        }
        let Some(&kind) = known.get(key.as_str()) else {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("unknown `[tui.theme]` key `{key}`"),
            });
            continue;
        };
        let raw = match value.as_str() {
            Some(s) => s.to_string(),
            None => {
                diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: format!(
                        "`[tui.theme].{key}` must be a string (got `{}`)",
                        value.type_str()
                    ),
                });
                continue;
            }
        };
        match kind {
            ThemeKeyKind::Color => match parse_color(&raw) {
                Ok(color) => {
                    theme.set_color(&key, color);
                }
                Err(err) => diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: format!("`[tui.theme].{key}`: {err}"),
                }),
            },
            ThemeKeyKind::Modifier => match parse_modifier(&raw) {
                Ok(modifier) => {
                    theme.set_modifier(&key, modifier);
                }
                Err(err) => diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: format!("`[tui.theme].{key}`: {err}"),
                }),
            },
            ThemeKeyKind::StyleSpec => match parse_style_spec(&raw) {
                Ok(spec) => {
                    theme.set_style_spec(&key, spec);
                }
                Err(err) => diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: format!("`[tui.theme].{key}`: {err}"),
                }),
            },
        }
    }
}

/// Apply `[tui.theme.icons]` overrides to `theme.icons` (ADR 0073).
/// Non-table values, unknown keys, non-string values, and glyphs
/// whose display width is not 1 each produce a `ConfigDiagnostic`
/// and leave the corresponding kind on its default glyph.
fn merge_tui_theme_icons(
    theme: &mut Theme,
    value: toml::Value,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(table) = value.as_table() else {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_path_buf(),
            message: format!(
                "`[tui.theme.icons]` must be a table (got `{}`)",
                value.type_str()
            ),
        });
        return;
    };
    for (key, value) in table {
        let Some(raw) = value.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "`[tui.theme.icons].{key}` must be a string (got `{}`)",
                    value.type_str()
                ),
            });
            continue;
        };
        match parse_icon_override(key, raw) {
            Ok((kind, glyph)) => {
                theme.icons.insert(kind, glyph);
            }
            Err(err) => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: err,
            }),
        }
    }
}

/// Apply `[tui.theme.harness]` overrides to `theme.harness_colors`
/// (H-EXT-003). Table keys are harness keys registered via the
/// adapter registry; unknown keys emit a diagnostic listing the
/// registered set and leave the target map entry alone. Non-table
/// values and non-string entries produce diagnostics without
/// touching state.
fn merge_tui_theme_harness(
    theme: &mut Theme,
    value: toml::Value,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(table) = value.as_table() else {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_path_buf(),
            message: format!(
                "`[tui.theme.harness]` must be a table (got `{}`)",
                value.type_str()
            ),
        });
        return;
    };
    let registered: std::collections::BTreeSet<&'static str> =
        crate::discovery::harness::harness_keys()
            .iter()
            .copied()
            .collect();
    for (key, value) in table {
        if !registered.contains(key.as_str()) {
            let registered_list: Vec<String> = registered.iter().map(|s| s.to_string()).collect();
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "unknown `[tui.theme.harness]` key `{key}`; registered harnesses: {}",
                    registered_list.join(", ")
                ),
            });
            continue;
        }
        let Some(raw) = value.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "`[tui.theme.harness].{key}` must be a string (got `{}`)",
                    value.type_str()
                ),
            });
            continue;
        };
        match parse_color(raw) {
            Ok(color) => {
                theme.harness_colors.insert(key.clone(), color);
            }
            Err(err) => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("`[tui.theme.harness].{key}`: {err}"),
            }),
        }
    }
}

fn merge_tui_view(
    config: &mut TuiViewConfig,
    file: Option<TuiViewFile>,
    view: View,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    let Some(file) = file else { return };
    if let Some(raw) = file.grouping.as_deref() {
        match Grouping::parse_for(view, raw) {
            Some(g) => config.grouping = Some(g),
            None => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!(
                    "invalid `[tui.views.{}].grouping` value `{raw}`; expected one of {}",
                    view_config_key(view),
                    grouping_choices_for(view)
                ),
            }),
        }
    }
    if let Some(filters) = file.filters {
        let merged = merge_view_filter_entries(filters.entries(), view, path, diagnostics);
        config.filter = merged;
    }
}

/// Combine one or more filter entries into a single [`RowFilter`].
/// Multiple entries OR their predicates per dimension (set union),
/// matching the operator mental model that "I want claude OR codex,
/// and 7d OR 24h" expands the visible set.
fn merge_view_filter_entries(
    entries: Vec<TuiViewFilterEntry>,
    view: View,
    path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> RowFilter {
    let mut harnesses: Vec<String> = Vec::new();
    let mut max_age_secs: Option<u64> = None;
    let mut mux_states: Vec<MuxStateKey> = Vec::new();

    for entry in entries {
        if let Some(values) = entry.harness {
            harnesses.extend(values);
        }
        if let Some(raw) = entry.max_age.as_deref() {
            match parse_config_duration(raw) {
                Ok(secs) => {
                    // Multiple entries OR — take the widest window
                    // because OR-ing predicates admits more rows.
                    max_age_secs = Some(max_age_secs.map_or(secs, |existing| existing.max(secs)));
                }
                Err(err) => diagnostics.push(ConfigDiagnostic {
                    path: path.to_path_buf(),
                    message: format!(
                        "invalid `[tui.views.{}.filters].max_age` value `{raw}`: {err}",
                        view_config_key(view)
                    ),
                }),
            }
        }
        if let Some(values) = entry.mux_state {
            for raw in values {
                match MuxStateKey::from_str_ci(&raw) {
                    Some(key) => mux_states.push(key),
                    None => diagnostics.push(ConfigDiagnostic {
                        path: path.to_path_buf(),
                        message: format!(
                            "invalid `[tui.views.{}.filters].mux_state` value `{raw}`; \
                             expected one of attached, ambiguous, unmuxed",
                            view_config_key(view)
                        ),
                    }),
                }
            }
        }
    }

    RowFilter {
        harness: (!harnesses.is_empty()).then(|| HarnessFilter::from_values(harnesses)),
        max_age: max_age_secs.map(std::time::Duration::from_secs),
        mux_state: (!mux_states.is_empty()).then(|| MuxStateFilter::from_values(mux_states)),
        ..RowFilter::default()
    }
}

fn view_config_key(view: View) -> &'static str {
    match view {
        View::Sessions => "sessions",
        View::Mux => "mux",
        View::Union => "union",
        View::Prs => "prs",
        View::Forks => "forks",
    }
}

fn grouping_choices_for(view: View) -> String {
    Grouping::values_for(view)
        .iter()
        .map(|g| g.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse a small subset of duration strings used in config: `<n>ms`,
/// `<n>s`, `<n>m`, `<n>h`, `<n>d`. Mirrors the CLI parser so a
/// value written in `.conspectus.toml` matches the CLI invocation
/// shape. Returns the duration as whole seconds — sub-second config
/// values quantize to zero.
fn parse_config_duration(raw: &str) -> Result<u64, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty duration".to_string());
    }
    let (digits, suffix) = match raw.find(|c: char| !c.is_ascii_digit()) {
        Some(idx) => raw.split_at(idx),
        None => (raw, "s"),
    };
    let value: u64 = digits
        .parse()
        .map_err(|_| format!("non-integer magnitude `{digits}`"))?;
    let secs = match suffix.trim() {
        "" | "s" => value,
        "ms" => value / 1_000,
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(60 * 60),
        "d" => value.saturating_mul(60 * 60 * 24),
        other => return Err(format!("unknown unit `{other}` (expected ms, s, m, h, d)")),
    };
    Ok(secs)
}

/// Expand a leading `~` or `~/<rest>` token against `home`. Other
/// forms pass through unchanged. When `home` is `None`, the raw
/// path is returned even if it starts with `~` — the loader has no
/// home to splice in and the operator-visible result is at least
/// stable rather than half-resolved.
fn expand_home(raw: &str, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from(raw);
    };
    if raw == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

fn merge_table(config: &mut TableConfig, file: TableFile) {
    merge_table_row(&mut config.sessions, file.sessions);
    merge_table_row(&mut config.mux, file.mux);
    merge_table_row(&mut config.union, file.union);
    merge_table_row(&mut config.prs, file.prs);
    merge_table_row(&mut config.forks, file.forks);
}

fn merge_table_row(config: &mut TableRowConfig, file: Option<TableRowFile>) {
    let Some(file) = file else { return };
    if let Some(columns) = file.columns {
        config.columns = Some(columns);
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
