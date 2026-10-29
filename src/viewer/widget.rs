//! The transcript as rows, and as the framework pager's content.
//!
//! A [`Transcript`] lays its turns out as rows (`tuikit::pager`'s
//! [`PagerContent`]): a gutter chip per turn and a thread beside its
//! body, the bodies rendered from Markdown, tables, and tool output,
//! and only the turns the toggles show. The pager draws them in its
//! full-screen chrome:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ <harness>:<key> · <cwd> · N turns                           │  header
//! │─────────────────────────────────────────────────────────────│  rule
//! │                                                             │
//! │       you │ what's up?                                      │  body
//! │           │                                                 │
//! │        ai │ Not much.                                       │
//! │           │                                                 │
//! │  thinking │ <italic dim thinking text>                      │
//! │      tool │ Read: /path                                     │
//! │  ↳ result │ <dim tool output>                               │
//! │                                                             │
//! │─────────────────────────────────────────────────────────────│  rule
//! │ [ 42/137 ] · tools·off (t) · think·off (T) · … · Esc/q close │  footer
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! The pager owns the scrolling, the find prompt, the help page, and the
//! footer; the content owns the rows, the header, the toggles' chips and
//! keys, a find whose unit is a turn, and the count of matches the
//! toggles hide. Per ADR 0052 the renderer never branches on harness; it
//! consumes only the normalized model.

use std::borrow::Cow;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::chip::{PositionChip, StatusChip, ToggleChip, Tone};
use tuikit::glyphs::marks;
use tuikit::keys::{KeyRow, PagerKey};
use tuikit::pager::{Find, PagerContent, PagerEffect};
use tuikit::theme::Theme;

use crate::viewer::highlight::{highlight_matches, paint_turns};
use crate::viewer::keys::{TRANSCRIPT_KEYS, TranscriptKey};
use crate::viewer::model::{TranscriptTurn, TurnKind};
use crate::viewer::render::{
    ToolCategory, aggregate_tool_phrase, categorize_tool_name, chip_label, chip_label_width,
    into_owned_line, leader_width, render_aggregated_tool_summary, render_turn,
    tool_name_from_body,
};
use crate::viewer::search::{ContentKinds, ContentQuery, matching_turns};
use crate::viewer::state::{RenderCache, ToolDetail, Transcript, TurnStart, ViewerState};

impl<M> PagerContent for Transcript<M> {
    type Message = M;
    type Action = TranscriptKey;
    const KEYS: &'static [KeyRow<TranscriptKey>] = TRANSCRIPT_KEYS;

    fn layout(&mut self, width: u16, theme: &Theme) -> usize {
        self.lines(theme, width).len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.rendered
            .as_ref()
            .map_or_else(Vec::new, |cache| cache.lines[range].to_vec())
    }

