//! The TUI palette (ADR 0032): the framework's design tokens plus the
//! roles only Conspectus knows.
//!
//! [`Theme`] wraps [`tuikit::theme::Theme`] and dereferences to it, so
//! render code reads the framework's typed tokens as fields
//! (`theme.accent`, `theme.muted`, `theme.inert`, `theme.warning`) and
//! Conspectus's own roles through accessors (`theme.pr_open()`,
//! `theme.harness_color(..)`). The roles are registered in the framework
//! theme's named-style map from [`ROLES`] and [`HARNESS_COLORS`], and the
//! keys older configuration files use are registered as aliases from
//! [`ALIASES`]. Those three tables and the framework's token table are the
//! only list of `[tui.theme]` keys: the config loader, the docs drift test,
//! and the widget preview all read [`tuikit::theme::Theme::keys`] and
//! [`tuikit::theme::Theme::aliases`].
//!
//! [`Theme::default`] draws exactly what the TUI drew before the tokens
//! moved into the framework. Five older muted keys now feed two tokens:
//! `placeholder`, `divider`, and `mux_unmuxed` set `muted` (a modifier,
//! `DIM`), and `secondary_text` and `ghost` set `inert` (a color, dark
//! gray). Within one file a token's own key wins, then its aliases in the
//! order [`ALIASES`] lists them; `docs/operations.md` spells this out for
//! operators.
//!
//! The TUI does not detect light or dark terminals; selection highlights
//! use reversed video so they flip against whatever colors the terminal
//! provides.

use std::ops::{Deref, DerefMut};

use ratatui::style::Color;

pub use tuikit::theme::{
    Alias, DEFAULT_BADGE_WIDTH, KeyOrigin, MIN_BADGE_WIDTH, OverrideError, OverrideProblem,
    OverrideValue, StyleSpec, ThemeKey, TokenKind, TokenValue, parse_color, parse_modifier,
    parse_style_spec,
};

use crate::tui::icons::IconOverrides;

/// The names Conspectus registers its roles under, which are also their
/// `[tui.theme]` keys.
pub mod role {
    /// Badge color of a harness no other entry names.
    pub const HARNESS_UNKNOWN: &str = "harness_unknown";
    /// Prefix of the per-harness badge colors, `harness.<key>`, which
    /// `[tui.theme.harness]` sets.
    pub const HARNESS_PREFIX: &str = "harness.";
    /// The linked or attached mux glyph.
    pub const MUX_ATTACHED: &str = "mux_attached";
    /// The `(cwd)` launch-context mark.
    pub const CWD_MARK: &str = "cwd_mark";
    /// Background of the pane-command chip.
    pub const COMMAND_BADGE: &str = "command_badge";
    /// Text of the pane-command chip.
    pub const COMMAND_BADGE_TEXT: &str = "command_badge_text";
    /// An open PR.
    pub const PR_OPEN: &str = "pr_open";
    /// A closed PR.
    pub const PR_CLOSED: &str = "pr_closed";
    /// A merged PR.
    pub const PR_MERGED: &str = "pr_merged";
    /// A draft PR.
    pub const PR_DRAFT: &str = "pr_draft";
    /// The workspace node-kind glyph.
    pub const NODE_WORKSPACE: &str = "node_workspace";
    /// The repo node-kind glyph.
    pub const NODE_REPO: &str = "node_repo";
    /// The checkout node-kind glyph.
    pub const NODE_CHECKOUT: &str = "node_checkout";
    /// The agent-session node-kind glyph.
    pub const NODE_AGENT_SESSION: &str = "node_agent_session";
    /// The mux-session node-kind glyph, which pins share.
    pub const NODE_MUX_SESSION: &str = "node_mux_session";
    /// The runtime-process node-kind glyph.
    pub const NODE_RUNTIME_PROCESS: &str = "node_runtime_process";
    /// The branch node-kind glyph.
    pub const NODE_BRANCH: &str = "node_branch";
    /// The fork node-kind glyph.
    pub const NODE_FORK: &str = "node_fork";
    /// A non-winning `AltOf` link row in the detail pane.
    pub const EDGE_ALT_OF: &str = "edge_alt_of";
    /// A `Conflict` link row in the detail pane.
    pub const EDGE_CONFLICT: &str = "edge_conflict";
    /// A compaction summary's chip in the transcript viewer; the name is
    /// the viewer's own.
    pub const TRANSCRIPT_COMPACTION: &str = crate::viewer::theme::COMPACTION;
    /// No longer drawn; kept so older theme files load.
    pub const PIN_PLACEHOLDER: &str = "pin_placeholder";
}

/// One role Conspectus registers: its key, its default, and what it styles.
pub struct Role {
    pub key: &'static str,
    pub default: TokenValue,
    pub about: &'static str,
}

