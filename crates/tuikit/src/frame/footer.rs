//! The footer every modal draws: its keys in one look, under a blank
//! row, fitted to its frame by one rule.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};

use crate::keys::Footer;
use crate::theme::Theme;

/// Frames this narrow, or narrower, keep a footer on one row, cut at the
/// edge: wrapped, it would take more rows than such a frame has.
const ONE_ROW_WIDTH: usize = 8;

/// A modal's footer: the keys its table generates, the line a modal keeps
/// above them (an error line, a find prompt), and the blank row that
/// separates them from the body.
///
/// Every framed control draws its footer through this type, so footers
/// share one look and one fit:
///
/// - **One look.** The keys are drawn in [`ModalFooter::style`], the
///   theme's `muted` over the color of where they sit (the body's
///   foreground, or the border's accent on a bottom border), as the
///   status bar draws its hints. A segment that cannot act now keeps its
///   own look (`inert` and `muted`). The status line keeps its own style.
/// - **Under a blank row.** The footer's first row is blank, then the
///   status line, then the keys, broken only between segments, on the
///   frame's last rows.
/// - **One fit.** A frame too short for all of it gives up the blank row
///   first. Then the status line stays, the keys keep their last rows
///   (a footer lists the keys that leave last), and their first rows go,
///   behind the blank row, after the body's last row, where they scroll
///   with it: see [`fit`](Self::fit).
///
/// ```
/// use ratatui::style::Modifier;
/// use tuikit::frame::ModalFooter;
/// use tuikit::keys::{Footer, MENU_KEYS};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let footer = ModalFooter::new(Footer::new(MENU_KEYS), &theme);
/// // At 40 cells the keys take one row, under the blank row.
/// assert_eq!(footer.height(40), 2);
/// let fit = footer.fit(40, 5);
/// assert_eq!(fit.kept.len(), 2);
/// assert_eq!(fit.kept[1].to_string(), "↑/↓ move · Enter pick · Esc close");
/// assert!(fit.kept[1].spans.iter().all(|span| span.style.add_modifier.contains(Modifier::DIM)));
///
/// // At 12 cells the keys take three rows; in two rows the last two stay
/// // and the first goes after the body, behind the blank row.
/// let fit = footer.fit(12, 2);
/// assert_eq!(fit.kept.iter().map(ToString::to_string).collect::<Vec<_>>(), ["Enter pick", "Esc close"]);
/// assert_eq!(fit.spill.iter().map(ToString::to_string).collect::<Vec<_>>(), ["", "↑/↓ move"]);
/// ```
#[derive(Clone, Debug)]
pub struct ModalFooter<'a> {
    footer: Footer,
    status: Option<Line<'static>>,
    theme: &'a Theme,
}

impl<'a> ModalFooter<'a> {
    /// The footer of `footer`'s segments, in `theme`'s look, with no
    /// status line.
    pub fn new(footer: Footer, theme: &'a Theme) -> Self {
        Self {
            footer,
            status: None,
            theme,
        }
    }

    /// Keeps `line` above the keys: a form's error line, a reader's find
    /// prompt or find chip. It keeps its own style, and a short frame
    /// keeps it ahead of every key row. `None` shows none.
    #[must_use]
    pub fn status(mut self, line: Option<Line<'static>>) -> Self {
        self.status = line;
        self
    }

    /// The look of every modal footer: the theme's `muted`, receding,
    /// with no color of its own, so it takes the color of where it is
    /// drawn.
    pub fn style(theme: &Theme) -> Style {
        Style::default().add_modifier(theme.muted)
    }

