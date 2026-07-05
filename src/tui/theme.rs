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

/// Palette + modifier set for every styled surface in the TUI.
///
/// Fields are grouped by purpose with a `Color` for foreground-only
/// values and a `Modifier` for emphasis-only values. A handful of
/// fields combine both via [`StyleSpec`] so the operator can pair a
/// color with bold/italic without two config keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    // ---- harness identity ---------------------------------------------------
    /// Per-harness identity colors, keyed by adapter harness key
    /// (H-EXT-003). The default populates the four v1 harnesses;
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
    pub mux_ambiguous: Color,
    pub mux_unmuxed: Modifier,

    // ---- selection / focus --------------------------------------------------
    pub selection_active: Modifier,
    pub selection_inactive: Modifier,
    pub panel_focus_accent: Color,

    // ---- structural / semantic ---------------------------------------------
    pub cwd_mark: Color,
    pub link_id: Color,
    pub placeholder: Modifier,
    /// Foreground color for the placeholder-pin glyph (`◌`) that
    /// stands in for the attached-state glyph on unbound-pin rows.
    /// Default is a bright yellow so the dotted circle reads as a
    /// distinct affordance against the solid `◉` / `◯` glyphs without
    /// being mistaken for the dim `mux_unmuxed` state.
    pub pin_placeholder: Color,
    /// Reliable foreground color for "secondary" text the operator
    /// shouldn't put the same visual weight on as primary columns —
    /// row short ids and the inline preview snippet. Modeled as a
    /// concrete `Color` rather than the `placeholder` modifier
    /// because the `DIM` modifier renders inconsistently across
    /// terminals (no effect in some popular configurations).
    pub secondary_text: Color,
    /// Color for the ▶ / ▼ disclosure glyphs so the affordance is
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

/// Color + modifier pair. Used for theme fields where the operator
/// might want to combine a hue with emphasis (e.g. `"green,bold"`
/// for the fresh-recency bucket). When [`color`] is `None`, the
/// renderer inherits the default fg; when [`modifier`] is empty, no
/// modifier is added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleSpec {
    pub color: Option<Color>,
    pub modifier: Modifier,
}

impl StyleSpec {
    pub const fn fg(color: Color) -> Self {
        Self {
            color: Some(color),
            modifier: Modifier::empty(),
        }
    }

    pub const fn modifier(modifier: Modifier) -> Self {
        Self {
            color: None,
            modifier,
        }
    }

    pub const fn fg_mod(color: Color, modifier: Modifier) -> Self {
        Self {
            color: Some(color),
            modifier,
        }
    }

    /// Build a `Style` for use in `Span::styled` calls.
    pub fn into_style(self) -> Style {
        let mut style = Style::default();
        if let Some(color) = self.color {
            style = style.fg(color);
        }
        if !self.modifier.is_empty() {
            style = style.add_modifier(self.modifier);
        }
        style
    }
}