const fn color(key: &'static str, default: Color, about: &'static str) -> Role {
    Role {
        key,
        default: TokenValue::Color(default),
        about,
    }
}

/// The roles Conspectus registers in the framework theme, besides the
/// per-harness colors.
pub const ROLES: &[Role] = &[
    color(
        role::HARNESS_UNKNOWN,
        Color::White,
        "badge of a harness no entry names",
    ),
    color(
        role::MUX_ATTACHED,
        Color::Green,
        "linked or attached mux glyph and counts",
    ),
    color(role::CWD_MARK, Color::Cyan, "the (cwd) launch-context mark"),
    color(
        role::COMMAND_BADGE,
        Color::Black,
        "background of a mux row's pane-command chip",
    ),
    color(
        role::COMMAND_BADGE_TEXT,
        Color::White,
        "text of a mux row's pane-command chip (drawn bold)",
    ),
    color(
        role::PR_OPEN,
        Color::Green,
        "open PRs, and a PR glyph drawn without a state",
    ),
    color(role::PR_CLOSED, Color::Red, "closed PRs"),
    color(role::PR_MERGED, Color::Magenta, "merged PRs"),
    color(role::PR_DRAFT, Color::Yellow, "draft PRs"),
    color(role::NODE_WORKSPACE, Color::LightBlue, "workspace glyph"),
    color(role::NODE_REPO, Color::Blue, "repo glyph"),
    color(role::NODE_CHECKOUT, Color::Cyan, "checkout glyph"),
    color(
        role::NODE_AGENT_SESSION,
        Color::LightGreen,
        "agent session glyph",
    ),
    color(
        role::NODE_MUX_SESSION,
        Color::Magenta,
        "mux session and pin glyph",
    ),
    color(
        role::NODE_RUNTIME_PROCESS,
        Color::DarkGray,
        "runtime process glyph",
    ),
    color(role::NODE_BRANCH, Color::Green, "branch glyph"),
    color(role::NODE_FORK, Color::LightMagenta, "fork glyph"),
    color(
        role::EDGE_ALT_OF,
        Color::DarkGray,
        "a non-winning alternative link row in the detail pane",
    ),
    color(
        role::EDGE_CONFLICT,
        Color::Yellow,
        "a conflicting link row in the detail pane",
    ),
    color(
        role::TRANSCRIPT_COMPACTION,
        Color::Magenta,
        "the chip of a compaction summary in the transcript viewer",
    ),
    color(
        role::PIN_PLACEHOLDER,
        Color::LightYellow,
        "no longer drawn; kept so older theme files load",
    ),
];

/// Default badge color of each harness, by harness key, registered as
/// `harness.<key>`. Every registered adapter has an entry, and every entry
/// is a registered adapter (a test checks both).
pub const HARNESS_COLORS: &[(&str, Color)] = &[
    ("claude-code", Color::Magenta),
    ("codex", Color::Cyan),
    ("opencode", Color::Green),
    ("aider", Color::Red),
    ("pi", Color::LightBlue),
];

/// Keys older configuration files use, and the token or role each now
/// sets. For one target, the order here is the precedence among its
/// aliases; the target's own key beats them all.
pub const ALIASES: &[(&str, &str)] = &[
    ("placeholder", "muted"),
    ("divider", "muted"),
    ("mux_unmuxed", "muted"),
    ("secondary_text", "inert"),
    ("ghost", "inert"),
    ("link_id", "link"),
    ("panel_focus_accent", "accent"),
    ("harness_claude", "harness.claude-code"),
    ("harness_codex", "harness.codex"),
    ("harness_opencode", "harness.opencode"),
    ("harness_aider", "harness.aider"),
    ("harness_pi", "harness.pi"),
];

/// The name of a harness's color in the theme, `harness.<key>`, for the
/// harness key (`claude-code`) or display label (`claude`) of a
/// registered adapter, or [`role::HARNESS_UNKNOWN`] for any other: what a
/// surface that reads named styles (the transcript viewer's title) is
/// handed.
pub fn harness_role(label_or_key: &str) -> String {
    crate::discovery::harness::registered_adapters()
        .find(|a| a.harness_key() == label_or_key || a.display_label() == label_or_key)
        .map_or_else(
            || role::HARNESS_UNKNOWN.to_string(),
            |adapter| format!("{}{}", role::HARNESS_PREFIX, adapter.harness_key()),
        )
}

/// The TUI's theme: the framework theme with Conspectus's roles and
/// aliases registered, plus the node-kind glyph overrides from
/// `[tui.theme.icons]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    kit: tuikit::theme::Theme,
    /// Operator overrides for individual node-kind glyphs, parsed from
    /// `[tui.theme.icons]`. Empty by default; lookups in
    /// [`crate::tui::icons::node_kind_style`] fall through to the default
    /// slate when no override is present.
    pub icons: IconOverrides,
}

