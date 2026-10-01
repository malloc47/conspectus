//! Rendering for the pin-create form and its field rows.

use super::*;
use ratatui::macros::line;

pub(super) struct PinCreateWidget<'a> {
    pub(super) state: &'a PinCreateState,
    pub(super) theme: &'a Theme,
}

impl<'a> PinCreateWidget<'a> {
    pub(super) fn new(state: &'a PinCreateState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinCreateWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Framing through `tui_popup::Popup`.
        let cursor = self.state.render_cursor();
        let spec = self.state.spec();
        let content_lines = 16
            + usize::from(self.state.has_launch_options())
            + usize::from(spec.worktree_enabled())
            + usize::from(spec.error().is_some()) * 2;
        let modal = pin_create_modal_rect(area, content_lines);
        let inner_width = modal.width.saturating_sub(2) as usize;
        let mut lines = vec![
            pin_create_input_field(
                PinCreateState::FIELD_NAME,
                "name",
                &self.state.name,
                cursor,
                inner_width,
            ),
            pin_create_mode_field(self.state, cursor, inner_width),
            pin_create_path_omnibox_field(self.state, cursor, inner_width),
        ];
        // Worktree toggle (ADR 0094) + branch when enabled.
        lines.push(pin_create_static_field(
            PinCreateState::FIELD_WORKTREE_TOGGLE,
            "worktree",
            if spec.worktree_enabled() {
                "[x] create worktree (realized at launch)"
            } else {
                "[ ] create worktree"
            },
            cursor,
            inner_width,
        ));
        if spec.worktree_enabled() {
            lines.push(pin_create_input_field(
                PinCreateState::FIELD_WORKTREE_BRANCH,
                "wt branch",
                spec.worktree_branch(),
                cursor,
                inner_width,
            ));
        }
        lines.push(pin_create_harness_field(self.state, cursor, inner_width));
        if self.state.has_launch_options() {
            lines.push(pin_create_launch_options_field(
                self.state,
                cursor,
                inner_width,
            ));
        }
        lines.extend([
            pin_create_input_field(
                PinCreateState::FIELD_LAUNCH_ARGV,
                "launch argv",
                spec.launch_argv(),
                cursor,
                inner_width,
            ),
            pin_create_launch_preview_field(self.state, inner_width),
            line![""],
            line![span!(Modifier::DIM; "Advanced identity")],
            pin_create_input_field(
                PinCreateState::FIELD_ID,
                "id",
                &self.state.id,
                cursor,
                inner_width,
            ),
            pin_create_input_field(
                PinCreateState::FIELD_DISPLAY,
                "display",
                &self.state.display_name,
                cursor,
                inner_width,
            ),
            pin_create_value_field(
                PinCreateState::FIELD_MUX_NAME,
                "mux.name",
                &self.state.mux_name_display(),
                spec.mux_name().cursor(),
                cursor,
                inner_width,
            ),
            pin_create_input_field(
                PinCreateState::FIELD_MUX_SOCKET,
                "mux.socket",
                spec.mux_socket(),
                cursor,
                inner_width,
            ),
            pin_create_store_field(self.state.store, cursor, inner_width),
        ]);
        if let Some(error) = spec.error() {
            lines.push(line![""]);
            lines.push(line![span!(Modifier::BOLD; "{}", error.to_string())]);
        }
        lines.push(line![""]);
        lines.push(line![span!(
            Modifier::DIM;
            "↑/↓/Tab move · S-Tab back · ←/→/Space option · Enter create · Esc cancel"
        )]);

        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(
                pin_create_cursor_line(self.state),
                modal.height.saturating_sub(2) as usize,
                lines.len(),
            ),
            lines,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        themed_popup(body, line![" Create Pin "], self.theme).render(area, buf);
    }
}

pub(super) fn pin_create_input_field(
    idx: usize,
    label: &'static str,
    input: &TextInputState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    pin_create_value_field(
        idx,
        label,
        input.value(),
        input.cursor(),
        cursor,
        inner_width,
    )
}

pub(super) fn pin_create_path_omnibox_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let is_focused = cursor == PinCreateState::FIELD_CWD;
    pin_path_omnibox_field(
        PinCreateState::FIELD_CWD,
        state.spec.cwd(),
        is_focused,
        cursor,
        inner_width,
    )
}

