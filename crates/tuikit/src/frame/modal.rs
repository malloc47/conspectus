//! The modal frame, its body, and the lines body.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph, Widget};

use super::size::ModalSize;
use crate::scroll::ScrollWindow;
use crate::theme::Theme;

/// What a modal draws inside its frame.
///
/// The frame sizes a body once per draw: it tells the body the width its
/// policy gives ([`measure`](Self::measure)), places itself for the rows
/// the body reports, and hands the body the area inside the border and
/// padding ([`render`](Self::render)). A body lays itself out in
/// `measure` and keeps what it laid out for `render`, so nothing is laid
/// out twice and the frame is placed from the rows the body draws.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::widgets::{Paragraph, Widget};
/// use tuikit::frame::{ModalBody, ModalFrame, ModalSize};
/// use tuikit::text;
/// use tuikit::theme::Theme;
///
/// /// A note wrapped to the frame's width.
/// struct Note {
///     text: &'static str,
///     rows: Vec<String>,
/// }
///
/// impl ModalBody for Note {
///     type Output = ();
///
///     fn measure(&mut self, width: u16) -> u16 {
///         self.rows = text::wrap(self.text, usize::from(width));
///         u16::try_from(self.rows.len()).unwrap_or(u16::MAX)
///     }
///
///     fn render(self, area: Rect, buf: &mut Buffer) {
///         Paragraph::new(self.rows.join("\n")).render(area, buf);
///     }
/// }
///
/// let theme = Theme::default();
/// let area = Rect::new(0, 0, 40, 12);
/// let mut buf = Buffer::empty(area);
/// let note = Note { text: "a note long enough to wrap onto a second row", rows: Vec::new() };
/// ModalFrame::new(&theme, ModalSize::COMPACT).title("Note").render(area, &mut buf, note);
/// // At least 20 columns on a 40-column screen: 36, less the border.
/// assert_eq!(buf[(2, 4)].symbol(), "┌");
/// assert_eq!(buf[(3, 6)].symbol(), "s", "the second row of the note");
/// ```
pub trait ModalBody {
    /// What drawing the body gives back to its caller: the window a
    /// scrolled body showed, or `()`.
    type Output;

    /// Lays the body out `width` cells wide and returns the rows it
    /// needs. Called once per draw, before the frame places itself. A
    /// frame whose height fills its extent does not use the rows, so a
    /// body drawn only in such a frame may return 0.
    fn measure(&mut self, width: u16) -> u16;

    /// Draws the body in `area`, the frame's inner area: as wide as
    /// `measure` was told, and as tall as the policy allowed, which may
    /// be fewer rows than the body asked for.
    fn render(self, area: Rect, buf: &mut Buffer) -> Self::Output;
}

/// The frame every modal draws in: a cleared rect sized by a
/// [`ModalSize`], an accent border, one title form, and optional
/// padding, around a [`ModalBody`].
///
/// - **Surface.** The rect is cleared and painted with the theme's
///   `surface`, so text drawn under the modal never shows through it.
/// - **Border.** All four sides, plain lines, in the theme's `accent`.
/// - **Title.** Plain text on the top border, in the border's color and
///   regular weight, left-aligned with one space on each side; spaces the
///   caller put at its ends are dropped, so every title has the same
///   form. A title may carry chips (a segmented choice); they keep their
///   own look, and nothing else in a title is styled.
/// - **Padding.** None by default; [`ModalFrame::padding`] adds cells
///   between the border and the body, and the sizing counts them.
#[derive(Clone, Debug)]
pub struct ModalFrame<'a> {
    theme: &'a Theme,
    size: ModalSize,
    title: Line<'a>,
    padding: Padding,
}

impl<'a> ModalFrame<'a> {
    /// A frame sized by `size`, with no title and no padding.
    pub fn new(theme: &'a Theme, size: ModalSize) -> Self {
        Self {
            theme,
            size,
            title: Line::default(),
            padding: Padding::ZERO,
        }
    }

    /// Sets the title. Its spans keep their styles; the frame adds one
    /// space on each side and drops any the caller added.
    #[must_use]
    pub fn title(mut self, title: impl Into<Line<'a>>) -> Self {
        self.title = title.into();
        self
    }

    /// Puts `padding` between the border and the body.
    #[must_use]
    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    /// The sizing policy.
    pub fn size(&self) -> ModalSize {
        self.size
    }

    /// The columns a body gets on `area`: the policy's width less the
    /// border and the padding. A body laid out ahead of the draw (see
    /// [`Lines`]) is laid out for this width.
    pub fn inner_width(&self, area: Rect) -> u16 {
        self.size
            .width(area.width)
            .saturating_sub(2 + self.padding.left + self.padding.right)
    }