    /// ` <harness>:<key> · <cwd> · N turns`, the title in the named style
    /// the opener chose; just the title when the rest does not fit.
    fn header(&self, width: usize, theme: &Theme) -> Line<'static> {
        let meta = &self.document.meta;
        let title = Span::styled(
            format!("{}:{}", meta.harness, meta.session_key),
            title_style(&self.title_role, theme),
        );
        let count = self.document.turns.len();
        let turns = if count == 1 { "turn" } else { "turns" };
        let rest = Tone::Inert.style(theme);
        let full = vec![
            Span::raw(" "),
            title.clone(),
            Span::styled(" · ", rest),
            Span::styled(
                meta.cwd.clone().unwrap_or_else(|| "(no cwd)".to_string()),
                Style::new().fg(theme.accent).add_modifier(theme.muted),
            ),
            Span::styled(format!(" · {count} {turns}"), rest),
        ];
        if tuikit::text::spans_width(&full) <= width {
            Line::from(full)
        } else {
            Line::from(vec![Span::raw(" "), title])
        }
    }

    /// The toggles' chips, each naming its key (`tools·sum (t)`): tool
    /// detail, thinking, aborted turns, and, for a transcript whose parser
    /// marked turns hidden, hidden records. A narrow footer drops them
    /// from the last.
    fn chips(&self, theme: &Theme) -> Vec<Span<'static>> {
        let tool_states = ToolDetail::ALL.map(ToolDetail::footer_label);
        let mut chips = vec![
            tool_chip(self.tool_detail, &tool_states).span(theme),
            think_chip(self.show_thinking).span(theme),
            abort_chip(self.show_aborted).span(theme),
        ];
        // The hidden chip only appears for transcripts whose parser marked
        // turns hidden; for the rest it would toggle nothing.
        if self.document.turns.iter().any(|turn| turn.hidden) {
            chips.push(hidden_chip(self.show_hidden).span(theme));
        }
        chips
    }

    /// The turns the toggles show that hold `query`: a match is a turn.
    fn find(&self, query: &str) -> Vec<usize> {
        self.matching(query)
    }

    fn match_rows(&self, turn: usize) -> Range<usize> {
        self.rows_of(turn)
    }

    /// Every match after the gutter in the theme's match look, and the
    /// current turn's in the current-match look; a tool run collapsed to
    /// one summary line is a count, not the turns' text, so it is not
    /// painted.
    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
        if let Some(cache) = self.rendered.as_ref() {
            let holds = |turn: usize| find.holds(turn);
            paint_turns(
                rows,
                first,
                cache,
                find.query(),
                holds,
                find.current_unit(),
                theme,
            );
        }
    }

    /// How many turns the toggles hide hold `query` (`2 in hidden
    /// turns`), when any do.
    fn find_detail(&self, query: &str) -> Option<String> {
        let hidden = self.hidden_matches(query);
        (hidden > 0).then(|| format!("{hidden} in hidden turns"))
    }

    fn describe(key: PagerKey) -> Option<&'static str> {
        crate::viewer::help::describe(key)
    }

    fn empty(&self) -> Cow<'_, str> {
        Cow::Borrowed(if self.document.is_empty() {
            "transcript unavailable"
        } else {
            "every turn here is hidden"
        })
    }

    /// A toggle shows or hides turns, so the rows are laid out again and
    /// the find stays on its turn while it is still shown.
    fn apply(&mut self, key: TranscriptKey) -> PagerEffect<M> {
        match key {
            TranscriptKey::Tools => self.tool_detail = self.tool_detail.cycle(),
            TranscriptKey::Thinking => self.show_thinking = !self.show_thinking,
            TranscriptKey::Aborted => self.show_aborted = !self.show_aborted,
            TranscriptKey::Hidden => self.show_hidden = !self.show_hidden,
        }
        PagerEffect::Relayout
    }
}

impl<M> Transcript<M> {
    /// Compose the transcript body at `width` columns, gutter included,
    /// and return its lines. The composition is cached and reused while
    /// the width and the display toggles are unchanged, so a draw that
    /// only scrolls slices rather than re-renders. The full-screen viewer
    /// and the inline preview (ADR 0112) both draw from it. An empty
    /// document composes no lines.
    pub fn lines(&mut self, theme: &Theme, width: u16) -> &[Line<'static>] {
        if self.document.is_empty() {
            return &[];
        }
        let gutter_inner_width = self.gutter_inner_width();
        let content_width = width.saturating_sub(leader_width(gutter_inner_width));

        if !self.cache_is_current(content_width) {
            let mut turn_starts = Vec::new();
            let owned: Vec<Line<'static>> = self
                .build_body_lines(theme, content_width, gutter_inner_width, &mut turn_starts)
                .into_iter()
                .map(into_owned_line)
                .collect();
            self.rendered = Some(RenderCache {
                content_width,
                tool_detail: self.tool_detail,
                show_thinking: self.show_thinking,
                show_aborted: self.show_aborted,
                show_hidden: self.show_hidden,
                lines: owned,
                turn_starts,
                leader: usize::from(leader_width(gutter_inner_width)),
            });
        }
        &self.rendered.as_ref().expect("just populated").lines
    }

