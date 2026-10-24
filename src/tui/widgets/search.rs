//! `/` in-view search overlay.
//!
//! Reuses the ADR 0030 text-input primitive for the query line and
//! displays a list of matches ranked by [`crate::tui::search::rank`].
//!
//! Per ADR 0031 the overlay ranks within the active filter set —
//! the host runtime hands the overlay only the visible row tree's
//! [`SearchItem`]s, which have already had filters applied
//! upstream.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::{line, span};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tui_popup::KnownSize;

use crate::tui::Theme;

use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::rows::RowId;
use crate::tui::search::{SearchItem, SearchMatch, rank, snippet_around};
use crate::tui::widgets::input::TextInputState;
use crate::viewer::search::{ContentKinds, Grouping};

/// How long the query rests before a contents scan starts, so typing a
/// word does not start a scan per letter.
pub const DEBOUNCE: Duration = Duration::from_millis(200);

/// Why contents search cannot be chosen when live preview is off.
pub const CONTENT_SEARCH_OFF: &str = "Content search is off while live preview is disabled";

/// What the overlay searches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    /// The rows of the current view, as `/` always has.
    #[default]
    Rows,
    /// What was said in the sessions.
    Contents,
}

/// One result in the contents scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRow {
    pub session: crate::model::AgentSessionId,
    pub label: String,
    /// The harness's name, drawn as a chip.
    pub harness: String,
    /// Matching turns in the session.
    pub hits: usize,
    /// Per-message results say which turn this row is.
    pub turn: Option<String>,
    pub snippet: String,
    /// Byte range of the match inside `snippet`.
    pub highlight: Option<std::ops::Range<usize>>,
    pub turn_index: usize,
}

/// What the host tells the widget about the running contents scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentView {
    pub rows: Vec<ContentRow>,
    pub scanned: usize,
    pub total: usize,
    pub skipped: usize,
    pub done: bool,
    /// Set when contents search cannot run, and why.
    pub unavailable: Option<&'static str>,
}

/// What the overlay wants the host to do about the contents scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DueSearch {
    Preview {
        text: String,
        kinds: ContentKinds,
        group: Grouping,
    },
    /// Stop previewing.
    Clear,
}

/// Pure state for the search overlay. Holds the query text input,
/// the latest match list (recomputed by the host whenever the
/// query changes), and the cursor inside that list.
#[derive(Debug, Clone)]
pub struct SearchOverlayState {
    input: TextInputState,
    matches: Vec<SearchMatch>,
    cursor: usize,
    scope: SearchScope,
    group: Grouping,
    kinds: ContentKinds,
    /// Whether the contents scope can run (live preview is on).
    contents_enabled: bool,
    /// Results in the contents scope, set by the host before a key.
    content_len: usize,
    /// When the query or options last changed and a scan is owed.
    edited_at: Option<Instant>,
    /// Why the contents scope could not be chosen, shown once.
    notice: Option<&'static str>,
}

impl SearchOverlayState {
    pub fn new() -> Self {
        Self {
            input: TextInputState::new(" search ", String::new()),
            matches: Vec::new(),
            cursor: 0,
            scope: SearchScope::Rows,
            group: Grouping::default(),
            kinds: ContentKinds::default(),
            contents_enabled: true,
            content_len: 0,
            edited_at: None,
            notice: None,
        }
    }

    /// An overlay set up the way the operator left it, or with a search
    /// already active: `query` fills the field, and a non-empty one in
    /// the contents scope is searched at once.
    pub fn configured(
        scope: SearchScope,
        group: Grouping,
        kinds: ContentKinds,
        contents_enabled: bool,
        query: &str,
    ) -> Self {
        let mut state = Self::new();
        state.input = TextInputState::new(" search ", query.to_string());
        state.group = group;
        state.kinds = kinds;
        state.contents_enabled = contents_enabled;
        if scope == SearchScope::Contents && contents_enabled {
            state.scope = SearchScope::Contents;
        }
        state
    }

    pub fn scope(&self) -> SearchScope {
        self.scope
    }

    pub fn group(&self) -> Grouping {
        self.group
    }

    pub fn kinds(&self) -> ContentKinds {
        self.kinds
    }