    /// The frame's rect on `area` for a body of `rows` rows: the policy
    /// applied to the rows, the border, and the padding, centered.
    pub fn rect(&self, area: Rect, rows: u16) -> Rect {
        let chrome = 2 + self.padding.top + self.padding.bottom;
        self.size.rect(area, rows.saturating_add(chrome))
    }

    /// Measures `body` once at [`inner_width`](Self::inner_width), places
    /// the frame for its rows, draws the frame, and draws the body inside
    /// it. Returns what the body's draw returns.
    pub fn render<B: ModalBody>(self, area: Rect, buf: &mut Buffer, mut body: B) -> B::Output {
        let area = area.intersection(buf.area);
        let rows = body.measure(self.inner_width(area));
        let rect = self.rect(area, rows);
        let block = bordered(
            self.theme,
            Style::default().fg(self.theme.accent),
            self.title,
        )
        .padding(self.padding);
        let inner = block.inner(rect);
        Clear.render(rect, buf);
        block.render(rect, buf);
        body.render(inner, buf)
    }
}

/// The border and title every frame draws: all four sides in
/// `border`, the surface behind, and the title in its one form.
fn bordered<'a>(theme: &Theme, border: Style, title: Line<'a>) -> Block<'a> {
    let block = Block::bordered()
        .border_style(border)
        .style(Style::default().bg(theme.surface));
    match framed_title(title) {
        Some(title) => block.title(title),
        None => block,
    }
}

/// `title` with one space on each side and the spaces it had at its ends
/// dropped; `None` when it has no text.
fn framed_title(title: Line<'_>) -> Option<Line<'_>> {
    let Line {
        style,
        alignment,
        spans,
    } = title;
    let mut spans: Vec<Span<'_>> = spans
        .into_iter()
        .filter(|span| !span.content.is_empty())
        .collect();
    trim_ends(&mut spans);
    if spans.is_empty() {
        return None;
    }
    spans.insert(0, Span::raw(" "));
    spans.push(Span::raw(" "));
    Some(Line {
        style,
        alignment,
        spans,
    })
}

/// Drops the spaces at the start of the first spans and at the end of the
/// last ones, removing spans that become empty.
fn trim_ends(spans: &mut Vec<Span<'_>>) {
    while let Some(first) = spans.first_mut() {
        let trimmed = first.content.trim_start();
        if trimmed.is_empty() {
            spans.remove(0);
            continue;
        }
        if trimmed.len() != first.content.len() {
            first.content = trimmed.to_string().into();
        }
        break;
    }
    while let Some(last) = spans.last_mut() {
        let trimmed = last.content.trim_end();
        if trimmed.is_empty() {
            spans.pop();
            continue;
        }
        if trimmed.len() != last.content.len() {
            last.content = trimmed.to_string().into();
        }
        break;
    }
}

/// A body of lines laid out ahead of the draw, drawn top to bottom and
/// scrolled so a span of them stays in view.
///
/// The lines are drawn as given: a host lays them out for the frame's
/// [`inner_width`](ModalFrame::inner_width) (cut, padded, or wrapped
/// through the text module) before it builds the body, and a line wider
/// than the area is clipped. The body asks for one row per line. When the
/// frame gives it fewer rows, it scrolls by the follow rule from the top
/// so the span set by [`following`](Self::following) stays in view, and
/// its draw returns the window it showed.
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::text::Line;
/// use tuikit::frame::{Lines, ModalFrame, ModalSize};
/// use tuikit::theme::Theme;
///
/// let theme = Theme::default();
/// let area = Rect::new(0, 0, 60, 8);
/// let lines: Vec<Line> = (0..20).map(|n| Line::from(format!("row {n}"))).collect();
/// let mut buf = Buffer::empty(area);
/// let window = ModalFrame::new(&theme, ModalSize::MENU)
///     .render(area, &mut buf, Lines::new(lines).following(9..10));
/// // Eight rows less a margin row above and below and the border: 4 shown.
/// assert_eq!(window.visible(), 6..10);
/// ```
#[derive(Clone, Debug, Default)]
pub struct Lines<'a> {
    lines: Vec<Line<'a>>,
    follow: Range<usize>,
}

impl<'a> Lines<'a> {
    /// A body showing `lines` from the top.
    pub fn new(lines: Vec<Line<'a>>) -> Self {
        Self {
            lines,
            follow: 0..0,
        }
    }

    /// Keeps the lines in `rows` in view (the cursor's line, or every row
    /// of a cursor that takes several).
    #[must_use]
    pub fn following(mut self, rows: Range<usize>) -> Self {
        self.follow = rows;
        self
    }
}

impl ModalBody for Lines<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, _width: u16) -> u16 {
        u16::try_from(self.lines.len()).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let window =
            ScrollWindow::new(usize::from(area.height), self.lines.len()).following(self.follow);
        Paragraph::new(self.lines)
            .scroll(window.paragraph_scroll())
            .render(area, buf);
        window
    }
}
