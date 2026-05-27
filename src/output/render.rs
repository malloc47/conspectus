//! Backend-agnostic rendering substrate (P10-003 / ADR 0043).
//!
//! Pulled out of [`super::table`] so the in-memory renderer and the
//! SQLite-backed renderer can share the same column registries,
//! [`RenderOptions`] surface, style palette, and width-aware
//! columnar/card layouts without duplicating code.
//!
//! Invariant: this module has **no `crate::model::*` dependencies**.
//! Everything here operates on primitives (`String`, `i64`,
//! `&'static str`) plus types from `crate::filter` and
//! `crate::config::Projection`. The `substrate_has_no_model_deps`
//! test catches accidental drift.
//!
//! What this module owns:
//!
//! - [`RenderOptions`], [`Layout`], the row-filter knob
//! - [`ColumnSpec`] + per-projection column registries
//!   (`SESSIONS_COLUMNS`, `MUX_COLUMNS`, …) + [`columns_for`] /
//!   [`default_columns`]
//! - `--columns` parsing ([`parse_columns_spec`],
//!   [`resolve_explicit_columns`], [`ColumnsError`])
//! - [`render_columns_listing`] for the `conspectus columns` surface
//! - [`render_rows`] (the columnar/card dispatcher) and its width-
//!   fitting helpers ([`natural_widths`], [`fit_to_width`],
//!   [`display_width`], [`truncate_to_width`])
//! - The ADR 0022 color palette helpers ([`header_style`],
//!   [`push_styled`], …)
//! - The recency formatter ([`format_relative_age`],
//!   [`current_epoch`])
//! - The short-id hash ([`node_short_id_from_display`],
//!   [`unique_prefix_len`])
//!
//! Backend-specific code that depends on `GraphSnapshot` or typed
//! `*Node` structs stays in [`super::table`] (in-memory) or
//! [`super::agent_sqlite`] (SQLite spike). Each backend calls into
//! the substrate's primitives to assemble the cell strings, then
//! delegates final rendering to [`render_rows`].

use std::fmt::Write as _;

use anstyle::{Ansi256Color, AnsiColor, Effects, Reset, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub use crate::config::Projection;

// -----------------------------------------------------------------------------
// RenderOptions / Layout
// -----------------------------------------------------------------------------

/// Rendering knobs for the text-table projections.
///
/// `width = None` renders untruncated (the historical behavior) and is the
/// default for callers that just want a wide table — pipes, JSON-adjacent
/// tooling, or tests that want byte-for-byte stable output independent of
/// the developer's terminal size.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Target total width in display columns. `None` means unbounded; the
    /// renderer uses each column's natural width.
    pub width: Option<usize>,
    /// Body layout: `Columnar` (default tabular) or `Card` (one column
    /// per line per row).
    pub layout: Layout,
    /// Selected column keys, in render order. `None` falls back to the
    /// row-type's [`default_columns`] set. Use [`parse_columns_spec`] to
    /// translate a `--columns LIST` string into a value for this field.
    pub columns: Option<Vec<&'static str>>,
    /// When true, the renderer wraps headers and selected cell content
    /// in ANSI escape codes per the palette described in ADR 0022.
    /// Defaults to `false` so existing snapshot tests stay
    /// byte-for-byte stable.
    pub color: bool,
    /// Active row filter (ADR 0031). Empty filter (the
    /// [`crate::filter::RowFilter::default`] value) admits every row,
    /// preserving today's behavior. In v1 the predicate applies to
    /// the agent (sessions) projection only; other projections
    /// silently pass it through until per-projection dimensions
    /// land.
    pub filter: crate::filter::RowFilter,
    /// Wall-clock epoch (Unix seconds) used as the recency anchor
    /// when evaluating `max-age`. `None` disables age-based
    /// filtering (the predicate admits the row). Callers pass a
    /// fixed value in tests for determinism.
    pub now_epoch: Option<i64>,
}

impl RenderOptions {
    /// Untruncated, columnar layout. Matches the pre-H-TBL-003 renderer.
    pub fn wide() -> Self {
        Self {
            width: None,
            layout: Layout::Columnar,
            columns: None,
            color: false,
            filter: crate::filter::RowFilter::default(),
            now_epoch: None,
        }
    }

    /// Columnar layout truncated to the given display width.
    pub fn columnar_width(width: usize) -> Self {
        Self {
            width: Some(width),
            layout: Layout::Columnar,
            columns: None,
            color: false,
            filter: crate::filter::RowFilter::default(),
            now_epoch: None,
        }
    }

