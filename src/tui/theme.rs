//! Centralized TUI palette per ADR 0032.
//!
//! Every color, modifier, and badge style the TUI renderer reads
//! lives on the [`Theme`] struct. [`Theme::default`] reproduces the
//! pre-ADR-0032 inline literals byte-for-byte so existing buffer
//! snapshots stay stable when the module is introduced. Operators
//! override individual entries through the `[tui.theme]` config
//! table loaded by [`crate::config`].
//!
//! The TUI does not detect light vs dark terminals at runtime;
//! selection highlights use [`Modifier::REVERSED`] so they flip
//! against whatever fg/bg pair the terminal provides. Operators on
//! light terminals who want a different foreground palette override
//! the corresponding theme fields.

use std::collections::BTreeMap;

use ratatui::style::{Color, Modifier, Style};

use crate::tui::icons::IconOverrides;

pub use tuikit::theme::{StyleSpec, parse_color, parse_modifier, parse_style_spec};

/// Default `badge_width`: the longest registered harness label
/// (`opencode`), so harness names never truncate.
pub const DEFAULT_BADGE_WIDTH: usize = 8;

/// Narrowest `badge_width` the config accepts: one character plus `…`.
pub const MIN_BADGE_WIDTH: usize = 2;

/// Palette + modifier set for every styled surface in the TUI.
///
/// Fields are grouped by purpose with a `Color` for foreground-only
/// values and a `Modifier` for emphasis-only values. A handful of
/// fields combine both via [`StyleSpec`] so the operator can pair a
/// color with bold/italic without two config keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    // ---- harness identity ---------------------------------------------------
    /// Per-harness identity colors, keyed by adapter harness key.
    /// The default populates the four v1 harnesses;
    /// operators add or override entries via `[tui.theme.harness]`
    /// in config. Lookup goes through [`Theme::harness_color`],
    /// which walks the adapter registry so callers can pass
    /// either a harness key or a display label. Missing entries
    /// fall back to [`Theme::harness_unknown`].
    pub harness_colors: BTreeMap<String, Color>,
    pub harness_unknown: Color,

    // ---- recency buckets (Phase 3 introduces the buckets; the colors
    // ---- live here so they can be themed alongside everything else) -------
    pub recency_fresh: StyleSpec,
    pub recency_active: StyleSpec,
    pub recency_recent: StyleSpec,
    pub recency_cold: StyleSpec,

    // ---- mux state ----------------------------------------------------------
    pub mux_attached: Color,
    pub mux_unmuxed: Modifier,

    // ---- selection / focus --------------------------------------------------
    pub selection_active: Modifier,
    pub selection_inactive: Modifier,
    pub panel_focus_accent: Color,

    // ---- structural / semantic ---------------------------------------------
    pub cwd_mark: Color,
    pub link_id: Color,
    pub placeholder: Modifier,
    /// Foreground of ghost rows: entities that are gone or pins that
    /// aren't running. A concrete color, because `placeholder`'s DIM
    /// does nothing in some terminals; ghost rows are italic as well.
    pub ghost: Color,
    /// Foreground of the text a search matched, drawn bold and
    /// underlined wherever a match is picked out of a snippet, so one
    /// look means "this is what you searched for".
    pub match_highlight: Color,
    /// No longer drawn: pin slots have their own view and no
    /// placeholder rows. The key stays so existing theme files keep
    /// loading.
    pub pin_placeholder: Color,
    /// Background of the pane-command chip a mux row shows when no
    /// agent is linked (`npm`, `zsh`, …). One color for every
    /// program, drawn behind bold white text so it reads on dark and
    /// light terminals alike.
    pub command_badge: Color,
    /// Reliable foreground color for "secondary" text the operator
    /// shouldn't put the same visual weight on as primary columns —
    /// row short ids and the inline preview snippet. Modeled as a
    /// concrete `Color` rather than the `placeholder` modifier
    /// because the `DIM` modifier renders inconsistently across
    /// terminals (no effect in some popular configurations).
    pub secondary_text: Color,
    /// Color for the ► / ▼ disclosure glyphs so the affordance is
    /// distinct from the label text it sits next to. Subtle accent
    /// by default; operators can swap for a dimmer color if the
    /// glyph competes with their group labels.
    pub disclosure: Color,
    pub divider: Modifier,
    pub warning: Color,
    pub error: Color,
    pub success: Color,

    // ---- PR state (mirrors ADR 0022's table palette by default) -------------
    pub pr_open: Color,
    pub pr_closed: Color,
    pub pr_merged: Color,
    pub pr_draft: Color,

    // ---- badge composition --------------------------------------------------
    /// Modifier applied to badge spans (harness chips, status chips).
    /// Default is `REVERSED | BOLD` so the badge reads as a filled
    /// pill against any terminal theme.
    pub badge: Modifier,
    /// Label characters in every agent / program badge, set by
    /// `[tui.theme] badge_width`. Shorter labels are padded and longer
    /// ones cut with `…`, so all badges share one width and the columns
    /// after them stay aligned.
    pub badge_width: usize,

    // ---- per-node-kind identity (ADR 0073) ----------------------------------
    /// Foreground color for the per-row node-kind glyph. One field
    /// per `NodeKind` variant that owns a color; `ForgePr` does not
    /// own one (its hue comes from `pr_*` based on PR state, see
    /// ADR 0073 §2).
    pub node_workspace: Color,
    pub node_repo: Color,
    pub node_checkout: Color,
    pub node_agent_session: Color,
    pub node_mux_session: Color,
    pub node_runtime_process: Color,
    pub node_branch: Color,
    pub node_fork: Color,

    // ---- detail-pane edge state (ADR 0075) ----------------------------------
    /// Color for non-winning `EdgeStateLabel::AltOf(_)` rows in the
    /// detail-pane Other zone. Defaults to the `secondary_text`
    /// hue; operators can shift it independently of the broader
    /// secondary palette via `[tui.theme] edge_alt_of`.
    pub edge_alt_of: Color,
    /// Color for `EdgeStateLabel::Conflict` rows in the detail-pane
    /// Other zone. Defaults to the `warning` hue; operators can
    /// shift it independently of the broader warning palette via
    /// `[tui.theme] edge_conflict`.
    pub edge_conflict: Color,
    /// Operator overrides for individual node-kind glyphs, parsed
    /// from `[tui.theme.icons]`. Empty by default; lookups in
    /// [`crate::tui::icons::node_kind_style`] fall through to the
    /// ADR 0073 default slate when no override is present.
    pub icons: IconOverrides,
}

