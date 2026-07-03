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

/// Resolved configuration after project + user + defaults are merged.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub table: TableConfig,
    pub tui: TuiConfig,
    pub server: ServerConfig,
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
    /// Legacy `[session]` key from before ADR 0021. Its presence
    /// triggers a diagnostic so users discover the schema migrated;
    /// its contents are not read.
    #[serde(default)]
    session: Option<toml::Value>,
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
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, contents).expect("write config file");
    }

    #[test]
    fn defaults_apply_when_no_config_files_exist() {
        let temp = TempDir::new().expect("temp dir");
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(temp.path().join("xdg"));

        let outcome = loader.load_from(temp.path());

        assert_eq!(outcome.config, Config::default());
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.project_path.is_none());
        assert!(outcome.user_path.is_none());
    }

    #[test]
    fn tui_detail_show_edge_meta_loads_from_config() {
        // T8-042: `[tui.detail].show_edge_meta` flips the runtime
        // default for the explorer's link-row meta visibility.
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.detail]\nshow_edge_meta = true\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.config.tui.detail.show_edge_meta);
    }

    #[test]
    fn tui_detail_show_edge_meta_defaults_to_false_when_absent() {
        let temp = TempDir::new().expect("temp dir");
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(temp.path().join("xdg"));
        let outcome = loader.load_from(temp.path());

        assert!(
            !outcome.config.tui.detail.show_edge_meta,
            "T8-042: edge meta should default to hidden",
        );
    }

    #[test]
    fn project_config_parses_empty_table_subsections() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[table.sessions]\n[table.mux]\n[table.union]\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty());
        assert_eq!(
            outcome.project_path.as_deref(),
            Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
        );
    }

    #[test]
    fn project_config_search_walks_up_to_home_boundary() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        let nested = project.join("nested").join("deep");
        fs::create_dir_all(&nested).expect("create nested");
        write_file(&project.join(PROJECT_CONFIG_FILENAME), "[table.sessions]\n");

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&nested);

        assert!(outcome.diagnostics.is_empty());
        assert_eq!(
            outcome.project_path.as_deref(),
            Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
        );
    }

    #[test]
    fn project_config_search_stops_at_home() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        // Place a file outside $HOME that would match if the walk
        // didn't stop. The loader must not pick it up.
        write_file(
            &temp.path().join(PROJECT_CONFIG_FILENAME),
            "[table.sessions]\n",
        );

        let loader = ConfigLoader::new().with_home(&home);
        let outcome = loader.load_from(&home);

        assert!(outcome.project_path.is_none());
    }

    #[test]
    fn server_intervals_default_when_absent() {
        // No `[server]` block at all → every class keeps its ADR
        // 0038 default. P7-003 phase 3's TTL gate falls back to
        // these numbers on a fresh install.
        let temp = TempDir::new().expect("temp dir");
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(temp.path().join("xdg"));
        let outcome = loader.load_from(temp.path());
        let intervals = outcome.config.server.intervals;
        assert_eq!(intervals.harness, Duration::from_secs(5));
        assert_eq!(intervals.mux, Duration::from_secs(5));
        assert_eq!(intervals.git, Duration::from_secs(30));
        assert_eq!(intervals.forge, Duration::from_secs(300));
    }

    #[test]
    fn server_intervals_parse_each_supported_unit() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[server.intervals]\n\
             harness = \"500ms\"\n\
             mux     = \"10s\"\n\
             git     = \"2m\"\n\
             forge   = \"1h\"\n",
        );
        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);
        assert!(
            outcome.diagnostics.is_empty(),
            "expected no diagnostics; got {:?}",
            outcome.diagnostics
        );
        let intervals = outcome.config.server.intervals;
        assert_eq!(intervals.harness, Duration::from_millis(500));
        assert_eq!(intervals.mux, Duration::from_secs(10));
        assert_eq!(intervals.git, Duration::from_secs(120));
        assert_eq!(intervals.forge, Duration::from_secs(3600));
    }

    #[test]
    fn server_intervals_unset_field_keeps_default() {
        // Partial override: only `forge` is bumped; the other
        // classes keep their defaults rather than collapsing to
        // zero. Per-field independence is the ADR 0079 contract.
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[server.intervals]\nforge = \"10m\"\n",
        );
        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);
        assert!(outcome.diagnostics.is_empty());
        let intervals = outcome.config.server.intervals;
        assert_eq!(intervals.harness, Duration::from_secs(5));
        assert_eq!(intervals.mux, Duration::from_secs(5));
        assert_eq!(intervals.git, Duration::from_secs(30));
        assert_eq!(intervals.forge, Duration::from_secs(600));
    }

    #[test]
    fn server_intervals_malformed_value_diagnoses_and_keeps_default() {
        // A single malformed value emits a diagnostic and falls
        // back to the default for that field; the rest of the
        // table still merges. Matches the broader best-effort
        // merge contract elsewhere in this loader.
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[server.intervals]\nharness = \"5q\"\nmux = \"7s\"\n",
        );
        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);
        assert_eq!(
            outcome.diagnostics.len(),
            1,
            "exactly one diagnostic for the malformed value"
        );
        assert!(
            outcome.diagnostics[0].message.contains("harness"),
            "diagnostic should name the offending field; got {}",
            outcome.diagnostics[0].message
        );
        // Field falls back to default; other field still merged.
        assert_eq!(
            outcome.config.server.intervals.harness,
            Duration::from_secs(5)
        );
        assert_eq!(outcome.config.server.intervals.mux, Duration::from_secs(7));
    }

    #[test]
    fn legacy_session_section_emits_diagnostic_but_does_not_abort() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0].message.contains("session"),
            "diagnostic should mention the legacy section: {:?}",
            outcome.diagnostics[0].message,
        );
        assert!(outcome.diagnostics[0].message.contains("table"),);
        // Defaults still apply; the run is not aborted.
        assert_eq!(outcome.config, Config::default());
    }

    #[test]
    fn malformed_toml_yields_diagnostic_and_default() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "this isn't toml = =",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config, Config::default());
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message.starts_with("malformed TOML"))
        );
    }

    #[test]
    fn unknown_keys_are_silently_ignored() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[table.sessions]\nfuture_key = 1\n\n[unknown]\nx = \"y\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty());
    }

    #[test]
    fn projection_round_trips_through_parse_and_as_str() {
        for variant in [
            Projection::Agent,
            Projection::Mux,
            Projection::Union,
            Projection::Pr,
            Projection::Fork,
        ] {
            assert_eq!(Projection::parse(variant.as_str()).unwrap(), variant);
        }
        // The `sessions` alias resolves to the same projection as the
        // historical `agent` token; both back the same row-type.
        assert_eq!(Projection::parse("sessions").unwrap(), Projection::Agent);
        // `pr` (singular) and `prs` (plural) both reach the Pr variant.
        assert_eq!(Projection::parse("pr").unwrap(), Projection::Pr);
        // Same alias pattern for forks.
        assert_eq!(Projection::parse("fork").unwrap(), Projection::Fork);
        assert!(Projection::parse("garbage").is_err());
    }

    #[test]
    fn xdg_config_home_overrides_home_fallback() {
        let temp = TempDir::new().expect("temp dir");
        let xdg = temp.path().join("xdg");
        write_file(&xdg.join(USER_CONFIG_RELATIVE), "[table.sessions]\n");
        // Also place a colliding file under $HOME/.config that
        // should be ignored when XDG_CONFIG_HOME is set.
        write_file(
            &temp.path().join(".config").join(USER_CONFIG_RELATIVE),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);
        let outcome = loader.load_from(temp.path());

        // No diagnostic because the legacy file under $HOME/.config
        // was never opened (XDG took priority).
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.user_path.unwrap(), xdg.join(USER_CONFIG_RELATIVE));
    }

    #[test]
    fn tui_scan_roots_parse_and_expand_home() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui]\nscan_roots = [\"~\", \"~/src/oss\", \"/abs/path\"]\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty());
        assert_eq!(
            outcome.config.tui.scan_roots,
            vec![
                temp.path().to_path_buf(),
                temp.path().join("src/oss"),
                PathBuf::from("/abs/path"),
            ]
        );
    }

    #[test]
    fn tui_scan_roots_default_empty() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(&project.join(PROJECT_CONFIG_FILENAME), "[table.sessions]\n");

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.config.tui.scan_roots.is_empty());
    }

    #[test]
    fn tui_views_grouping_parses_per_view() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.views.sessions]\ngrouping = \"repo\"\n\
             [tui.views.mux]\ngrouping = \"host\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(
            outcome.config.tui.views.sessions.grouping,
            Some(Grouping::Sessions(crate::tui::SessionsGrouping::Repo))
        );
        assert_eq!(
            outcome.config.tui.views.mux.grouping,
            Some(Grouping::Mux(crate::tui::MuxGrouping::Host))
        );
    }

    #[test]
    fn tui_views_grouping_accepts_none_sessions_grouping() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.views.sessions]\ngrouping = \"none\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(
            outcome.config.tui.views.sessions.grouping,
            Some(Grouping::Sessions(crate::tui::SessionsGrouping::None))
        );
    }

    #[test]
    fn tui_views_grouping_rejects_value_from_other_view() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.views.sessions]\ngrouping = \"host\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        // `host` is a mux grouping, not a sessions one.
        assert!(outcome.config.tui.views.sessions.grouping.is_none());
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("invalid `[tui.views.sessions].grouping`")
        );
    }

    #[test]
    fn tui_views_filters_inline_single_entry() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.views.sessions]\n\
             filters = { harness = [\"claude-code\", \"codex\"], max_age = \"7d\", \
                         mux_state = [\"unmuxed\"] }\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        let filter = &outcome.config.tui.views.sessions.filter;
        assert_eq!(
            filter.harness.as_ref().map(|h| h.values().to_vec()),
            Some(vec!["claude-code".to_string(), "codex".to_string()])
        );
        assert_eq!(
            filter.max_age,
            Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        );
        assert_eq!(
            filter.mux_state.as_ref().map(|m| m.values().to_vec()),
            Some(vec![MuxStateKey::Unmuxed])
        );
    }

    #[test]
    fn tui_views_filters_array_of_tables_unions_per_dimension() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[[tui.views.sessions.filters]]\nharness = [\"claude-code\"]\nmax_age = \"24h\"\n\
             [[tui.views.sessions.filters]]\nharness = [\"codex\"]\nmax_age = \"7d\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        let filter = &outcome.config.tui.views.sessions.filter;
        // Harness sets union: both claude and codex.
        assert_eq!(
            filter.harness.as_ref().map(|h| h.values().to_vec()),
            Some(vec!["claude-code".to_string(), "codex".to_string()])
        );
        // Max-age takes the widest window (7d).
        assert_eq!(
            filter.max_age,
            Some(std::time::Duration::from_secs(7 * 24 * 60 * 60))
        );
    }

    #[test]
    fn tui_views_filter_invalid_mux_state_emits_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.views.sessions]\n\
             filters = { mux_state = [\"unmuxed\", \"frobnicated\"] }\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("invalid `[tui.views.sessions.filters].mux_state` value `frobnicated`")
        );
        // The valid entry still lands.
        let filter = &outcome.config.tui.views.sessions.filter;
        assert_eq!(
            filter.mux_state.as_ref().map(|m| m.values().to_vec()),
            Some(vec![MuxStateKey::Unmuxed])
        );
    }

    #[test]
    fn legacy_sessions_grouping_seeds_new_key_with_deprecation_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui]\nsessions_grouping = \"checkout\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("`[tui].sessions_grouping` is deprecated")
        );
        assert_eq!(
            outcome.config.tui.views.sessions.grouping,
            Some(Grouping::Sessions(crate::tui::SessionsGrouping::Checkout))
        );
    }

    #[test]
    fn new_views_grouping_wins_over_legacy_when_both_present() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui]\nsessions_grouping = \"checkout\"\n\
             [tui.views.sessions]\ngrouping = \"scan-root\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        // Deprecation diagnostic still emits.
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message.contains("deprecated"))
        );
        // But the new key wins.
        assert_eq!(
            outcome.config.tui.views.sessions.grouping,
            Some(Grouping::Sessions(crate::tui::SessionsGrouping::ScanRoot))
        );
    }

    #[test]
    fn legacy_sessions_grouping_invalid_value_emits_error_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui]\nsessions_grouping = \"nope\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        // Invalid legacy value emits an error diagnostic and does
        // not seed the new key.
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("invalid `[tui].sessions_grouping`")
        );
        assert!(outcome.config.tui.views.sessions.grouping.is_none());
    }

    #[test]
    fn parse_config_duration_handles_each_unit() {
        assert_eq!(parse_config_duration("30s"), Ok(30));
        assert_eq!(parse_config_duration("2m"), Ok(120));
        assert_eq!(parse_config_duration("1h"), Ok(3_600));
        assert_eq!(parse_config_duration("7d"), Ok(7 * 24 * 60 * 60));
        assert_eq!(parse_config_duration("2000ms"), Ok(2));
        assert_eq!(parse_config_duration("42"), Ok(42));
        assert!(parse_config_duration("").is_err());
        assert!(parse_config_duration("nope").is_err());
        assert!(parse_config_duration("12years").is_err());
    }

    #[test]
    fn tui_theme_overrides_named_color_and_modifier() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\n\
             harness_claude = \"bright_red\"\n\
             mux_attached = \"#00ff88\"\n\
             selection_active = \"reversed,italic\"\n\
             recency_cold = \"dim,italic\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        let theme = &outcome.config.tui.theme;
        assert_eq!(
            theme.harness_color("claude-code"),
            ratatui::style::Color::LightRed
        );
        assert_eq!(
            theme.mux_attached,
            ratatui::style::Color::Rgb(0x00, 0xff, 0x88)
        );
        assert!(
            theme
                .selection_active
                .contains(ratatui::style::Modifier::REVERSED)
        );
        assert!(
            theme
                .selection_active
                .contains(ratatui::style::Modifier::ITALIC)
        );
        assert!(
            theme
                .recency_cold
                .modifier
                .contains(ratatui::style::Modifier::DIM)
        );
    }

    #[test]
    fn tui_theme_unspecified_keys_keep_defaults() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\nharness_codex = \"yellow\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        let theme = &outcome.config.tui.theme;
        let defaults = Theme::default();
        assert_eq!(theme.harness_color("codex"), ratatui::style::Color::Yellow);
        // Untouched fields stay at default values.
        assert_eq!(
            theme.harness_color("claude-code"),
            defaults.harness_color("claude-code")
        );
        assert_eq!(theme.mux_attached, defaults.mux_attached);
    }

    #[test]
    fn tui_theme_unknown_key_emits_diagnostic_and_keeps_defaults() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\nnot_a_field = \"red\"\nharness_claude = \"green\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("unknown `[tui.theme]` key `not_a_field`")
        );
        // Recognized neighbors still apply.
        assert_eq!(
            outcome.config.tui.theme.harness_color("claude-code"),
            ratatui::style::Color::Green
        );
    }

    #[test]
    fn tui_theme_malformed_value_warns_and_falls_back() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\nharness_claude = \"#zzzzzz\"\nmux_attached = \"green\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("`[tui.theme].harness_claude`")
        );
        let defaults = Theme::default();
        assert_eq!(
            outcome.config.tui.theme.harness_color("claude-code"),
            defaults.harness_color("claude-code"),
            "bad spec falls back to default"
        );
        // Sibling fields still parse.
        assert_eq!(
            outcome.config.tui.theme.mux_attached,
            ratatui::style::Color::Green
        );
    }

    #[test]
    fn tui_theme_non_string_value_emits_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\nharness_claude = 42\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message.contains("must be a string"));
    }

    #[test]
    fn tui_theme_harness_table_overrides_per_key_colors() {
        // H-EXT-003: `[tui.theme.harness].<key> = "color"` sets
        // theme.harness_colors[<key>]. Adapter-registry-registered
        // keys are accepted; unknown keys emit a diagnostic
        // pointing at the registered set.
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme.harness]\nclaude-code = \"bright_blue\"\ncodex = \"magenta\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        let theme = &outcome.config.tui.theme;
        assert_eq!(
            theme.harness_color("claude-code"),
            ratatui::style::Color::LightBlue
        );
        assert_eq!(theme.harness_color("codex"), ratatui::style::Color::Magenta);
        // Unlisted registered adapters keep their defaults.
        let defaults = Theme::default();
        assert_eq!(
            theme.harness_color("opencode"),
            defaults.harness_color("opencode")
        );
    }

    #[test]
    fn tui_theme_harness_table_unknown_key_emits_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme.harness]\nnever-registered = \"red\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        let msg = &outcome.diagnostics[0].message;
        assert!(
            msg.contains("unknown `[tui.theme.harness]` key `never-registered`"),
            "{msg}"
        );
        assert!(msg.contains("registered harnesses:"), "{msg}");
    }

    #[test]
    fn tui_theme_flat_harness_alias_still_works() {
        // Pre-H-EXT-003 flat keys stay as aliases per ADR 0031.
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme]\nharness_codex = \"magenta\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(
            outcome.config.tui.theme.harness_color("codex"),
            ratatui::style::Color::Magenta
        );
    }

    #[test]
    fn tui_theme_icons_overrides_glyph_per_node_kind() {
        use crate::tui::icons::NodeKind;

        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme.icons]\nnode_fork = \"Y\"\nnode_repo = \"R\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert!(
            outcome.diagnostics.is_empty(),
            "no diagnostics expected, got {:?}",
            outcome.diagnostics
        );
        let icons = &outcome.config.tui.theme.icons;
        assert_eq!(icons.glyph(NodeKind::Fork).as_deref(), Some("Y"));
        assert_eq!(icons.glyph(NodeKind::Repo).as_deref(), Some("R"));
        assert!(
            icons.glyph(NodeKind::Workspace).is_none(),
            "untouched kinds fall through to the default slate"
        );
    }

    #[test]
    fn tui_theme_icons_rejects_wide_glyph_with_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        // CJK ideograph is unambiguously East Asian Wide (2 cells).
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme.icons]\nnode_workspace = \"中\"\nnode_fork = \"Y\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0].message.contains("display width"),
            "expected width diagnostic, got: {}",
            outcome.diagnostics[0].message
        );
        // Sibling valid override still applies.
        assert_eq!(
            outcome
                .config
                .tui
                .theme
                .icons
                .glyph(crate::tui::icons::NodeKind::Fork)
                .as_deref(),
            Some("Y"),
        );
    }

    #[test]
    fn tui_theme_icons_unknown_key_emits_diagnostic() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[tui.theme.icons]\nworkspace_glyph = \"▦\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message
                .contains("unknown `[tui.theme.icons]` key `workspace_glyph`"),
            "got: {}",
            outcome.diagnostics[0].message,
        );
    }

    #[test]
    fn expand_home_preserves_paths_without_tilde() {
        let home = PathBuf::from("/home/op");
        assert_eq!(
            expand_home("/etc/conspectus", Some(&home)),
            PathBuf::from("/etc/conspectus")
        );
        assert_eq!(expand_home("~", Some(&home)), home);
        assert_eq!(expand_home("~/src", Some(&home)), home.join("src"));
        // `~suffix` without `/` is not a recognized form; leave as-is
        // so we don't accidentally splice the username's home for
        // some other user.
        assert_eq!(expand_home("~root", Some(&home)), PathBuf::from("~root"));
        // Without a home directory we cannot expand; keep the raw
        // string rather than silently dropping the `~`.
        assert_eq!(expand_home("~/src", None), PathBuf::from("~/src"));
    }
}
