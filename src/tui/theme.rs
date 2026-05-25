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

use ratatui::style::{Color, Modifier, Style};

/// Palette + modifier set for every styled surface in the TUI.
///
/// Fields are grouped by purpose with a `Color` for foreground-only
/// values and a `Modifier` for emphasis-only values. A handful of
/// fields combine both via [`StyleSpec`] so the operator can pair a
/// color with bold/italic without two config keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    // ---- harness identity ---------------------------------------------------
    pub harness_claude: Color,
    pub harness_codex: Color,
    pub harness_opencode: Color,
    pub harness_aider: Color,
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
        Self {
            harness_claude: Color::Magenta,
            harness_codex: Color::Cyan,
            harness_opencode: Color::Green,
            harness_aider: Color::Red,
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
        }
    }
}

impl Theme {
    /// Color associated with a harness label. Unknown harnesses fall
    /// back to [`Self::harness_unknown`] so the renderer always has a
    /// hue to use.
    pub fn harness_color(&self, label: &str) -> Color {
        match label {
            "claude" => self.harness_claude,
            "codex" => self.harness_codex,
            "opencode" => self.harness_opencode,
            "aider" => self.harness_aider,
            _ => self.harness_unknown,
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
        ]
    }

    /// Apply a parsed color to the named field. Returns `false` when
    /// the name is not a `Color`-kind field; the caller treats that
    /// as a routing-error diagnostic.
    pub fn set_color(&mut self, name: &str, color: Color) -> bool {
        match name {
            "harness_claude" => self.harness_claude = color,
            "harness_codex" => self.harness_codex = color,
            "harness_opencode" => self.harness_opencode = color,
            "harness_aider" => self.harness_aider = color,
            "harness_unknown" => self.harness_unknown = color,
            "mux_attached" => self.mux_attached = color,
            "mux_ambiguous" => self.mux_ambiguous = color,
            "panel_focus_accent" => self.panel_focus_accent = color,
            "cwd_mark" => self.cwd_mark = color,
            "link_id" => self.link_id = color,
            "secondary_text" => self.secondary_text = color,
            "disclosure" => self.disclosure = color,
            "warning" => self.warning = color,
            "error" => self.error = color,
            "success" => self.success = color,
            "pr_open" => self.pr_open = color,
            "pr_closed" => self.pr_closed = color,
            "pr_merged" => self.pr_merged = color,
            "pr_draft" => self.pr_draft = color,
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
mod tests {
    use super::*;

    #[test]
    fn default_preserves_inline_literals() {
        let theme = Theme::default();
        assert_eq!(theme.harness_color("claude"), Color::Magenta);
        assert_eq!(theme.harness_color("codex"), Color::Cyan);
        assert_eq!(theme.harness_color("opencode"), Color::Green);
        assert_eq!(theme.harness_color("aider"), Color::Red);
        assert_eq!(theme.harness_color("unknown-harness"), Color::White);
        assert_eq!(theme.mux_attached, Color::Green);
        assert_eq!(theme.mux_ambiguous, Color::Yellow);
        assert_eq!(theme.mux_unmuxed, Modifier::DIM);
        assert_eq!(
            theme.selection_active,
            Modifier::REVERSED.union(Modifier::BOLD)
        );
    }

    #[test]
    fn style_spec_into_style_combines_color_and_modifier() {
        let spec = StyleSpec::fg_mod(Color::Green, Modifier::BOLD);
        let style = spec.into_style();
        assert_eq!(style.fg, Some(Color::Green));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn style_spec_modifier_only_omits_fg() {
        let spec = StyleSpec::modifier(Modifier::DIM);
        let style = spec.into_style();
        assert_eq!(style.fg, None);
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn parse_color_accepts_named_ansi() {
        assert_eq!(parse_color("magenta").unwrap(), Color::Magenta);
        assert_eq!(parse_color("Bright_Green").unwrap(), Color::LightGreen);
        assert_eq!(parse_color("default").unwrap(), Color::Reset);
    }

    #[test]
    fn parse_color_accepts_ansi256_and_hex() {
        assert_eq!(parse_color("ansi256:42").unwrap(), Color::Indexed(42));
        assert_eq!(
            parse_color("#ff8800").unwrap(),
            Color::Rgb(0xff, 0x88, 0x00)
        );
        assert_eq!(parse_color("#FFFFFF").unwrap(), Color::Rgb(255, 255, 255));
    }

    #[test]
    fn parse_color_rejects_garbage() {
        assert!(parse_color("").is_err());
        assert!(parse_color("not-a-color").is_err());
        assert!(parse_color("ansi256:999").is_err());
        assert!(parse_color("#zzzzzz").is_err());
        assert!(parse_color("#abc").is_err());
    }

    #[test]
    fn parse_modifier_accepts_comma_list() {
        let m = parse_modifier("bold,italic").unwrap();
        assert!(m.contains(Modifier::BOLD));
        assert!(m.contains(Modifier::ITALIC));
        assert_eq!(parse_modifier("").unwrap(), Modifier::empty());
        assert_eq!(
            parse_modifier("reverse").unwrap(),
            Modifier::REVERSED,
            "`reverse` is an accepted alias for `reversed`"
        );
    }

    #[test]
    fn parse_modifier_rejects_unknown_token() {
        assert!(parse_modifier("bold,not-a-mod").is_err());
    }

    #[test]
    fn parse_style_spec_handles_color_plus_modifier() {
        let spec = parse_style_spec("green,bold").unwrap();
        assert_eq!(spec.color, Some(Color::Green));
        assert!(spec.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn parse_style_spec_handles_modifier_only() {
        let spec = parse_style_spec("dim").unwrap();
        assert_eq!(spec.color, None);
        assert!(spec.modifier.contains(Modifier::DIM));
    }

    #[test]
    fn parse_style_spec_handles_color_only() {
        let spec = parse_style_spec("#ff8800").unwrap();
        assert_eq!(spec.color, Some(Color::Rgb(0xff, 0x88, 0x00)));
        assert_eq!(spec.modifier, Modifier::empty());
    }

    #[test]
    fn known_keys_cover_every_settable_field() {
        // If a field is in known_keys with kind Color, set_color must
        // accept its name; same for Modifier and StyleSpec. Catches
        // drift when fields are added to the struct but missed in
        // known_keys / set_*.
        let mut theme = Theme::default();
        for entry in Theme::known_keys() {
            let accepted = match entry.kind {
                ThemeKeyKind::Color => theme.set_color(entry.name, Color::Black),
                ThemeKeyKind::Modifier => theme.set_modifier(entry.name, Modifier::BOLD),
                ThemeKeyKind::StyleSpec => {
                    theme.set_style_spec(entry.name, StyleSpec::fg(Color::Black))
                }
            };
            assert!(accepted, "set_* refused known key `{}`", entry.name);
        }
    }
}