    pub fn notice(&self) -> Option<&'static str> {
        self.notice
    }

    /// Switch to the contents scope if it is available and not already
    /// chosen, as the Search view's `/` does.
    pub fn start_in_contents(&mut self) {
        if self.scope == SearchScope::Rows && self.contents_enabled {
            self.toggle_scope();
        }
    }

    /// The host reports how many contents results there are, so the
    /// cursor can move among them.
    pub fn set_content_len(&mut self, len: usize) {
        self.content_len = len;
        if self.cursor >= len {
            self.cursor = 0;
        }
    }

    /// Owe a contents scan for the current query, as soon as the host
    /// asks (`force`) or after the debounce.
    pub fn mark_edited(&mut self, at: Instant) {
        self.edited_at = Some(at);
    }

    /// The scan the overlay is owed once the query has rested, if any.
    /// `Clear` says to stop previewing (rows scope, or an empty query).
    pub fn take_due_search(&mut self, now: Instant, force: bool) -> Option<DueSearch> {
        let at = self.edited_at?;
        if !force && now.duration_since(at) < DEBOUNCE {
            return None;
        }
        self.edited_at = None;
        let text = self.input.value().trim().to_string();
        if self.scope == SearchScope::Contents && !text.is_empty() {
            Some(DueSearch::Preview {
                text,
                kinds: self.kinds,
                group: self.group,
            })
        } else {
            Some(DueSearch::Clear)
        }
    }

    /// The query as the contents scan will see it.
    pub fn content_text(&self) -> String {
        self.input.value().trim().to_string()
    }

    /// Current query text (post-edit, pre-rank).
    pub fn query(&self) -> &str {
        self.input.value()
    }

    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Refresh the match list against `items` using `backend`. The
    /// host calls this after every key dispatch so the operator
    /// sees the list shrink/grow as they type. Resets the cursor
    /// to 0 so a newly typed character doesn't strand it past the
    /// end of the new result list.
    pub fn refresh_matches(&mut self, items: &[SearchItem<'_>]) {
        self.matches = rank(self.input.value(), items);
        // The cursor belongs to the contents results in that scope.
        if self.scope == SearchScope::Rows && self.cursor >= self.matches.len() {
            self.cursor = 0;
        }
    }
}

impl Default for SearchOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// What the host should do after passing a key event through the
/// overlay. `Continue` keeps it open; `Cancel` closes without a
/// selection change; `Confirm(id)` closes and asks the host to
/// move the selection to `id`. The id is boxed so the enum stays
/// small (RowId carries a NodeId that can be large for sessions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchOutcome {
    Continue,
    Cancel,
    Confirm(Box<RowId>),
    /// Contents scope: open the result under the cursor.
    OpenHit(usize),
    /// Contents scope: keep the query as the filter on the lists, and
    /// with `show_results` open the Search view on it.
    Promote {
        show_results: bool,
    },
}

