//! Full-screen viewer for a mux pane's scrollback.
//!
//! It is the pane's counterpart of the transcript viewer and keeps its
//! shape: a header, the body under a rule, and a footer with the
//! position, the find chip, and the hints; `j`/`k`, paging, `g`/`G`,
//! `/` with `n`/`N`, a generated `?` help page, `Esc` stepping back
//! (help, then the find, then the viewer) and `q` leaving at once. What
//! is its own is `a`, which attaches to the pane's mux.
//!
//! The viewer holds a snapshot of the scrollback taken when it opened,
//! so it can scroll and find without touching tmux again. Opening it from
//! a search result lands on the matching line with that text picked out.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuikit::chip::{PositionChip, StatusChip, Tone};
use tuikit::glyphs::marks;
use tuikit::scroll::ScrollWindow;

use crate::content_search::PaneRef;
use crate::tui::Theme;
use crate::tui::keybindings::overlay::{
    KeyRow, PANE_VIEWER_KEYS, VIEWER_FIND_KEYS, resolve, typed_char,
};
use crate::viewer::find::{FindPrompt, ViewerFindMsg};

use super::help::{binding_lines, blank, push_footer, render_help_page, section};
use super::viewer_help::key_notation;

/// Lines of context kept above a line the viewer jumps to.
const CONTEXT_ABOVE: usize = 2;
/// The most cells the query takes in the footer's find chip, the
/// ellipsis of a cut query included (the transcript viewer's chip uses
/// the same budget).
const FIND_CHIP_QUERY_CELLS: usize = 17;

/// Everything the pane viewer can be told to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneViewerMsg {
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    JumpToStart,
    JumpToEnd,
    /// Open the find prompt (`/`).
    FindOpen,
    /// Go to the next match of the active find (`n`).
    FindNext,
    /// Go to the previous match of the active find (`N`).
    FindPrev,
    ToggleHelp,
    /// Attach to the pane's mux (`a`).
    Attach,
    /// Step back: close the help, then clear the find, then leave (`Esc`).
    Close,
    /// Leave at once; with the help up it closes the help only (`q`).
    Quit,
}

impl PaneViewerMsg {
    /// Every message, in the order the help page lists them.
    pub const ALL: [PaneViewerMsg; 15] = [
        PaneViewerMsg::ScrollDown,
        PaneViewerMsg::ScrollUp,
        PaneViewerMsg::PageDown,
        PaneViewerMsg::PageUp,
        PaneViewerMsg::HalfPageDown,
        PaneViewerMsg::HalfPageUp,
        PaneViewerMsg::JumpToStart,
        PaneViewerMsg::JumpToEnd,
        PaneViewerMsg::FindOpen,
        PaneViewerMsg::FindNext,
        PaneViewerMsg::FindPrev,
        PaneViewerMsg::Attach,
        PaneViewerMsg::ToggleHelp,
        PaneViewerMsg::Close,
        PaneViewerMsg::Quit,
    ];

    /// The help section a message is listed under, and what it does.
    pub fn help(self) -> (&'static str, &'static str) {
        match self {
            PaneViewerMsg::ScrollDown => ("Moving", "Scroll one line down"),
            PaneViewerMsg::ScrollUp => ("Moving", "Scroll one line up"),
            PaneViewerMsg::PageDown => ("Moving", "Page down"),
            PaneViewerMsg::PageUp => ("Moving", "Page up"),
            PaneViewerMsg::HalfPageDown => ("Moving", "Half a page down"),
            PaneViewerMsg::HalfPageUp => ("Moving", "Half a page up"),
            PaneViewerMsg::JumpToStart => ("Moving", "Jump to the oldest line"),
            PaneViewerMsg::JumpToEnd => ("Moving", "Jump to the newest line"),
            PaneViewerMsg::FindOpen => ("Finding", "Find in the scrollback"),
            PaneViewerMsg::FindNext => ("Finding", "Next match, wrapping at the end"),
            PaneViewerMsg::FindPrev => ("Finding", "Previous match, wrapping at the start"),
            PaneViewerMsg::Attach => (
                "Pane",
                "Attach to the pane's mux (refused when it is stopped or gone)",
            ),
            PaneViewerMsg::ToggleHelp => ("Viewer", "Show or hide this help"),
            PaneViewerMsg::Close => (
                "Viewer",
                "Step back: close help, then the find, then the viewer",
            ),
            PaneViewerMsg::Quit => (
                "Viewer",
                "Leave the viewer at once, even with a find active",
            ),
        }
    }
}

