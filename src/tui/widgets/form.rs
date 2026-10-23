//! Row rendering shared by the multi-field forms: the mux launch form,
//! the new mux form, and the pin edit form.
//!
//! Every form draws the same kinds of rows: a text value with a cursor
//! and scroll indicators, a path field with a validity mark and a Tab
//! completion hint, the harness choices as a row of chips, a row of
//! launch option checkboxes, and one-of-N choice rows. Sharing them keeps
//! the forms looking and behaving alike.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::discovery::harness::{HarnessLaunchOption, argv_contains_fragment};
use crate::tui::widgets::input::TextInputState;
use crate::tui::widgets::path_omnibox::{PathOmniboxState, PathValidation};

pub const LABEL_WIDTH: usize = 11;

/// Cells the harness value gets before the chips.
const VALUE_SLOT_WIDTH: usize = 18;

/// The widths a row is drawn into: the popup's inner width and the
/// label column.
#[derive(Debug, Clone, Copy)]
pub struct Cols {
    pub inner: usize,
    pub label: usize,
}

impl Cols {
    pub fn new(inner: usize) -> Self {
        Self {
            inner,
            label: LABEL_WIDTH,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: usize) -> Self {
        self.label = label;
        self
    }
}
const VALUE_GAP: &str = "  ";
const TEXT_GUTTER: &str = "  ";
const PATH_SUFFIX_WIDTH: usize = 32;

/// Spans drawn after a row's value, and the width they take.
pub struct Suffix {
    pub width: usize,
    pub spans: Vec<Span<'static>>,
}

/// The marker and label that start a row, bold when the cursor is on it.
pub fn prefix(label: &str, active: bool, label_width: usize) -> (String, Style) {
    let marker = if active { "> " } else { "  " };
    let style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    (format!("{marker}{label:label_width$}{VALUE_GAP}"), style)
}

pub fn entry_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn edge_style(indicator: &str, edge: Style) -> Style {
    if indicator.trim().is_empty() {
        Style::default()
    } else {
        edge
    }
}

struct Window {
    left: &'static str,
    before: String,
    cursor_text: String,
    after: String,
    right: &'static str,
    edge: Style,
    cursor_style: Style,
}

/// The part of `value` that fits in `width` cells around the cursor,
/// with `<` and `>` marking what scrolled out of view.
///
/// The result is always exactly `width` cells wide, focused or not: the
/// cell the cursor sits on exists either way, so moving the focus never
/// shifts what follows the value.
fn window(value: &str, cursor: usize, width: usize, active: bool) -> Window {
    let width = width.max(1);
    let edge = if active {
        Style::default()
            .fg(Color::Cyan)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };
    let cursor_style = if active {
        Style::default().fg(Color::Black).bg(Color::White)
    } else {
        Style::default()
    };
    let value = if value.trim().is_empty() { "-" } else { value };
    // The characters, plus the blank cell a cursor at the end sits on.
    let mut cells: Vec<char> = value.chars().collect();
    let total = cells.len();
    cells.push(' ');
    let cursor = cursor.min(total);

    if width <= 4 {
        let ch = cells[cursor];
        return Window {
            left: "",
            before: String::new(),
            cursor_text: ch.to_string(),
            after: " ".repeat(width.saturating_sub(1)),
            right: "",
            edge,
            cursor_style,
        };
    }

    let content_width = width - 4;
    let start = if cells.len() <= content_width || cursor < content_width {
        0
    } else {
        (cursor + 1 - content_width).min(cells.len() - content_width)
    };
    let end = (start + content_width).min(cells.len());
    let before: String = cells[start..cursor].iter().collect();
    let cursor_text = cells[cursor].to_string();
    let mut after: String = cells[cursor + 1..end].iter().collect();
    let used = before.chars().count() + 1 + after.chars().count();
    after.push_str(&" ".repeat(content_width.saturating_sub(used)));
    Window {
        left: if start > 0 { "< " } else { "  " },
        before,
        cursor_text,
        after,
        right: if end < total { " >" } else { "  " },
        edge,
        cursor_style,
    }
}

/// `value` windowed around `cursor` into exactly `width` cells.
pub fn value_cells(value: &str, cursor: usize, width: usize, active: bool) -> Vec<Span<'static>> {
    window_spans(&window(value, cursor, width, active), entry_style(active))
}

fn window_spans(w: &Window, value_style: Style) -> Vec<Span<'static>> {
    vec![
        Span::styled(w.left.to_string(), edge_style(w.left, w.edge)),
        Span::styled(w.before.clone(), value_style),
        Span::styled(w.cursor_text.clone(), w.cursor_style),
        Span::styled(w.after.clone(), value_style),
        Span::styled(w.right.to_string(), edge_style(w.right, w.edge)),
    ]
}

