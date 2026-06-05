//! Controls overlay (ADR 0031, F8-004).
//!
//! Single navigable modal that fronts view switching, per-view
//! grouping, per-view filter editing, and the global sort toggle.
//! The controls overlay is the discoverable surface; single-key
//! accelerators (`v`, `1`–`5`, `]`/`[`, `f`, `F`, `G`) reach the
//! same outcomes for muscle-memory operators (wired in F8-005).
//!
//! Pin CRUD lives in its own [`pins`](super::pins) overlay, opened
//! with `p` or the direct shortcuts (`N`/`B`/`A`/`b`/`R`/`Delete`).
//!
//! ## Architecture
//!
//! The overlay holds **only** what is unique to its UI: a cursor
//! pointing at the currently-highlighted row and an optional
//! sub-editor (multi-select list for harness / mux-state, text
//! input for max-age). Live state — the active view, grouping,
//! filter, and sort — lives on [`crate::tui::app::App`]. Each
//! [`ControlsOverlayState::handle_key`] call takes a
//! [`ControlsContext`] borrowed from the app so the overlay always
//! reflects the current world.
//!
//! Key handling returns a [`ControlsOutcome`] describing what the
//! caller should do: keep the overlay open, close it, or apply a
//! [`ControlsAction`] (and either close or stay open per the
//! action). The reducer applies actions; the overlay never mutates
//! the app directly.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::multi_select::{MultiSelectOutcome, MultiSelectState};
use crate::tui::{Grouping, Sort, View};

/// Static harness option set surfaced in the harness sub-editor.
/// Matches the four supported harness keys today; future additions
/// land here.
pub const HARNESS_OPTIONS: &[&str] = &["claude-code", "codex", "opencode", "aider"];

/// Display order of the mux-state sub-editor entries.
pub const MUX_STATE_OPTIONS: &[MuxStateKey] = &[
    MuxStateKey::Attached,
    MuxStateKey::Ambiguous,
    MuxStateKey::Unmuxed,
];

/// All views displayed in the View section, in stable order.
pub const VIEW_OPTIONS: &[View] = &[
    View::Sessions,
    View::Mux,
    View::Union,
    View::Prs,
    View::Forks,
];

/// Sort options surfaced in the Sort section, in stable order.
pub const SORT_OPTIONS: &[Sort] = &[Sort::Hierarchy, Sort::Recency];

/// Read-only snapshot of the live state the overlay renders against.
/// The renderer and the key dispatcher both consume this so the
/// overlay never holds a stale copy.
#[derive(Debug, Clone)]
pub struct ControlsContext<'a> {
    pub view: View,
    pub grouping: Grouping,
    pub filter: &'a RowFilter,
    pub sort: Sort,
}

/// One landable row in the controls overlay's flat list.
/// Section headers are not landable — the cursor skips them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsCursor {
    /// View option at index in [`VIEW_OPTIONS`].
    View(usize),
    /// Grouping option at index in
    /// [`Grouping::values_for(ctx.view)`].
    Grouping(usize),
    FilterHarness,
    FilterMaxAge,
    FilterMuxState,
    /// Sessions-view-only checkbox: float muxed sessions to the top.
    FilterFloatMuxedSessions,
    /// Mux-view-only checkbox: float attached muxes to the top.
    FilterFloatAttachedMuxes,
    FilterClear,
    /// Sort option at index in [`SORT_OPTIONS`].
    Sort(usize),
}

/// Sub-editor that owns key input while open. The host overlay
/// suspends its own bindings until the sub-editor commits or
/// cancels.
#[derive(Debug, Clone)]
pub enum SubEditor {
    Harness(MultiSelectState),
    MaxAge(TextInputState),
    MuxState(MultiSelectState),
}

/// What the controls overlay returned from a single key event.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlsOutcome {
    /// Nothing to do — overlay stays open, no app state change.
    Continue,
    /// Close the overlay without applying anything (Esc at top level).
    Close,
    /// Apply the action and leave the overlay open so the operator
    /// can see chips update in place.
    ApplyAndStay(ControlsAction),
    /// Apply the action and close (view switch is the canonical
    /// case — operators expect the overlay to dismiss after picking
    /// a view).
    ApplyAndClose(ControlsAction),
}

