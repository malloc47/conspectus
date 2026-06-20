//! Centered modal text-input overlay, per ADR 0030.
//!
//! Hosts a [`tui_input::Input`] inside a bordered Ratatui frame and
//! locks the surrounding focus cycle while open. Three callers share
//! this primitive: the rename overlay (`H-RENAME-011`), the `/`
//! search overlay (`T8-017`), and the inline mux picker (`P8-014`).
//! Bug fixes for Unicode cursor math, word navigation, and paste
//! handling land once instead of three times.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use tui_popup::KnownSize;

use crate::tui::Theme;

/// What the host should do after passing a key event through the
/// overlay. `Continue` means the overlay stays open; `Confirm`
/// carries the trimmed buffer value back to the host; `Cancel`
/// means the overlay closes without committing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputOutcome {
    Continue,
    Confirm(String),
    Cancel,
}

/// Caller-owned state for a single text-input overlay. The state
/// holds the underlying [`tui_input::Input`] plus the title that
/// renders as the modal's border label.
#[derive(Debug, Clone)]
pub struct TextInputState {
    title: String,
    input: Input,
}

impl TextInputState {
    /// Create a new state pre-populated with `initial`. The cursor
    /// lands at the end so typing appends naturally — the rename
    /// flow wants the caller to be able to edit an existing alias
    /// or harness title verbatim.
    pub fn new(title: impl Into<String>, initial: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            input: Input::new(initial.into()),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    pub fn cursor(&self) -> usize {
        self.input.cursor()
    }

    /// Pass a crossterm key event through the overlay.
    ///
    /// `Enter` returns `Confirm(value)`; `Esc` returns `Cancel`;
    /// every other key is forwarded to the line editor. ADR 0030
    /// locks the surrounding `Tab` focus cycle, so `Tab` is
    /// swallowed here (the line editor itself drops it too).
    pub fn handle_key(&mut self, event: KeyEvent) -> InputOutcome {
        match event.code {
            KeyCode::Enter => InputOutcome::Confirm(self.input.value().to_string()),
            KeyCode::Esc => InputOutcome::Cancel,
            KeyCode::Tab => InputOutcome::Continue,
            _ => {
                if event.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(event.code, KeyCode::Char('c'))
                {
                    return InputOutcome::Cancel;
                }
                self.input.handle_event(&CrosstermEvent::Key(event));
                InputOutcome::Continue
            }
        }
    }
}

/// Status-bar legend rendered while the overlay is open. Mirrors
/// ADR 0030's locked key map so the operator always sees the
/// active bindings.
pub const STATUS_LEGEND: &str = "Enter confirm · Esc cancel";

/// Centered modal rendering of [`TextInputState`]. Defaults to the
/// upstream popup palette when no `.theme(&Theme)` is provided so
/// the constructor stays a one-arg call from existing sites.
pub struct TextInputWidget<'a> {
    state: &'a TextInputState,
    theme: Option<&'a Theme>,
}

impl<'a> TextInputWidget<'a> {
    pub fn new(state: &'a TextInputState) -> Self {
        Self { state, theme: None }
    }

    /// Honor operator `[tui.theme]` overrides (ADR 0032). Without
    /// this the bordered modal renders with the upstream defaults.
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = Some(theme);
        self
    }
}

impl Widget for TextInputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // H-WIDG-004: framing through `tui_popup::Popup`. When a
        // theme is set we route through `themed_popup`; otherwise
        // fall back to upstream defaults so call sites that don't
        // pass a theme still render legibly.
        let modal = centered_modal_rect(area);
        let body = TextInputBody {
            state: self.state,
            inner_width: modal.width.saturating_sub(2) as usize,
            inner_height: modal.height.saturating_sub(2) as usize,
        };
        let title = Line::from(self.state.title.clone());
        if let Some(theme) = self.theme {
            let popup = crate::tui::widgets::popup_frame::themed_popup(body, title, theme);
            popup.render(area, buf);
        } else {
            let popup = tui_popup::Popup::new(body).title(title);
            popup.render(area, buf);
        }
    }
}

/// Body wrapper that renders the visible-window text and reverses
/// the cursor cell. Sizing follows the cap dims so the popup auto-
/// sizing reproduces the in-tree 3-row rect.
struct TextInputBody<'a> {
    state: &'a TextInputState,
    inner_width: usize,
    inner_height: usize,
}

impl KnownSize for TextInputBody<'_> {
    fn width(&self) -> usize {
        self.inner_width
    }

    fn height(&self) -> usize {
        self.inner_height
    }
}

