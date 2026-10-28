//! Design tokens: the colors and modifiers every control draws with.
//!
//! This is a layer 1 foundation. Controls take a [`Theme`] and style their
//! output from it, so a host changes the look of every control in one
//! place and no control carries a color of its own.
//!
//! A theme holds three kinds of thing:
//!
//! - **Typed tokens** for the roles the framework's own controls draw:
//!   the accent, muted and inert text, the selection pair, the status
//!   colors, links, search matches, chips, the recency scale, surfaces,
//!   and input fields. Each is a public field, so a control reads
//!   `theme.accent` and the compiler checks the name. The token list is
//!   written once (a table inside this module); the fields, their
//!   defaults, the configuration keys, and [`Theme::keys`] all come from
//!   it, so they cannot drift.
//! - **Named styles** the host registers for roles only it knows (a
//!   color per kind of record, per state of an external item). A control
//!   that draws such a role is handed the name and looks it up with
//!   [`Theme::style`] or [`Theme::color`].
//! - **Aliases**: extra configuration keys that also set a token, kept so
//!   an older configuration still loads after tokens are merged or
//!   renamed. See [`Theme::register_alias`] for the precedence rule.
//!
//! [`Theme::apply_overrides`] applies a configuration table the host has
//! read: every key is a token, a named style, or an alias, and every
//! value is written in the style-spec grammar ([`parse_style_spec`] and
//! its siblings). The walk reports problems instead of failing, so a bad
//! entry keeps its default and the rest still apply.
//!
//! ```
//! use ratatui::style::{Color, Modifier, Style};
//! use tuikit::theme::{OverrideValue, StyleSpec, Theme};
//!
//! let mut theme = Theme::default();
//! theme.register("state_open", Color::Green, "an open item's state glyph");
//! theme.register("chip_fill", StyleSpec::on(Color::White, Color::Black), "a filled chip");
//! theme.register_alias("faint", "muted");
//!
//! let problems = theme.apply_overrides([
//!     ("accent", OverrideValue::Text("magenta")),
//!     ("faint", OverrideValue::Text("italic")),
//!     ("state_open", OverrideValue::Text("#22aa66")),
//! ]);
//! assert!(problems.is_empty());
//!
//! // Controls read typed tokens directly and host roles by name.
//! assert_eq!(theme.accent, Color::Magenta);
//! assert_eq!(theme.panel_focus_accent, Color::Magenta, "the focus cue follows");
//! assert_eq!(theme.muted, Modifier::ITALIC);
//! assert_eq!(theme.style("state_open"), Style::default().fg(Color::Rgb(0x22, 0xaa, 0x66)));
//! assert_eq!(theme.style("chip_fill").bg, Some(Color::Black));
//! ```

mod overrides;
mod spec;

use std::collections::BTreeMap;

use ratatui::style::{Color, Modifier, Style};

pub use overrides::{OverrideError, OverrideProblem, OverrideValue};
pub use spec::{StyleSpec, parse_color, parse_modifier, parse_style_spec};

/// The default width of a chip's label, in cells: wide enough for an
/// eight-letter name.
pub const DEFAULT_BADGE_WIDTH: usize = 8;

/// The narrowest chip label a theme accepts: one character and `…`.
pub const MIN_BADGE_WIDTH: usize = 2;

/// What a configuration key accepts, which is also how its value is
/// stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// One color ([`parse_color`]).
    Color,
    /// A modifier list ([`parse_modifier`]).
    Modifier,
    /// A style spec: colors and modifiers together ([`parse_style_spec`]).
    Style,
    /// A count of terminal cells, given as an integer no smaller than
    /// `min`.
    Width {
        /// The smallest count accepted.
        min: usize,
    },
}

/// A token's value. Its variant always matches the token's [`TokenKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenValue {
    /// A color.
    Color(Color),
    /// A modifier set.
    Modifier(Modifier),
    /// A style spec.
    Style(StyleSpec),
    /// A count of terminal cells.
    Width(usize),
}

impl TokenValue {
    /// The kind of key that stores this value. A width carries no minimum
    /// of its own, so its kind accepts any count.
    pub fn kind(self) -> TokenKind {
        match self {
            Self::Color(_) => TokenKind::Color,
            Self::Modifier(_) => TokenKind::Modifier,
            Self::Style(_) => TokenKind::Style,
            Self::Width(_) => TokenKind::Width { min: 0 },
        }
    }

    /// The value as a [`Style`]: a color is a foreground, a modifier set is
    /// added, and a width styles nothing.
    pub fn style(self) -> Style {
        match self {
            Self::Color(color) => Style::default().fg(color),
            Self::Modifier(modifier) => Style::default().add_modifier(modifier),
            Self::Style(spec) => spec.into_style(),
            Self::Width(_) => Style::default(),
        }
    }
}