impl SearchOverlayState {
    /// Dispatch a crossterm key event.
    ///
    /// - `Esc` / `Ctrl-C`: cancel.
    /// - `Enter`: confirm with the cursor's match, or cancel if no
    ///   matches are present.
    /// - `Down` / `Ctrl-N`: move cursor down (wraps).
    /// - `Up` / `Ctrl-P`: move cursor up (wraps).
    /// - Everything else: passed through to the text input. We do
    ///   not intercept `j` / `k` here because typing them into the
    ///   query is the common case; result-list navigation uses
    ///   arrow keys + ctrl-N/P instead.
    pub fn handle_key(&mut self, event: KeyEvent) -> SearchOutcome {
        use crate::tui::keybindings::overlay::{
            SEARCH_CONTENT_KEYS, SEARCH_KEYS, SearchKey, resolve,
        };
        let table = match self.scope {
            SearchScope::Rows => SEARCH_KEYS,
            SearchScope::Contents => SEARCH_CONTENT_KEYS,
        };
        let contents = self.scope == SearchScope::Contents;
        match resolve(table, event) {
            Some(SearchKey::Abort | SearchKey::Cancel) => SearchOutcome::Cancel,
            Some(SearchKey::Pick) if contents => {
                if self.content_len == 0 {
                    SearchOutcome::Cancel
                } else {
                    SearchOutcome::OpenHit(self.cursor)
                }
            }
            Some(SearchKey::Pick) => match self.matches.get(self.cursor) {
                Some(m) => SearchOutcome::Confirm(Box::new(m.id.clone())),
                None => SearchOutcome::Cancel,
            },
            Some(SearchKey::Down) => {
                self.move_cursor(1);
                SearchOutcome::Continue
            }
            Some(SearchKey::Up) => {
                self.move_cursor(-1);
                SearchOutcome::Continue
            }
            Some(SearchKey::Scope) => {
                self.toggle_scope();
                SearchOutcome::Continue
            }
            Some(SearchKey::Results) => {
                self.group = match self.group {
                    Grouping::Session => Grouping::Message,
                    Grouping::Message => Grouping::Session,
                };
                self.cursor = 0;
                self.edited_at = Some(Instant::now());
                SearchOutcome::Continue
            }
            Some(SearchKey::Tools) => {
                self.kinds.tools = !self.kinds.tools;
                self.edited_at = Some(Instant::now());
                SearchOutcome::Continue
            }
            Some(SearchKey::Thinking) => {
                self.kinds.thinking = !self.kinds.thinking;
                self.edited_at = Some(Instant::now());
                SearchOutcome::Continue
            }
            Some(SearchKey::ApplyFilter) => self.promote(false),
            Some(SearchKey::ApplyView) => self.promote(true),
            None => {
                // Let the text-input primitive handle the key. We
                // ignore its outcome here because Enter/Esc are
                // intercepted above; everything else is "still
                // editing the query."
                let before = self.input.value().to_string();
                let _ = self.input.handle_key(event);
                if contents && self.input.value() != before {
                    self.cursor = 0;
                    self.edited_at = Some(Instant::now());
                }
                SearchOutcome::Continue
            }
        }
    }

    fn promote(&self, show_results: bool) -> SearchOutcome {
        if self.content_text().is_empty() {
            SearchOutcome::Continue
        } else {
            SearchOutcome::Promote { show_results }
        }
    }

    fn toggle_scope(&mut self) {
        self.cursor = 0;
        match self.scope {
            SearchScope::Contents => {
                self.scope = SearchScope::Rows;
                // Stop the scan the contents scope was running.
                self.edited_at = Some(Instant::now());
            }
            SearchScope::Rows if self.contents_enabled => {
                self.scope = SearchScope::Contents;
                self.notice = None;
                self.edited_at = Some(Instant::now());
            }
            SearchScope::Rows => {
                self.notice = Some(CONTENT_SEARCH_OFF);
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = match self.scope {
            SearchScope::Rows => self.matches.len(),
            SearchScope::Contents => self.content_len,
        };
        if len == 0 {
            self.cursor = 0;
            return;
        }
        self.cursor = crate::tui::cursor::wrap_step(self.cursor, len, delta);
    }
}

/// Centered modal that renders the query line on top and a
/// ranked match list below. Caps the visible match list at the
/// available height; the cursor scrolls within it.
pub struct SearchOverlayWidget<'a> {
    state: &'a SearchOverlayState,
    items: &'a [SearchItem<'a>],
    theme: &'a Theme,
    content: Option<&'a ContentView>,
}

impl<'a> SearchOverlayWidget<'a> {
    pub fn new(
        state: &'a SearchOverlayState,
        items: &'a [SearchItem<'a>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            state,
            items,
            theme,
            content: None,
        }
    }

    /// What the contents scope shows; ignored in the rows scope.
    #[must_use]
    pub fn with_content(mut self, content: &'a ContentView) -> Self {
        self.content = Some(content);
        self
    }
}