    /// The lines [`Self::lines`] last composed, when they were composed
    /// at `width` with the current display toggles. Lets a caller that
    /// only holds `&Transcript` reuse the cache it warmed earlier in the
    /// frame.
    pub fn cached_lines(&self, width: u16) -> Option<&[Line<'static>]> {
        let content_width = width.saturating_sub(leader_width(self.gutter_inner_width()));
        if !self.cache_is_current(content_width) {
            return None;
        }
        self.rendered.as_ref().map(|cache| cache.lines.as_slice())
    }

    /// The kinds of turn the toggles show, which is what a find searches.
    pub fn shown_kinds(&self) -> ContentKinds {
        ContentKinds {
            messages: true,
            tools: self.tool_detail.is_visible(),
            thinking: self.show_thinking,
            aborted: self.show_aborted,
            hidden: self.show_hidden,
        }
    }

    /// The shown turns that hold `query`, in order (the shared content
    /// matcher, so a find agrees with content search about what a match
    /// is).
    pub fn matching(&self, query: &str) -> Vec<usize> {
        matching_turns(
            &self.document,
            &ContentQuery::new(query, self.shown_kinds()),
        )
        .collect()
    }

    /// How many turns the toggles hide hold `query`.
    pub fn hidden_matches(&self, query: &str) -> usize {
        let everything = ContentQuery::new(query, ContentKinds::EVERYTHING);
        matching_turns(&self.document, &everything)
            .count()
            .saturating_sub(self.matching(query).len())
    }

    /// The rows turn `turn` takes in the last composition: from its first
    /// line up to the next turn's (a collapsed tool run's turns share its
    /// summary line). A turn the toggles hide takes none, where the next
    /// shown turn starts.
    pub fn rows_of(&self, turn: usize) -> Range<usize> {
        let Some(cache) = self.rendered.as_ref() else {
            return 0..0;
        };
        let starts = &cache.turn_starts;
        let at = starts.partition_point(|start| start.turn_index < turn);
        let Some(start) = starts.get(at) else {
            return cache.lines.len()..cache.lines.len();
        };
        if start.turn_index != turn {
            return start.line..start.line;
        }
        let end = starts[at..]
            .iter()
            .find(|next| next.line > start.line)
            .map_or(cache.lines.len(), |next| next.line);
        start.line..end
    }

    /// The inner gutter chip-label width: the widest label among the
    /// shown turns, so the gutter shrinks when wider labels (e.g.
    /// `thinking`) are toggled off.
    fn gutter_inner_width(&self) -> usize {
        let mut max_width = 0usize;
        for turn in &self.document.turns {
            if !self.passes_turn_filters(turn) || !self.is_visible(turn.kind) {
                continue;
            }
            let label = chip_label(turn);
            max_width = max_width.max(chip_label_width(label));
        }
        max_width.max(1)
    }

    /// Whether the cached composition matches `content_width` and the
    /// current display toggles. This is the difference between scrolling
    /// by keystroke being O(turns × markdown render) and being O(1) on a
    /// large session with tools shown.
    fn cache_is_current(&self, content_width: u16) -> bool {
        self.rendered.as_ref().is_some_and(|cache| {
            cache.content_width == content_width
                && cache.tool_detail == self.tool_detail
                && cache.show_thinking == self.show_thinking
                && cache.show_aborted == self.show_aborted
                && cache.show_hidden == self.show_hidden
        })
    }