    /// Card layout (one column per line per row, blank line between rows).
    pub fn card() -> Self {
        Self {
            width: None,
            layout: Layout::Card,
            columns: None,
            color: false,
            filter: crate::filter::RowFilter::default(),
            now_epoch: None,
        }
    }

    /// Card layout truncated to the given display width.
    pub fn card_width(width: usize) -> Self {
        Self {
            width: Some(width),
            layout: Layout::Card,
            columns: None,
            color: false,
            filter: crate::filter::RowFilter::default(),
            now_epoch: None,
        }
    }

    /// Builder helper: render exactly `columns` (in this order). Pass
    /// keys obtained from [`parse_columns_spec`] or
    /// [`resolve_explicit_columns`].
    pub fn with_columns(mut self, columns: Vec<&'static str>) -> Self {
        self.columns = Some(columns);
        self
    }

    /// Builder helper: enable ANSI color escapes per ADR 0022.
    pub fn with_color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }

    /// Builder helper: attach an active row filter (ADR 0031).
    pub fn with_filter(mut self, filter: crate::filter::RowFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Builder helper: set the recency anchor for `max-age`
    /// evaluation. Tests pin this to a fixture value; the CLI uses
    /// the real wall clock.
    pub fn with_now_epoch(mut self, now: Option<i64>) -> Self {
        self.now_epoch = now;
        self
    }
}

/// Body layout for the renderer. See [`RenderOptions`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Layout {
    /// One row per record, columns aligned to per-column budgets.
    #[default]
    Columnar,
    /// One column per line per row, blank line between rows. Useful when
    /// many columns are relevant and the columnar layout would truncate.
    Card,
}

// -----------------------------------------------------------------------------
// Short-id hash
// -----------------------------------------------------------------------------

/// Minimum length for the short, content-addressed row identifier emitted
/// in the leftmost `ID` column of each session-table projection. The
/// renderer grows the prefix beyond this floor only to break collisions
/// within the rendered snapshot.
pub const SHORT_ID_FLOOR: usize = 6;

/// FNV-1a 64-bit over a `NodeId`'s `Display` form, as a string.
/// Used by both the in-memory renderer (which holds a typed `NodeId`
/// and calls [`super::table::node_short_id`]) and the SQLite-backed
/// renderer (which holds the `Display` form as `TEXT` and calls this
/// directly).
pub fn node_short_id_from_display(node_id_text: &str) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for &byte in node_id_text.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

/// Shortest prefix length that uniquely identifies every id in
/// `full_ids` against the others, floored at [`SHORT_ID_FLOOR`]. All
/// inputs are expected to be the 16-char hex output of
/// [`node_short_id_from_display`]; the cap is therefore 16.
pub fn unique_prefix_len(full_ids: &[String]) -> usize {
    if full_ids.len() <= 1 {
        return SHORT_ID_FLOOR;
    }
    let cap = full_ids
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(SHORT_ID_FLOOR);
    for len in SHORT_ID_FLOOR..=cap {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut unique = true;
        for id in full_ids {
            let prefix = &id[..len.min(id.len())];
            if !seen.insert(prefix) {
                unique = false;
                break;
            }
        }
        if unique {
            return len;
        }
    }
    cap
}

// -----------------------------------------------------------------------------
// Column registry (H-TBL-007)
// -----------------------------------------------------------------------------

/// One column in a row-type's column registry.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSpec {
    /// Stable token used in `--columns LIST` and `[table.<rows>].columns`.
    pub key: &'static str,
    /// Header label rendered in the columnar table and as the key in card layout.
    pub header: &'static str,
    /// One-line description for the column-discovery surface (H-TBL-012).
    pub description: &'static str,
    /// `true` when the column is part of the row-type's default set.
    pub default: bool,
}