impl Default for Theme {
    fn default() -> Self {
        let mut kit = tuikit::theme::Theme::default();
        for role in ROLES {
            kit.register(role.key, role.default, role.about);
        }
        for (key, color) in HARNESS_COLORS {
            kit.register(
                format!("{}{key}", role::HARNESS_PREFIX),
                *color,
                format!("{key} badge and chip"),
            );
        }
        for (key, target) in ALIASES {
            kit.register_alias(*key, *target);
        }
        Self {
            kit,
            icons: IconOverrides::default(),
        }
    }
}

impl Deref for Theme {
    type Target = tuikit::theme::Theme;

    fn deref(&self) -> &Self::Target {
        &self.kit
    }
}

impl DerefMut for Theme {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.kit
    }
}

impl Theme {
    /// The color registered under `name`; every role in [`ROLES`] is
    /// registered, so the default only answers for a name that is not.
    fn role(&self, name: &str) -> Color {
        self.kit.color(name).unwrap_or_default()
    }

    /// Color associated with a harness. Accepts either the canonical
    /// harness key (`claude-code`) or the display label (`claude`): both
    /// resolve to the same `harness.<key>` entry by walking the adapter
    /// registry. Unknown harnesses take [`Self::harness_unknown`].
    pub fn harness_color(&self, label_or_key: &str) -> Color {
        self.kit
            .color(&harness_role(label_or_key))
            .unwrap_or_else(|| self.harness_unknown())
    }

    /// Badge color of a harness no entry names.
    pub fn harness_unknown(&self) -> Color {
        self.role(role::HARNESS_UNKNOWN)
    }

    /// The linked or attached mux glyph.
    pub fn mux_attached(&self) -> Color {
        self.role(role::MUX_ATTACHED)
    }

    /// The `(cwd)` launch-context mark.
    pub fn cwd_mark(&self) -> Color {
        self.role(role::CWD_MARK)
    }

    /// Background of the pane-command chip a mux row shows when no agent
    /// is linked (`npm`, `zsh`, …): one color for every program.
    pub fn command_badge(&self) -> Color {
        self.role(role::COMMAND_BADGE)
    }

    /// Text of the pane-command chip, drawn bold over
    /// [`Self::command_badge`].
    pub fn command_badge_text(&self) -> Color {
        self.role(role::COMMAND_BADGE_TEXT)
    }

    /// An open PR, and a PR glyph drawn where the state is unknown.
    pub fn pr_open(&self) -> Color {
        self.role(role::PR_OPEN)
    }

    /// A closed PR.
    pub fn pr_closed(&self) -> Color {
        self.role(role::PR_CLOSED)
    }

    /// A merged PR.
    pub fn pr_merged(&self) -> Color {
        self.role(role::PR_MERGED)
    }

    /// A draft PR.
    pub fn pr_draft(&self) -> Color {
        self.role(role::PR_DRAFT)
    }

    /// The workspace node-kind glyph (ADR 0073).
    pub fn node_workspace(&self) -> Color {
        self.role(role::NODE_WORKSPACE)
    }

    /// The repo node-kind glyph.
    pub fn node_repo(&self) -> Color {
        self.role(role::NODE_REPO)
    }

    /// The checkout node-kind glyph.
    pub fn node_checkout(&self) -> Color {
        self.role(role::NODE_CHECKOUT)
    }

    /// The agent-session node-kind glyph.
    pub fn node_agent_session(&self) -> Color {
        self.role(role::NODE_AGENT_SESSION)
    }

    /// The mux-session node-kind glyph, which pins share.
    pub fn node_mux_session(&self) -> Color {
        self.role(role::NODE_MUX_SESSION)
    }

    /// The runtime-process node-kind glyph.
    pub fn node_runtime_process(&self) -> Color {
        self.role(role::NODE_RUNTIME_PROCESS)
    }

    /// The branch node-kind glyph.
    pub fn node_branch(&self) -> Color {
        self.role(role::NODE_BRANCH)
    }

    /// The fork node-kind glyph.
    pub fn node_fork(&self) -> Color {
        self.role(role::NODE_FORK)
    }

    /// A non-winning `EdgeStateLabel::AltOf(_)` row in the detail pane's
    /// Other zone (ADR 0075). Defaults to the `inert` hue.
    pub fn edge_alt_of(&self) -> Color {
        self.role(role::EDGE_ALT_OF)
    }

    /// An `EdgeStateLabel::Conflict` row in the detail pane's Other zone.
    /// Defaults to the `warning` hue.
    pub fn edge_conflict(&self) -> Color {
        self.role(role::EDGE_CONFLICT)
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
