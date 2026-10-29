//! A palette's options pane: named settings that change its results.

use ratatui::text::{Line, Span};

use crate::glyphs::marks;
use crate::selection::{self, Cursor, Ends};
use crate::theme::Theme;

/// The options a palette's pane lists: named settings, each on or off,
/// that change what the results are (what a search looks in, whether
/// hidden things count).
///
/// The pane opens over the results on the palette's
/// [`Options`](crate::keys::PaletteKey::Options) key and takes the keys
/// of [`PALETTE_OPTION_KEYS`](crate::keys::PALETTE_OPTION_KEYS) while it
/// is open: the arrows move, `Space` (and `Enter`) turns the option under
/// the cursor on or off, and `Esc` goes back to the results. Each option
/// is a row through [`selection::row`](crate::selection::row) with its
/// box (`[x]` or `[ ]`). A change resets the results' cursor and, in a
/// palette with a [`Debounce`](super::Debounce), owes the host a search;
/// the host reads the options back with [`is_on`](Self::is_on).
///
/// ```
/// use tuikit::palette::Options;
///
/// let options = Options::new([("Hidden files", false), ("Archives", true)]);
/// assert_eq!(options.len(), 2);
/// assert_eq!(options.label(1), Some("Archives"));
/// assert!(!options.is_on(0) && options.is_on(1));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    options: Vec<(String, bool)>,
    cursor: usize,
    open: bool,
}

impl Options {
    /// A closed pane of `options`, each a label and whether it starts on.
    pub fn new<I, S>(options: I) -> Self
    where
        I: IntoIterator<Item = (S, bool)>,
        S: Into<String>,
    {
        Self {
            options: options
                .into_iter()
                .map(|(label, on)| (label.into(), on))
                .collect(),
            cursor: 0,
            open: false,
        }
    }

    /// How many options there are.
    pub fn len(&self) -> usize {
        self.options.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    /// The label of option `index`, if there is one.
    pub fn label(&self, index: usize) -> Option<&str> {
        self.options.get(index).map(|(label, _)| label.as_str())
    }

    /// Whether option `index` is on; `false` past the last option.
    pub fn is_on(&self, index: usize) -> bool {
        self.options.get(index).is_some_and(|(_, on)| *on)
    }

    /// Turns option `index` on or off, as the host restores a setting.
    pub fn set(&mut self, index: usize, on: bool) {
        if let Some(option) = self.options.get_mut(index) {
            option.1 = on;
        }
    }

    /// The option the pane's cursor is on.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the pane is open over the results.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the pane with its cursor on the first option.
    pub(super) fn open(&mut self) {
        self.open = true;
        self.cursor = 0;
    }

    /// Closes the pane; the results show again.
    pub(super) fn close(&mut self) {
        self.open = false;
    }

    /// Moves the cursor one option by `delta`, wrapping.
    pub(super) fn step(&mut self, delta: i32) {
        self.cursor = Ends::Wrap.step(self.cursor, self.options.len(), delta);
    }

    /// Turns the option under the cursor on or off; `false` when there is
    /// none.
    pub(super) fn toggle(&mut self) -> bool {
        match self.options.get_mut(self.cursor) {
            Some(option) => {
                option.1 = !option.1;
                true
            }
            None => false,
        }
    }

    /// The pane's rows: the cursor gutter, the box, and the label.
    pub(super) fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        self.options
            .iter()
            .enumerate()
            .map(|(index, (label, on))| {
                let mark = if *on {
                    marks::CHECKED
                } else {
                    marks::UNCHECKED
                };
                selection::row(
                    [Span::raw(format!("{mark} {label}"))],
                    Cursor::of(index == self.cursor, true),
                    theme,
                )
            })
            .collect()
    }
}