    /// Build the flat body line list. Filters out tool turns at
    /// `ToolDetail::Hidden` and thinking turns when `show_thinking`
    /// is off. At `ToolDetail::Summary` consecutive runs of tool
    /// turns are aggregated into a single claude-history-style line
    /// ("Read 2 files, ran 4 shell commands, edited 2 files") rather
    /// than emitting one chip per call. Other tool-detail levels
    /// affect how each turn renders — that's `render_turn`'s job.
    fn build_body_lines<'a>(
        &'a self,
        theme: &Theme,
        content_width: u16,
        gutter_inner_width: usize,
        turn_starts: &mut Vec<TurnStart>,
    ) -> Vec<Line<'a>> {
        let mut lines: Vec<Line<'a>> = Vec::new();
        let turns = &self.document.turns;
        let mut i = 0;
        while i < turns.len() {
            let turn = &turns[i];
            if !self.passes_turn_filters(turn) || !self.is_visible(turn.kind) {
                i += 1;
                continue;
            }
            // Summary mode: walk the contiguous tool-turn run and
            // emit one aggregated line for the whole run.
            if self.tool_detail == ToolDetail::Summary && is_tool_kind(turn.kind) {
                let (counts, run_len) = self.collect_tool_run(&turns[i..]);
                for offset in 0..run_len.max(1) {
                    if self.passes_turn_filters(&turns[i + offset]) {
                        turn_starts.push(TurnStart {
                            turn_index: i + offset,
                            line: lines.len(),
                            collapsed: true,
                        });
                    }
                }
                if !counts.is_empty() {
                    let phrase = aggregate_tool_phrase(&counts);
                    let owned: Vec<Line<'static>> = render_aggregated_tool_summary(
                        &phrase,
                        theme,
                        content_width,
                        gutter_inner_width,
                    );
                    lines.extend(owned);
                }
                i += run_len.max(1);
                continue;
            }
            turn_starts.push(TurnStart {
                turn_index: i,
                line: lines.len(),
                collapsed: false,
            });
            lines.extend(render_turn(
                turn,
                theme,
                content_width,
                self.tool_detail,
                gutter_inner_width,
            ));
            i += 1;
        }
        lines
    }

    /// Count consecutive tool turns by category. Only [`TurnKind::ToolUse`]
    /// turns contribute to the count (one call = one ToolUse in our
    /// model); the paired ToolResult lives in the same run but isn't
    /// double-counted.
    fn collect_tool_run(
        &self,
        turns: &[TranscriptTurn],
    ) -> (std::collections::BTreeMap<ToolCategory, usize>, usize) {
        let mut counts: std::collections::BTreeMap<ToolCategory, usize> =
            std::collections::BTreeMap::new();
        let mut end = 0usize;
        for turn in turns {
            if !is_tool_kind(turn.kind) {
                break;
            }
            // Skip aborted tool turns when the filter is on so the
            // aggregate phrase reflects what the user saw, not the
            // on-disk count.
            if !self.passes_turn_filters(turn) {
                end += 1;
                continue;
            }
            if matches!(turn.kind, TurnKind::ToolUse) {
                let name = tool_name_from_body(&turn.body);
                *counts.entry(categorize_tool_name(name)).or_insert(0) += 1;
            }
            end += 1;
        }
        (counts, end)
    }

    fn is_visible(&self, kind: TurnKind) -> bool {
        match kind {
            TurnKind::Message | TurnKind::CompactionSummary => true,
            TurnKind::ToolUse | TurnKind::ToolResult => self.tool_detail.is_visible(),
            TurnKind::Thinking => self.show_thinking,
        }
    }

    /// Per-turn filter applied before any kind-based visibility check.
    /// With `show_aborted` and `show_hidden` off (the default), turns the
    /// parser tagged `aborted` or `hidden` drop out — matching what the
    /// agent's UI showed the user. The toggles restore the full on-disk
    /// transcript.
    fn passes_turn_filters(&self, turn: &TranscriptTurn) -> bool {
        (self.show_aborted || !turn.aborted) && (self.show_hidden || !turn.hidden)
    }
}

fn is_tool_kind(k: TurnKind) -> bool {
    matches!(k, TurnKind::ToolUse | TurnKind::ToolResult)
}

/// `tools·sum (t)` / `tools·off (t)`: the current tool detail level and
/// its cycle key, the level padded to the widest of `states` so the chip
/// keeps its width as it cycles. On while tool turns show any body.
fn tool_chip<'a>(detail: ToolDetail, states: &'a [&'a str]) -> ToggleChip<'a> {
    ToggleChip::new("tools", detail.footer_label(), detail.is_visible())
        .states(states)
        .key("t")
}

/// `think·on  (T)` / `think·off (T)`. The key binding is inlined so the
/// operator doesn't need a separate hint to know how to flip it.
fn think_chip(on: bool) -> ToggleChip<'static> {
    ToggleChip::new("think", if on { "on" } else { "off" }, on)
        .states(&["on", "off"])
        .key("T")
}