impl From<Color> for TokenValue {
    fn from(color: Color) -> Self {
        Self::Color(color)
    }
}

impl From<Modifier> for TokenValue {
    fn from(modifier: Modifier) -> Self {
        Self::Modifier(modifier)
    }
}

impl From<StyleSpec> for TokenValue {
    fn from(spec: StyleSpec) -> Self {
        Self::Style(spec)
    }
}

/// Where a configuration key's value is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOrigin {
    /// A typed token, a field of [`Theme`].
    Token,
    /// A named style the host registered.
    Named,
}

/// One configuration key a theme stores a value under, as
/// [`Theme::keys`] lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeKey {
    /// The key, as a configuration file spells it.
    pub name: String,
    /// What the key accepts.
    pub kind: TokenKind,
    /// Whether the key is a typed token or a registered name.
    pub origin: KeyOrigin,
    /// What the key styles, in a few words.
    pub about: String,
}

/// A configuration key that also sets another key's value, as
/// [`Theme::aliases`] lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    /// The extra key.
    pub key: String,
    /// The token or named style it sets.
    pub target: String,
}

/// A registered named style: its value and what it styles.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Named {
    value: TokenValue,
    about: String,
}

/// Conversion between a token field's type and [`TokenValue`].
trait TokenType: Copy {
    fn into_value(self) -> TokenValue;
    fn from_value(value: TokenValue) -> Option<Self>;
}

impl TokenType for Color {
    fn into_value(self) -> TokenValue {
        TokenValue::Color(self)
    }
    fn from_value(value: TokenValue) -> Option<Self> {
        match value {
            TokenValue::Color(color) => Some(color),
            _ => None,
        }
    }
}

impl TokenType for Modifier {
    fn into_value(self) -> TokenValue {
        TokenValue::Modifier(self)
    }
    fn from_value(value: TokenValue) -> Option<Self> {
        match value {
            TokenValue::Modifier(modifier) => Some(modifier),
            _ => None,
        }
    }
}

impl TokenType for StyleSpec {
    fn into_value(self) -> TokenValue {
        TokenValue::Style(self)
    }
    fn from_value(value: TokenValue) -> Option<Self> {
        match value {
            TokenValue::Style(spec) => Some(spec),
            _ => None,
        }
    }
}

impl TokenType for usize {
    fn into_value(self) -> TokenValue {
        TokenValue::Width(self)
    }
    fn from_value(value: TokenValue) -> Option<Self> {
        match value {
            TokenValue::Width(width) => Some(width),
            _ => None,
        }
    }
}

/// One typed token as the table declares it.
struct TokenDef {
    key: &'static str,
    kind: TokenKind,
    about: &'static str,
}