/// What the runtime does after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneViewerOutcome {
    Continue,
    Close,
    /// Attach to the pane's mux. The reducer decides whether it can.
    Attach,
}

/// The pane viewer's state: a snapshot of the scrollback and where the
/// operator is in it.
#[derive(Clone, Debug)]
pub struct PaneViewerState {
    pub pane: PaneRef,
    /// The header's label: the mux, window, and pane.
    pub label: String,
    pub lines: Vec<String>,
    /// The first visible line, clamped on every draw.
    pub scroll: usize,
    /// Lines the body showed last draw; paging moves by it.
    pub viewport: u16,
    /// The prompt while it is open.
    pub prompt: Option<FindPrompt>,
    /// The active query; empty when no find is active.
    pub query: String,
    /// Indices of the lines that hold the query, in order.
    pub matches: Vec<usize>,
    /// The match the viewer is on, as an index into `matches`.
    pub current: Option<usize>,
    pub show_help: bool,
    pub help_scroll: usize,
    /// Rows of the help page the last draw showed; its page keys move by
    /// this many.
    pub help_page: usize,
    /// Why the last key did nothing, shown in the footer until the next.
    pub note: Option<String>,
    /// The find hint for the prompt's footer, generated from its key table.
    pub find_hint: String,
}

impl PaneViewerState {
    /// A viewer over `lines`, opened on the line nearest `line` (from 1)
    /// that holds `find`. The scrollback may have moved since the search
    /// read it, so the match is looked for rather than the line number
    /// trusted; with no match near it says so and opens on `line`.
    pub fn open(pane: PaneRef, lines: Vec<String>, line: usize, find: &str) -> Self {
        let label = format!(
            "{} {}.{} scrollback",
            pane.mux.session, pane.window_index, pane.pane_index
        );
        let mut state = Self {
            pane,
            label,
            lines,
            scroll: 0,
            viewport: 0,
            prompt: None,
            query: String::new(),
            matches: Vec::new(),
            current: None,
            show_help: false,
            help_scroll: 0,
            help_page: 0,
            note: None,
            find_hint: crate::tui::keybindings::overlay::footer(VIEWER_FIND_KEYS),
        };
        let wanted = line
            .saturating_sub(1)
            .min(state.lines.len().saturating_sub(1));
        state.run_find(find);
        let nearest = state
            .matches
            .iter()
            .enumerate()
            .min_by_key(|(_, index)| index.abs_diff(wanted))
            .map(|(position, _)| position);
        if let Some(position) = nearest {
            state.go_to_match(position);
        } else {
            if !find.trim().is_empty() {
                state.note = Some("the scrollback no longer holds the match".to_string());
            }
            state.scroll = wanted.saturating_sub(CONTEXT_ABOVE);
        }
        state
    }

    fn max_scroll(&self) -> usize {
        self.lines.len().saturating_sub(self.viewport as usize)
    }

    /// Whether a find is active (confirmed with a non-empty query).
    pub fn find_is_active(&self) -> bool {
        !self.query.is_empty()
    }

    fn run_find(&mut self, query: &str) {
        self.query = query.trim().to_string();
        self.matches = if self.query.is_empty() {
            Vec::new()
        } else {
            self.lines
                .iter()
                .enumerate()
                .filter(|(_, text)| tuikit::text::find_ci(text, &self.query).is_some())
                .map(|(index, _)| index)
                .collect()
        };
        self.current = None;
    }

    fn go_to_match(&mut self, position: usize) {
        self.current = Some(position);
        let line = self.matches[position];
        let visible = self.scroll..self.scroll + (self.viewport as usize).max(1);
        if !visible.contains(&line) {
            self.scroll = line.saturating_sub(CONTEXT_ABOVE);
        }
    }