impl Widget for SearchOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`; the body
        // wrapper reports the same cap dims `centered_modal_rect`
        // produces so auto-sizing reproduces the legacy rect.
        let modal = centered_modal_rect(area);
        let contents = self.state.scope() == SearchScope::Contents;
        let title = match (contents, self.content) {
            (true, Some(view)) => {
                let n = view.rows.len();
                line![format!(
                    " / search contents ({n} {}) ",
                    if n == 1 { "result" } else { "results" }
                )]
            }
            (true, None) => line![" / search contents "],
            (false, _) => {
                let n = self.state.matches().len();
                line![format!(
                    " / search ({n} {}) ",
                    if n == 1 { "match" } else { "matches" }
                )]
            }
        };
        let body = SearchBody {
            state: self.state,
            items: self.items,
            content: self.content,
            theme: self.theme,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let popup = crate::tui::widgets::popup_frame::themed_popup(body, title, self.theme);
        popup.render(area, buf);
    }
}

/// Body wrapper for `tui_popup::Popup`. Renders the query row +
/// ranked match list into the inner area; sizing is reported via
/// the cap dims so the popup's auto-sizing reproduces the in-tree
/// rect.
struct SearchBody<'a> {
    state: &'a SearchOverlayState,
    items: &'a [SearchItem<'a>],
    content: Option<&'a ContentView>,
    theme: &'a Theme,
    inner_width: usize,
    inner_height: usize,
}

impl KnownSize for SearchBody<'_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for SearchBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // First row: the query input (rendered as plain text with a
        // leading `/` so the operator sees the live query). Reuse
        // TextInputState's value rather than instantiating its
        // widget, since we want the query and the result list in a
        // single bordered modal.
        let query_line = line![
            span!(Style::default().fg(self.theme.panel_focus_accent); "/"),
            self.state.query().to_string(),
        ];
        let query_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        Paragraph::new(query_line).render(query_area, buf);

        if self.state.scope() == SearchScope::Contents {
            self.render_contents(area, buf);
            return;
        }

        // Footer hints at the bottom, then the notice (if Tab was refused)
        // under the query, then the ranked list.
        let hint_rows = self.render_footer(
            area,
            buf,
            &crate::tui::keybindings::overlay::footer(
                crate::tui::keybindings::overlay::SEARCH_KEYS,
            ),
        );
        let notice_rows = u16::from(self.state.notice().is_some());
        if let Some(notice) = self.state.notice() {
            let rect = Rect {
                x: area.x,
                y: area.y.saturating_add(1),
                width: area.width,
                height: 1,
            };
            let text = crate::tui::widgets::popup_frame::ellipsize(notice, area.width as usize);
            Paragraph::new(line![
                span!(Style::default().fg(self.theme.warning); "{text}")
            ])
            .render(rect, buf);
        }
        let list_area = Rect {
            x: area.x,
            y: area.y.saturating_add(1 + notice_rows),
            width: area.width,
            height: area
                .height
                .saturating_sub(1 + notice_rows)
                .saturating_sub(hint_rows),
        };
        if self.state.matches().is_empty() {
            let label = if self.state.query().is_empty() {
                "(start typing)"
            } else {
                "(no matches)"
            };
            Paragraph::new(line![span!(self.theme.placeholder; "{label}")]).render(list_area, buf);
            return;
        }

        let visible_rows = list_area.height as usize;
        let scroll = compute_scroll(
            self.state.cursor(),
            visible_rows,
            self.state.matches().len(),
        );
        let mut lines: Vec<Line<'static>> = Vec::new();
        let id_to_item: std::collections::HashMap<&RowId, &SearchItem<'_>> =
            self.items.iter().map(|item| (&item.id, item)).collect();
        // Budget the snippet width against the modal so the line
        // (label + separator + snippet) fits without wrapping; the
        // label itself can be wide for verbose aliases, so the
        // snippet always reserves at least ~24 chars.
        let snippet_budget = (list_area.width as usize)
            .saturating_sub(8 /* prefix + label spacer + ellipsis */)
            .max(24);
        for idx in scroll..(scroll + visible_rows).min(self.state.matches().len()) {
            let m = &self.state.matches()[idx];
            let item = id_to_item.get(&m.id);
            let label = item.map_or_else(|| "<missing>".to_string(), |i| i.label.to_string());
            let is_cursor = idx == self.state.cursor();
            let line = build_match_line(
                label,
                item.copied(),
                m,
                is_cursor,
                snippet_budget,
                self.theme,
            );
            lines.push(line);
        }
        Paragraph::new(lines).render(list_area, buf);
    }
}

impl SearchBody<'_> {
    /// The contents scope: scan status, results, and the key hints.
    fn render_contents(&self, area: Rect, buf: &mut Buffer) {
        use crate::tui::keybindings::overlay::{SEARCH_CONTENT_KEYS, footer};
        use crate::tui::widgets::popup_frame::ellipsize;

        let width = area.width as usize;
        let empty = ContentView::default();
        let view = self.content.unwrap_or(&empty);

        let hint_rows = self.render_footer(area, buf, &footer(SEARCH_CONTENT_KEYS));
        let hint_top = area.y + area.height - hint_rows;

        // Status row: what is searched and how far the scan has got.
        let status = ellipsize(&self.status_text(view), width);
        let status_rect = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: 1,
        };
        Paragraph::new(line![span!(self.theme.placeholder; "{status}")]).render(status_rect, buf);

        let list_top = area.y.saturating_add(2);
        let list_height = hint_top.saturating_sub(list_top) as usize;
        let list_area = Rect {
            x: area.x,
            y: list_top,
            width: area.width,
            height: u16::try_from(list_height).unwrap_or(0),
        };
        if view.rows.is_empty() {
            let label = if let Some(reason) = view.unavailable {
                reason
            } else if self.state.content_text().is_empty() {
                "(type to search what was said)"
            } else if view.done {
                "(no sessions match)"
            } else {
                "(searching…)"
            };
            let label = ellipsize(label, width);
            Paragraph::new(line![span!(self.theme.placeholder; "{label}")]).render(list_area, buf);
            return;
        }
        let scroll = compute_scroll(self.state.cursor(), list_height, view.rows.len());
        let lines: Vec<Line<'static>> = (scroll..(scroll + list_height).min(view.rows.len()))
            .map(|idx| {
                content_row_line(
                    &view.rows[idx],
                    idx == self.state.cursor(),
                    width,
                    self.theme,
                )
            })
            .collect();
        Paragraph::new(lines).render(list_area, buf);
    }

    /// The key hints, wrapped to the modal and drawn on its last rows.
    /// Returns how many rows they took.
    fn render_footer(&self, area: Rect, buf: &mut Buffer, hints: &str) -> u16 {
        let lines = crate::tui::widgets::popup_frame::wrapped_lines(hints, 0, area.width as usize);
        let rows = lines
            .len()
            .min(4)
            .min(area.height.saturating_sub(3) as usize);
        let rows = u16::try_from(rows).unwrap_or(0);
        let top = area.y + area.height - rows;
        for (i, text) in lines.iter().take(usize::from(rows)).enumerate() {
            let rect = Rect {
                x: area.x,
                y: top + u16::try_from(i).unwrap_or(0),
                width: area.width,
                height: 1,
            };
            Paragraph::new(line![span!(self.theme.placeholder; "{text}")]).render(rect, buf);
        }
        rows
    }

    fn status_text(&self, view: &ContentView) -> String {
        if let Some(notice) = self.state.notice() {
            return notice.to_string();
        }
        let group = match self.state.group() {
            Grouping::Session => "results by session",
            Grouping::Message => "results by message",
        };
        let kinds = self.state.kinds();
        let mut what = vec!["messages"];
        if kinds.tools {
            what.push("tools");
        }
        if kinds.thinking {
            what.push("thinking");
        }
        let mut text = format!("{group} · {}", what.join(" + "));
        if view.total > 0 {
            if view.done {
                text.push_str(&format!(
                    " · {} {} searched",
                    view.total,
                    if view.total == 1 {
                        "session"
                    } else {
                        "sessions"
                    }
                ));
            } else {
                text.push_str(&format!(" · {}/{} sessions", view.scanned, view.total));
            }
            if view.skipped > 0 {
                text.push_str(&format!(" · {} skipped", view.skipped));
            }
        }
        text
    }
}

/// One contents result: cursor mark, harness chip, label, hit count,
/// and the snippet with the match marked.
fn content_row_line(
    row: &ContentRow,
    is_cursor: bool,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    use crate::tui::widgets::badge::harness_badge;
    use crate::tui::widgets::popup_frame::ellipsize;

    let mark = if is_cursor {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    };
    let plain = Style::default().add_modifier(mark);
    let dim = if is_cursor {
        plain
    } else {
        Style::default().add_modifier(theme.placeholder)
    };
    let mut spans: Vec<Span<'static>> = vec![
        span!(plain; "{}", if is_cursor { "> " } else { "  " }),
        harness_badge(&row.harness, theme),
        span!(plain; " "),
    ];
    let label = ellipsize(&row.label, 22);
    let mut used = 2 + 10 + 1 + label.chars().count();
    spans.push(span!(plain; "{label}"));
    let tag = match &row.turn {
        Some(turn) => turn.clone(),
        None => format!(
            "{} {}",
            row.hits,
            if row.hits == 1 { "hit" } else { "hits" }
        ),
    };
    spans.push(span!(dim; "  {tag}"));
    used += 2 + tag.chars().count();
    spans.push(span!(dim; "  "));
    used += 2;
    let budget = width.saturating_sub(used).max(8);
    let snippet = crate::tui::widgets::snippet::fit(
        &row.snippet,
        &row.highlight.clone().unwrap_or(0..0),
        budget,
    );
    match snippet.highlight.clone() {
        Some(range) => {
            let match_style = theme.match_style().add_modifier(mark);
            spans.push(span!(dim; "{}", &snippet.text[..range.start]));
            spans.push(span!(match_style; "{}", &snippet.text[range.clone()]));
            spans.push(span!(dim; "{}", &snippet.text[range.end..]));
        }
        None => spans.push(span!(dim; "{}", snippet.text)),
    }
    Line::from(spans)
}

/// Render a single match row with prefix, label, and a snippet of
/// the haystack around the matched range. The cursor row carries a
/// background highlight on every span; the matched bytes inside
/// the snippet are bolded so they stand out even on the highlighted
/// row.
fn build_match_line(
    label: String,
    item: Option<&SearchItem<'_>>,
    m: &SearchMatch,
    is_cursor: bool,
    snippet_budget: usize,
    theme: &Theme,
) -> Line<'static> {
    // Compose a per-span style. The cursor row uses
    // `Modifier::REVERSED` rather than an explicit background color
    // so the highlight adapts to whatever fg/bg the terminal theme
    // provides — a fixed dark-gray bg renders dark-on-dark on
    // light themes. Off-cursor rows dim the snippet context so the
    // matched portion pops; the cursor row skips DIM because the
    // reversed surface already separates it from neighbors.
    let span_style = |dim_when_not_cursor: bool, extra: Style| -> Style {
        let mut style = extra;
        if is_cursor {
            style = style.add_modifier(Modifier::REVERSED);
        } else if dim_when_not_cursor {
            style = style.add_modifier(theme.placeholder);
        }
        style
    };
    let prefix = if is_cursor { "> " } else { "  " };
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(span!(span_style(false, Style::default()); "{prefix}"));
    // Prepend a kind glyph so operators scan
    // results by symbol (`● session`, `▣ mux`, `⇄ pr`, …) instead
    // of relying on the textual `kind:` prefix some labels carry.
    // RowIds without a NodeKind mapping (Pin, Synthetic) get two
    // spaces so the label column stays aligned across the result
    // list — operators don't see the label jiggle row by row.
    spans.push(search_kind_glyph_span(&m.id, theme, is_cursor));
    spans.push(span!(span_style(false, Style::default()); "{}", label));

    // If we can show a snippet (haystack present and either
    // distinct from the label or carrying a match range), append
    // `· …<context>…` after the label so the operator sees *why*
    // the row matched.
    let haystack = item.map_or("", |i| i.haystack.as_ref());
    let matched_range = m.matched_range.clone().unwrap_or(0..0);
    let snippet_distinct = haystack != label;
    if !haystack.is_empty() && snippet_distinct {
        spans.push(span!(span_style(true, Style::default()); "  · "));
        let snippet = snippet_around(haystack, matched_range, snippet_budget);
        // The matched portion always renders bold + yellow so it
        // pops on both cursor and non-cursor rows; pre/post context
        // dims only when the row isn't the cursor.
        let match_style = span_style(false, theme.match_style());
        if let Some(range) = snippet.highlight.clone() {
            let pre = snippet.text[..range.start].to_string();
            let mid = snippet.text[range.clone()].to_string();
            let post = snippet.text[range.end..].to_string();
            let dim = span_style(true, Style::default());
            spans.push(span!(dim; "{pre}"));
            spans.push(span!(match_style; "{mid}"));
            spans.push(span!(dim; "{post}"));
        } else {
            spans.push(span!(span_style(true, Style::default()); "{}", snippet.text));
        }
    }

    Line::from(spans)
}

/// Resolve the NodeKind a search result row represents, when there
/// is one. `RowId::Pin` and `RowId::Synthetic` are not graph nodes
/// and return `None`; callers render two spaces in the glyph slot
/// to keep label-column alignment across the result list.
fn search_row_node_kind(id: &RowId) -> Option<NodeKind> {
    match id {
        RowId::Group(node_id) | RowId::AgentSession(node_id) | RowId::MuxSession(node_id) => {
            Some(NodeKind::from(node_id))
        }
        RowId::Hit { session, .. } => Some(NodeKind::from(session)),
        RowId::Pr(_) => Some(NodeKind::ForgePr),
        RowId::Fork(_) => Some(NodeKind::Fork),
        RowId::Pin { .. } => Some(NodeKind::Pin),
        RowId::Nested { node, .. } => Some(NodeKind::from(node)),
        RowId::History { .. } | RowId::Remote(_) | RowId::Bucket(_) | RowId::Synthetic(_) => None,
    }
}

/// Build the leading glyph span for a search result row. Two cells
/// wide: `<glyph> ` for graph-backed rows; `  ` for Pin / Synthetic
/// rows that don't carry a NodeKind. The glyph keeps its kind color
/// even on the cursor row — the surrounding `REVERSED` modifier on
/// the prefix and label already separates the highlighted row from
/// its neighbors, so leaving the color intact keeps the symbol's
/// identity legible.
fn search_kind_glyph_span(id: &RowId, theme: &Theme, is_cursor: bool) -> Span<'static> {
    let Some(kind) = search_row_node_kind(id) else {
        let style = if is_cursor {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        return span!(style; "  ");
    };
    let style = node_kind_style(kind, theme);
    // ForgePr's slate color is `Color::Reset`; mirror the dodge
    // `kind_chip_span` and the breadcrumb renderer use — fall back
    // to `theme.pr_open` since search results don't carry PR state.
    let color = if matches!(kind, NodeKind::ForgePr) {
        theme.pr_open
    } else {
        style.color
    };
    let mut span_style = Style::default().fg(color);
    if is_cursor {
        span_style = span_style.add_modifier(Modifier::REVERSED);
    }
    span!(span_style; "{} ", style.glyph)
}

fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(70, area.width.saturating_sub(4)).max(30);
    let max_height = area.height.saturating_sub(4);
    let height = max_height.clamp(8, 20.max(max_height));
    super::popup_frame::centered_rect(area, width, height)
}

fn compute_scroll(cursor: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if cursor >= visible_rows {
        cursor + 1 - visible_rows
    } else {
        0
    }
}

/// Per-event context the search overlay reads through the
/// [`crate::tui::Overlay`] trait. Carries the current
/// visible-row items so the widget can rerank matches on every
/// keystroke.
pub struct SearchContext<'a> {
    pub items: &'a [SearchItem<'a>],
    /// The contents results, in the order the overlay lists them.
    pub content: &'a [ContentRow],
}

impl crate::tui::Overlay for SearchOverlayState {
    type Ctx<'a> = SearchContext<'a>;

    fn handle(&mut self, ctx: SearchContext<'_>, key: KeyEvent) -> crate::tui::OverlayOutcome {
        self.set_content_len(ctx.content.len());
        let outcome = self.handle_key(key);
        self.refresh_matches(ctx.items);
        match outcome {
            SearchOutcome::Continue => crate::tui::OverlayOutcome::Consumed,
            SearchOutcome::Cancel => crate::tui::OverlayOutcome::Close,
            SearchOutcome::Confirm(id) => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::SelectRow(id)))
            }
            SearchOutcome::OpenHit(index) => match ctx.content.get(index) {
                Some(row) => {
                    crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::OpenContentHit {
                        session: row.session.clone(),
                        text: self.content_text(),
                        turn_index: row.turn_index,
                    }))
                }
                None => crate::tui::OverlayOutcome::Close,
            },
            SearchOutcome::Promote { show_results } => {
                crate::tui::OverlayOutcome::Commit(Box::new(crate::tui::Msg::ApplyContentSearch {
                    text: self.content_text(),
                    kinds: self.kinds,
                    group: self.group,
                    show_results,
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "search_content_tests.rs"]
mod content_tests;