/// `aborted·hide (I)` / `aborted·show (I)`. Uses `hide`/`show` instead
/// of `on`/`off` because the operator question isn't "is the flag on"
/// but "am I looking at the on-disk reality or the harness-faithful
/// view".
fn abort_chip(showing: bool) -> ToggleChip<'static> {
    show_hide_chip("aborted", showing, "I")
}

/// `hidden·hide (H)` / `hidden·show (H)` for records the harness's UI
/// never showed (ADR 0122). Same `hide`/`show` wording as the aborted
/// chip.
fn hidden_chip(showing: bool) -> ToggleChip<'static> {
    show_hide_chip("hidden", showing, "H")
}

fn show_hide_chip(name: &'static str, showing: bool, key: &'static str) -> ToggleChip<'static> {
    ToggleChip::new(name, if showing { "show" } else { "hide" }, showing)
        .states(&["show", "hide"])
        .key(key)
}

/// The title's look: bold, in the named style the opener chose for it.
fn title_style(role: &str, theme: &Theme) -> Style {
    Style::new()
        .fg(crate::viewer::theme::title(theme, role))
        .add_modifier(Modifier::BOLD)
}

// ---------------------------------------------------------------------
// The viewer's own chrome, until the TUI runs it on the pager.
// ---------------------------------------------------------------------

/// Draw the transcript viewer onto `area`. Mutates `state` to
/// record the layout it observed (viewport height, total line
/// count) so the reducer can use those numbers on the next key
/// event.
pub fn draw(state: &mut ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // separator
        Constraint::Min(1),    // body
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(area);

    let header = state.transcript.header(usize::from(area.width), theme);
    Widget::render(Paragraph::new(header), chunks[0], buf);
    draw_separator(theme, buf, chunks[1]);
    draw_body(state, theme, buf, chunks[2]);
    draw_separator(theme, buf, chunks[3]);
    draw_footer(state, theme, buf, chunks[4]);
}

fn draw_separator(theme: &Theme, buf: &mut Buffer, area: Rect) {
    let rule: String = marks::RULE.repeat(area.width as usize);
    Widget::render(
        Paragraph::new(Line::from(Span::styled(rule, Tone::Inert.style(theme)))),
        area,
        buf,
    );
}

fn draw_body(state: &mut ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    // Empty-document banner short-circuits before we ever touch the
    // turn list.
    if state.document.is_empty() {
        state.viewport_height = area.height;
        state.total_lines = 0;
        state.scroll_offset = 0;
        let banner = Line::from(Span::styled(
            "transcript unavailable",
            Style::new().add_modifier(theme.muted),
        ));
        let rows = tuikit::text::wrap_line(&banner, usize::from(area.width), 0);
        Widget::render(Paragraph::new(rows), area, buf);
        return;
    }

    let total = state.transcript.lines(theme, area.width).len();
    // A find that asked to scroll to a turn lands on its first line, one
    // line down so the turn's neighbor above shows.
    if let Some(turn) = state.find.pending_scroll.take()
        && let Some(cache) = state.transcript.rendered.as_ref()
        && let Some(start) = cache.turn_starts.iter().find(|s| s.turn_index == turn)
    {
        state.scroll_offset = start.line.saturating_sub(1);
        state.stick_to_end = false;
    }
    let stick_to_end = state.stick_to_end;
    let requested_offset = state.scroll_offset;
    let viewport_height = area.height;
    let max_offset = total.saturating_sub(viewport_height as usize);
    let scroll_offset = if stick_to_end {
        max_offset
    } else {
        requested_offset.min(max_offset)
    };

    // The cached lines are already wrapped to `content_width`, so the
    // visible window is a plain slice. Slicing (rather than
    // `Paragraph::scroll`, whose offset is a u16) keeps transcripts
    // longer than 65,535 lines scrollable and clones only what is shown.
    let visible_end = total.min(scroll_offset + viewport_height as usize);
    let cache = state.transcript.rendered.as_ref().expect("composed above");
    let mut visible = cache.lines[scroll_offset..visible_end].to_vec();
    highlight_matches(&mut visible, scroll_offset, cache, &state.find, theme);
    Widget::render(Paragraph::new(visible), area, buf);

    state.viewport_height = viewport_height;
    state.total_lines = total;
    state.scroll_offset = scroll_offset;
}