/// A text value row, with an optional suffix after the value.
pub fn value_row(
    label: &str,
    value: &str,
    value_cursor: usize,
    active: bool,
    cols: Cols,
    suffix: Option<Suffix>,
) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let suffix_width = suffix.as_ref().map_or(0, |s| s.width);
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(suffix_width)
        .max(1);
    let w = window(value, value_cursor, value_width, active);
    let mut spans = vec![Span::styled(prefix, label_style)];
    spans.extend(window_spans(&w, entry_style(active)));
    if let Some(suffix) = suffix {
        spans.extend(suffix.spans);
    }
    Line::from(spans)
}

pub fn input_row(label: &str, input: &TextInputState, active: bool, cols: Cols) -> Line<'static> {
    value_row(label, input.value(), input.cursor(), active, cols, None)
}

/// A text row followed by a dimmed note, such as where an inherited
/// value came from.
pub fn tagged_input_row(
    label: &str,
    input: &TextInputState,
    tag: &str,
    active: bool,
    cols: Cols,
) -> Line<'static> {
    let text = format!(" {tag}");
    let width = text.chars().count();
    value_row(
        label,
        input.value(),
        input.cursor(),
        active,
        cols,
        Some(Suffix {
            width,
            spans: vec![Span::styled(
                text,
                Style::default().add_modifier(Modifier::DIM),
            )],
        }),
    )
}

/// A path field: the value, a mark for whether it exists, and the
/// remainder Tab would complete.
pub fn path_row(
    label: &str,
    omnibox: &PathOmniboxState,
    active: bool,
    cols: Cols,
) -> Line<'static> {
    value_row(
        label,
        omnibox.value(),
        omnibox.cursor(),
        active,
        cols,
        Some(path_suffix(omnibox, active)),
    )
}

fn path_suffix(omnibox: &PathOmniboxState, focused: bool) -> Suffix {
    let (symbol, style) = match omnibox.validation() {
        PathValidation::Empty => ("?", Style::default().fg(Color::DarkGray)),
        PathValidation::Exists => ("✓", Style::default().fg(Color::Green)),
        PathValidation::Missing => ("!", Style::default().fg(Color::Yellow)),
    };
    let mut spans = vec![Span::raw(" "), Span::styled(symbol, style)];
    let used = 1 + symbol.chars().count();
    let remaining = PATH_SUFFIX_WIDTH.saturating_sub(used);
    if remaining > 1 {
        if let Some(hint) = completion_hint(omnibox, focused) {
            let hint = truncate_chars(&hint, remaining.saturating_sub(1));
            let used = used + 1 + hint.chars().count();
            spans.push(Span::raw(" "));
            spans.push(Span::styled(hint, Style::default().fg(Color::Cyan)));
            spans.push(Span::raw(
                " ".repeat(PATH_SUFFIX_WIDTH.saturating_sub(used)),
            ));
        } else {
            spans.push(Span::raw(" ".repeat(remaining)));
        }
    }
    Suffix {
        width: PATH_SUFFIX_WIDTH,
        spans,
    }
}

/// The validity mark after a path value: two cells, a space and `✓`
/// (exists), `!` (missing) or `?` (empty).
pub fn path_mark(omnibox: &PathOmniboxState) -> Vec<Span<'static>> {
    let (symbol, style) = match omnibox.validation() {
        PathValidation::Empty => ("?", Style::default().fg(Color::DarkGray)),
        PathValidation::Exists => ("✓", Style::default().fg(Color::Green)),
        PathValidation::Missing => ("!", Style::default().fg(Color::Yellow)),
    };
    vec![Span::raw(" "), Span::styled(symbol, style)]
}

pub fn completion_hint(omnibox: &PathOmniboxState, focused: bool) -> Option<String> {
    if !focused {
        return None;
    }
    let value = omnibox.value().trim();
    omnibox
        .suggestions()
        .into_iter()
        .find(|suggestion| suggestion.path.as_str() != value)
        .and_then(|suggestion| completion_remainder(value, &suggestion.path))
        .map(|remainder| format!("Tab: {remainder}"))
}

/// What typing `suggestion` would add to `typed`.
pub fn completion_remainder(typed: &str, suggestion: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Some(suggestion.to_string());
    }
    suggestion
        .strip_prefix(typed)
        .filter(|remainder| !remainder.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let typed_basename = typed.rsplit('/').next().unwrap_or(typed);
            let suggestion_basename = suggestion.rsplit('/').next().unwrap_or(suggestion);
            suggestion_basename
                .strip_prefix(typed_basename)
                .filter(|remainder| !remainder.is_empty())
                .map(str::to_string)
        })
}