/// Side-effecting outcome the reducer applies. The overlay never
/// touches the app directly.
#[derive(Debug, Clone, PartialEq)]
pub enum ControlsAction {
    SwitchView(View),
    SetGrouping(Grouping),
    SetFilter(RowFilter),
    SetSort(Sort),
}

/// Pure state for the controls overlay: cursor position plus the
/// active sub-editor (if any).
#[derive(Debug, Clone)]
pub struct ControlsOverlayState {
    cursor: ControlsCursor,
    sub_editor: Option<SubEditor>,
}

impl ControlsOverlayState {
    /// Open at the top of the overlay. Operators most often want to
    /// switch view, so the cursor lands on the active view row.
    pub fn new(ctx: &ControlsContext<'_>) -> Self {
        let active_view_idx = VIEW_OPTIONS
            .iter()
            .position(|v| *v == ctx.view)
            .unwrap_or(0);
        Self {
            cursor: ControlsCursor::View(active_view_idx),
            sub_editor: None,
        }
    }

    /// Open with the cursor positioned on the Filters > Harness row.
    /// Used by the `f` accelerator (F8-005) so jumping straight into
    /// filters skips the navigation step.
    pub fn new_at_filters(_ctx: &ControlsContext<'_>) -> Self {
        Self {
            cursor: ControlsCursor::FilterHarness,
            sub_editor: None,
        }
    }

    pub fn cursor(&self) -> ControlsCursor {
        self.cursor
    }

    pub fn sub_editor(&self) -> Option<&SubEditor> {
        self.sub_editor.as_ref()
    }