fn draw_footer(state: &ViewerState, theme: &Theme, buf: &mut Buffer, area: Rect) {
    if let Some(prompt) = &state.find.prompt {
        draw_find_prompt(prompt, &state.find_hint, theme, buf, area);
        return;
    }
    let position = PositionChip::new(state.scroll_offset, state.total_lines);

    let off_style = Tone::Inert.style(theme);
    let pipe = Span::styled(" · ", off_style);

    // Toggle chips inline the binding letter so the key + label
    // travel together (`tools·sum (t)`). Keeps the operator's eyes
    // on one column instead of cross-referencing a separate
    // shortcut hint.
    let tool_states = ToolDetail::ALL.map(ToolDetail::footer_label);
    let tools = tool_chip(state.tool_detail, &tool_states);
    let thinking = think_chip(state.show_thinking);
    let aborted = abort_chip(state.show_aborted);
    let hidden = hidden_chip(state.show_hidden);

    // Long hint trails. `?` for help is its own chip so it stands
    // out; `q close` is the last item per the operator-preferred
    // ordering (close should sit where the muscle memory lands
    // when ready to exit).
    let help_chip = "(?) help";
    let close_chip = "(q) close";
    let scroll_hint = "j/k scroll · g/G top/end · PgUp/PgDn";

    // The hidden chip only appears for transcripts whose parser marked
    // turns hidden; for the rest it would toggle nothing.
    let has_hidden = state.document.turns.iter().any(|turn| turn.hidden);

    let find_tone = if state.find.is_active() && state.find.matches.is_empty() {
        Tone::Warning
    } else {
        Tone::Success
    };
    let find = find_chip_text(state).map(|text| StatusChip::new(text, find_tone).bold());
    let mut spans: Vec<Span<'_>> = vec![
        position.span(),
        pipe.clone(),
        tools.span(theme),
        pipe.clone(),
        thinking.span(theme),
        pipe.clone(),
        aborted.span(theme),
        pipe.clone(),
    ];
    if has_hidden {
        spans.push(hidden.span(theme));
        spans.push(pipe.clone());
    }
    if let Some(find) = &find {
        spans.splice(2..2, [find.span(theme), pipe.clone()]);
    }
    spans.extend([
        Span::styled("(/) find", off_style),
        pipe.clone(),
        Span::styled(help_chip, off_style),
        pipe.clone(),
        Span::styled(scroll_hint, off_style),
        pipe,
        Span::styled(close_chip, off_style),
    ]);

    // Width-aware truncate. Tiers, picked by what fits:
    //   1. Full assembly above.
    //   2. Drop scroll hint + help chip, keep position · tools · think
    //      · aborted · hidden · close.
    //   3. Keep one filter chip: hidden when the transcript has hidden
    //      turns and no aborted ones, aborted otherwise.
    //   4. Drop that chip too. Those filters are rarely flipped, and the
    //      tools / think chips reflect ongoing state — those stay until
    //      close itself is at risk.
    let total = tuikit::text::spans_width(&spans);
    let cap = area.width as usize;
    let line = if total <= cap {
        Line::from(spans)
    } else {
        // The tiers lead each chip with its separator, in the chip's
        // own style.
        let after_pipe = |text: String, style: Style| Span::styled(format!(" · {text}"), style);
        let tools_span = after_pipe(tools.text(), tools.look(theme));
        let think_span = after_pipe(thinking.text(), thinking.look(theme));
        let aborted_span = after_pipe(aborted.text(), aborted.look(theme));
        let hidden_span = after_pipe(hidden.text(), hidden.look(theme));
        let close_span = Span::styled(format!(" · {close_chip}"), off_style);
        let mut base = vec![position.span()];
        if let Some(find) = &find {
            base.push(after_pipe(find.text(), find.look(theme)));
        }
        base.extend([tools_span, think_span]);
        let mut tiers: Vec<Vec<Span<'_>>> = Vec::new();
        if has_hidden {
            tiers.push(
                base.iter()
                    .cloned()
                    .chain([
                        aborted_span.clone(),
                        hidden_span.clone(),
                        close_span.clone(),
                    ])
                    .collect(),
            );
        }
        // With room for one filter chip, keep the one that filters
        // something in this transcript.
        let has_aborted = state.document.turns.iter().any(|turn| turn.aborted);
        let single = if has_hidden && !has_aborted {
            hidden_span
        } else {
            aborted_span
        };
        tiers.push(
            base.iter()
                .cloned()
                .chain([single, close_span.clone()])
                .collect(),
        );
        let width = |spans: &[Span<'_>]| -> usize { tuikit::text::spans_width(spans) };
        let chosen = tiers
            .into_iter()
            .find(|tier| width(tier) <= cap)
            .unwrap_or_else(|| base.into_iter().chain([close_span]).collect());
        Line::from(chosen)
    };
    Widget::render(Paragraph::new(line), area, buf);
}

