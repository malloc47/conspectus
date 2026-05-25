//! `/` in-view search overlay (T8-017).
//!
//! Reuses the ADR 0030 text-input primitive for the query line and
//! displays a ranked match list below. The backend is pluggable via
//! [`crate::tui::search::SearchBackend`] so swapping in a fuzzy
//! ranker later requires only a new implementation, not a
//! renderer change.
//!
//! Per ADR 0031 the overlay ranks within the active filter set —
//! the host runtime hands the overlay only the visible row tree's
//! [`SearchItem`]s, which have already had filters applied
//! upstream.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::Theme;

use crate::tui::rows::RowId;
use crate::tui::search::{SearchBackend, SearchItem, SearchMatch, snippet_around};
use crate::tui::widgets::input::TextInputState;

/// Pure state for the search overlay. Holds the query text input,
/// the latest match list (recomputed by the host whenever the
/// query changes), and the cursor inside that list.
#[derive(Debug, Clone)]
pub struct SearchOverlayState {
    input: TextInputState,
    matches: Vec<SearchMatch>,
    cursor: usize,
}

impl SearchOverlayState {
    pub fn new() -> Self {
        Self {
            input: TextInputState::new(" search ", String::new()),
            matches: Vec::new(),
            cursor: 0,
        }
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
    pub fn refresh_matches(&mut self, backend: &dyn SearchBackend, items: &[SearchItem<'_>]) {
        self.matches = backend.rank(self.input.value(), items);
        if self.cursor >= self.matches.len() {
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
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return SearchOutcome::Cancel;
        }
        match event.code {
            KeyCode::Esc => SearchOutcome::Cancel,
            KeyCode::Enter => match self.matches.get(self.cursor) {
                Some(m) => SearchOutcome::Confirm(Box::new(m.id.clone())),
                None => SearchOutcome::Cancel,
            },
            KeyCode::Down => {
                self.move_cursor(1);
                SearchOutcome::Continue
            }
            KeyCode::Up => {
                self.move_cursor(-1);
                SearchOutcome::Continue
            }
            KeyCode::Char('n') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(1);
                SearchOutcome::Continue
            }
            KeyCode::Char('p') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_cursor(-1);
                SearchOutcome::Continue
            }
            _ => {
                // Let the text-input primitive handle the key. We
                // ignore its outcome here because Enter/Esc are
                // intercepted above; everything else is "still
                // editing the query."
                let _ = self.input.handle_key(event);
                SearchOutcome::Continue
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.matches.is_empty() {
            self.cursor = 0;
            return;
        }
        let len = self.matches.len() as i32;
        let mut next = self.cursor as i32 + delta;
        next = ((next % len) + len) % len;
        self.cursor = next as usize;
    }
}

/// Centered modal that renders the query line on top and a
/// ranked match list below. Caps the visible match list at the
/// available height; the cursor scrolls within it.
pub struct SearchOverlayWidget<'a> {
    state: &'a SearchOverlayState,
    items: &'a [SearchItem<'a>],
    theme: &'a Theme,
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
        }
    }
}