pub(super) fn pin_path_omnibox_field(
    idx: usize,
    omnibox: &PathOmniboxState,
    is_focused: bool,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    pin_create_value_field_with_suffix(
        idx,
        "cwd",
        omnibox.value(),
        omnibox.cursor(),
        cursor,
        inner_width,
        Some(pin_omnibox_cwd_suffix(omnibox, is_focused)),
    )
}

pub(super) struct PinFieldSuffix {
    pub(super) width: usize,
    pub(super) spans: Vec<Span<'static>>,
}

pub(super) fn pin_create_value_field(
    idx: usize,
    label: &'static str,
    value: &str,
    value_cursor: usize,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    pin_create_value_field_with_suffix(idx, label, value, value_cursor, cursor, inner_width, None)
}

pub(super) fn pin_create_value_field_with_suffix(
    idx: usize,
    label: &'static str,
    value: &str,
    value_cursor: usize,
    cursor: usize,
    inner_width: usize,
    suffix: Option<PinFieldSuffix>,
) -> Line<'static> {
    let active = cursor == idx;
    let (prefix, label_style) = pin_create_prefix(label, active);
    let suffix_width = suffix
        .as_ref()
        .map(|suffix| suffix.width)
        .unwrap_or_default();
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(suffix_width)
        .max(1);
    let display = pin_field_visible_window(value, value_cursor, value_width, active);
    let value_style = pin_create_entry_style(active);
    let left_style = pin_field_edge_style(display.left_indicator, display.edge_style);
    let right_style = pin_field_edge_style(display.right_indicator, display.edge_style);
    let mut spans = vec![
        span!(label_style; "{prefix}"),
        span!(left_style; "{}", display.left_indicator),
        span!(value_style; "{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!(value_style; "{}", display.after_cursor),
        span!(right_style; "{}", display.right_indicator),
    ];
    if let Some(suffix) = suffix {
        spans.extend(suffix.spans);
    }
    Line::from(spans)
}

pub(super) fn pin_omnibox_cwd_suffix(
    omnibox: &PathOmniboxState,
    is_focused: bool,
) -> PinFieldSuffix {
    let (symbol, symbol_style) = pin_cwd_status_symbol(omnibox);
    let mut spans = vec![span!(" "), span!(symbol_style; "{symbol}")];
    let used = 1 + symbol.chars().count();
    let remaining = PIN_CREATE_CWD_SUFFIX_WIDTH.saturating_sub(used);
    if remaining > 1 {
        if let Some(hint) = pin_cwd_completion_hint(omnibox, is_focused) {
            let hint = truncate_chars(&hint, remaining.saturating_sub(1));
            spans.push(span!(" "));
            spans.push(span!(Style::default().fg(Color::Cyan); "{hint}"));
            let used = used + 1 + hint.chars().count();
            let pad = PIN_CREATE_CWD_SUFFIX_WIDTH.saturating_sub(used);
            if pad > 0 {
                spans.push(span!("{}", " ".repeat(pad)));
            }
        } else {
            spans.push(span!("{}", " ".repeat(remaining)));
        }
    }
    PinFieldSuffix {
        width: PIN_CREATE_CWD_SUFFIX_WIDTH,
        spans,
    }
}

pub(super) fn pin_cwd_status_symbol(omnibox: &PathOmniboxState) -> (&'static str, Style) {
    match omnibox.validation() {
        PathValidation::Empty => ("?", Style::default().fg(Color::DarkGray)),
        PathValidation::Exists => ("✓", Style::default().fg(Color::Green)),
        PathValidation::Missing => ("!", Style::default().fg(Color::Yellow)),
    }
}

pub(super) fn pin_cwd_completion_hint(
    omnibox: &PathOmniboxState,
    is_focused: bool,
) -> Option<String> {
    if !is_focused {
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

pub(super) fn pin_create_prefix(label: &'static str, active: bool) -> (String, Style) {
    let marker = if active { "> " } else { "  " };
    let label_style = if active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    (
        format!("{marker}{label:PIN_CREATE_LABEL_WIDTH$}{PIN_CREATE_VALUE_GAP}"),
        label_style,
    )
}

pub(super) fn pin_create_entry_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    }
}