/// The most cells the query takes in the footer chip, the ellipsis of a
/// cut query included.
const FIND_CHIP_QUERY_CELLS: usize = 17;

/// The find chip: the query with its position among the matches, or
/// `no match` (and how many matches the hidden turns hold), then the
/// one-key note if there is one. With no active find, just the note.
fn find_chip_text(state: &ViewerState) -> Option<String> {
    let find = &state.find;
    if !find.is_active() {
        return find.note.clone();
    }
    let query = tuikit::text::truncate_end(&find.query, FIND_CHIP_QUERY_CELLS);
    let mut text = match find.current {
        Some(index) => format!("find \"{query}\" {}/{}", index + 1, find.matches.len()),
        None => format!("find \"{query}\" no match"),
    };
    if find.matches.is_empty() && find.hidden_matches > 0 {
        text.push_str(&format!(" · {} in hidden turns", find.hidden_matches));
    }
    if let Some(note) = &find.note {
        text.push_str(" · ");
        text.push_str(note);
    }
    Some(text)
}

/// Draw the find prompt over the footer: `/` and the query as a text
/// field with its caret, then the key hint. A long query scrolls so the
/// cursor stays in view.
pub(crate) fn draw_find_prompt(
    prompt: &tuikit::text_edit::LineEdit,
    hint: &str,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
) {
    let hint_text = if hint.is_empty() {
        String::new()
    } else {
        format!("   {hint}")
    };
    let hint_cells = tuikit::text::width(&hint_text);
    // Cells for the text and the cursor after the leading `/`; the field
    // keeps a few even when the hint leaves none.
    let room = usize::from(area.width)
        .saturating_sub(1 + hint_cells)
        .max(4);
    let mut spans = tuikit::text_edit::TextField::new(prompt)
        .prefix(Span::styled("/", Style::new().add_modifier(Modifier::BOLD)))
        .spans(1 + room, theme);
    spans.push(Span::styled(hint_text, Tone::Inert.style(theme)));
    Widget::render(Paragraph::new(Line::from(spans)), area, buf);
}

#[cfg(test)]
pub fn render_to_buffer(
    state: &mut ViewerState,
    theme: &Theme,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    tuikit::testkit::render::render(width, height, |frame| {
        let area = frame.area();
        draw(state, theme, frame.buffer_mut(), area);
    })
}

#[cfg(test)]
pub fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    tuikit::testkit::buffer::text(buffer)
}

#[cfg(test)]
#[path = "widget_tests.rs"]
mod tests;