pub const SESSIONS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "agent",
        header: "AGENT",
        description: "Harness key and session key (or title when set).",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded by the harness.",
        default: true,
    },
    ColumnSpec {
        key: "mux",
        header: "MUX",
        description: "Preferred mux session attached to this agent session.",
        default: true,
    },
    ColumnSpec {
        key: "mux-conf",
        header: "MUX/CONF",
        description: "Provenance/confidence indicator for the mux cell.",
        default: true,
    },
    ColumnSpec {
        key: "pr",
        header: "PR",
        description: "Preferred forge PR attached via the checkout branch.",
        default: true,
    },
    ColumnSpec {
        key: "pr-conf",
        header: "PR/CONF",
        description: "Provenance/confidence indicator for the PR cell.",
        default: true,
    },
    ColumnSpec {
        key: "lineage",
        header: "LINEAGE",
        description: "Preferred parent-session, with a trailing ← when the chain extends past one hop.",
        default: true,
    },
    ColumnSpec {
        key: "workspace",
        header: "WORKSPACE",
        description: "Workspace context associated with this session.",
        default: false,
    },
    ColumnSpec {
        key: "checkout",
        header: "CHECKOUT",
        description: "Checkout root whose path matches the session's cwd.",
        default: false,
    },
    ColumnSpec {
        key: "branch",
        header: "BRANCH",
        description: "Branch checked out in the session's checkout (refs/heads/ stripped).",
        default: false,
    },
    ColumnSpec {
        key: "repo",
        header: "REPO",
        description: "Repo identifier (common_dir path) for the session's checkout.",
        default: false,
    },
    ColumnSpec {
        key: "fork",
        header: "FORK",
        description: "Fork label when the session is the `child_session` target of a fork.",
        default: false,
    },
    ColumnSpec {
        key: "declared",
        header: "DECLARED",
        description: "State of the strongest declared candidate sourced from this session (declared / ignored / overridden).",
        default: false,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "One-line snippet of the session's most recent user/assistant message (capped at 200 chars, ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "title",
        header: "TITLE",
        description: "Adapter-populated session title (opencode chat topic, claude-code compaction summary). `—` when unset.",
        default: false,
    },
    ColumnSpec {
        key: "activity",
        header: "ACTIVITY",
        description: "Relative recency from `AgentSessionNode.last_active_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
];

pub const MUX_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "mux",
        header: "MUX",
        description: "Backend and native session id.",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded by the mux backend.",
        default: true,
    },
    ColumnSpec {
        key: "agents",
        header: "AGENTS",
        description: "Agent sessions attached to this mux session.",
        default: true,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "Last-message preview of the first attached agent (ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "attached-count",
        header: "ATTACHED",
        description: "Count of agent sessions resolved as attached to this mux.",
        default: false,
    },
    ColumnSpec {
        key: "activity",
        header: "ACTIVITY",
        description: "Relative recency from `MuxSessionNode.activity_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
    ColumnSpec {
        key: "created",
        header: "CREATED",
        description: "Relative age from `MuxSessionNode.created_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
];

pub const UNION_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "kind",
        header: "KIND",
        description: "Node kind (`agent` or `mux`).",
        default: true,
    },
    ColumnSpec {
        key: "label",
        header: "LABEL",
        description: "Harness/mux label for the node.",
        default: true,
    },
    ColumnSpec {
        key: "cwd",
        header: "CWD",
        description: "Working directory recorded for the node.",
        default: true,
    },
    ColumnSpec {
        key: "relationship",
        header: "RELATIONSHIP",
        description: "Preferred relationship cell for agent rows; — for mux rows.",
        default: true,
    },
    ColumnSpec {
        key: "preview",
        header: "PREVIEW",
        description: "Last-message preview for agent rows; — for mux rows (ADR 0023).",
        default: false,
    },
    ColumnSpec {
        key: "title",
        header: "TITLE",
        description: "Adapter-populated agent session title; — for mux rows.",
        default: false,
    },
];

pub const PRS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "pr",
        header: "PR",
        description: "Forge identifier (`{owner}/{repo}#{number}`) with state/draft suffix.",
        default: true,
    },
    ColumnSpec {
        key: "state",
        header: "STATE",
        description: "Forge-reported PR state (open, closed, merged, …).",
        default: true,
    },
    ColumnSpec {
        key: "draft",
        header: "DRAFT",
        description: "`draft` when the PR is a draft; `—` otherwise.",
        default: false,
    },
    ColumnSpec {
        key: "branch",
        header: "BRANCH",
        description: "Head branch (refs/heads/ prefix stripped).",
        default: true,
    },
    ColumnSpec {
        key: "repo",
        header: "REPO",
        description: "Forge repository slug (`{owner}/{repo}`).",
        default: false,
    },
    ColumnSpec {
        key: "updated",
        header: "UPDATED",
        description: "Relative recency from `updated_epoch` (e.g. `2h`, `3d`).",
        default: false,
    },
    ColumnSpec {
        key: "attached",
        header: "ATTACHED",
        description: "Agent sessions whose checkout has the PR's branch checked out.",
        default: true,
    },
];

