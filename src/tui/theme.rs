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
}