/// The harness field: its value with a chip for every known harness,
/// the selected one highlighted, or a `[custom]` mark for a value that
/// isn't one of them.
pub fn harness_row(
    label: &str,
    input: &TextInputState,
    known: &[String],
    active: bool,
    cols: Cols,
) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let available = inner_width.saturating_sub(prefix.chars().count());
    let value = input.value().trim();
    if !value.is_empty() && !known.iter().any(|k| k == value) {
        let suffix = " [custom]";
        let value_width = available.saturating_sub(suffix.len()).max(1);
        let w = window(input.value(), input.cursor(), value_width, active);
        let mut spans = vec![Span::styled(prefix, label_style)];
        spans.extend(window_spans(&w, entry_style(active)));
        spans.push(Span::styled(suffix, Style::default().fg(Color::Yellow)));
        return Line::from(spans);
    }
    let value_width = VALUE_SLOT_WIDTH.min(available).max(1);
    let chips_budget = available.saturating_sub(value_width).min(48);
    let w = window(input.value(), input.cursor(), value_width, active);
    let shown = w.left.chars().count()
        + w.before.chars().count()
        + w.cursor_text.chars().count()
        + w.after.chars().count()
        + w.right.chars().count();
    let padding = value_width.saturating_sub(shown);
    let style = entry_style(active);
    let mut spans = vec![Span::styled(prefix, label_style)];
    spans.extend(window_spans(&w, style));
    if padding > 0 {
        spans.push(Span::styled(" ".repeat(padding), style));
    }
    spans.extend(harness_chips(value, known, chips_budget));
    Line::from(spans)
}

pub fn harness_chips(selected: &str, known: &[String], width: usize) -> Vec<Span<'static>> {
    if width < 3 {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut remaining = width;
    for key in known {
        let token = format!(" [{key}]");
        let needed = token.chars().count();
        if needed > remaining {
            if remaining >= 2 {
                spans.push(Span::styled(
                    " …",
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            break;
        }
        if key == selected {
            spans.push(Span::styled(
                token,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                token,
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
        remaining -= needed;
    }
    spans
}

/// The harness's launch options as checkboxes, checked when the
/// effective argv already carries the option's flags.
pub fn options_row(
    label: &str,
    options: &[HarnessLaunchOption],
    effective_argv: &[String],
    active: bool,
    cols: Cols,
) -> Line<'static> {
    let value = options
        .iter()
        .map(|option| {
            checkbox(
                option.label,
                argv_contains_fragment(effective_argv, option.argv),
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    static_value_row(label, &value, active, cols)
}

/// One-of-N choices: `[x] a  [ ] b`.
pub fn choice_row(
    label: &str,
    choices: &[(&str, bool)],
    active: bool,
    cols: Cols,
) -> Line<'static> {
    let value = choices
        .iter()
        .map(|(text, selected)| checkbox(text, *selected))
        .collect::<Vec<_>>()
        .join(" ");
    static_value_row(label, &value, active, cols)
}

/// A row of text that isn't edited in place (a toggle, a choice).
pub fn static_value_row(label: &str, value: &str, active: bool, cols: Cols) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, label_style) = prefix(label, active, cols.label);
    let width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(TEXT_GUTTER.chars().count());
    Line::from(vec![
        Span::styled(prefix, label_style),
        Span::raw(TEXT_GUTTER),
        Span::styled(truncate_chars(value, width), entry_style(active)),
    ])
}

/// A dimmed, read-only row, such as the command a form will run.
pub fn note_row(label: &str, value: &str, cols: Cols) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, _) = prefix(label, false, cols.label);
    let width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(TEXT_GUTTER.chars().count())
        .max(1);
    Line::from(vec![
        Span::styled(prefix, Style::default().add_modifier(Modifier::DIM)),
        Span::raw(TEXT_GUTTER),
        Span::styled(
            truncate_chars(value, width),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ])
}

pub fn warning_row(label: &str, value: &str, cols: Cols) -> Line<'static> {
    let inner_width = cols.inner;
    let (prefix, _) = prefix(label, false, cols.label);
    let width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(TEXT_GUTTER.chars().count())
        .max(1);
    Line::from(vec![
        Span::styled(prefix, Style::default().add_modifier(Modifier::DIM)),
        Span::raw(TEXT_GUTTER),
        Span::styled(
            truncate_chars(value, width),
            Style::default().fg(Color::Yellow),
        ),
    ])
}

/// Wrap a key hint footer to `width` cells, breaking at the ` · `
/// separators, dimmed.
pub fn wrap_footer(footer: &str, width: usize) -> Vec<Line<'static>> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut lines = Vec::new();
    let mut current = String::new();
    for segment in footer.split(" · ") {
        let extra = if current.is_empty() { 0 } else { 3 };
        if !current.is_empty() && current.chars().count() + extra + segment.chars().count() > width
        {
            lines.push(Line::styled(std::mem::take(&mut current), dim));
        }
        if !current.is_empty() {
            current.push_str(" · ");
        }
        current.push_str(segment);
    }
    if !current.is_empty() {
        lines.push(Line::styled(current, dim));
    }
    lines
}

/// The argv an empty argv field means for `harness`: a launch profile or
/// `atelier exec` when configured, else the harness default.
pub fn default_argv(
    defaults: Option<&crate::launch_default::LaunchDefaults>,
    harness: &str,
    cwd: &str,
) -> Vec<String> {
    let cwd = cwd.trim();
    let argv = defaults
        .and_then(|d| {
            d.default_argv(
                harness,
                (!cwd.is_empty()).then(|| std::path::Path::new(cwd)),
            )
        })
        .map_or_else(
            || crate::discovery::harness::launch_argv_for(harness),
            |(argv, _)| argv,
        );
    argv.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

pub fn checkbox(label: &str, selected: bool) -> String {
    if selected {
        format!("[x] {label}")
    } else {
        format!("[ ] {label}")
    }
}

pub fn truncate_chars(value: &str, width: usize) -> String {
    value.chars().take(width).collect()
}

/// `text` cut to `width` cells with a trailing `…` when it is longer,
/// and padded with spaces to exactly `width` cells when shorter.
pub fn fit(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return format!("{text}{}", " ".repeat(width - count));
    }
    if width == 0 {
        return String::new();
    }
    let mut cut: String = text.chars().take(width - 1).collect();
    cut.push('…');
    cut
}

/// `text` broken at spaces into lines of at most `width` cells.
pub fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let needed = usize::from(!current.is_empty()) + word.chars().count();
        if !current.is_empty() && current.chars().count() + needed > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// An argv shown the way a shell would take it back.
pub fn display_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_display_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_display_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '=' | '+')
    }) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}