impl Default for Theme {
    /// Reproduces the pre-overhaul inline palette so introducing the
    /// theme indirection does not move pixels in existing snapshots.
    /// New fields (recency buckets, badge composition) pick values
    /// that the styling overhaul plan's later phases will exercise;
    /// they are inert until those phases call them.
    fn default() -> Self {
        // Per-harness colors keyed by adapter harness key.
        let mut harness_colors = BTreeMap::new();
        harness_colors.insert("claude-code".to_string(), Color::Magenta);
        harness_colors.insert("codex".to_string(), Color::Cyan);
        harness_colors.insert("opencode".to_string(), Color::Green);
        harness_colors.insert("aider".to_string(), Color::Red);
        harness_colors.insert("pi".to_string(), Color::LightBlue);

        Self {
            harness_colors,
            harness_unknown: Color::White,

            recency_fresh: StyleSpec::fg_mod(Color::LightGreen, Modifier::BOLD),
            recency_active: StyleSpec::fg(Color::Green),
            recency_recent: StyleSpec::fg(Color::Yellow),
            recency_cold: StyleSpec::modifier(Modifier::DIM),

            mux_attached: Color::Green,
            mux_unmuxed: Modifier::DIM,

            selection_active: Modifier::REVERSED.union(Modifier::BOLD),
            selection_inactive: Modifier::BOLD,
            panel_focus_accent: Color::Cyan,

            cwd_mark: Color::Cyan,
            link_id: Color::Blue,
            placeholder: Modifier::DIM,
            ghost: Color::DarkGray,
            match_highlight: Color::Yellow,
            pin_placeholder: Color::LightYellow,
            command_badge: Color::Black,
            secondary_text: Color::DarkGray,
            disclosure: Color::Cyan,
            divider: Modifier::DIM,
            warning: Color::Yellow,
            error: Color::Red,
            success: Color::Green,

            pr_open: Color::Green,
            pr_closed: Color::Red,
            pr_merged: Color::Magenta,
            pr_draft: Color::Yellow,

            badge: Modifier::REVERSED.union(Modifier::BOLD),

            badge_width: DEFAULT_BADGE_WIDTH,

            node_workspace: Color::LightBlue,
            node_repo: Color::Blue,
            node_checkout: Color::Cyan,
            node_agent_session: Color::LightGreen,
            node_mux_session: Color::Magenta,
            node_runtime_process: Color::DarkGray,
            node_branch: Color::Green,
            node_fork: Color::LightMagenta,

            edge_alt_of: Color::DarkGray,
            edge_conflict: Color::Yellow,
            icons: IconOverrides::default(),
        }
    }
}