    /// Dispatch a crossterm key event. Returns the outcome the
    /// caller should apply (or [`ControlsOutcome::Continue`] for
    /// in-overlay navigation that didn't change anything).
    pub fn handle_key(&mut self, ctx: &ControlsContext<'_>, event: KeyEvent) -> ControlsOutcome {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c'))
        {
            return ControlsOutcome::Close;
        }

        // While a sub-editor owns input, route every key through it
        // and translate its outcome. We dispatch directly on the
        // `Option<SubEditor>` slot so the helper can clear it on
        // commit/cancel without taking a second mutable borrow.
        if self.sub_editor.is_some() {
            return Self::dispatch_sub_editor(&mut self.sub_editor, ctx, event);
        }

        match event.code {
            KeyCode::Esc => ControlsOutcome::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = move_cursor(self.cursor, ctx, -1);
                ControlsOutcome::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursor = move_cursor(self.cursor, ctx, 1);
                ControlsOutcome::Continue
            }
            KeyCode::Enter => self.activate(ctx),
            _ => ControlsOutcome::Continue,
        }
    }

    fn activate(&mut self, ctx: &ControlsContext<'_>) -> ControlsOutcome {
        match self.cursor {
            ControlsCursor::View(idx) => {
                let view = VIEW_OPTIONS[idx];
                if view == ctx.view {
                    // No-op view switch; close so the operator sees
                    // the cancel feedback rather than a stuck overlay.
                    ControlsOutcome::Close
                } else {
                    ControlsOutcome::ApplyAndClose(ControlsAction::SwitchView(view))
                }
            }
            ControlsCursor::Grouping(idx) => {
                let options = Grouping::values_for(ctx.view);
                let chosen = options.get(idx).copied().unwrap_or(ctx.grouping);
                ControlsOutcome::ApplyAndStay(ControlsAction::SetGrouping(chosen))
            }
            ControlsCursor::FilterHarness => {
                self.sub_editor = Some(SubEditor::Harness(build_harness_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterMaxAge => {
                self.sub_editor = Some(SubEditor::MaxAge(build_max_age_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterMuxState => {
                self.sub_editor = Some(SubEditor::MuxState(build_mux_state_editor(ctx.filter)));
                ControlsOutcome::Continue
            }
            ControlsCursor::FilterFloatMuxedSessions => {
                let mut new_filter = ctx.filter.clone();
                new_filter.float_muxed_sessions_top = !new_filter.float_muxed_sessions_top;
                ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(new_filter))
            }
            ControlsCursor::FilterFloatAttachedMuxes => {
                let mut new_filter = ctx.filter.clone();
                new_filter.float_attached_muxes_top = !new_filter.float_attached_muxes_top;
                ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(new_filter))
            }
            ControlsCursor::FilterClear => {
                if ctx.filter.is_empty() {
                    // Nothing to clear; keep the overlay open silently.
                    ControlsOutcome::Continue
                } else {
                    ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(RowFilter::default()))
                }
            }
            ControlsCursor::Sort(idx) => {
                let sort = SORT_OPTIONS.get(idx).copied().unwrap_or(ctx.sort);
                ControlsOutcome::ApplyAndStay(ControlsAction::SetSort(sort))
            }
        }
    }

    fn dispatch_sub_editor(
        slot: &mut Option<SubEditor>,
        ctx: &ControlsContext<'_>,
        event: KeyEvent,
    ) -> ControlsOutcome {
        let editor = slot.as_mut().expect("dispatch called with empty slot");
        match editor {
            SubEditor::Harness(state) => match state.handle_key(event) {
                MultiSelectOutcome::Continue => ControlsOutcome::Continue,
                MultiSelectOutcome::Cancel => {
                    *slot = None;
                    ControlsOutcome::Continue
                }
                MultiSelectOutcome::Confirm(indices) => {
                    *slot = None;
                    let new_filter =
                        with_harness(ctx.filter.clone(), indices_to_harness_values(&indices));
                    ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(new_filter))
                }
            },
            SubEditor::MuxState(state) => match state.handle_key(event) {
                MultiSelectOutcome::Continue => ControlsOutcome::Continue,
                MultiSelectOutcome::Cancel => {
                    *slot = None;
                    ControlsOutcome::Continue
                }
                MultiSelectOutcome::Confirm(indices) => {
                    *slot = None;
                    let new_filter =
                        with_mux_state(ctx.filter.clone(), indices_to_mux_states(&indices));
                    ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(new_filter))
                }
            },
            SubEditor::MaxAge(state) => match state.handle_key(event) {
                InputOutcome::Continue => ControlsOutcome::Continue,
                InputOutcome::Cancel => {
                    *slot = None;
                    ControlsOutcome::Continue
                }
                InputOutcome::Confirm(value) => {
                    let trimmed = value.trim();
                    let parsed = if trimmed.is_empty() {
                        Ok(None)
                    } else {
                        parse_max_age(trimmed).map(Some)
                    };
                    match parsed {
                        Ok(max_age) => {
                            *slot = None;
                            let new_filter = with_max_age(ctx.filter.clone(), max_age);
                            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(new_filter))
                        }
                        Err(_) => {
                            // Reject the commit and keep the editor open
                            // so the operator can fix the value. The
                            // status-bar surface lands with F8-007 +
                            // F8-005's wiring.
                            ControlsOutcome::Continue
                        }
                    }
                }
            },
        }
    }
}

fn build_harness_editor(filter: &RowFilter) -> MultiSelectState {
    let selected: Vec<usize> = match &filter.harness {
        Some(HarnessFilter::Any(values)) => HARNESS_OPTIONS
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| values.iter().any(|v| v == opt).then_some(idx))
            .collect(),
        None => Vec::new(),
    };
    MultiSelectState::new(" harness ", HARNESS_OPTIONS.len(), &selected)
}

fn build_mux_state_editor(filter: &RowFilter) -> MultiSelectState {
    let selected: Vec<usize> = match &filter.mux_state {
        Some(MuxStateFilter::Any(values)) => MUX_STATE_OPTIONS
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| values.contains(opt).then_some(idx))
            .collect(),
        None => Vec::new(),
    };
    MultiSelectState::new(" mux state ", MUX_STATE_OPTIONS.len(), &selected)
}