pub const FORKS_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        key: "id",
        header: "ID",
        description: "Short content-addressed row identifier.",
        default: true,
    },
    ColumnSpec {
        key: "fork",
        header: "FORK",
        description: "Fork label: `{provider}:{name}` (falls back to `provider_source_key` when `name` is absent).",
        default: true,
    },
    ColumnSpec {
        key: "provider",
        header: "PROVIDER",
        description: "Provider that recorded the fork (e.g. `atelier`).",
        default: true,
    },
    ColumnSpec {
        key: "scope",
        header: "SCOPE",
        description: "Provider-defined scope or workspace name (when set).",
        default: false,
    },
    ColumnSpec {
        key: "parent",
        header: "PARENT",
        description: "Preferred `parent_session` short id when the fork records one.",
        default: true,
    },
    ColumnSpec {
        key: "children",
        header: "CHILDREN",
        description: "Number of `child_session` candidates pointing at agent sessions.",
        default: true,
    },
    ColumnSpec {
        key: "capabilities",
        header: "CAPABILITIES",
        description: "Comma-joined `capabilities` from the fork metadata.",
        default: false,
    },
];

/// Registry slice for `projection`.
pub fn columns_for(projection: Projection) -> &'static [ColumnSpec] {
    match projection {
        Projection::Agent => SESSIONS_COLUMNS,
        Projection::Mux => MUX_COLUMNS,
        Projection::Union => UNION_COLUMNS,
        Projection::Pr => PRS_COLUMNS,
        Projection::Fork => FORKS_COLUMNS,
    }
}

/// Default column key set for `projection`, in registry order.
pub fn default_columns(projection: Projection) -> Vec<&'static str> {
    columns_for(projection)
        .iter()
        .filter(|spec| spec.default)
        .map(|spec| spec.key)
        .collect()
}

fn projection_row_type(projection: Projection) -> &'static str {
    match projection {
        Projection::Agent => "sessions",
        Projection::Mux => "mux",
        Projection::Union => "union",
        Projection::Pr => "prs",
        Projection::Fork => "forks",
    }
}

/// Errors produced when resolving a user-supplied column selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnsError {
    UnknownColumn {
        name: String,
        row_type: &'static str,
        available: Vec<&'static str>,
    },
    EmptyToken,
}

impl std::fmt::Display for ColumnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnsError::UnknownColumn {
                name,
                row_type,
                available,
            } => write!(
                f,
                "unknown column `{name}` for row-type `{row_type}`; available: {}",
                available.join(", "),
            ),
            ColumnsError::EmptyToken => write!(f, "empty column token in --columns spec"),
        }
    }
}

impl std::error::Error for ColumnsError {}

/// Resolve a `--columns LIST` spec for `projection` to an ordered list
/// of column keys.
///
/// Tokens (comma-separated, whitespace-trimmed):
/// - `default` — reset to the registered default set.
/// - `all` — reset to every registered column.
/// - `+name` — add the column if not already present.
/// - `-name` — remove the column if present.
/// - `name` — explicit-list mode: clears the running set on the first bare
///   token, then appends.
///
/// Unknown column names error with the registered names listed.
pub fn parse_columns_spec(
    projection: Projection,
    spec: &str,
) -> Result<Vec<&'static str>, ColumnsError> {
    let registry = columns_for(projection);
    let row_type = projection_row_type(projection);
    let available: Vec<&'static str> = registry.iter().map(|spec| spec.key).collect();

    let lookup = |name: &str| -> Result<&'static str, ColumnsError> {
        registry
            .iter()
            .find(|spec| spec.key == name)
            .map(|spec| spec.key)
            .ok_or_else(|| ColumnsError::UnknownColumn {
                name: name.to_string(),
                row_type,
                available: available.clone(),
            })
    };

    let mut current: Vec<&'static str> = default_columns(projection);
    let mut explicit_started = false;

    for raw in spec.split(',') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ColumnsError::EmptyToken);
        }
        if let Some(rest) = trimmed.strip_prefix('+') {
            let key = lookup(rest.trim())?;
            if !current.contains(&key) {
                current.push(key);
            }
        } else if let Some(rest) = trimmed.strip_prefix('-') {
            let key = lookup(rest.trim())?;
            current.retain(|k| *k != key);
        } else if trimmed == "default" {
            current = default_columns(projection);
            explicit_started = true;
        } else if trimmed == "all" {
            current = available.clone();
            explicit_started = true;
        } else {
            let key = lookup(trimmed)?;
            if !explicit_started {
                current.clear();
                explicit_started = true;
            }
            if !current.contains(&key) {
                current.push(key);
            }
        }
    }

    Ok(current)
}

