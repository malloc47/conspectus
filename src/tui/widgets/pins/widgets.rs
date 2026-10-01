//! Rendering for the pins menu and the edit, rebind, bind, and remove sub-editors.

use super::*;
use ratatui::macros::line;

/// Centered modal widget for the pins overlay.
pub struct PinsOverlayWidget<'a> {
    state: &'a PinsOverlayState,
    theme: &'a Theme,
}

impl<'a> PinsOverlayWidget<'a> {
    pub fn new(state: &'a PinsOverlayState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinsOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let cursor = self.state.cursor();
        let mut lines: Vec<Line<'static>> = Vec::new();
        for (idx, label) in PIN_ACTION_OPTIONS.iter().enumerate() {
            let row = PinsCursor::Action(idx);
            lines.push(row_line((*label).to_string(), cursor == row));
        }
        lines.push(line![""]);
        lines.push(line![
            span!(Modifier::DIM; "↑/↓ move · Enter pick · Esc close")
        ]);
        let modal = centered_modal_rect(area);

        let sub_editor_to_render = self.state.sub_editor();
        let theme = self.theme;
        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                pins_menu_cursor_line(cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let popup = themed_popup(body, line![" Pins "], theme);
        popup.render(area, buf);

        if let Some(editor) = sub_editor_to_render {
            render_sub_editor(editor, area, buf, theme);
        }
    }
}

pub(super) fn render_sub_editor(
    editor: &PinsSubEditor,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
) {
    match editor {
        PinsSubEditor::Create(state) => PinCreateWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Edit(state) => PinEditWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Rebind(state) => PinRebindWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Bind(state) => PinBindWidget::new(state, theme).render(area, buf),
        PinsSubEditor::Remove(state) => PinRemoveWidget::new(state, theme).render(area, buf),
    }
}

pub(super) struct PinEditWidget<'a> {
    pub(super) state: &'a PinEditState,
    pub(super) theme: &'a Theme,
}

impl<'a> PinEditWidget<'a> {
    pub(super) fn new(state: &'a PinEditState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinEditWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let state = self.state;
        let cursor = state.logical_cursor();
        // Match PinCreateWidget's row count so both modals sit at a
        // similar height: cwd, harness, launch argv, preview, blank,
        // "Advanced identity", id, display, mux.name, mux.socket,
        // static store line = 11 rows, plus 1 for options when
        // present, plus 2 for the optional error line.
        let content_lines =
            11 + usize::from(state.has_launch_options()) + usize::from(state.error.is_some()) * 2;
        let modal = pin_edit_modal_rect(area, content_lines);
        let inner_width = modal.width.saturating_sub(2) as usize;
        let mut lines = vec![
            pin_path_omnibox_field(
                PinEditState::FIELD_CWD,
                &state.cwd,
                cursor == PinEditState::FIELD_CWD,
                cursor,
                inner_width,
            ),
            pin_harness_field(
                PinEditState::FIELD_HARNESS,
                &state.harness,
                &state.known_harness_keys,
                cursor,
                inner_width,
            ),
        ];
        if state.has_launch_options() {
            let effective = state.effective_launch_argv_list().unwrap_or_default();
            lines.push(pin_launch_options_field(
                PinEditState::FIELD_LAUNCH_OPTIONS,
                state.launch_options(),
                &effective,
                cursor,
                inner_width,
            ));
        }
        lines.extend([
            pin_create_input_field(
                PinEditState::FIELD_LAUNCH_ARGV,
                "launch argv",
                &state.launch_argv,
                cursor,
                inner_width,
            ),
            pin_launch_preview_field(state.effective_launch_argv(), inner_width),
            line![""],
            line![span!(Modifier::DIM; "Advanced identity")],
            pin_create_input_field(PinEditState::FIELD_ID, "id", &state.id, cursor, inner_width),
            pin_create_input_field(
                PinEditState::FIELD_DISPLAY,
                "display",
                &state.display_name,
                cursor,
                inner_width,
            ),
            pin_create_input_field(
                PinEditState::FIELD_MUX_NAME,
                "mux.name",
                &state.mux_name,
                cursor,
                inner_width,
            ),
            pin_create_input_field(
                PinEditState::FIELD_MUX_SOCKET,
                "mux.socket",
                &state.mux_socket,
                cursor,
                inner_width,
            ),
            pin_edit_static_line("store", &state.target.store_path, inner_width),
        ]);
        if let Some(error) = &state.error {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.clone())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "↑/↓/Tab move · S-Tab back · ←/→/Space option · Enter save · Esc cancel"
        )]);

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                pin_edit_cursor_line(state),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Edit Pin "], self.theme).render(area, buf);
    }
}

pub(super) fn pin_edit_cursor_line(state: &PinEditState) -> Option<usize> {
    let cursor = state.logical_cursor();
    for (idx, field) in state.visible_fields().into_iter().enumerate() {
        if field == cursor {
            return Some(idx);
        }
    }
    Some(0)
}

pub(super) fn pin_edit_static_line(
    label: &'static str,
    value: &str,
    inner_width: usize,
) -> Line<'static> {
    let (prefix, _) = pin_create_prefix(label, false);
    let raw = format!("{prefix}{PIN_CREATE_TEXT_GUTTER}{value}");
    line![span!(Modifier::DIM; "{}", truncate_chars(&raw, inner_width))]
}