impl Widget for TextInputBody<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let value = self.state.input.value();
        let cursor = self.state.input.cursor();
        let inner_width = area.width as usize;

        let display = visible_window(value, cursor, inner_width);
        Paragraph::new(Line::from(display.text.clone())).render(area, buf);

        if let Some(cell) = buf.cell_mut((area.x + display.cursor_offset as u16, area.y)) {
            cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
        }
    }
}

/// 3-row × min(60, width-4) centered modal, per ADR 0030.
pub fn centered_modal_rect(area: Rect) -> Rect {
    let width = std::cmp::min(60, area.width.saturating_sub(4));
    let width = width.max(20);
    let height: u16 = 3;
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

struct VisibleWindow {
    text: String,
    cursor_offset: usize,
}

/// Slide the visible window so the cursor is always within
/// `inner_width`. Mirrors the behavior in
/// `tui_input::backend::crossterm::write` but written for Ratatui's
/// buffer-mutation API.
fn visible_window(value: &str, cursor: usize, inner_width: usize) -> VisibleWindow {
    let inner_width = inner_width.max(1);
    let chars: Vec<char> = value.chars().collect();
    let total = chars.len();
    // Leave one column for the cursor so the operator can see the
    // tail of the value.
    let visible = inner_width.saturating_sub(1).max(1);
    let start = if total <= visible {
        0
    } else if cursor >= visible {
        cursor.saturating_sub(visible)
    } else {
        0
    };
    let end = (start + inner_width).min(total);
    let text: String = chars[start..end].iter().collect();
    let cursor_offset = cursor.saturating_sub(start);
    VisibleWindow {
        text,
        cursor_offset,
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

    #[test]
    fn enter_confirms_with_current_value() {
        let mut state = TextInputState::new(" rename ", "hello");
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(outcome, InputOutcome::Confirm("hello".to_string()));
    }

    #[test]
    fn esc_cancels_without_committing() {
        let mut state = TextInputState::new(" rename ", "hello");
        let outcome = state.handle_key(key(KeyCode::Esc));
        assert_eq!(outcome, InputOutcome::Cancel);
        // Cancel must not mutate the buffer.
        assert_eq!(state.value(), "hello");
    }

    #[test]
    fn tab_is_swallowed_while_overlay_is_open() {
        let mut state = TextInputState::new(" rename ", "hello");
        let outcome = state.handle_key(key(KeyCode::Tab));
        assert_eq!(outcome, InputOutcome::Continue);
        assert_eq!(state.value(), "hello");
    }

    #[test]
    fn ctrl_c_cancels() {
        let mut state = TextInputState::new(" rename ", "hello");
        let event = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let outcome = state.handle_key(event);
        assert_eq!(outcome, InputOutcome::Cancel);
    }

    #[test]
    fn typing_appends_to_buffer() {
        let mut state = TextInputState::new(" rename ", "abc");
        let outcome = state.handle_key(key(KeyCode::Char('d')));
        assert_eq!(outcome, InputOutcome::Continue);
        assert_eq!(state.value(), "abcd");
    }

    #[test]
    fn backspace_deletes_char_before_cursor() {
        let mut state = TextInputState::new(" rename ", "abc");
        state.handle_key(key(KeyCode::Backspace));
        assert_eq!(state.value(), "ab");
    }

    #[test]
    fn enter_returns_trimmed_or_raw_value_per_caller_choice() {
        // The widget never trims itself — callers decide how to
        // treat whitespace. Round-trip an all-whitespace buffer to
        // prove the widget keeps it intact.
        let mut state = TextInputState::new(" rename ", "   ");
        let outcome = state.handle_key(key(KeyCode::Enter));
        assert_eq!(outcome, InputOutcome::Confirm("   ".to_string()));
    }

    #[test]
    fn centered_modal_caps_width_at_60() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 30,
        };
        let modal = centered_modal_rect(area);
        assert_eq!(modal.width, 60);
        assert_eq!(modal.height, 3);
        assert!(modal.x > 0);
        assert!(modal.y > 0);
    }

    #[test]
    fn centered_modal_scales_down_on_narrow_terminal() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 12,
        };
        let modal = centered_modal_rect(area);
        assert_eq!(modal.width, 26); // 30 - 4
        assert_eq!(modal.height, 3);
    }

    #[test]
    fn centered_modal_floors_width_at_20() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 18,
            height: 6,
        };
        let modal = centered_modal_rect(area);
        assert_eq!(modal.width, 20);
    }

    #[test]
    fn visible_window_keeps_cursor_in_view_for_long_values() {
        let value: String = (b'a'..=b'z').map(|b| b as char).collect();
        let cursor = value.chars().count();
        let window = visible_window(&value, cursor, 10);
        assert!(window.text.len() <= 10);
        assert!(window.cursor_offset < window.text.len() + 1);
    }
}