/// Resolve an explicit list of column names (no `+`/`-` semantics) — the
/// config-file variant for `[table.<rows>].columns`. Unknown names error.
pub fn resolve_explicit_columns(
    projection: Projection,
    names: &[String],
) -> Result<Vec<&'static str>, ColumnsError> {
    let registry = columns_for(projection);
    let row_type = projection_row_type(projection);
    let available: Vec<&'static str> = registry.iter().map(|spec| spec.key).collect();

    names
        .iter()
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(ColumnsError::EmptyToken);
            }
            registry
                .iter()
                .find(|spec| spec.key == trimmed)
                .map(|spec| spec.key)
                .ok_or_else(|| ColumnsError::UnknownColumn {
                    name: trimmed.to_string(),
                    row_type,
                    available: available.clone(),
                })
        })
        .collect()
}

/// Render a human-readable listing of the registered columns for
/// `projection`. Each line is `<key>  <description>` with `(default)`
/// appended for columns in the default set. The leading key column is
/// padded so descriptions line up. Used by `conspectus columns <ROWS>`
/// (H-TBL-012).
pub fn render_columns_listing(projection: Projection, color: bool) -> String {
    let registry = columns_for(projection);
    let key_width = registry
        .iter()
        .map(|spec| spec.key.chars().count())
        .max()
        .unwrap_or(0);
    let dim = Style::new().effects(Effects::DIMMED);
    let mut out = String::new();
    for spec in registry {
        let key_pad = key_width.saturating_sub(spec.key.chars().count());
        // Bold the column key so it pops out of the description text.
        push_styled(&mut out, spec.key, header_style(), color);
        for _ in 0..(key_pad + 2) {
            out.push(' ');
        }
        out.push_str(spec.description);
        if spec.default {
            out.push_str("  ");
            push_styled(&mut out, "(default)", dim, color);
        }
        out.push('\n');
    }
    out
}

pub fn header_label(registry: &[ColumnSpec], key: &str) -> String {
    registry
        .iter()
        .find(|spec| spec.key == key)
        .map(|spec| spec.header.to_string())
        .unwrap_or_else(|| key.to_uppercase())
}

// -----------------------------------------------------------------------------
// Color palette (ADR 0022)
// -----------------------------------------------------------------------------

/// Style applied to header-row cells (and section headers in
/// `node show`) when color is enabled.
pub fn header_style() -> Style {
    Style::new().effects(Effects::BOLD)
}

/// Style applied to card-layout key labels (`KEY:`) when color is on.
fn card_key_style() -> Style {
    Style::new().effects(Effects::BOLD)
}

/// Style for a body-row cell given the column key and the cell text.
/// Returns an empty style when no rule matches; callers should skip
/// the ANSI envelope in that case via [`Style::is_plain`].
fn cell_style(column_key: &str, cell_text: &str) -> Style {
    if cell_text == "—" {
        return placeholder_style();
    }
    match column_key {
        "id" => id_style(),
        "mux-conf" | "pr-conf" => indicator_style(cell_text),
        "state" => pr_state_style(cell_text),
        "draft" => {
            if cell_text == "draft" {
                Style::new().fg_color(Some(AnsiColor::Yellow.into()))
            } else {
                Style::new()
            }
        }
        "declared" => declared_state_style(cell_text),
        _ => Style::new(),
    }
}

/// Style for the leftmost short-row-id column. Uses a steady blue so
/// the id stays scannable down a column of varying widths without
/// competing with green/cyan/yellow used elsewhere in the palette.
fn id_style() -> Style {
    Style::new().fg_color(Some(AnsiColor::Blue.into()))
}

/// Style for the `—` placeholder cell. Uses a 256-color mid-gray
/// (xterm 244) so the dash sinks visually below the rest of the row
/// rather than just losing intensity. Falls back gracefully on
/// 16-color terminals (anstyle downgrades the value).
fn placeholder_style() -> Style {
    Style::new().fg_color(Some(Ansi256Color(244).into()))
}

/// Resolve a colour for an agent harness key. Each known harness
/// gets a stable, distinct hue so a column full of `claude-code:…`,
/// `codex:…`, and `opencode:…` rows is easy to scan. Unknown
/// harnesses fall back to the default colour.
///
/// Choices use bright ANSI variants so the harness prefix stands out
/// from the regular-intensity palette used for state/indicator cells.
fn harness_style(harness: &str) -> Style {
    let color = match harness {
        "claude-code" => AnsiColor::BrightYellow,
        "codex" => AnsiColor::BrightBlue,
        "opencode" => AnsiColor::BrightGreen,
        "aider" => AnsiColor::BrightRed,
        _ => return Style::new(),
    };
    Style::new().fg_color(Some(color.into()))
}

