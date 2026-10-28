//! A switch and its state, with the key that changes it.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::Tone;
use crate::glyphs::marks;
use crate::text;
use crate::theme::Theme;

/// A switch's chip: `name·state (key)`, such as `wrap·on  (w)`.
///
/// The state is padded to the widest of the words it can show
/// ([`ToggleChip::states`]), so the chip keeps its width as the state
/// changes and the chips after it hold still. The key, when given,
/// travels with the label, so the reader learns the switch and how to
/// flip it in one place. An on chip is drawn in the theme's `success`
/// color and bold; an off chip recedes ([`Tone::Inert`]).
///
/// ```
/// use tuikit::chip::ToggleChip;
/// use tuikit::theme::Theme;
///
/// let detail = ToggleChip::new("detail", "full", true)
///     .states(&["off", "brief", "full"])
///     .key("d");
/// assert_eq!(detail.text(), "detail·full  (d)");
/// let span = ToggleChip::new("wrap", "off", false).span(&Theme::default());
/// assert_eq!(span.content, "wrap·off");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleChip<'a> {
    name: &'a str,
    state: &'a str,
    on: bool,
    states: &'a [&'a str],
    key: Option<&'a str>,
}

impl<'a> ToggleChip<'a> {
    /// A chip for the switch `name` showing `state`; `on` picks the look.
    pub fn new(name: &'a str, state: &'a str, on: bool) -> Self {
        Self {
            name,
            state,
            on,
            states: &[],
            key: None,
        }
    }

    /// Every word the state can show, so the chip is as wide as the
    /// widest of them whichever is shown.
    #[must_use]
    pub fn states(mut self, states: &'a [&'a str]) -> Self {
        self.states = states;
        self
    }

    /// The key that flips the switch, shown after the state in
    /// parentheses.
    #[must_use]
    pub fn key(mut self, key: &'a str) -> Self {
        self.key = Some(key);
        self
    }

    /// Whether the switch is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The chip's text: the name, the separator mark, the padded state,
    /// and the key in parentheses.
    pub fn text(&self) -> String {
        let cells = self
            .states
            .iter()
            .map(|state| text::width(state))
            .fold(text::width(self.state), usize::max);
        let mut out = format!(
            "{}{}{}",
            self.name,
            marks::SEPARATOR,
            text::pad(self.state, cells)
        );
        if let Some(key) = self.key {
            out.push_str(&format!(" ({key})"));
        }
        out
    }

    /// The style the chip draws in: bold `success` when on, receding
    /// when off.
    pub fn look(&self, theme: &Theme) -> Style {
        if self.on {
            Tone::Success.style(theme).add_modifier(Modifier::BOLD)
        } else {
            Tone::Inert.style(theme)
        }
    }

    /// The chip as one styled span.
    pub fn span(&self, theme: &Theme) -> Span<'static> {
        Span::styled(self.text(), self.look(theme))
    }
}