impl Default for Theme {
    /// Reproduces the pre-overhaul inline palette so introducing the
    /// theme indirection does not move pixels in existing snapshots.
    /// New fields (recency buckets, badge composition) pick values
    /// that the styling overhaul plan's later phases will exercise;
    /// they are inert until those phases call them.
    fn default() -> Self {
        // H-EXT-003: per-harness colors keyed by adapter harness
        // key. Values match the pre-H-EXT-003 flat-field defaults
        // so existing snapshots stay stable.
        let mut harness_colors = BTreeMap::new();
        harness_colors.insert("claude-code".to_string(), Color::Magenta);
        harness_colors.insert("codex".to_string(), Color::Cyan);
        harness_colors.insert("opencode".to_string(), Color::Green);
        harness_colors.insert("aider".to_string(), Color::Red);

        Self {
            harness_colors,
            harness_unknown: Color::White,

            recency_fresh: StyleSpec::fg_mod(Color::LightGreen, Modifier::BOLD),
            recency_active: StyleSpec::fg(Color::Green),
            recency_recent: StyleSpec::fg(Color::Yellow),
            recency_cold: StyleSpec::modifier(Modifier::DIM),

            mux_attached: Color::Green,
            mux_ambiguous: Color::Yellow,
            mux_unmuxed: Modifier::DIM,

            selection_active: Modifier::REVERSED.union(Modifier::BOLD),
            selection_inactive: Modifier::BOLD,
            panel_focus_accent: Color::Cyan,

            cwd_mark: Color::Cyan,
            link_id: Color::Blue,
            placeholder: Modifier::DIM,
            pin_placeholder: Color::LightYellow,
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
    /// Color associated with a harness. Accepts either the
    /// canonical harness key (`claude-code`) or the display label
    /// (`claude`) — both resolve to the same entry in
    /// [`Self::harness_colors`] by walking the adapter registry
    /// (H-EXT-003). Unknown harnesses fall back to
    /// [`Self::harness_unknown`] so the renderer always has a
    /// hue to use.
    pub fn harness_color(&self, label_or_key: &str) -> Color {
        // Resolve label → key via the registry so the
        // pre-H-EXT-003 badge callers (which pass display
        // labels) hit the same map entry as new callers that
        // pass harness keys.
        let canonical_key = crate::discovery::harness::registered_adapters()
            .find(|a| a.harness_key() == label_or_key || a.display_label() == label_or_key)
            .map(|a| a.harness_key());
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
                name: "mux_ambiguous",
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
                name: "pin_placeholder",
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
            // Legacy flat harness aliases (pre-H-EXT-003). Kept
            // for config back-compat per ADR 0031's precedent for
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
            "harness_unknown" => self.harness_unknown = color,
            "mux_attached" => self.mux_attached = color,
            "mux_ambiguous" => self.mux_ambiguous = color,
            "panel_focus_accent" => self.panel_focus_accent = color,
            "cwd_mark" => self.cwd_mark = color,
            "link_id" => self.link_id = color,
            "pin_placeholder" => self.pin_placeholder = color,
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

/// Parse a single color token: a named ANSI color, `ansi256:N`, or
/// `#RRGGBB`. Returns the rejected reason on failure so the loader
/// can surface it in a per-key warning.
pub fn parse_color(raw: &str) -> Result<Color, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty color value".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(color) = parse_named_color(&lower) {
        return Ok(color);
    }
    if let Some(idx) = lower.strip_prefix("ansi256:") {
        return idx
            .trim()
            .parse::<u8>()
            .map(Color::Indexed)
            .map_err(|_| format!("`{trimmed}`: expected `ansi256:N` with N in 0..=255"));
    }
    if let Some(hex) = lower.strip_prefix('#') {
        return parse_hex_color(hex)
            .ok_or_else(|| format!("`{trimmed}`: expected `#RRGGBB` hex color"));
    }
    Err(format!(
        "`{trimmed}` is not a recognized color (try a named color, `ansi256:N`, or `#RRGGBB`)"
    ))
}

fn parse_named_color(lower: &str) -> Option<Color> {
    Some(match lower {
        "default" | "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" | "dark_gray" | "dark_grey" => Color::DarkGray,
        "bright_black" => Color::DarkGray,
        "bright_red" => Color::LightRed,
        "bright_green" => Color::LightGreen,
        "bright_yellow" => Color::LightYellow,
        "bright_blue" => Color::LightBlue,
        "bright_magenta" => Color::LightMagenta,
        "bright_cyan" => Color::LightCyan,
        "bright_white" => Color::White,
        _ => return None,
    })
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Parse a comma-joined modifier list: `"bold,italic"` →
/// `BOLD | ITALIC`. Empty input yields the empty modifier set.
pub fn parse_modifier(raw: &str) -> Result<Modifier, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Modifier::empty());
    }
    let mut acc = Modifier::empty();
    for token in trimmed.split(',') {
        let token = token.trim().to_ascii_lowercase();
        let bit = match token.as_str() {
            "bold" => Modifier::BOLD,
            "dim" => Modifier::DIM,
            "italic" => Modifier::ITALIC,
            "underline" | "underlined" => Modifier::UNDERLINED,
            "reversed" | "reverse" => Modifier::REVERSED,
            "crossed_out" | "strikethrough" => Modifier::CROSSED_OUT,
            "slow_blink" => Modifier::SLOW_BLINK,
            "rapid_blink" => Modifier::RAPID_BLINK,
            "hidden" => Modifier::HIDDEN,
            "" => continue,
            other => {
                return Err(format!(
                    "`{other}` is not a recognized modifier (expected bold, dim, italic, \
                         underline, reversed, crossed_out)"
                ));
            }
        };
        acc |= bit;
    }
    Ok(acc)
}

/// Parse a style spec: `"color"`, `"color,mod1,mod2"`, or
/// `"mod1,mod2"`. The first comma-separated token may be a color or
/// a modifier; any subsequent token must be a modifier.
pub fn parse_style_spec(raw: &str) -> Result<StyleSpec, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(StyleSpec {
            color: None,
            modifier: Modifier::empty(),
        });
    }
    let mut parts = trimmed.split(',').map(str::trim);
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();

    // Try the first token as a color; fall through to modifier-only
    // parsing on failure so `"bold"` and `"dim,italic"` both work
    // without a leading color.
    if let Ok(color) = parse_color(first) {
        let modifier = if rest.is_empty() {
            Modifier::empty()
        } else {
            parse_modifier(&rest.join(","))?
        };
        return Ok(StyleSpec {
            color: Some(color),
            modifier,
        });
    }
    let modifier = parse_modifier(trimmed)?;
    Ok(StyleSpec {
        color: None,
        modifier,
    })
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