/// Map a `<tier>/<conf>[*]` indicator cell to its tier color.
fn indicator_style(text: &str) -> Style {
    let tier = text.split('/').next().unwrap_or("");
    match tier {
        "LD" | "GD" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "SD" => Style::new().fg_color(Some(AnsiColor::Cyan.into())),
        "C" | "$" => Style::new().effects(Effects::DIMMED),
        _ => Style::new(),
    }
}

fn pr_state_style(text: &str) -> Style {
    match text {
        "open" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "closed" => Style::new().fg_color(Some(AnsiColor::Red.into())),
        "merged" => Style::new().fg_color(Some(AnsiColor::Magenta.into())),
        _ => Style::new(),
    }
}

fn declared_state_style(text: &str) -> Style {
    match text {
        "declared" => Style::new().fg_color(Some(AnsiColor::Green.into())),
        "ignored" => Style::new().effects(Effects::DIMMED),
        "overridden" => Style::new().fg_color(Some(AnsiColor::Yellow.into())),
        _ => Style::new(),
    }
}

/// Write `text` to `out`, wrapping it in `style`'s ANSI escapes when
/// `color` is true and the style isn't plain. Otherwise emit `text`
/// verbatim. Used by both columnar and card layouts (and by
/// `output::node_show` for section headers) so the wrapping rule is
/// consistent.
pub fn push_styled(out: &mut String, text: &str, style: Style, color: bool) {
    if color && !style.is_plain() {
        let _ = write!(out, "{style}{text}{Reset}");
    } else {
        out.push_str(text);
    }
}

/// Render a body cell, applying any per-segment styling the column
/// asks for. Most columns just call back to [`push_styled`] with
/// [`cell_style`]'s single style. Columns that contain multi-segment
/// content take a dispatch below so the `harness:` prefix can be
/// coloured independently of the session-key tail and the
/// `[indicator]` suffix:
///
/// - `agent`: sessions projection's `harness:session_key` cell.
/// - `agents`: mux projection's joined `label [indicator], …` cell.
/// - `label`: union projection's per-row label. Agent rows render
///   `harness:session_key` (coloured); mux rows render
///   `backend:native_id` and fall through to plain text because
///   the mux backend is not in [`harness_style`]'s palette.
///
/// The emitted text's *visible* width equals `text.chars()` width
/// in every branch — the ANSI envelope adds zero display columns —
/// so this function is safe to call after width-aware truncation
/// has already trimmed `text` to fit the column budget.
fn push_cell(out: &mut String, column_key: &str, text: &str, color: bool) {
    if color {
        match column_key {
            "agent" | "label" => {
                push_agent_label(out, text);
                return;
            }
            "agents" => {
                push_agents_cell(out, text);
                return;
            }
            _ => {}
        }
    }
    push_styled(out, text, cell_style(column_key, text), color);
}

/// Style a `harness:session_key` agent label, colouring only the
/// harness prefix. Falls back to the placeholder style for `—` and
/// to plain text for unknown harness keys so a future tool slots in
/// without requiring a palette edit first.
fn push_agent_label(out: &mut String, label: &str) {
    if label == "—" {
        push_styled(out, label, placeholder_style(), true);
        return;
    }
    if let Some((harness, rest)) = label.split_once(':') {
        let style = harness_style(harness);
        if style.is_plain() {
            out.push_str(label);
        } else {
            let _ = write!(out, "{style}{harness}{Reset}:{rest}");
        }
    } else {
        out.push_str(label);
    }
}

/// Style the mux projection's `agents` cell. [`mux_cell`] builds the
/// cell as `label [indicator], label [indicator]`; the styler splits
/// on the comma boundary, then on the ` [` boundary inside each
/// entry, so only the `harness:` prefix of each label is wrapped in
/// its per-harness colour. The trailing `[indicator]` suffix stays
/// uncoloured by this story; if it grows indicator-tier styling
/// later, this is the seam.
fn push_agents_cell(out: &mut String, text: &str) {
    if text == "—" {
        push_styled(out, text, placeholder_style(), true);
        return;
    }
    for (idx, entry) in text.split(", ").enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        match entry.split_once(" [") {
            Some((label, indicator_tail)) => {
                push_agent_label(out, label);
                out.push(' ');
                out.push('[');
                out.push_str(indicator_tail);
            }
            None => push_agent_label(out, entry),
        }
    }
}

// -----------------------------------------------------------------------------
// Width-aware layout
// -----------------------------------------------------------------------------