    fn step_find(&mut self, forward: bool) {
        if self.matches.is_empty() {
            self.note = Some(if self.find_is_active() {
                "no match".to_string()
            } else {
                "no find: press / to search".to_string()
            });
            return;
        }
        let count = self.matches.len();
        let next = match (self.current, forward) {
            (Some(at), true) => (at + 1) % count,
            (Some(at), false) => (at + count - 1) % count,
            (None, true) => self
                .matches
                .iter()
                .position(|line| *line >= self.scroll)
                .unwrap_or(0),
            (None, false) => self
                .matches
                .iter()
                .rposition(|line| *line < self.scroll + self.viewport as usize)
                .unwrap_or(count - 1),
        };
        if let Some(at) = self.current
            && ((forward && next <= at) || (!forward && next >= at))
        {
            self.note = Some(if forward {
                "wrapped to the first match".to_string()
            } else {
                "wrapped to the last match".to_string()
            });
        }
        self.go_to_match(next);
    }

    /// Apply one message. The footer note from the last key is dropped
    /// first.
    pub fn apply(&mut self, msg: PaneViewerMsg) -> PaneViewerOutcome {
        self.note = None;
        // While the help page is up it owns the movement keys.
        if self.show_help
            && !matches!(
                msg,
                PaneViewerMsg::ToggleHelp | PaneViewerMsg::Close | PaneViewerMsg::Quit
            )
        {
            self.scroll_help(msg);
            return PaneViewerOutcome::Continue;
        }
        let page = usize::from(self.viewport.max(1));
        match msg {
            PaneViewerMsg::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
            PaneViewerMsg::ScrollDown => self.scroll = (self.scroll + 1).min(self.max_scroll()),
            PaneViewerMsg::PageUp => self.scroll = self.scroll.saturating_sub(page),
            PaneViewerMsg::PageDown => self.scroll = (self.scroll + page).min(self.max_scroll()),
            PaneViewerMsg::HalfPageUp => self.scroll = self.scroll.saturating_sub(page / 2),
            PaneViewerMsg::HalfPageDown => {
                self.scroll = (self.scroll + page / 2).min(self.max_scroll());
            }
            PaneViewerMsg::JumpToStart => self.scroll = 0,
            PaneViewerMsg::JumpToEnd => self.scroll = self.max_scroll(),
            PaneViewerMsg::FindOpen => {
                self.prompt = Some(FindPrompt::with_text(&self.query));
            }
            PaneViewerMsg::FindNext => self.step_find(true),
            PaneViewerMsg::FindPrev => self.step_find(false),
            PaneViewerMsg::ToggleHelp => {
                self.show_help = !self.show_help;
                self.help_scroll = 0;
            }
            PaneViewerMsg::Attach => return PaneViewerOutcome::Attach,
            PaneViewerMsg::Close => {
                if self.show_help {
                    self.show_help = false;
                } else if self.find_is_active() {
                    self.run_find("");
                } else {
                    return PaneViewerOutcome::Close;
                }
            }
            PaneViewerMsg::Quit => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    return PaneViewerOutcome::Close;
                }
            }
        }
        PaneViewerOutcome::Continue
    }

    /// Move the help page. A page is the rows the help showed, not the
    /// scrollback's taller viewport behind it.
    fn scroll_help(&mut self, msg: PaneViewerMsg) {
        let page = self.help_page.max(1);
        let half = (page / 2).max(1);
        self.help_scroll = match msg {
            PaneViewerMsg::ScrollUp => self.help_scroll.saturating_sub(1),
            PaneViewerMsg::ScrollDown => self.help_scroll.saturating_add(1),
            PaneViewerMsg::PageUp => self.help_scroll.saturating_sub(page),
            PaneViewerMsg::PageDown => self.help_scroll.saturating_add(page),
            PaneViewerMsg::HalfPageUp => self.help_scroll.saturating_sub(half),
            PaneViewerMsg::HalfPageDown => self.help_scroll.saturating_add(half),
            PaneViewerMsg::JumpToStart => 0,
            PaneViewerMsg::JumpToEnd => usize::MAX,
            _ => self.help_scroll,
        };
    }

    /// Edit the find prompt. Confirming runs the find and jumps to the
    /// first match at or after the top of the screen.
    pub fn apply_find(&mut self, msg: ViewerFindMsg) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        match msg {
            ViewerFindMsg::Cancel => self.prompt = None,
            ViewerFindMsg::Confirm => {
                let text = prompt.text.clone();
                self.prompt = None;
                self.run_find(&text);
                if self.find_is_active() {
                    if self.matches.is_empty() {
                        self.note = Some("no match".to_string());
                    } else {
                        let first = self
                            .matches
                            .iter()
                            .position(|line| *line >= self.scroll)
                            .unwrap_or(0);
                        self.go_to_match(first);
                    }
                }
            }
            ViewerFindMsg::Backspace => prompt.backspace(),
            ViewerFindMsg::Delete => prompt.delete(),
            ViewerFindMsg::Left => prompt.left(),
            ViewerFindMsg::Right => prompt.right(),
            ViewerFindMsg::WordLeft => prompt.word_left(),
            ViewerFindMsg::WordRight => prompt.word_right(),
            ViewerFindMsg::Home => prompt.home(),
            ViewerFindMsg::End => prompt.end(),
            ViewerFindMsg::ClearLine => prompt.clear_line(),
            ViewerFindMsg::Insert(c) => prompt.insert(c),
        }
    }

    /// Route a key: to the find prompt while it is open, otherwise
    /// through the viewer's key table.
    pub fn handle_key(&mut self, key: KeyEvent) -> PaneViewerOutcome {
        if self.prompt.is_some() {
            let msg = resolve(VIEWER_FIND_KEYS, key)
                .or_else(|| typed_char(key).map(ViewerFindMsg::Insert));
            if let Some(msg) = msg {
                self.apply_find(msg);
            }
            return PaneViewerOutcome::Continue;
        }
        match resolve(PANE_VIEWER_KEYS, key) {
            Some(msg) => self.apply(msg),
            None => PaneViewerOutcome::Continue,
        }
    }
}