pub(super) fn pin_field_edge_style(indicator: &str, edge_style: Style) -> Style {
    if indicator.trim().is_empty() {
        Style::default()
    } else {
        edge_style
    }
}

pub(super) fn pin_create_mode_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    if state.can_toggle_mode() {
        option_pair_line(
            cursor == PinCreateState::FIELD_MODE,
            "mode",
            "new",
            state.mode == PinCreateMode::NewVariation,
            "adopt selected",
            state.mode == PinCreateMode::AdoptSelected,
            inner_width,
        )
    } else {
        pin_create_static_field(
            PinCreateState::FIELD_MODE,
            "mode",
            state.mode.label(),
            cursor,
            inner_width,
        )
    }
}

pub(super) fn pin_create_harness_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    pin_harness_field(
        PinCreateState::FIELD_HARNESS,
        state.spec.harness(),
        state.spec.known_harness_keys(),
        cursor,
        inner_width,
    )
}

pub(super) fn pin_harness_field(
    idx: usize,
    input: &TextInputState,
    known_harness_keys: &[String],
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    const HARNESS_VALUE_SLOT_WIDTH: usize = 18;

    let active = cursor == idx;
    let (prefix, label_style) = pin_create_prefix("harness", active);
    let available = inner_width.saturating_sub(prefix.chars().count());
    if pin_harness_is_custom(input, known_harness_keys) {
        return pin_create_harness_custom_field(input, active, label_style, prefix, available);
    }
    let value_width = HARNESS_VALUE_SLOT_WIDTH.min(available).max(1);
    let suffix_budget = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(value_width)
        .min(48);
    let suffix = pin_harness_suffix(input, known_harness_keys, suffix_budget);
    let display = pin_field_visible_window(input.value(), input.cursor(), value_width, active);
    let value_style = pin_create_entry_style(active);
    let left_style = pin_field_edge_style(display.left_indicator, display.edge_style);
    let display_width = display.left_indicator.chars().count()
        + display.before_cursor.chars().count()
        + display.cursor_text.chars().count()
        + display.after_cursor.chars().count()
        + display.right_indicator.chars().count();
    let padding = value_width.saturating_sub(display_width);
    let right_style = if active && padding > 0 && display.right_indicator.trim().is_empty() {
        value_style
    } else {
        pin_field_edge_style(display.right_indicator, display.edge_style)
    };
    let mut spans = vec![
        span!(label_style; "{prefix}"),
        span!(left_style; "{}", display.left_indicator),
        span!(value_style; "{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!(value_style; "{}", display.after_cursor),
        span!(right_style; "{}", display.right_indicator),
    ];
    if padding > 0 {
        spans.push(span!(value_style; "{}", " ".repeat(padding)));
    }
    spans.extend(suffix);
    Line::from(spans)
}

pub(super) fn pin_create_launch_options_field(
    state: &PinCreateState,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let options = state.launch_options();
    let effective_argv = state.effective_launch_argv_list().unwrap_or_default();
    pin_launch_options_field(
        PinCreateState::FIELD_LAUNCH_OPTIONS,
        options,
        &effective_argv,
        cursor,
        inner_width,
    )
}

pub(super) fn pin_launch_options_field(
    idx: usize,
    options: &[HarnessLaunchOption],
    effective_argv: &[String],
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let active = cursor == idx;
    let (prefix, style) = pin_create_prefix("options", active);
    let value = options
        .iter()
        .map(|option| {
            option_token(
                option.label,
                argv_contains_fragment(effective_argv, option.argv),
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(PIN_CREATE_TEXT_GUTTER.chars().count());
    line![
        span!(style; "{prefix}"),
        span!("{}", PIN_CREATE_TEXT_GUTTER),
        span!(pin_create_entry_style(active); "{}", truncate_chars(&value, value_width))
    ]
}

pub(super) fn pin_create_harness_custom_field(
    input: &TextInputState,
    active: bool,
    label_style: Style,
    prefix: String,
    available: usize,
) -> Line<'static> {
    let suffix = vec![span!(Style::default().fg(Color::Yellow); " [custom]")];
    let suffix_width = span_width(&suffix);
    let value_width = available.saturating_sub(suffix_width).max(1);
    let display = pin_field_visible_window(input.value(), input.cursor(), value_width, active);
    let value_style = pin_create_entry_style(active);
    let left_style = pin_field_edge_style(display.left_indicator, display.edge_style);
    let right_style = pin_field_edge_style(display.right_indicator, display.edge_style);
    let mut spans = vec![
        span!(label_style; "{prefix}"),
        span!(left_style; "{}", display.left_indicator),
        span!(value_style; "{}", display.before_cursor),
        span!(display.cursor_style; "{}", display.cursor_text),
        span!(value_style; "{}", display.after_cursor),
        span!(right_style; "{}", display.right_indicator),
    ];
    spans.extend(suffix);
    Line::from(spans)
}

pub(super) fn pin_harness_is_custom(input: &TextInputState, known_harness_keys: &[String]) -> bool {
    let value = input.value().trim();
    !value.is_empty() && !known_harness_keys.iter().any(|known| known == value)
}

pub(super) fn pin_harness_suffix(
    input: &TextInputState,
    known_harness_keys: &[String],
    width: usize,
) -> Vec<Span<'static>> {
    if width < 3 {
        return Vec::new();
    }
    if known_harness_keys.is_empty() {
        return Vec::new();
    }
    let selected = input.value().trim();
    let mut spans = Vec::new();
    let mut remaining = width;
    for key in known_harness_keys {
        let token = format!(" [{key}]");
        let needed = token.chars().count();
        if needed > remaining {
            if remaining >= 2 {
                spans.push(span!(Modifier::DIM; " …"));
            }
            break;
        }
        if key == selected {
            spans.push(
                span!(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD); "{}", token),
            );
        } else {
            spans.push(span!(Modifier::DIM; "{}", token));
        }
        remaining = remaining.saturating_sub(needed);
    }
    spans
}

pub(super) fn span_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

pub(super) fn pin_create_launch_preview_field(
    state: &PinCreateState,
    inner_width: usize,
) -> Line<'static> {
    pin_launch_preview_field(state.effective_launch_argv(), inner_width)
}