impl Theme {
    /// The style of matched text in a search snippet: the same on every
    /// surface, a foreground color rather than a background so it reads
    /// on a selected (reversed) row too.
    pub fn match_style(&self) -> Style {
        Style::default()
            .fg(self.match_highlight)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// Color associated with a harness. Accepts either the
    /// canonical harness key (`claude-code`) or the display label
    /// (`claude`) — both resolve to the same entry in
    /// [`Self::harness_colors`] by walking the adapter registry.
    /// Unknown harnesses fall back to
    /// [`Self::harness_unknown`] so the renderer always has a
    /// hue to use.
    pub fn harness_color(&self, label_or_key: &str) -> Color {
        // Resolve label → key via the registry so callers that
        // pass display labels hit the same map entry as callers
        // that pass harness keys.
        let canonical_key = crate::discovery::harness::registered_adapters()
            .find(|a| a.harness_key() == label_or_key || a.display_label() == label_or_key)
            .map(super::super::discovery::harness::HarnessAdapter::harness_key);
        match canonical_key {
            Some(key) => self
                .harness_colors
                .get(key)
                .copied()
                .unwrap_or(self.harness_unknown),
            None => self
                .harness_colors
                .get(label_or_key)
                .copied()
                .unwrap_or(self.harness_unknown),
        }
    }
}

// -----------------------------------------------------------------------------
// Color / modifier spec parsing (ADR 0032)
// -----------------------------------------------------------------------------

/// Theme key kinds, used by the config loader to route each
/// `[tui.theme]` entry to the right parser. Returned by
/// [`Theme::known_keys`] so the loader can iterate without
/// hardcoding the list in two places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKeyKind {
    /// Foreground color only.
    Color,
    /// Modifier set only.
    Modifier,
    /// Color + optional comma-joined modifiers.
    StyleSpec,
}

/// One entry in [`Theme::known_keys`]. The config loader walks this
/// to validate the user's `[tui.theme]` table and dispatch on kind.
pub struct ThemeKey {
    pub name: &'static str,
    pub kind: ThemeKeyKind,
}

