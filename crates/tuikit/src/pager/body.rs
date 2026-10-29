//! How a pager draws in each mode: the body every mode shares, the
//! full-screen chrome and its fitted footer, the modal frame, and the
//! find prompt.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use super::{CONTEXT_ROWS, Pager, PagerContent};
use crate::chip::{PositionChip, StatusChip, Tone};
use crate::frame::{Fit, ModalBody, ModalFrame, ModalSize};
use crate::glyphs::marks;
use crate::keys::{FIND_KEYS, Footer, HINT_SEPARATOR};
use crate::scroll::{ScrollGutter, ScrollWindow};
use crate::text;
use crate::text_edit::TextField;
use crate::theme::Theme;

/// Cells the find prompt's field keeps even when its hint leaves none.
const MIN_FIELD: usize = 4;
/// Pages this narrow, or narrower, leave a framed footer on one row.
const MIN_FOOTER: usize = 8;

/// Draws a full-screen pager: header, rule, body, rule, footer.
pub(super) fn screen<C: PagerContent>(
    pager: &mut Pager<C>,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let [header, top_rule, body, bottom_rule, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let width = usize::from(area.width);
    let title = pager.content.header(width, theme);
    let title = Line {
        spans: text::fit_spans(title.spans, width),
        ..title
    };
    Paragraph::new(title).render(header, buf);
    rule(top_rule, buf, theme);
    draw_body(pager, body, buf, theme, false, &[]);
    rule(bottom_rule, buf, theme);
    let line = if pager.find.prompt().is_some() {
        prompt_line(pager, width, theme)
    } else {
        footer_line(pager, width, theme)
    };
    Paragraph::new(line).render(footer, buf);
}

/// Draws a framed pager: the header as the frame's title, the body with
/// its scrollbar, and under it the find chip or prompt and the footer,
/// which keep the frame's last rows (see [`Below::fit`]).
pub(super) fn modal<C: PagerContent>(
    pager: &mut Pager<C>,
    size: ModalSize,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    let frame = ModalFrame::new(theme, size);
    let width = usize::from(frame.inner_width(area));
    let title = pager.content.header(width, theme);
    let fills = size.fit == Fit::Fill;
    frame.title(title).render(
        area,
        buf,
        Framed {
            pager,
            theme,
            fills,
        },
    );
}

/// Draws an embedded pager: the body and its scrollbar, with `tail`
/// scrolling after the content's last row (the rows of a host's footer
/// that do not fit under the pager; none for most).
pub(super) fn embedded<C: PagerContent>(
    pager: &mut Pager<C>,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    tail: &[Line<'static>],
) {
    draw_body(pager, area, buf, theme, true, tail);
}

/// A framed pager inside its frame.
struct Framed<'p, 't, C> {
    pager: &'p mut Pager<C>,
    theme: &'t Theme,
    /// Whether the frame fills its extent, so its height does not follow
    /// the rows.
    fills: bool,
}

impl<C: PagerContent> ModalBody for Framed<'_, '_, C> {
    type Output = ();

    /// The rows the content lays out at `width`, and the status and
    /// footer rows under them, for a frame that hugs its body; nothing
    /// for one that fills its extent, which lays the rows out in
    /// `render` only.
    fn measure(&mut self, width: u16) -> u16 {
        if self.fills {
            return 0;
        }
        let rows = self.pager.content.layout(width, self.theme);
        let (status, footer) = below(self.pager, usize::from(width), self.theme);
        let below = usize::from(status.is_some()) + footer.len();
        u16::try_from(rows + below).unwrap_or(u16::MAX)
    }

    fn render(self, area: Rect, buf: &mut Buffer) {
        let below = Below::fit(self.pager, area, self.theme);
        let kept = u16::try_from(below.kept.len()).unwrap_or(area.height);
        let body = Rect {
            height: area.height - kept,
            ..area
        };
        draw_body(self.pager, body, buf, self.theme, true, &below.spill);
        let rows = Rect {
            y: area.y + body.height,
            width: below.width,
            height: kept,
            ..area
        };
        Paragraph::new(below.kept).render(rows, buf);
    }
}

/// What a framed pager draws under its body, fitted to its frame.
struct Below {
    /// The rows kept under the body: the find prompt, or the find chip
    /// and the note, then the footer's last rows.
    kept: Vec<Line<'static>>,
    /// The footer's first rows, which do not fit under the body: they
    /// scroll with it, after its last row.
    spill: Vec<Line<'static>>,
    /// The cells the rows are laid out in: the frame's inner width, one
    /// fewer when the body overflows and the scrollbar takes a column.
    width: u16,
}

impl Below {
    /// Fits the rows under the body to `area`, the frame's inside.
    ///
    /// The find prompt (or the find chip and the note) and the footer
    /// keep the frame's last rows and leave the body at least one. A
    /// footer taller than the room left keeps its last rows, the keys
    /// that leave, and its first rows, the keys that scroll, go after the
    /// content's last row and scroll with it, so no key is lost. When the
    /// body overflows, everything is laid out a cell narrower, beside the
    /// scrollbar's column.
    fn fit<C: PagerContent>(pager: &mut Pager<C>, area: Rect, theme: &Theme) -> Self {
        let room = usize::from(area.height).saturating_sub(1);
        let split = |pager: &Pager<C>, width: u16| {
            let (status, mut footer) = below(pager, usize::from(width), theme);
            let mut kept: Vec<Line<'static>> = status.into_iter().take(room).collect();
            let stay = footer.len().min(room - kept.len());
            let spill: Vec<Line<'static>> = footer.drain(..footer.len() - stay).collect();
            kept.extend(footer);
            Self { kept, spill, width }
        };
        let below = split(pager, area.width);
        let rows = pager.content.layout(area.width, theme) + below.spill.len();
        if rows <= usize::from(area.height) - below.kept.len() || area.width < 2 {
            return below;
        }
        split(pager, area.width - 1)
    }
}

/// What a framed pager draws under its body at `width`: the find prompt,
/// or the find chip and the note, or neither; and the footer, receding and
/// broken only between its segments.
fn below<C: PagerContent>(
    pager: &Pager<C>,
    width: usize,
    theme: &Theme,
) -> (Option<Line<'static>>, Vec<Line<'static>>) {
    let status = if pager.find.prompt().is_some() {
        Some(prompt_line(pager, width, theme))
    } else {
        let status = status_spans(pager, theme);
        (!status.is_empty()).then(|| Line::from(text::fit_spans(status, width)))
    };
    let footer = pager.footer();
    if footer.is_empty() {
        return (status, Vec::new());
    }
    let width = if width <= MIN_FOOTER {
        usize::MAX
    } else {
        width
    };
    let muted = Style::default().add_modifier(theme.muted);
    let lines = footer.lines(width, theme).into_iter().map(|mut line| {
        for span in &mut line.spans {
            span.style = muted.patch(span.style);
        }
        line
    });
    (status, lines.collect())
}

/// Lays the content out for `area`, settles the window (a find's landing,
/// the end it keeps to, the clamp), draws the rows it shows with their
/// matches picked out, and keeps the window. `tail` is rows that scroll
/// after the content's last (a framed footer's that do not fit under the
/// body). With `gutter`, rows that overflow give their last column to the
/// scrollbar and are laid out one cell narrower.
fn draw_body<C: PagerContent>(
    pager: &mut Pager<C>,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    gutter: bool,
    tail: &[Line<'static>],
) {
    let mut width = area.width;
    let mut rows = pager.content.layout(width, theme);
    let bar = gutter.then(|| ScrollGutter::layout(area, rows + tail.len()));
    if let Some(bar) = bar.filter(|bar| bar.bar().is_some()) {
        width = bar.content().width;
        rows = pager.content.layout(width, theme);
    }
    pager.laid_out = Some(width);
    let total = rows + tail.len();
    let viewport = usize::from(area.height);
    // A find's landing puts the match one row of context below the top;
    // the window below brings that inside the content.
    if let Some(unit) = pager.find.take_landing() {
        let row = pager.content.match_rows(unit).start;
        pager.scroll.offset = row.saturating_sub(CONTEXT_ROWS);
        pager.scroll.stick = false;
    }
    let offset = if pager.scroll.stick {
        usize::MAX
    } else {
        pager.scroll.offset
    };
    let window = ScrollWindow::new(viewport, total).at(offset);
    pager.scroll.keep(window);
    let target = bar.map_or(area, |bar| bar.content());
    if rows == 0 {
        let empty = Line::from(Span::styled(
            pager.content.empty().into_owned(),
            Style::default().add_modifier(theme.muted),
        ));
        let mut shown = text::wrap_line(&empty, usize::from(target.width), 0);
        shown.extend(tail.iter().cloned());
        Paragraph::new(shown).render(target, buf);
        return;
    }
    let visible = window.visible();
    let first = visible.start;
    let mut shown = pager
        .content
        .rows(first.min(rows)..visible.end.min(rows), theme);
    if pager.find.is_active() && !pager.find.matches().is_empty() {
        pager.content.paint(&mut shown, first, &pager.find, theme);
    }
    let after = first.saturating_sub(rows)..visible.end.saturating_sub(rows);
    shown.extend(tail[after].iter().cloned());
    Paragraph::new(shown).render(target, buf);
    if let Some(bar) = bar {
        bar.render(&window, buf);
    }
}

/// A rule across `area`, receding.
fn rule(area: Rect, buf: &mut Buffer, theme: &Theme) {
    Paragraph::new(Line::from(Span::styled(
        marks::RULE.repeat(usize::from(area.width)),
        Tone::Inert.style(theme),
    )))
    .render(area, buf);
}

/// The find chip and the note, joined, or nothing.
fn status_spans<C: PagerContent>(pager: &Pager<C>, theme: &Theme) -> Vec<Span<'static>> {
    let separator = Span::styled(HINT_SEPARATOR, Tone::Inert.style(theme));
    let detail = || {
        (pager.find.is_active() && pager.find.matches().is_empty())
            .then(|| pager.content.find_detail(pager.find.query()))
            .flatten()
    };
    let mut spans = Vec::new();
    if let Some(chip) = pager.find.chip(detail()) {
        spans.push(chip.span(theme));
    }
    if let Some(note) = &pager.note {
        if !spans.is_empty() {
            spans.push(separator);
        }
        spans.push(
            StatusChip::new(note.text.clone(), note.severity.tone())
                .bold()
                .span(theme),
        );
    }
    spans
}

/// A full-screen pager's footer: the position chip, the find chip and the
/// note, the content's chips, and the keys, all fitted to `width`.
///
/// When everything does not fit, the footer drops, in turn, the keys that
/// scroll, the pager's other keys, the content's chips from the last, and
/// the content's keys; the keys that leave stay, and what is still too
/// wide is cut with the ellipsis mark.
fn footer_line<C: PagerContent>(pager: &Pager<C>, width: usize, theme: &Theme) -> Line<'static> {
    let inert = Tone::Inert.style(theme);
    let window = pager.scroll.window;
    let mut lead = vec![PositionChip::new(window.offset(), window.content()).span()];
    let status = status_spans(pager, theme);
    if !status.is_empty() {
        lead.push(Span::styled(HINT_SEPARATOR, inert));
        lead.extend(status);
    }
    let chips = pager.content.chips(theme);
    let [scroll, other, content, leave] = pager.hint_groups();
    let compose = |chips: &[Span<'static>], groups: &[&Footer]| -> Vec<Span<'static>> {
        let mut spans = lead.clone();
        for chip in chips {
            spans.push(Span::styled(HINT_SEPARATOR, inert));
            spans.push(chip.clone());
        }
        let hints: Footer = groups
            .iter()
            .flat_map(|group| group.segments().iter().cloned())
            .collect();
        if !hints.is_empty() {
            spans.push(Span::styled(HINT_SEPARATOR, inert));
            spans.push(Span::styled(hints.text(), inert));
        }
        spans
    };
    let mut tries = vec![
        compose(&chips, &[&scroll, &other, &content, &leave]),
        compose(&chips, &[&other, &content, &leave]),
        compose(&chips, &[&content, &leave]),
    ];
    for kept in (0..chips.len()).rev() {
        tries.push(compose(&chips[..kept], &[&content, &leave]));
    }
    tries.push(compose(&[], &[&leave]));
    let fitting = tries
        .iter()
        .position(|spans| text::spans_width(spans) <= width)
        .unwrap_or(tries.len() - 1);
    Line::from(text::fit_spans(tries.swap_remove(fitting), width))
}

/// The find prompt in a footer row: `/` and the query as a text field
/// with its caret, then the prompt's keys.
fn prompt_line<C: PagerContent>(pager: &Pager<C>, width: usize, theme: &Theme) -> Line<'static> {
    let Some(prompt) = pager.find.prompt() else {
        return Line::default();
    };
    let hint = format!("   {}", Footer::new(FIND_KEYS).text());
    let hint_cells = text::width(&hint);
    let room = width.saturating_sub(1 + hint_cells).max(MIN_FIELD);
    let mut spans = TextField::new(prompt)
        .prefix(Span::styled(
            "/",
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .spans(1 + room, theme);
    spans.push(Span::styled(hint, Tone::Inert.style(theme)));
    Line::from(text::fit_spans(spans, width))
}