pub(super) fn pin_launch_preview_field(
    effective: Result<LaunchArgvPreview, String>,
    inner_width: usize,
) -> Line<'static> {
    let (prefix, _) = pin_create_prefix("command", false);
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(PIN_CREATE_TEXT_GUTTER.chars().count())
        .max(1);
    let mut spans = vec![span!(Modifier::DIM; "{prefix}")];
    match effective {
        Ok(LaunchArgvPreview { source: _, argv }) if argv.is_empty() => {
            spans.push(span!("{}", PIN_CREATE_TEXT_GUTTER));
            spans.push(span!(Style::default().fg(Color::Yellow); "no default for harness"));
        }
        Ok(LaunchArgvPreview { source, argv }) => {
            let source_label = match source {
                LaunchArgvSource::Default => "default",
                LaunchArgvSource::Override => "override",
            };
            let raw = format!("{source_label}: {}", display_launch_argv(&argv));
            spans.push(span!("{}", PIN_CREATE_TEXT_GUTTER));
            spans.push(span!(Modifier::DIM; "{}", truncate_chars(&raw, value_width)));
        }
        Err(err) => {
            spans.push(span!("{}", PIN_CREATE_TEXT_GUTTER));
            spans.push(
                span!(Style::default().fg(Color::Yellow); "{}", truncate_chars(&err, value_width)),
            );
        }
    }
    Line::from(spans)
}

pub(super) fn pin_create_store_field(
    store: PinCreateStore,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let active = cursor == PinCreateState::FIELD_STORE;
    let (prefix, label_style) = pin_create_prefix("store", active);
    let value = format!(
        "{} {} {}",
        option_token("auto", store == PinCreateStore::Auto),
        option_token("project", store == PinCreateStore::Project),
        option_token("user", store == PinCreateStore::User),
    );
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(PIN_CREATE_TEXT_GUTTER.chars().count());
    line![
        span!(label_style; "{prefix}"),
        span!("{}", PIN_CREATE_TEXT_GUTTER),
        span!(pin_create_entry_style(active); "{}", truncate_chars(&value, value_width))
    ]
}

pub(super) fn pin_create_static_field(
    idx: usize,
    label: &'static str,
    value: &str,
    cursor: usize,
    inner_width: usize,
) -> Line<'static> {
    let (prefix, style) = pin_create_prefix(label, cursor == idx);
    let value = if value.trim().is_empty() { "-" } else { value };
    let raw = format!("{prefix}{PIN_CREATE_TEXT_GUTTER}{value}");
    line![span!(style; "{}", truncate_chars(&raw, inner_width))]
}