/// A centered popup rect sized for `content_lines` rows.
pub fn modal_rect(area: Rect, width_cap: u16, content_lines: usize, min_height: u16) -> Rect {
    let width = std::cmp::min(width_cap, area.width.saturating_sub(4)).max(44);
    let desired = u16::try_from(content_lines.saturating_add(2)).unwrap_or(u16::MAX);
    let height = desired.clamp(min_height, area.height.max(min_height));
    super::popup_frame::centered_rect(area, width, height)
}

/// Popup body that scrolls so the cursor row stays visible.
pub struct ScrollLinesBody {
    pub lines: Vec<Line<'static>>,
    pub inner_width: usize,
    pub inner_height: usize,
    pub scroll_offset: u16,
}

impl KnownSize for ScrollLinesBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for ScrollLinesBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.lines)
            .scroll((self.scroll_offset, 0))
            .render(area, buf);
    }
}

/// Popup body in three bands: lines that stay put at the top and bottom,
/// and between them rows that scroll to keep the cursor row visible.
/// The fixed bands keep their place however many rows there are.
pub struct SplitBody {
    pub top: Vec<Line<'static>>,
    pub rows: Vec<Line<'static>>,
    pub bottom: Vec<Line<'static>>,
    pub cursor_row: usize,
    pub inner_width: usize,
    pub inner_height: usize,
}

impl KnownSize for SplitBody {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for SplitBody {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = usize::from(area.height);
        let bottom = self.bottom.len().min(height);
        let top = self.top.len().min(height - bottom);
        let rows_height = height - bottom - top;
        let offset = scroll_offset_for_cursor(self.cursor_row, rows_height, self.rows.len());
        let line_at = |y: usize| {
            Rect::new(
                area.x,
                area.y + u16::try_from(y).unwrap_or(0),
                area.width,
                1,
            )
        };
        for (i, line) in self.top.into_iter().take(top).enumerate() {
            Paragraph::new(line).render(line_at(i), buf);
        }
        for (i, line) in self
            .rows
            .into_iter()
            .skip(usize::from(offset))
            .take(rows_height)
            .enumerate()
        {
            Paragraph::new(line).render(line_at(top + i), buf);
        }
        let first = height - bottom;
        for (i, line) in self.bottom.into_iter().take(bottom).enumerate() {
            Paragraph::new(line).render(line_at(first + i), buf);
        }
    }
}

/// How far to scroll so line `cursor_line` is the last one visible.
pub fn scroll_offset_for_cursor(
    cursor_line: usize,
    inner_height: usize,
    content_height: usize,
) -> u16 {
    if inner_height == 0 || cursor_line < inner_height {
        return 0;
    }
    let max_scroll = content_height.saturating_sub(inner_height);
    let offset = cursor_line
        .saturating_sub(inner_height.saturating_sub(1))
        .min(max_scroll);
    u16::try_from(offset).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod tests;