impl Theme {
    /// Every key the operator may override under `[tui.theme]`, with
    /// its expected value kind. The loader iterates this to validate
    /// unknown keys and route value parsing.
    pub fn known_keys() -> &'static [ThemeKey] {
        use ThemeKeyKind::*;
        &[
            ThemeKey {
                name: "harness_claude",
                kind: Color,
            },
            ThemeKey {
                name: "harness_codex",
                kind: Color,
            },
            ThemeKey {
                name: "harness_opencode",
                kind: Color,
            },
            ThemeKey {
                name: "harness_aider",
                kind: Color,
            },
            ThemeKey {
                name: "harness_pi",
                kind: Color,
            },
            ThemeKey {
                name: "harness_unknown",
                kind: Color,
            },
            ThemeKey {
                name: "recency_fresh",
                kind: StyleSpec,
            },
            ThemeKey {
                name: "recency_active",
                kind: StyleSpec,
            },
            ThemeKey {
                name: "recency_recent",
                kind: StyleSpec,
            },
            ThemeKey {
                name: "recency_cold",
                kind: StyleSpec,
            },
            ThemeKey {
                name: "mux_attached",
                kind: Color,
            },
            ThemeKey {
                name: "mux_unmuxed",
                kind: Modifier,
            },
            ThemeKey {
                name: "selection_active",
                kind: Modifier,
            },
            ThemeKey {
                name: "selection_inactive",
                kind: Modifier,
            },
            ThemeKey {
                name: "panel_focus_accent",
                kind: Color,
            },
            ThemeKey {
                name: "cwd_mark",
                kind: Color,
            },
            ThemeKey {
                name: "link_id",
                kind: Color,
            },
            ThemeKey {
                name: "placeholder",
                kind: Modifier,
            },
            ThemeKey {
                name: "ghost",
                kind: Color,
            },
            ThemeKey {
                name: "match_highlight",
                kind: Color,
            },
            ThemeKey {
                name: "pin_placeholder",
                kind: Color,
            },
            ThemeKey {
                name: "command_badge",
                kind: Color,
            },
            ThemeKey {
                name: "secondary_text",
                kind: Color,
            },
            ThemeKey {
                name: "disclosure",
                kind: Color,
            },
            ThemeKey {
                name: "divider",
                kind: Modifier,
            },
            ThemeKey {
                name: "warning",
                kind: Color,
            },
            ThemeKey {
                name: "error",
                kind: Color,
            },
            ThemeKey {
                name: "success",
                kind: Color,
            },
            ThemeKey {
                name: "pr_open",
                kind: Color,
            },
            ThemeKey {
                name: "pr_closed",
                kind: Color,
            },
            ThemeKey {
                name: "pr_merged",
                kind: Color,
            },
            ThemeKey {
                name: "pr_draft",
                kind: Color,
            },
            ThemeKey {
                name: "badge",
                kind: Modifier,
            },
            ThemeKey {
                name: "node_workspace",
                kind: Color,
            },
            ThemeKey {
                name: "node_repo",
                kind: Color,
            },
            ThemeKey {
                name: "node_checkout",
                kind: Color,
            },
            ThemeKey {
                name: "node_agent_session",
                kind: Color,
            },
            ThemeKey {
                name: "node_mux_session",
                kind: Color,
            },
            ThemeKey {
                name: "node_runtime_process",
                kind: Color,
            },
            ThemeKey {
                name: "node_branch",
                kind: Color,
            },
            ThemeKey {
                name: "node_fork",
                kind: Color,
            },
            ThemeKey {
                name: "edge_alt_of",
                kind: Color,
            },
            ThemeKey {
                name: "edge_conflict",
                kind: Color,
            },
        ]
    }

    /// Apply a parsed color to the named field. Returns `false` when
    /// the name is not a `Color`-kind field; the caller treats that
    /// as a routing-error diagnostic.
    pub fn set_color(&mut self, name: &str, color: Color) -> bool {
        match name {
            // Legacy flat harness aliases. Kept for config
            // back-compat per ADR 0031's precedent for
            // grandfathered keys; they route to the same
            // `harness_colors` entry the new
            // `[tui.theme.harness].<key>` form would.
            "harness_claude" => {
                self.harness_colors.insert("claude-code".to_string(), color);
            }
            "harness_codex" => {
                self.harness_colors.insert("codex".to_string(), color);
            }
            "harness_opencode" => {
                self.harness_colors.insert("opencode".to_string(), color);
            }
            "harness_aider" => {
                self.harness_colors.insert("aider".to_string(), color);
            }
            "harness_pi" => {
                self.harness_colors.insert("pi".to_string(), color);
            }
            "harness_unknown" => self.harness_unknown = color,
            "mux_attached" => self.mux_attached = color,
            "panel_focus_accent" => self.panel_focus_accent = color,
            "cwd_mark" => self.cwd_mark = color,
            "link_id" => self.link_id = color,
            "ghost" => self.ghost = color,
            "match_highlight" => self.match_highlight = color,
            "pin_placeholder" => self.pin_placeholder = color,
            "command_badge" => self.command_badge = color,
            "secondary_text" => self.secondary_text = color,
            "disclosure" => self.disclosure = color,
            "warning" => self.warning = color,
            "error" => self.error = color,
            "success" => self.success = color,
            "pr_open" => self.pr_open = color,
            "pr_closed" => self.pr_closed = color,
            "pr_merged" => self.pr_merged = color,
            "pr_draft" => self.pr_draft = color,
            "node_workspace" => self.node_workspace = color,
            "node_repo" => self.node_repo = color,
            "node_checkout" => self.node_checkout = color,
            "node_agent_session" => self.node_agent_session = color,
            "node_mux_session" => self.node_mux_session = color,
            "node_runtime_process" => self.node_runtime_process = color,
            "node_branch" => self.node_branch = color,
            "node_fork" => self.node_fork = color,
            "edge_alt_of" => self.edge_alt_of = color,
            "edge_conflict" => self.edge_conflict = color,
            _ => return false,
        }
        true
    }

    pub fn set_modifier(&mut self, name: &str, modifier: Modifier) -> bool {
        match name {
            "mux_unmuxed" => self.mux_unmuxed = modifier,
            "selection_active" => self.selection_active = modifier,
            "selection_inactive" => self.selection_inactive = modifier,
            "placeholder" => self.placeholder = modifier,
            "divider" => self.divider = modifier,
            "badge" => self.badge = modifier,
            _ => return false,
        }
        true
    }

    pub fn set_style_spec(&mut self, name: &str, spec: StyleSpec) -> bool {
        match name {
            "recency_fresh" => self.recency_fresh = spec,
            "recency_active" => self.recency_active = spec,
            "recency_recent" => self.recency_recent = spec,
            "recency_cold" => self.recency_cold = spec,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