pub(super) fn option_pair_line(
    active: bool,
    label: &'static str,
    left: &'static str,
    left_selected: bool,
    right: &'static str,
    right_selected: bool,
    inner_width: usize,
) -> Line<'static> {
    let (prefix, style) = pin_create_prefix(label, active);
    let value = format!(
        "{} {}",
        option_token(left, left_selected),
        option_token(right, right_selected)
    );
    let value_width = inner_width
        .saturating_sub(prefix.chars().count())
        .saturating_sub(PIN_CREATE_TEXT_GUTTER.chars().count());
    line![
        span!(style; "{prefix}"),
        span!("{}", PIN_CREATE_TEXT_GUTTER),
        span!(pin_create_entry_style(active); "{}", truncate_chars(&value, value_width))
    ]
}

pub(super) fn option_token(label: &str, selected: bool) -> String {
    if selected {
        format!("[x] {label}")
    } else {
        format!("[ ] {label}")
    }
}

pub(super) fn pin_create_field(
    idx: usize,
    label: &'static str,
    value: &str,
    cursor: usize,
) -> Line<'static> {
    pin_create_static_field(idx, label, value, cursor, usize::MAX / 2)
}

pub(super) struct PinFieldVisibleWindow {
    pub(super) left_indicator: &'static str,
    pub(super) before_cursor: String,
    pub(super) cursor_text: String,
    pub(super) after_cursor: String,
    pub(super) right_indicator: &'static str,
    pub(super) edge_style: Style,
    pub(super) cursor_style: Style,
}

pub(super) fn pin_field_visible_window(
    value: &str,
    cursor: usize,
    width: usize,
    active: bool,
) -> PinFieldVisibleWindow {
    let width = width.max(1);
    let edge_style = if active {
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
    let chars: Vec<char> = value.chars().collect();
    let total = chars.len();
    let cursor = cursor.min(total);

    if width == 1 {
        let ch = chars.get(cursor).copied().unwrap_or(' ');
        return PinFieldVisibleWindow {
            left_indicator: if cursor > 0 { "<" } else { " " },
            before_cursor: String::new(),
            cursor_text: ch.to_string(),
            after_cursor: String::new(),
            right_indicator: if cursor + usize::from(cursor < total) < total {
                ">"
            } else {
                " "
            },
            edge_style,
            cursor_style,
        };
    }

    let content_width = width.saturating_sub(4).max(1);
    let start = if total <= content_width {
        0
    } else if cursor >= content_width {
        cursor.saturating_sub(content_width.saturating_sub(1))
    } else {
        0
    };
    let end = (start + content_width).min(total);
    let cursor_offset = cursor.saturating_sub(start).min(content_width);
    let before_cursor: String = chars[start..(start + cursor_offset).min(end)]
        .iter()
        .collect();
    let cursor_text = if cursor < end {
        chars[start + cursor_offset].to_string()
    } else if active {
        " ".to_string()
    } else {
        String::new()
    };
    let after_start = (start + cursor_offset + usize::from(cursor < end)).min(end);
    let after_cursor: String = chars[after_start..end].iter().collect();

    PinFieldVisibleWindow {
        left_indicator: if start > 0 { "< " } else { "  " },
        before_cursor,
        cursor_text,
        after_cursor,
        right_indicator: if end < total { " >" } else { "  " },
        edge_style,
        cursor_style,
    }
}

pub(super) fn truncate_chars(value: &str, width: usize) -> String {
    if width == usize::MAX / 2 {
        return value.to_string();
    }
    value.chars().take(width).collect()
}

pub(super) fn pin_create_modal_rect(area: Rect, content_lines: usize) -> Rect {
    let width = std::cmp::min(76, area.width.saturating_sub(4)).max(44);
    modal_rect_for_content(area, width, content_lines, 10)
}

pub(super) fn pin_create_cursor_line(state: &PinCreateState) -> Option<usize> {
    let cursor = state.logical_cursor();
    let mut line_idx = 0;
    for field in state.visible_fields() {
        if field == cursor {
            return Some(line_idx);
        }
        line_idx += 1;
    }
    Some(line_idx)
}