fn build_max_age_editor(filter: &RowFilter) -> TextInputState {
    let initial = filter
        .max_age
        .map(format_duration_for_input)
        .unwrap_or_default();
    TextInputState::new(" max age ", initial)
}

fn indices_to_harness_values(indices: &[usize]) -> Vec<String> {
    indices
        .iter()
        .filter_map(|idx| HARNESS_OPTIONS.get(*idx).map(|s| s.to_string()))
        .collect()
}

fn indices_to_mux_states(indices: &[usize]) -> Vec<MuxStateKey> {
    indices
        .iter()
        .filter_map(|idx| MUX_STATE_OPTIONS.get(*idx).copied())
        .collect()
}

fn with_harness(mut filter: RowFilter, values: Vec<String>) -> RowFilter {
    filter.harness = if values.is_empty() {
        None
    } else {
        Some(HarnessFilter::from_values(values))
    };
    filter
}

fn with_mux_state(mut filter: RowFilter, values: Vec<MuxStateKey>) -> RowFilter {
    filter.mux_state = if values.is_empty() {
        None
    } else {
        Some(MuxStateFilter::from_values(values))
    };
    filter
}

fn with_max_age(mut filter: RowFilter, max_age: Option<std::time::Duration>) -> RowFilter {
    filter.max_age = max_age;
    filter
}