/// Draw the viewer onto `area`, and its help page over it when open.
/// Records the body height for paging.
pub fn draw(state: &mut PaneViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // rule
        Constraint::Min(1),    // body
        Constraint::Length(1), // rule
        Constraint::Length(1), // footer
    ])
    .split(area);
    draw_header(state, theme, frame, chunks[0]);
    draw_rule(theme, frame, chunks[1]);
    draw_body(state, theme, frame, chunks[2]);
    draw_rule(theme, frame, chunks[3]);
    draw_footer(state, theme, frame, chunks[4]);
    if state.show_help {
        let window = render_help(area, frame.buffer_mut(), theme, state.help_scroll);
        state.help_scroll = window.offset();
        state.help_page = window.viewport();
    }
}

fn dim(theme: &Theme) -> Style {
    Style::new().fg(theme.inert).add_modifier(theme.muted)
}

fn draw_header(state: &PaneViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    let count = state.lines.len();
    let command = state.pane.command.as_deref().unwrap_or("(unknown command)");
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            state.label.clone(),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", dim(theme)),
        Span::styled(command.to_string(), dim(theme)),
        Span::styled(" · ", dim(theme)),
        Span::styled(
            format!("{count} {}", if count == 1 { "line" } else { "lines" }),
            dim(theme),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_rule(theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            marks::RULE.repeat(area.width as usize),
            dim(theme),
        ))),
        area,
    );
}

fn draw_body(state: &mut PaneViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    state.viewport = area.height;
    state.scroll = state.scroll.min(state.max_scroll());
    if state.lines.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "the pane has no scrollback",
                Style::new().add_modifier(theme.muted),
            ))),
            area,
        );
        return;
    }
    let end = state.lines.len().min(state.scroll + area.height as usize);
    let anchor = state.current.map(|position| state.matches[position]);
    let gutter = Style::new().fg(theme.inert);
    let anchored = Style::new().fg(theme.accent).add_modifier(Modifier::BOLD);
    let visible: Vec<Line<'static>> = (state.scroll..end)
        .map(|index| {
            let number = if Some(index) == anchor {
                anchored
            } else {
                gutter
            };
            let mut spans = vec![Span::styled(format!("{:>5} ", index + 1), number)];
            spans.extend(highlighted(&state.lines[index], &state.query, theme));
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(visible), area);
}