impl Widget for SearchOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area);
        // Repaint so dimmed body content doesn't bleed through.
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(format!(
                " / search ({} matches) ",
                self.state.matches().len()
            )));
        let inner = block.inner(modal);
        block.render(modal, buf);

        // First row: the query input (rendered as plain text with a
        // leading `/` so the operator sees the live query). Reuse
        // TextInputState's value rather than instantiating its
        // widget, since we want the query and the result list in a
        // single bordered modal.
        let query_line = Line::from(vec![
            Span::styled("/", Style::default().fg(self.theme.panel_focus_accent)),
            Span::raw(self.state.query().to_string()),
        ]);
        let query_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        Paragraph::new(query_line).render(query_area, buf);

        // Divider + ranked list.
        let list_area = Rect {
            x: inner.x,
            y: inner.y.saturating_add(1),
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        if self.state.matches().is_empty() {
            let label = if self.state.query().is_empty() {
                "(start typing)"
            } else {
                "(no matches)"
            };
            Paragraph::new(Line::from(Span::styled(
                label,
                Style::default().add_modifier(self.theme.placeholder),
            )))
            .render(list_area, buf);
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
            let label = item
                .map(|i| i.label.to_string())
                .unwrap_or_else(|| "<missing>".to_string());
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
    spans.push(Span::styled(
        prefix.to_string(),
        span_style(false, Style::default()),
    ));
    spans.push(Span::styled(
        label.clone(),
        span_style(false, Style::default()),
    ));

    // If we can show a snippet (haystack present and either
    // distinct from the label or carrying a match range), append
    // `· …<context>…` after the label so the operator sees *why*
    // the row matched.
    let haystack = item.map(|i| i.haystack.as_ref()).unwrap_or("");
    let matched_range = m.matched_range.clone().unwrap_or(0..0);
    let snippet_distinct = haystack != label;
    if !haystack.is_empty() && snippet_distinct {
        spans.push(Span::styled(
            "  · ".to_string(),
            span_style(true, Style::default()),
        ));
        let snippet = snippet_around(haystack, matched_range, snippet_budget);
        // The matched portion always renders bold + yellow so it
        // pops on both cursor and non-cursor rows; pre/post context
        // dims only when the row isn't the cursor.
        let match_style = span_style(
            false,
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        );
        if let Some(range) = snippet.highlight.clone() {
            let pre = snippet.text[..range.start].to_string();
            let mid = snippet.text[range.start..range.end].to_string();
            let post = snippet.text[range.end..].to_string();
            spans.push(Span::styled(pre, span_style(true, Style::default())));
            spans.push(Span::styled(mid, match_style));
            spans.push(Span::styled(post, span_style(true, Style::default())));
        } else {
            spans.push(Span::styled(
                snippet.text,
                span_style(true, Style::default()),
            ));
        }
    }

    Line::from(spans)
}

fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(70, area.width.saturating_sub(4)).max(30);
    let max_height = area.height.saturating_sub(4);
    let height = max_height.clamp(8, 20.max(max_height));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentSessionId, NodeId};
    use crate::tui::rows::{AgentSessionRow, MuxIndicator, Row, RowKind};
    use crate::tui::search::{SubstringBackend, items_from_rows};
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn agent_row(key: &str, alias: Option<&str>) -> Row {
        let session_id = AgentSessionId::new("claude-code", "/state", key);
        let primary = NodeId::AgentSession(session_id.clone());
        Row {
            id: RowId::AgentSession(primary.clone()),
            depth: 1,
            expandable: false,
            kind: RowKind::AgentSession(AgentSessionRow {
                session: session_id,
                short_id: key.to_string(),
                harness_label: "claude-code".to_string(),
                cwd_display: Some("~/proj".to_string()),
                project_display: None,
                recency: None,
                activity_epoch: None,
                mux_state: MuxIndicator::Unmuxed,
                preview: None,
                title: None,
                alias: alias.map(str::to_string),
                primary_node: primary,
            }),
        }
    }

    #[test]
    fn typing_query_filters_matches() {
        let rows = vec![
            agent_row("a", Some("puffin")),
            agent_row("b", Some("other")),
        ];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        // Type "puf".
        for c in "puf".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        assert_eq!(state.matches().len(), 1);
        assert_eq!(state.matches()[0].id, rows[0].id);
    }

    #[test]
    fn enter_confirms_with_cursor_match() {
        let rows = vec![agent_row("a", Some("puffin")), agent_row("b", Some("puff"))];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puf".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        // Cursor at 0; Enter confirms the first match.
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(
            outcome,
            SearchOutcome::Confirm(Box::new(rows[0].id.clone()))
        );
    }

    #[test]
    fn arrow_down_moves_cursor_within_matches() {
        let rows = vec![agent_row("a", Some("puffin")), agent_row("b", Some("puff"))];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puf".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        state.handle_key(key(KeyCode::Down));
        assert_eq!(state.cursor(), 1);
        // Wraps.
        state.handle_key(key(KeyCode::Down));
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn enter_on_empty_match_list_cancels() {
        let rows = vec![agent_row("a", None)];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "no-such-match".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        assert!(state.matches().is_empty());
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(outcome, SearchOutcome::Cancel);
    }

    #[test]
    fn esc_cancels() {
        let mut state = SearchOverlayState::new();
        let outcome = state.handle_key(key(KeyCode::Esc));
        assert_eq!(outcome, SearchOutcome::Cancel);
    }

    #[test]
    fn ctrl_n_p_navigate_results() {
        let rows = vec![agent_row("a", Some("puff")), agent_row("b", Some("puffin"))];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puf".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        let ctrl_n = KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        state.handle_key(ctrl_n);
        assert_eq!(state.cursor(), 1);
        let ctrl_p = KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        state.handle_key(ctrl_p);
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn match_line_includes_snippet_with_matched_bytes_highlighted() {
        // Agent row whose alias is short but whose preview contains
        // the actual match — the rendered line should expose the
        // matching preview snippet alongside the alias.
        let row = agent_row("nice", Some("nice"));
        let mut row_with_preview = row.clone();
        if let crate::tui::rows::RowKind::AgentSession(s) = &mut row_with_preview.kind {
            s.preview = Some("lots of stuff and then puffin shows up here".to_string());
        }
        let items = items_from_rows(std::slice::from_ref(&row_with_preview));
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puffin".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        let m = &state.matches()[0];
        let theme = Theme::default();
        let line = build_match_line(
            items[0].label.to_string(),
            Some(&items[0]),
            m,
            true,
            40,
            &theme,
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(rendered.contains("nice"), "label rendered: {rendered}");
        assert!(rendered.contains("puffin"), "snippet rendered: {rendered}");
        // The matched portion should land in its own span so the
        // renderer can style it.
        let has_match_span = line.spans.iter().any(|s| s.content.as_ref() == "puffin");
        assert!(has_match_span, "match span missing in: {:?}", line.spans);
    }

    #[test]
    fn match_line_skips_snippet_when_label_equals_haystack() {
        // Group rows have label == haystack — a snippet would just
        // duplicate the label, so we omit it.
        use crate::tui::rows::{GroupRow, Row, RowKind};
        let row = Row {
            id: RowId::Synthetic("g"),
            depth: 0,
            expandable: true,
            kind: RowKind::Group(GroupRow {
                display_path: "puffin/dir".to_string(),
                primary_node: None,
                is_launch_context: false,
            }),
        };
        let items = items_from_rows(std::slice::from_ref(&row));
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puffin".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        let m = &state.matches()[0];
        let theme = Theme::default();
        let line = build_match_line(
            items[0].label.to_string(),
            Some(&items[0]),
            m,
            false,
            40,
            &theme,
        );
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        // No `· ` separator means no duplicate snippet.
        assert!(!rendered.contains("  · "), "rendered: {rendered}");
        // Label still appears.
        assert!(rendered.contains("puffin/dir"));
    }

    #[test]
    fn refresh_matches_resets_cursor_when_truncated() {
        let rows = vec![agent_row("a", Some("puff")), agent_row("b", Some("puffin"))];
        let items = items_from_rows(&rows);
        let mut state = SearchOverlayState::new();
        let backend = SubstringBackend;
        for c in "puf".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        state.handle_key(key(KeyCode::Down));
        assert_eq!(state.cursor(), 1);
        // Narrow the query so only one match survives — cursor
        // shouldn't dangle past the new end.
        for c in "fin".chars() {
            state.handle_key(key(KeyCode::Char(c)));
            state.refresh_matches(&backend, &items);
        }
        assert_eq!(state.matches().len(), 1);
        assert_eq!(state.cursor(), 0);
    }
}