pub const COLUMN_GAP: &str = "  ";
pub const COLUMN_GAP_WIDTH: usize = 2;
/// Floor on a column's minimum budget before truncation. Header width is
/// also considered: a column with a wider header keeps the header's width
/// as its floor when its natural content is wider than this constant. Set
/// to 4 so a column can still emit `xxx…` after truncation.
pub const MIN_COLUMN_BUDGET: usize = 4;

pub fn render_rows(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    match options.layout {
        Layout::Columnar => render_columnar(rows, columns, options),
        Layout::Card => render_card(rows, columns, options),
    }
}

/// Render `rows` (header row at index 0) as a stack of cards: one block
/// per body row, with `KEY: value` lines aligned to the longest key and
/// blank lines separating blocks. The header row contributes the key
/// names but is not itself emitted as a card.
fn render_card(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    if rows.len() < 2 {
        return String::new();
    }
    let header = &rows[0];
    let key_width = header.iter().map(|s| display_width(s)).max().unwrap_or(0);
    let value_budget = options.width.map(|w| w.saturating_sub(key_width + 2));

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate().skip(1) {
        if row_idx > 1 {
            out.push('\n');
        }
        for (col_idx, cell) in row.iter().enumerate() {
            let key = header.get(col_idx).map(String::as_str).unwrap_or("");
            let key_pad = key_width.saturating_sub(display_width(key));
            // `KEY:` is the label half — bolded when color is on.
            push_styled(&mut out, key, card_key_style(), options.color);
            out.push(':');
            for _ in 0..(key_pad + 1) {
                out.push(' ');
            }
            let value = match value_budget {
                Some(budget) => truncate_to_width(cell, budget),
                None => cell.clone(),
            };
            let column_key = columns.get(col_idx).copied().unwrap_or("");
            push_cell(&mut out, column_key, &value, options.color);
            out.push('\n');
        }
    }
    out
}

fn render_columnar(
    rows: Vec<Vec<String>>,
    columns: &[&'static str],
    options: &RenderOptions,
) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let column_count = rows[0].len();
    let naturals = natural_widths(&rows, column_count);
    let budgets = match options.width {
        None => naturals,
        Some(target) => fit_to_width(&naturals, &rows[0], target),
    };

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        for (idx, cell) in row.iter().enumerate() {
            if idx > 0 {
                out.push_str(COLUMN_GAP);
            }
            let budget = budgets[idx];
            let truncated = truncate_to_width(cell, budget);
            if row_idx == 0 {
                push_styled(&mut out, &truncated, header_style(), options.color);
            } else {
                let column_key = columns.get(idx).copied().unwrap_or("");
                push_cell(&mut out, column_key, &truncated, options.color);
            }
            if idx + 1 < column_count {
                let truncated_width = display_width(&truncated);
                let pad = budget.saturating_sub(truncated_width);
                for _ in 0..pad {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
        if row_idx == 0 {
            for (idx, width) in budgets.iter().enumerate() {
                if idx > 0 {
                    out.push_str(COLUMN_GAP);
                }
                for _ in 0..*width {
                    out.push('-');
                }
            }
            out.push('\n');
        }
    }
    out
}

pub fn natural_widths(rows: &[Vec<String>], columns: usize) -> Vec<usize> {
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx >= widths.len() {
                continue;
            }
            widths[idx] = widths[idx].max(display_width(cell));
        }
    }
    widths
}

/// Greedily shrink column budgets toward `target` total width, never below
/// each column's floor. When the floor sum already exceeds the target the
/// columns settle at their floors; the final row may exceed `target`, which
/// is intentional — narrow terminals get a best-effort fit rather than
/// degenerate output.
pub fn fit_to_width(naturals: &[usize], header: &[String], target: usize) -> Vec<usize> {
    let columns = naturals.len();
    if columns == 0 {
        return Vec::new();
    }
    let gap_total = COLUMN_GAP_WIDTH.saturating_mul(columns.saturating_sub(1));

    let floors: Vec<usize> = naturals
        .iter()
        .enumerate()
        .map(|(idx, &natural)| {
            let header_width = header
                .get(idx)
                .map(|s| display_width(s))
                .unwrap_or(MIN_COLUMN_BUDGET);
            natural.min(header_width.max(MIN_COLUMN_BUDGET))
        })
        .collect();

    let mut budgets = naturals.to_vec();
    loop {
        let used: usize = budgets.iter().sum::<usize>().saturating_add(gap_total);
        if used <= target {
            break;
        }
        let mut best: Option<(usize, usize)> = None;
        for (idx, &budget) in budgets.iter().enumerate() {
            let margin = budget.saturating_sub(floors[idx]);
            if margin == 0 {
                continue;
            }
            match best {
                None => best = Some((idx, margin)),
                Some((_, current)) if margin > current => best = Some((idx, margin)),
                _ => {}
            }
        }
        match best {
            Some((idx, _)) => budgets[idx] -= 1,
            None => break, // every column at its floor; live with overflow
        }
    }
    budgets
}

pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate `s` to fit within `budget` display columns, appending `…` when
/// truncation actually happens. `budget == 0` yields an empty string;
/// `budget == 1` yields a bare `…`.
pub fn truncate_to_width(s: &str, budget: usize) -> String {
    if budget == 0 {
        return String::new();
    }
    if display_width(s) <= budget {
        return s.to_string();
    }
    if budget == 1 {
        return "…".to_string();
    }
    // Reserve one column for the ellipsis.
    let limit = budget - 1;
    let mut out = String::new();
    let mut used = 0usize;
    for ch in s.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > limit {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

// -----------------------------------------------------------------------------
// Recency formatter
// -----------------------------------------------------------------------------

/// Format `then_epoch` relative to `now_epoch` as a compact recency
/// string (`12s`, `5m`, `2h`, `3d`, `4w`). Future-dated values render
/// as `now`.
pub fn format_relative_age(then_epoch: i64, now_epoch: i64) -> String {
    let delta = now_epoch.saturating_sub(then_epoch);
    if delta < 0 {
        return "now".to_string();
    }
    let secs = delta as u64;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else if secs < 7 * 86_400 {
        format!("{}d", secs / 86_400)
    } else {
        format!("{}w", secs / (7 * 86_400))
    }
}

// -----------------------------------------------------------------------------
// Provenance/confidence indicator (ADR 0006)
// -----------------------------------------------------------------------------

/// Compact `provenance/confidence[*]` cell, given the serde tag
/// strings (e.g. `"strong_discovered"`, `"high"`). String-driven so
/// the substrate stays free of `crate::model::{Provenance,
/// Confidence}` dependencies; SQL-backed renderers in
/// [`super::agent`] read the tag columns directly. The typed-enum
/// variant lives at `super::table::indicator` for callers that
/// already hold typed values.
pub fn indicator_from_tags(provenance_tag: &str, confidence_tag: &str, ambiguous: bool) -> String {
    let mut buf = String::with_capacity(6);
    buf.push_str(provenance_code_from_tag(provenance_tag));
    buf.push('/');
    buf.push_str(confidence_code_from_tag(confidence_tag));
    if ambiguous {
        buf.push('*');
    }
    buf
}

/// Numeric precedence for a provenance serde tag, mirroring the
/// `Provenance::precedence` mapping in `crate::model`. Higher beats
/// lower; ties on `LocalDeclared` `>` `GlobalDeclared` `>`
/// `StrongDiscovered` `>` `Discovered` = `Convention` `>` `Cached`
/// are resolved by the caller. Substrate-safe (no model dep).
pub fn provenance_precedence(tag: &str) -> u8 {
    match tag {
        "local_declared" => 5,
        "global_declared" => 4,
        "strong_discovered" => 3,
        "discovered" | "convention" => 2,
        "cached" => 1,
        _ => 0,
    }
}

pub fn provenance_code_from_tag(tag: &str) -> &'static str {
    match tag {
        "local_declared" => "LD",
        "global_declared" => "GD",
        "strong_discovered" => "SD",
        "discovered" => "D",
        "convention" => "C",
        "cached" => "$",
        _ => "?",
    }
}

pub fn confidence_code_from_tag(tag: &str) -> &'static str {
    match tag {
        "high" => "H",
        "medium" => "M",
        "low" => "L",
        _ => "?",
    }
}

// -----------------------------------------------------------------------------
// Recency formatter
// -----------------------------------------------------------------------------

// (kept below for context; format_relative_age moved earlier in the file)

pub fn current_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// Module-boundary invariant: the rendering substrate must not
    /// import anything from `crate::model`. A violation here means a
    /// renderer-backend coupling has leaked into the shared layer.
    /// Catches drift introduced after P10-003 lands. Checks only `use`
    /// statements so the assertion message can name the constraint
    /// without tripping itself.
    #[test]
    fn substrate_has_no_model_deps() {
        let source = include_str!("render.rs");
        for (line_no, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("use ") || trimmed.starts_with("pub use ")) {
                continue;
            }
            assert!(
                !trimmed.contains("crate::model"),
                "render.rs:{} imports from crate model; substrate must stay model-free\n    {line}",
                line_no + 1,
            );
        }
    }
}