pub(super) struct PinRebindWidget<'a> {
    pub(super) state: &'a PinRebindState,
    pub(super) theme: &'a Theme,
}

impl<'a> PinRebindWidget<'a> {
    pub(super) fn new(state: &'a PinRebindState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinRebindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let mut lines = vec![
            line![format!("  id          {}", self.state.target.id)],
            line![format!("  display     {}", self.state.target.display_name)],
            pin_create_field(
                0,
                "mux.name",
                self.state.mux_name.value(),
                self.state.cursor,
            ),
            pin_create_field(
                1,
                "mux.socket",
                self.state.mux_socket.value(),
                self.state.cursor,
            ),
            line![format!("  store       {}", self.state.target.store_path)],
        ];
        if let Some(error) = &self.state.error {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.clone())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "Up/Down field · type to edit · Enter save · Esc cancel"
        )]);
        let modal = pin_rebind_modal_rect(area, lines.len());

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(2 + self.state.cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Rebind Pin "], self.theme).render(area, buf);
    }
}

pub(super) fn pin_rebind_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(70, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 9)
}

pub(super) fn pin_edit_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(78, area.width.saturating_sub(4)).max(46);
    modal_rect_for_content(area, width, content_lines, 10)
}

pub(super) struct PinBindWidget<'a> {
    pub(super) state: &'a PinBindState,
    pub(super) theme: &'a Theme,
}

impl<'a> PinBindWidget<'a> {
    pub(super) fn new(state: &'a PinBindState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinBindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let mut lines = Vec::new();
        if let Some(first) = self.state.options.first() {
            lines.push(line![format!("pin       {}", first.pin_id)]);
        }
        for (idx, option) in self.state.options.iter().enumerate() {
            let marker = if idx == self.state.cursor { "> " } else { "  " };
            let style = if idx == self.state.cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(line![span!(style; "{marker}{}", option.label)]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "Up/Down choose · Enter bind · Esc cancel"
        )]);
        let modal = pin_bind_modal_rect(area, lines.len());
        let option_start = usize::from(!self.state.options.is_empty());

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                Some(option_start + self.state.cursor),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Bind Pin "], self.theme).render(area, buf);
    }
}

pub(super) fn pin_bind_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 7)
}

pub(super) struct PinRemoveWidget<'a> {
    pub(super) state: &'a PinRemoveState,
    pub(super) theme: &'a Theme,
}

impl<'a> PinRemoveWidget<'a> {
    pub(super) fn new(state: &'a PinRemoveState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinRemoveWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let lines = vec![
            line![
                "id       ",
                span!(Modifier::BOLD; "{}", self.state.target.id.clone()),
            ],
            line![format!("display  {}", self.state.target.display_name)],
            line![format!("store    {}", self.state.target.store_path)],
            line![""],
            line![span!(Modifier::DIM; "Enter remove · Esc cancel")],
        ];
        let modal = pin_remove_modal_rect(area, lines.len());
        let body = ScrollLinesBody {
            scroll_offset: 0,
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Remove Pin "], self.theme).render(area, buf);
    }
}

pub(super) fn pin_remove_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 7)
}

pub(super) fn row_line(label: String, cursored: bool) -> Line<'static> {
    let marker = if cursored { "> " } else { "  " };
    let mut style = Style::default();
    if cursored {
        style = style.add_modifier(Modifier::REVERSED);
    }
    line![span!(style; "{marker}{label}")]
}

pub(super) fn centered_modal_rect(area: Rect) -> Rect {
    centered_modal_rect_for_content(area, pins_menu_content_lines())
}

pub(super) fn centered_modal_rect_for_content(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(50, area.width.saturating_sub(4)).max(32);
    modal_rect_for_content(area, width, content_lines, 8)
}

pub(super) fn modal_rect_for_content(
    area: Rect,
    width: u16,
    content_lines: usize,
    min_height: u16,
) -> Rect {
    let max_height = area.height;
    let desired = u16::try_from(content_lines.saturating_add(2)).unwrap_or(u16::MAX);
    let height = desired.clamp(min_height, max_height.max(min_height));
    crate::tui::widgets::popup_frame::centered_rect(area, width, height)
}

pub(super) fn pins_menu_content_lines() -> usize {
    PIN_ACTION_OPTIONS.len() + 2
}

pub(super) fn pins_menu_cursor_line(cursor: PinsCursor) -> Option<usize> {
    let PinsCursor::Action(idx) = cursor;
    Some(idx)
}

pub(super) struct ScrollLinesBody {
    pub(super) lines: Vec<Line<'static>>,
    pub(super) inner_width: usize,
    pub(super) inner_height: usize,
    pub(super) scroll_offset: u16,
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

pub(super) fn scroll_offset_for_cursor(
    cursor_line: Option<usize>,
    inner_height: usize,
    content_height: usize,
) -> u16 {
    let Some(cursor_line) = cursor_line else {
        return 0;
    };
    if inner_height == 0 || cursor_line < inner_height {
        return 0;
    }
    let max_scroll = content_height.saturating_sub(inner_height);
    let offset = cursor_line
        .saturating_sub(inner_height.saturating_sub(1))
        .min(max_scroll);
    u16::try_from(offset).unwrap_or(u16::MAX)
}