/// Render a duration as the most compact `<n><unit>` shape that
/// round-trips through [`parse_max_age`]. Used to seed the
/// text-input editor with a value the operator can edit in place.
pub fn format_duration_for_input(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        return "0s".to_string();
    }
    if secs.is_multiple_of(86_400) {
        format!("{}d", secs / 86_400)
    } else if secs.is_multiple_of(3_600) {
        format!("{}h", secs / 3_600)
    } else if secs.is_multiple_of(60) {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// Parse a duration token used in the max-age editor. Mirrors the
/// CLI `parse_filter_duration` shape so a value typed in the
/// overlay matches what the operator would put on the command
/// line. Returns the duration as a [`std::time::Duration`]; failure
/// is propagated as a short error string the host can show inline.
pub fn parse_max_age(raw: &str) -> Result<std::time::Duration, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h/d)".to_string())?;
    let (digits, suffix) = trimmed.split_at(split);
    let value: u64 = digits
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{digits}`"))?;
    let secs = match suffix {
        "ms" => return Ok(std::time::Duration::from_millis(value)),
        "s" => value,
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(3_600),
        "d" => value.saturating_mul(86_400),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h/d)")),
    };
    Ok(std::time::Duration::from_secs(secs))
}

/// Move the cursor by `delta` rows across the section-skipping flat
/// list. Wraps top-to-bottom and bottom-to-top so j/k keep moving
/// off either edge.
fn move_cursor(cursor: ControlsCursor, ctx: &ControlsContext<'_>, delta: i32) -> ControlsCursor {
    let list = flatten_rows(ctx);
    let idx = list.iter().position(|c| *c == cursor).unwrap_or(0) as i32;
    let len = list.len() as i32;
    if len == 0 {
        return cursor;
    }
    let next = ((idx + delta) % len + len) % len;
    list[next as usize]
}

/// All actionable rows in display order. Section headers are
/// implicit (they live in the renderer) so the cursor only needs to
/// know about the rows it can land on.
fn flatten_rows(ctx: &ControlsContext<'_>) -> Vec<ControlsCursor> {
    let mut rows: Vec<ControlsCursor> = Vec::new();
    for idx in 0..VIEW_OPTIONS.len() {
        rows.push(ControlsCursor::View(idx));
    }
    for idx in 0..Grouping::values_for(ctx.view).len() {
        rows.push(ControlsCursor::Grouping(idx));
    }
    rows.push(ControlsCursor::FilterHarness);
    rows.push(ControlsCursor::FilterMaxAge);
    rows.push(ControlsCursor::FilterMuxState);
    if matches!(ctx.view, View::Sessions) {
        rows.push(ControlsCursor::FilterFloatMuxedSessions);
    }
    if matches!(ctx.view, View::Mux) {
        rows.push(ControlsCursor::FilterFloatAttachedMuxes);
    }
    rows.push(ControlsCursor::FilterClear);
    for idx in 0..SORT_OPTIONS.len() {
        rows.push(ControlsCursor::Sort(idx));
    }
    rows
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Centered modal widget for the controls overlay. Borrows the
/// state plus a live context so the rendered values never lag
/// behind the app.
pub struct ControlsOverlayWidget<'a> {
    state: &'a ControlsOverlayState,
    ctx: ControlsContext<'a>,
}

impl<'a> ControlsOverlayWidget<'a> {
    pub fn new(state: &'a ControlsOverlayState, ctx: ControlsContext<'a>) -> Self {
        Self { state, ctx }
    }
}

impl Widget for ControlsOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal = centered_modal_rect(area);
        // Repaint the modal area so any dimmed body content doesn't
        // bleed through.
        for y in modal.top()..modal.bottom() {
            for x in modal.left()..modal.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(" Controls "));
        let inner = block.inner(modal);
        block.render(modal, buf);

        let lines = self.body_lines();
        let footer = Line::from(Span::styled(
            "↑/↓ move · Enter pick · Esc close",
            Style::default().add_modifier(Modifier::DIM),
        ));
        let mut all_lines = lines;
        all_lines.push(Line::from(""));
        all_lines.push(footer);
        let para = Paragraph::new(all_lines);
        para.render(inner, buf);

        // If a sub-editor is open, render it on top of the overlay.
        if let Some(editor) = self.state.sub_editor() {
            render_sub_editor(editor, area, buf);
        }
    }
}

fn render_sub_editor(editor: &SubEditor, area: Rect, buf: &mut Buffer) {
    match editor {
        SubEditor::Harness(state) => {
            use crate::tui::widgets::multi_select::MultiSelectWidget;
            MultiSelectWidget::new(state, HARNESS_OPTIONS).render(area, buf);
        }
        SubEditor::MuxState(state) => {
            use crate::tui::widgets::multi_select::MultiSelectWidget;
            let labels: Vec<&'static str> = MUX_STATE_OPTIONS.iter().map(|k| k.as_str()).collect();
            // The MultiSelectWidget requires a slice that owns the
            // items by reference; since `as_str` returns &'static
            // str the temporary Vec is fine.
            MultiSelectWidget::new(state, &labels).render(area, buf);
        }
        SubEditor::MaxAge(state) => {
            use crate::tui::widgets::input::TextInputWidget;
            TextInputWidget::new(state).render(area, buf);
        }
    }
}

impl ControlsOverlayWidget<'_> {
    fn body_lines(&self) -> Vec<Line<'static>> {
        let cursor = self.state.cursor();
        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(section_header("View"));
        for (idx, view) in VIEW_OPTIONS.iter().enumerate() {
            let row = ControlsCursor::View(idx);
            let active = *view == self.ctx.view;
            let label = format!("{} [{}]", view_label(*view), idx + 1);
            lines.push(row_line(label, active, cursor == row));
        }
        lines.push(Line::from(""));

        lines.push(section_header(&format!(
            "Grouping ({})",
            view_label(self.ctx.view)
        )));
        for (idx, grouping) in Grouping::values_for(self.ctx.view).iter().enumerate() {
            let row = ControlsCursor::Grouping(idx);
            let active = *grouping == self.ctx.grouping;
            let label = grouping.as_str().to_string();
            lines.push(row_line(label, active, cursor == row));
        }
        lines.push(Line::from(""));

        lines.push(section_header(&format!(
            "Filters ({})",
            view_label(self.ctx.view)
        )));
        lines.push(filter_row(
            "harness",
            harness_value(self.ctx.filter),
            cursor == ControlsCursor::FilterHarness,
        ));
        lines.push(filter_row(
            "max age",
            max_age_value(self.ctx.filter),
            cursor == ControlsCursor::FilterMaxAge,
        ));
        lines.push(filter_row(
            "mux state",
            mux_state_value(self.ctx.filter),
            cursor == ControlsCursor::FilterMuxState,
        ));
        if matches!(self.ctx.view, View::Sessions) {
            lines.push(checkbox_row(
                "Float muxed sessions to top",
                self.ctx.filter.float_muxed_sessions_top,
                cursor == ControlsCursor::FilterFloatMuxedSessions,
            ));
        }
        if matches!(self.ctx.view, View::Mux) {
            lines.push(checkbox_row(
                "Float attached muxes to top",
                self.ctx.filter.float_attached_muxes_top,
                cursor == ControlsCursor::FilterFloatAttachedMuxes,
            ));
        }
        lines.push(row_line(
            "Clear all filters".to_string(),
            false,
            cursor == ControlsCursor::FilterClear,
        ));
        lines.push(Line::from(""));

        lines.push(section_header("Sort"));
        for (idx, sort) in SORT_OPTIONS.iter().enumerate() {
            let row = ControlsCursor::Sort(idx);
            let active = *sort == self.ctx.sort;
            lines.push(row_line(
                sort_label(*sort).to_string(),
                active,
                cursor == row,
            ));
        }
        lines
    }
}

fn section_header(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_string(),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

fn row_line(label: String, active: bool, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    // REVERSED (not a fixed bg color) so the cursor row stays
    // readable on both light and dark terminal themes.
    let mut style = Style::default();
    if cursored {
        style = style.add_modifier(Modifier::REVERSED);
    }
    if active {
        style = style.add_modifier(Modifier::BOLD);
    }
    let active_tag = if active { "  active" } else { "" };
    Line::from(Span::styled(format!("{marker}{label}{active_tag}"), style))
}

fn filter_row(name: &str, value: String, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    let style = if cursored {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(Span::styled(
        format!("{marker}{name:9} {value}  [ Edit ]"),
        style,
    ))
}

fn checkbox_row(label: &str, checked: bool, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    let mut style = Style::default();
    if cursored {
        style = style.add_modifier(Modifier::REVERSED);
    }
    if checked {
        style = style.add_modifier(Modifier::BOLD);
    }
    let box_glyph = if checked { "[x]" } else { "[ ]" };
    Line::from(Span::styled(format!("{marker}{box_glyph} {label}"), style))
}

fn harness_value(filter: &RowFilter) -> String {
    match &filter.harness {
        Some(HarnessFilter::Any(values)) if !values.is_empty() => values.join(", "),
        _ => "—".to_string(),
    }
}

fn max_age_value(filter: &RowFilter) -> String {
    filter
        .max_age
        .map(format_duration_for_input)
        .unwrap_or_else(|| "—".to_string())
}

fn mux_state_value(filter: &RowFilter) -> String {
    match &filter.mux_state {
        Some(MuxStateFilter::Any(values)) if !values.is_empty() => values
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => "—".to_string(),
    }
}

fn view_label(view: View) -> &'static str {
    match view {
        View::Sessions => "Sessions",
        View::Mux => "Mux",
        View::Union => "Union",
        View::Prs => "PRs",
        View::Forks => "Forks",
    }
}

fn sort_label(sort: Sort) -> &'static str {
    match sort {
        Sort::Hierarchy => "Hierarchy",
        Sort::Recency => "Recency",
    }
}

/// 60-column centered modal sized to its content. Falls back to a
/// taller window when the terminal can spare the rows; the
/// renderer truncates extra lines rather than scrolling.
pub fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(64, area.width.saturating_sub(4)).max(40);
    let max_height = area.height.saturating_sub(2);
    // Content is 5 view rows + 5 grouping rows + 4 filter rows
    // (+ 1 view-scoped checkbox on sessions/mux) + 2 sort rows +
    // 4 section headers + 3 blank lines + 1 footer +
    // 1 blank-before-footer = up to 21 lines, plus 2 for the border.
    let desired = 23;
    let height = (desired as u16).clamp(8, max_height.max(8));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctx_with(
        view: View,
        grouping: Grouping,
        filter: &RowFilter,
        sort: Sort,
    ) -> ControlsContext<'_> {
        ControlsContext {
            view,
            grouping,
            filter,
            sort,
        }
    }

    #[test]
    fn opens_with_active_view_selected() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Mux,
            Grouping::default_for(View::Mux),
            &filter,
            Sort::Hierarchy,
        );
        let state = ControlsOverlayState::new(&ctx);
        assert_eq!(state.cursor(), ControlsCursor::View(1));
    }

    #[test]
    fn arrow_keys_skip_section_headers() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new(&ctx);
        // Five views + 5 sessions groupings + 5 filter rows
        // (3 predicates + 1 sessions-only float checkbox + clear) +
        // 2 sort rows = 17 actionable rows on Sessions.
        for _ in 0..flatten_rows(&ctx).len() {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(
            state.cursor(),
            ControlsCursor::View(0),
            "down wraps back to the top after one full cycle",
        );
    }

    #[test]
    fn enter_on_active_view_closes_without_action() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new(&ctx);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, ControlsOutcome::Close);
    }

    #[test]
    fn enter_on_different_view_switches_and_closes() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new(&ctx);
        state.handle_key(&ctx, key(KeyCode::Down));
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            ControlsOutcome::ApplyAndClose(ControlsAction::SwitchView(View::Mux))
        );
    }

    #[test]
    fn enter_on_grouping_applies_and_stays_open() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new(&ctx);
        // Advance 5 rows = past all View options into Grouping(0).
        for _ in 0..5 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        // Land on the second grouping row (Graph).
        state.handle_key(&ctx, key(KeyCode::Down));
        assert_eq!(state.cursor(), ControlsCursor::Grouping(1));
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        let expected = Grouping::values_for(View::Sessions)[1];
        assert_eq!(
            outcome,
            ControlsOutcome::ApplyAndStay(ControlsAction::SetGrouping(expected))
        );
    }

    #[test]
    fn enter_on_harness_opens_multi_select_sub_editor() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        assert_eq!(state.cursor(), ControlsCursor::FilterHarness);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, ControlsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(SubEditor::Harness(_))));
    }

    #[test]
    fn harness_sub_editor_confirm_emits_set_filter() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        state.handle_key(&ctx, key(KeyCode::Enter)); // open editor
        // Toggle the first harness (claude-code) on.
        state.handle_key(&ctx, key(KeyCode::Char(' ')));
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        match outcome {
            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(f)) => {
                let values = f
                    .harness
                    .as_ref()
                    .map(|h| h.values().to_vec())
                    .unwrap_or_default();
                assert_eq!(values, vec!["claude-code".to_string()]);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
        // Editor closed after commit.
        assert!(state.sub_editor().is_none());
    }

    #[test]
    fn harness_sub_editor_cancel_keeps_filter() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        state.handle_key(&ctx, key(KeyCode::Enter));
        state.handle_key(&ctx, key(KeyCode::Char(' ')));
        let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
        assert_eq!(outcome, ControlsOutcome::Continue);
        assert!(state.sub_editor().is_none());
    }

    #[test]
    fn max_age_sub_editor_parses_typed_value() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        state.handle_key(&ctx, key(KeyCode::Down)); // → FilterMaxAge
        state.handle_key(&ctx, key(KeyCode::Enter)); // open editor
        for c in "7d".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(c)));
        }
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        match outcome {
            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(f)) => {
                assert_eq!(f.max_age, Some(std::time::Duration::from_secs(7 * 86_400)));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn max_age_invalid_value_keeps_editor_open() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        state.handle_key(&ctx, key(KeyCode::Down));
        state.handle_key(&ctx, key(KeyCode::Enter));
        for c in "nope".chars() {
            state.handle_key(&ctx, key(KeyCode::Char(c)));
        }
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, ControlsOutcome::Continue);
        assert!(matches!(state.sub_editor(), Some(SubEditor::MaxAge(_))));
    }

    #[test]
    fn clear_all_with_empty_filter_is_silent_noop() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        // Step past the three predicate rows and the sessions-only
        // float checkbox to land on FilterClear.
        for _ in 0..4 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), ControlsCursor::FilterClear);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(outcome, ControlsOutcome::Continue);
    }

    #[test]
    fn clear_all_with_active_filter_emits_empty_filter() {
        let filter = RowFilter {
            harness: Some(HarnessFilter::from_values(["claude-code"])),
            ..RowFilter::default()
        };
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        for _ in 0..4 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), ControlsCursor::FilterClear);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        assert_eq!(
            outcome,
            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(RowFilter::default()))
        );
    }

    #[test]
    fn float_muxed_checkbox_only_appears_on_sessions_view() {
        let filter = RowFilter::default();
        let rows_sessions = flatten_rows(&ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        ));
        assert!(rows_sessions.contains(&ControlsCursor::FilterFloatMuxedSessions));
        assert!(!rows_sessions.contains(&ControlsCursor::FilterFloatAttachedMuxes));

        let rows_mux = flatten_rows(&ctx_with(
            View::Mux,
            Grouping::default_for(View::Mux),
            &filter,
            Sort::Hierarchy,
        ));
        assert!(!rows_mux.contains(&ControlsCursor::FilterFloatMuxedSessions));
        assert!(rows_mux.contains(&ControlsCursor::FilterFloatAttachedMuxes));

        for view in [View::Union, View::Prs, View::Forks] {
            let rows = flatten_rows(&ctx_with(
                view,
                Grouping::default_for(view),
                &filter,
                Sort::Hierarchy,
            ));
            assert!(!rows.contains(&ControlsCursor::FilterFloatMuxedSessions));
            assert!(!rows.contains(&ControlsCursor::FilterFloatAttachedMuxes));
        }
    }

    #[test]
    fn enter_on_float_muxed_sessions_toggles_bool() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        for _ in 0..3 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), ControlsCursor::FilterFloatMuxedSessions);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        match outcome {
            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(f)) => {
                assert!(f.float_muxed_sessions_top);
                assert!(!f.float_attached_muxes_top);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn enter_on_float_attached_muxes_toggles_bool() {
        let filter = RowFilter {
            float_attached_muxes_top: true,
            ..RowFilter::default()
        };
        let ctx = ctx_with(
            View::Mux,
            Grouping::default_for(View::Mux),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new_at_filters(&ctx);
        for _ in 0..3 {
            state.handle_key(&ctx, key(KeyCode::Down));
        }
        assert_eq!(state.cursor(), ControlsCursor::FilterFloatAttachedMuxes);
        let outcome = state.handle_key(&ctx, key(KeyCode::Enter));
        match outcome {
            ControlsOutcome::ApplyAndStay(ControlsAction::SetFilter(f)) => {
                assert!(!f.float_attached_muxes_top, "Enter toggles bool off");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn ordering_bool_keeps_filter_non_empty_for_clear_all() {
        let filter = RowFilter {
            float_muxed_sessions_top: true,
            ..RowFilter::default()
        };
        assert!(!filter.is_empty());
        assert!(!filter.has_narrowing_predicates());
    }

    #[test]
    fn esc_at_top_level_closes_overlay() {
        let filter = RowFilter::default();
        let ctx = ctx_with(
            View::Sessions,
            Grouping::default_for(View::Sessions),
            &filter,
            Sort::Hierarchy,
        );
        let mut state = ControlsOverlayState::new(&ctx);
        let outcome = state.handle_key(&ctx, key(KeyCode::Esc));
        assert_eq!(outcome, ControlsOutcome::Close);
    }

    #[test]
    fn format_and_parse_round_trip_for_common_durations() {
        let cases = [
            (std::time::Duration::from_secs(7 * 86_400), "7d"),
            (std::time::Duration::from_secs(2 * 3_600), "2h"),
            (std::time::Duration::from_secs(15 * 60), "15m"),
            (std::time::Duration::from_secs(45), "45s"),
        ];
        for (dur, expected) in cases {
            assert_eq!(format_duration_for_input(dur), expected);
            assert_eq!(parse_max_age(expected), Ok(dur));
        }
    }
}