    /// The key rows `width` cells wide in the footer's look, broken only
    /// between segments ([`Footer::lines`]); on one row, cut at the edge,
    /// when `width` is 8 cells or fewer. None for a footer with no
    /// segments.
    pub fn keys(&self, width: usize) -> Vec<Line<'static>> {
        if self.footer.is_empty() {
            return Vec::new();
        }
        let width = if width <= ONE_ROW_WIDTH {
            usize::MAX
        } else {
            width
        };
        let look = Self::style(self.theme);
        self.footer
            .lines(width, self.theme)
            .into_iter()
            .map(|mut line| {
                for span in &mut line.spans {
                    span.style = look.patch(span.style);
                }
                line
            })
            .collect()
    }

    /// The rows the footer asks for at `width`: the blank row, the status
    /// line, and the key rows. None for a footer with no segments and no
    /// status line.
    pub fn height(&self, width: usize) -> usize {
        let rows = usize::from(self.status.is_some()) + self.keys(width).len();
        if rows == 0 { 0 } else { rows + 1 }
    }

    /// The footer `width` cells wide fitted to `room` rows, the rows a
    /// frame leaves it under the body.
    ///
    /// While everything fits, the footer keeps all of it: the blank row,
    /// the status line, and every key row. Otherwise the blank row goes
    /// first; then the status line takes a row (while there is one), the
    /// keys keep their last rows in the rest, and their first rows spill:
    /// they go after the body's last row, behind a blank row, and scroll
    /// with it, so no key is lost and the keys that leave, which a footer
    /// lists last, are the last to go out of view.
    pub fn fit(&self, width: usize, room: usize) -> FooterFit {
        let mut keys = self.keys(width);
        let status = usize::from(self.status.is_some());
        let width = u16::try_from(width).unwrap_or(u16::MAX);
        let wanted = status + keys.len();
        if wanted == 0 {
            return FooterFit {
                width,
                ..FooterFit::default()
            };
        }
        let mut kept = Vec::with_capacity(wanted + 1);
        if wanted < room {
            kept.push(Line::default());
        }
        let status_rows = status.min(room);
        if status_rows > 0
            && let Some(line) = &self.status
        {
            kept.push(line.clone());
        }
        let stay = keys.len().min(room - status_rows);
        let first: Vec<Line<'static>> = keys.drain(..keys.len() - stay).collect();
        kept.extend(keys);
        let spill = if first.is_empty() {
            Vec::new()
        } else {
            std::iter::once(Line::default()).chain(first).collect()
        };
        FooterFit { kept, spill, width }
    }

    /// The footer fitted under a body in `area`, a frame's inside, which
    /// keeps at least `keep` rows for the body.
    ///
    /// The footer is [fitted](Self::fit) to the rows the body leaves, at
    /// the area's width. When the body's rows at that width
    /// (`body_rows(width)`) and the spilled rows do not fit the rows left
    /// above the footer, a scrollbar takes the body's last column, so the
    /// footer is fitted again a cell narrower and the body is laid out at
    /// [`FooterFit::width`] beside it: the spilled rows scroll beside the
    /// bar, and the kept rows line up with them.
    pub fn fit_beside(
        &self,
        area: Rect,
        keep: usize,
        body_rows: impl Fn(u16) -> usize,
    ) -> FooterFit {
        let height = usize::from(area.height);
        let room = height.saturating_sub(keep);
        let fit = self.fit(usize::from(area.width), room);
        let left = height - fit.kept.len();
        if area.width < 2 || body_rows(area.width) + fit.spill.len() <= left {
            return fit;
        }
        self.fit(usize::from(area.width - 1), room)
    }

    /// The footer as a frame's bottom border shows it
    /// ([`ModalFrame::footer`](super::ModalFrame::footer)), for a modal
    /// whose body has no row to spare: its last key row at `width`, the
    /// cells between the border's corners less the space on each side.
    /// With no row under the body, the first rows cannot scroll, so the
    /// keys that leave, which a footer lists last, are the ones shown.
    pub fn border(&self, width: usize) -> Line<'static> {
        self.keys(width).pop().unwrap_or_default()
    }
}

/// A footer fitted to its frame: the rows it keeps on the frame's last
/// rows, and the rows that scroll after the body's last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FooterFit {
    /// The rows on the frame's last rows, top to bottom: the blank row
    /// while everything fits, the status line, the key rows that stay.
    pub kept: Vec<Line<'static>>,
    /// The rows that do not fit: a blank row and the first key rows. A
    /// body draws them after its last row, where they scroll with it.
    /// Empty when every key row stays.
    pub spill: Vec<Line<'static>>,
    /// The cells the rows were laid out in.
    pub width: u16,
}

impl FooterFit {
    /// The rows the footer keeps on the frame's last rows.
    pub fn height(&self) -> u16 {
        u16::try_from(self.kept.len()).unwrap_or(u16::MAX)
    }

    /// `area` split into the body's rows, above, and the footer's, the
    /// area's last [`height`](Self::height) rows.
    pub fn split(&self, area: Rect) -> [Rect; 2] {
        let footer = self.height().min(area.height);
        let body = Rect {
            height: area.height - footer,
            ..area
        };
        let rows = Rect {
            y: area.y + body.height,
            height: footer,
            ..area
        };
        [body, rows]
    }

    /// Draws the kept rows in `area`, the footer's part of a
    /// [`split`](Self::split), [`width`](Self::width) cells wide from its
    /// left edge.
    pub fn render(self, area: Rect, buf: &mut Buffer) {
        let area = Rect {
            width: self.width.min(area.width),
            ..area
        };
        Paragraph::new(self.kept).render(area, buf);
    }
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