/// Declares the typed tokens once: the [`Theme`] fields with their docs,
/// the default value of each, and the key table every lookup, the
/// configuration walk, and [`Theme::keys`] read.
macro_rules! tokens {
    ($(
        $(#[doc = $doc:literal])*
        $field:ident: $ty:ty = $default:expr, $kind:expr, $about:literal;
    )*) => {
        /// The design tokens a host passes to every control: typed tokens
        /// for the framework's own roles, the named styles the host
        /// registered, and the alias keys an older configuration may
        /// still use. See the [module documentation](self).
        ///
        /// [`Theme::default`] is the framework's palette, chosen to read on
        /// dark and light terminals alike: emphasis comes from reversed
        /// video and the sixteen ANSI colors, so the terminal's own scheme
        /// decides the exact shades.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct Theme {
            $(
                $(#[doc = $doc])*
                pub $field: $ty,
            )*
            named: BTreeMap<String, Named>,
            aliases: Vec<Alias>,
        }

        impl Default for Theme {
            fn default() -> Self {
                Self {
                    $( $field: $default, )*
                    named: BTreeMap::new(),
                    aliases: default_aliases(),
                }
            }
        }

        /// Every typed token, in declaration order.
        const TOKENS: &[TokenDef] = &[
            $( TokenDef { key: stringify!($field), kind: $kind, about: $about }, )*
        ];

        impl Theme {
            /// The value of the typed token `key`.
            fn token_value(&self, key: &str) -> Option<TokenValue> {
                match key {
                    $( stringify!($field) => Some(TokenType::into_value(self.$field)), )*
                    _ => None,
                }
            }

            /// Stores `value` in the typed token `key` when the kinds match.
            fn set_token(&mut self, key: &str, value: TokenValue) -> bool {
                match key {
                    $(
                        stringify!($field) => match TokenType::from_value(value) {
                            Some(value) => {
                                self.$field = value;
                                true
                            }
                            None => false,
                        },
                    )*
                    _ => false,
                }
            }
        }
    };
}

tokens! {
    /// The primary accent: popup borders and titles, the key column of a
    /// help page, a chosen chip, scroll markers, and completion hints.
    accent: Color = Color::Cyan, TokenKind::Color,
        "primary accent: popup borders, help keys, chosen chips, hints";
    /// The focus cue: the marker on the focused pane's title. A table
    /// that sets `accent` but not this key sets this one too.
    panel_focus_accent: Color = Color::Cyan, TokenKind::Color,
        "the focused pane's title marker";
    /// The expand and collapse arrows on rows that have children.
    disclosure: Color = Color::Cyan, TokenKind::Color,
        "expand and collapse arrows";
    /// Text that should recede: hints, placeholders, notes, rules, and
    /// footers. A modifier, `DIM` by default, so it keeps the color of
    /// what it dims.
    muted: Modifier = Modifier::DIM, TokenKind::Modifier,
        "receding text: hints, placeholders, notes, rules, footers";
    /// Text that must recede even where `DIM` draws nothing, and things
    /// that are gone or disabled: secondary columns and inactive rows. A
    /// color, dark gray by default.
    inert: Color = Color::DarkGray, TokenKind::Color,
        "secondary columns, and gone or disabled items";
    /// The selected row of the focused list.
    selection_active: Modifier = Modifier::REVERSED.union(Modifier::BOLD), TokenKind::Modifier,
        "selected row, focused list";
    /// The selected row of an unfocused list.
    selection_inactive: Modifier = Modifier::BOLD, TokenKind::Modifier,
        "selected row, unfocused list";
    /// Success: something exists, worked, or is on.
    success: Color = Color::Green, TokenKind::Color,
        "success, existing, on";
    /// Warning: something needs attention.
    warning: Color = Color::Yellow, TokenKind::Color,
        "warnings and pending changes";
    /// Error: something failed or was refused.
    error: Color = Color::Red, TokenKind::Color,
        "errors and refusals";
    /// Information worth noting that is neither success nor trouble.
    info: Color = Color::Green, TokenKind::Color,
        "informational messages";
    /// Identifiers and links: names that lead to something else.
    link: Color = Color::Blue, TokenKind::Color,
        "identifiers and links";
    /// The text a search matched, drawn bold and underlined in this color
    /// (see [`Theme::match_style`]).
    match_highlight: Color = Color::Yellow, TokenKind::Color,
        "text a search matched (bold, underlined)";
    /// The look of a chip: reversed and bold by default, a filled pill on
    /// any terminal.
    badge: Modifier = Modifier::REVERSED.union(Modifier::BOLD), TokenKind::Modifier,
        "the chip look";
    /// The label width of a chip, in cells. Shorter labels are padded and
    /// longer ones cut with `…`, so chips line up.
    badge_width: usize = DEFAULT_BADGE_WIDTH, TokenKind::Width { min: MIN_BADGE_WIDTH },
        "chip label width, in cells";
    /// The freshest step of the recency scale.
    recency_fresh: StyleSpec = StyleSpec::fg_mod(Color::LightGreen, Modifier::BOLD), TokenKind::Style,
        "recency scale: freshest";
    /// The second step of the recency scale.
    recency_active: StyleSpec = StyleSpec::fg(Color::Green), TokenKind::Style,
        "recency scale: active";
    /// The third step of the recency scale.
    recency_recent: StyleSpec = StyleSpec::fg(Color::Yellow), TokenKind::Style,
        "recency scale: recent";
    /// The oldest step of the recency scale.
    recency_cold: StyleSpec = StyleSpec::modifier(Modifier::DIM), TokenKind::Style,
        "recency scale: cold";
    /// The background a floating surface (a toast) paints. `Reset` keeps
    /// the terminal's own background.
    surface: Color = Color::Reset, TokenKind::Color,
        "background of floating surfaces";
    /// An input field's value when the field is not focused.
    field: StyleSpec = StyleSpec::fg(Color::Gray), TokenKind::Style,
        "input value, unfocused field";
    /// An input field's value when the field is focused: the fill that
    /// shows where typing goes.
    field_active: StyleSpec = StyleSpec::on(Color::White, Color::DarkGray), TokenKind::Style,
        "input value, focused field";
    /// The cell under the text cursor in a focused field.
    field_cursor: StyleSpec = StyleSpec::on(Color::Black, Color::White), TokenKind::Style,
        "text cursor cell";
}

/// Aliases every theme starts with: the focus cue follows the accent
/// unless a table sets it.
fn default_aliases() -> Vec<Alias> {
    vec![Alias {
        key: "accent".to_string(),
        target: "panel_focus_accent".to_string(),
    }]
}

impl Theme {
    /// The style of text a search matched: the same on every surface, a
    /// foreground rather than a background so it reads on a selected
    /// (reversed) row too.
    pub fn match_style(&self) -> Style {
        Style::default()
            .fg(self.match_highlight)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// Registers a named style under `name` with its default `value`, which
    /// also fixes the kind of value a configuration may give it. `about`
    /// says what it styles. Registering a name again replaces it.
    ///
    /// # Panics
    ///
    /// When `name` is a typed token's key: a host cannot shadow a token.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        value: impl Into<TokenValue>,
        about: impl Into<String>,
    ) {
        let name = name.into();
        assert!(
            self.token_value(&name).is_none(),
            "`{name}` is a typed token and cannot be registered as a named style"
        );
        self.named.insert(
            name,
            Named {
                value: value.into(),
                about: about.into(),
            },
        );
    }

    /// Registers `key` as an alias that also sets `target`, a typed token
    /// or a registered name.
    ///
    /// Within one table passed to [`Theme::apply_overrides`], `target`
    /// takes the value of its own key when that key is present and valid;
    /// otherwise the value of the first alias, in registration order, that
    /// is present and valid; otherwise it keeps its value. An alias whose
    /// value loses to another key is reported (see
    /// [`OverrideProblem::Shadowed`]). An alias key may itself be a token's
    /// key: it then sets its own token and, as a fallback, the target.
    ///
    /// # Panics
    ///
    /// When `target` is neither a typed token nor a registered name, or
    /// equals `key`.
    pub fn register_alias(&mut self, key: impl Into<String>, target: impl Into<String>) {
        let key = key.into();
        let target = target.into();
        assert!(
            self.value(&target).is_some(),
            "alias `{key}` targets `{target}`, which is neither a token nor a registered name"
        );
        assert_ne!(key, target, "`{key}` cannot be an alias of itself");
        self.aliases.push(Alias { key, target });
    }

    /// The value stored under `key`, a typed token or a registered name.
    /// An alias stores nothing of its own and returns `None`.
    pub fn value(&self, key: &str) -> Option<TokenValue> {
        self.token_value(key)
            .or_else(|| self.named.get(key).map(|named| named.value))
    }

    /// Stores `value` under `key`, a typed token or a registered name.
    /// Returns `false`, changing nothing, when `key` stores nothing or
    /// stores another kind of value.
    pub fn set(&mut self, key: &str, value: TokenValue) -> bool {
        if self.token_value(key).is_some() {
            return self.set_token(key, value);
        }
        match self.named.get_mut(key) {
            Some(named) if same_kind(named.value, value) => {
                named.value = value;
                true
            }
            _ => false,
        }
    }

    /// The style stored under `name` (see [`TokenValue::style`]), or the
    /// empty style when nothing is stored there.
    pub fn style(&self, name: &str) -> Style {
        self.value(name).map(TokenValue::style).unwrap_or_default()
    }

    /// The color stored under `name`: a color, or a style spec's
    /// foreground. `None` for anything else.
    pub fn color(&self, name: &str) -> Option<Color> {
        match self.value(name)? {
            TokenValue::Color(color) => Some(color),
            TokenValue::Style(spec) => spec.fg,
            TokenValue::Modifier(_) | TokenValue::Width(_) => None,
        }
    }

    /// The modifiers stored under `name`: a modifier set, or a style
    /// spec's modifiers. Empty for anything else.
    pub fn modifier(&self, name: &str) -> Modifier {
        match self.value(name) {
            Some(TokenValue::Modifier(modifier)) => modifier,
            Some(TokenValue::Style(spec)) => spec.modifier,
            _ => Modifier::empty(),
        }
    }

    /// Every key that stores a value: the typed tokens in declaration
    /// order, then the registered names in sorted order. Aliases are
    /// listed by [`Theme::aliases`].
    pub fn keys(&self) -> Vec<ThemeKey> {
        let tokens = TOKENS.iter().map(|token| ThemeKey {
            name: token.key.to_string(),
            kind: token.kind,
            origin: KeyOrigin::Token,
            about: token.about.to_string(),
        });
        let named = self.named.iter().map(|(name, named)| ThemeKey {
            name: name.clone(),
            kind: named.value.kind(),
            origin: KeyOrigin::Named,
            about: named.about.clone(),
        });
        tokens.chain(named).collect()
    }

    /// Every alias, in registration order: the defaults first, then the
    /// host's.
    pub fn aliases(&self) -> &[Alias] {
        &self.aliases
    }

    /// The kind of value `key` accepts: its own for a token or a name,
    /// the first target's for an alias that is only an alias.
    fn kind_of(&self, key: &str) -> Option<TokenKind> {
        if let Some(token) = TOKENS.iter().find(|token| token.key == key) {
            return Some(token.kind);
        }
        if let Some(named) = self.named.get(key) {
            return Some(named.value.kind());
        }
        let alias = self.aliases.iter().find(|alias| alias.key == key)?;
        self.kind_of(&alias.target)
    }
}

/// Whether two values are of the same kind; widths match whatever their
/// minimum.
fn same_kind(a: TokenValue, b: TokenValue) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