/// `text` with each occurrence of `find` in the theme's match style.
fn highlighted(text: &str, find: &str, theme: &Theme) -> Vec<Span<'static>> {
    tuikit::text::paint(
        &[Span::raw(text)],
        tuikit::text::find_all_ci(text, find),
        |base| base.patch(theme.match_style()),
    )
}

fn draw_footer(state: &PaneViewerState, theme: &Theme, frame: &mut Frame<'_>, area: Rect) {
    if let Some(prompt) = &state.prompt {
        crate::viewer::widget::draw_find_prompt(prompt, &state.find_hint, theme, frame, area);
        return;
    }
    let pipe = Span::styled(" · ", dim(theme));
    let mut spans = vec![
        PositionChip::new(state.scroll, state.lines.len()).span(),
        pipe.clone(),
    ];
    if state.find_is_active() {
        let query = tuikit::text::truncate_end(&state.query, FIND_CHIP_QUERY_CELLS);
        let chip = match state.current {
            Some(at) => StatusChip::new(
                format!("find \"{query}\" {}/{}", at + 1, state.matches.len()),
                Tone::Success,
            ),
            None => StatusChip::new(format!("find \"{query}\" no match"), Tone::Warning),
        };
        spans.push(chip.bold().span(theme));
        spans.push(pipe.clone());
    }
    if let Some(note) = &state.note {
        spans.push(
            StatusChip::new(note.as_str(), Tone::Warning)
                .bold()
                .span(theme),
        );
        spans.push(pipe.clone());
    }
    // The keys that matter most stay when the footer is tight; the
    // movement reminder goes first.
    let used = tuikit::text::spans_width(&spans);
    let chips = "(a) attach · (?) help · (q) close";
    let scroll = "j/k scroll · g/G top/end · / find · ";
    let full = format!("{scroll}{chips}");
    let hint = if used + tuikit::text::width(&full) <= area.width as usize {
        full
    } else {
        chips.to_string()
    };
    spans.push(Span::styled(hint, dim(theme)));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// One help row: the keys bound to a message and what it does.
struct HelpRow {
    keys: String,
    description: &'static str,
}

/// The help page's rows by section, generated from the key table and
/// the descriptions on [`PaneViewerMsg::help`]. A message with no key is
/// left out, as there is nothing to press.
fn help_sections(table: &[KeyRow<PaneViewerMsg>]) -> Vec<(&'static str, Vec<HelpRow>)> {
    let mut sections: Vec<(&'static str, Vec<HelpRow>)> = Vec::new();
    for msg in PaneViewerMsg::ALL {
        let mut keys: Vec<String> = table
            .iter()
            .filter_map(|row| match row {
                KeyRow::Bind { key, action, .. } if *action == msg => Some(key_notation(key)),
                _ => None,
            })
            .collect();
        if keys.is_empty() {
            continue;
        }
        keys.sort_by_key(|k| k.starts_with("Ctrl-"));
        let (title, description) = msg.help();
        let row = HelpRow {
            keys: keys.join(" / "),
            description,
        };
        match sections.iter_mut().find(|(t, _)| *t == title) {
            Some((_, rows)) => rows.push(row),
            None => sections.push((title, vec![row])),
        }
    }
    sections
}

/// The help page's text, wrapped to `width`.
pub fn help_lines(theme: &Theme, width: Option<usize>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (title, rows) in help_sections(PANE_VIEWER_KEYS) {
        section(&mut lines, title);
        for row in &rows {
            lines.extend(binding_lines(&row.keys, row.description, theme, width));
        }
        blank(&mut lines);
    }
    push_footer(&mut lines, theme, width);
    lines
}

fn render_help(area: Rect, buf: &mut Buffer, theme: &Theme, scroll: usize) -> ScrollWindow {
    render_help_page(area, buf, theme, scroll, &|width| {
        help_lines(theme, Some(width))
    })
}

#[cfg(test)]
#[path = "pane_viewer_tests.rs"]
mod tests;
