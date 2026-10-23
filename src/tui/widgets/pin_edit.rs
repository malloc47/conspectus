//! The pin edit form: every field a pin can change, listed with its
//! current value and where that value comes from.
//!
//! The first row renames the pin (a live mux pin renames its tmux
//! session with it). Below it are the label and the fields the pin's
//! kind can override. A row shows the value in effect; editing it makes
//! an override, and Ctrl-R drops the override so the field follows the
//! entity again. Paths complete with Tab, the harness is chosen among
//! the known harnesses, and a harness's launch options (skipping
//! permissions) toggle on the argv row.
//!
//! Up and Down move between rows. Tab completes a path and moves on
//! elsewhere. Left, Right, and Space act on the harness and options
//! rows and edit the text on the others.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::macros::line;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::discovery::harness::{
    argv_without_fragment, launch_options_for, strip_known_launch_option_fragments,
};
use crate::model::{FieldSource, FieldValue, PinKind, RecipeField, SlotFields};
use crate::tui::Msg;
use crate::tui::modal::{Overlay, OverlayOutcome};
use crate::tui::theme::Theme;
use crate::tui::widgets::form::{
    self, Cols, ScrollLinesBody, display_argv, harness_row, note_row, options_row,
    scroll_offset_for_cursor, value_row, wrap_footer,
};
use crate::tui::widgets::input::{InputOutcome, TextInputState};
use crate::tui::widgets::launch_spec_form::parse_launch_argv;
use crate::tui::widgets::path_omnibox::{PathCandidate, PathOmniboxOutcome, PathOmniboxState};

/// What the form saves: a rename, overrides to set, overrides to drop.
#[derive(Debug, Clone, PartialEq)]
pub struct PinEditRequest {
    pub pin: String,
    pub rename: Option<String>,
    pub set: Vec<(String, FieldValue)>,
    pub unset: Vec<String>,
}

/// What opening the form needs to know about the pin.
#[derive(Debug, Clone)]
pub struct PinEditInit {
    pub pin: String,
    pub kind: PinKind,
    /// The status word shown next to the kind (`live`, `gone`, ...).
    pub status: String,
    /// Whether this is a live mux pin, whose rename renames the session.
    pub live_mux: bool,
    /// Each field's value in effect and the layer it came from.
    pub effective: Vec<RecipeField>,
    /// The overrides the pin has now.
    pub overrides: SlotFields,
    /// Each field's value without the pin's overrides.
    pub inherited: Vec<RecipeField>,
    pub known_pin_names: Vec<String>,
    pub known_harness_keys: Vec<String>,
    pub cwd_candidates: Vec<PathCandidate>,
    pub defaults: Option<crate::launch_default::LaunchDefaults>,
}

#[derive(Debug, Clone)]
enum Control {
    Text(TextInputState),
    Path(PathOmniboxState),
    Harness(TextInputState),
}

impl Control {
    fn value(&self) -> &str {
        match self {
            Self::Text(input) | Self::Harness(input) => input.value(),
            Self::Path(path) => path.value(),
        }
    }

    fn replace(&mut self, value: String) {
        match self {
            Self::Text(input) => *input = TextInputState::new(input.title().to_string(), value),
            Self::Harness(input) => {
                *input = TextInputState::new(input.title().to_string(), value);
            }
            Self::Path(path) => path.set_value(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    /// The pin's own name.
    PinName,
    /// A field of the pin's recipe (the label is one).
    Field(&'static str),
}

#[derive(Debug, Clone)]
struct Row {
    which: Which,
    label: String,
    control: Control,
    /// The text when the form opened.
    initial: String,
    /// The pin had an override for this field when the form opened.
    overridden: bool,
    /// The value and layer the field has without an override.
    inherited: (String, Option<FieldSource>),
    /// The layer in effect when the form opened.
    source: Option<FieldSource>,
    /// Ctrl-R was pressed: the override goes away on save.
    cleared: bool,
}

impl Row {
    fn edited(&self) -> bool {
        self.control.value() != self.initial
    }

    /// The note after the value: where it comes from, and what saving
    /// will do.
    fn tag(&self) -> String {
        match self.which {
            Which::PinName => "pin name".to_string(),
            Which::Field(_) => {
                if self.cleared {
                    return match self.inherited.1 {
                        Some(layer) => format!("← inherits {}", layer.as_str()),
                        None => "← inherits (unset)".to_string(),
                    };
                }
                if self.edited() {
                    return "→ will override".to_string();
                }
                match (self.overridden, self.source) {
                    (true, _) => "override".to_string(),
                    (false, Some(layer)) => layer.as_str().to_string(),
                    (false, None) => "unset".to_string(),
                }
            }
        }
    }
}

/// Form state: one row per editable field, and the cursor.
#[derive(Debug, Clone)]
pub struct PinEditFormState {
    pin: String,
    kind: PinKind,
    status: String,
    live_mux: bool,
    rows: Vec<Row>,
    focus: usize,
    known_pin_names: Vec<String>,
    known_harness_keys: Vec<String>,
    defaults: Option<crate::launch_default::LaunchDefaults>,
    error: Option<String>,
}

/// What a cursor position is on: a row, or the harness options row that
/// sits under the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Row(usize),
    Options,
}

fn text_of(value: &FieldValue) -> String {
    match value {
        FieldValue::Text(text) => text.clone(),
        FieldValue::List(list) => display_argv(list),
    }
}

fn lookup(fields: &[RecipeField], name: &str) -> (String, Option<FieldSource>) {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| (text_of(&field.value), Some(field.source)))
        .unwrap_or_default()
}

impl PinEditFormState {
    pub fn new(init: PinEditInit) -> Self {
        let mut rows = vec![Row {
            which: Which::PinName,
            label: "pin name".to_string(),
            control: Control::Text(TextInputState::new("pin name", init.pin.clone())),
            initial: init.pin.clone(),
            overridden: false,
            inherited: (init.pin.clone(), None),
            source: None,
            cleared: false,
        }];
        let fields: Vec<&'static str> = std::iter::once("label")
            .chain(init.kind.overridable_fields().iter().copied())
            .collect();
        for field in fields {
            let (effective, source) = lookup(&init.effective, field);
            let inherited = lookup(&init.inherited, field);
            let overridden = init.overrides.contains_key(field);
            let control = match (init.kind, field) {
                (PinKind::Mux, "cwd")
                | (PinKind::Checkout, "location")
                | (PinKind::Repo, "path") => {
                    let mut path = PathOmniboxState::new(field, effective.clone());
                    path.set_known_candidates(init.cwd_candidates.clone());
                    Control::Path(path)
                }
                (PinKind::Mux, "harness") => {
                    Control::Harness(TextInputState::new(field, effective.clone()))
                }
                _ => Control::Text(TextInputState::new(field, effective.clone())),
            };
            rows.push(Row {
                which: Which::Field(field),
                label: field.to_string(),
                control,
                initial: effective,
                overridden,
                inherited,
                source,
                cleared: false,
            });
        }
        Self {
            pin: init.pin,
            kind: init.kind,
            status: init.status,
            live_mux: init.live_mux,
            rows,
            focus: 0,
            known_pin_names: init.known_pin_names,
            known_harness_keys: init.known_harness_keys,
            defaults: init.defaults,
            error: None,
        }
    }

    pub fn pin(&self) -> &str {
        &self.pin
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The text of the row for `field` (`None` for an unknown field).
    pub fn field_text(&self, field: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|row| matches!(row.which, Which::Field(name) if name == field))
            .map(|row| row.control.value())
    }

    /// The pin name row's text.
    pub fn name_text(&self) -> &str {
        self.rows[0].control.value()
    }

    /// The labels of the rows, in order.
    pub fn row_labels(&self) -> Vec<&str> {
        self.rows.iter().map(|row| row.label.as_str()).collect()
    }

    /// Which row the cursor is on, by label (`None` on the options row).
    pub fn focused_label(&self) -> Option<&str> {
        match self.stops().get(self.focus) {
            Some(Stop::Row(idx)) => Some(self.rows[*idx].label.as_str()),
            _ => None,
        }
    }

    fn harness_text(&self) -> String {
        self.rows
            .iter()
            .find(|row| row.which == Which::Field("harness"))
            .map(|row| row.control.value().trim().to_string())
            .unwrap_or_default()
    }

    fn has_options(&self) -> bool {
        self.kind == PinKind::Mux && !launch_options_for(&self.harness_text()).is_empty()
    }

    fn stops(&self) -> Vec<Stop> {
        let mut stops = Vec::new();
        for (idx, row) in self.rows.iter().enumerate() {
            stops.push(Stop::Row(idx));
            if row.which == Which::Field("harness") && self.has_options() {
                stops.push(Stop::Options);
            }
        }
        stops
    }

    fn move_focus(&mut self, delta: i32) {
        let count = self.stops().len();
        self.focus = crate::tui::cursor::wrap_step(self.focus.min(count - 1), count, delta);
    }

    fn row_index(&self, field: &'static str) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.which == Which::Field(field))
    }

    /// The argv a pin with this harness starts from when it has none of
    /// its own: its inherited argv, else a profile or the harness default.
    fn default_argv(&self, harness: &str) -> Vec<String> {
        let cwd = self
            .row_index("cwd")
            .map(|idx| self.rows[idx].control.value().trim().to_string())
            .unwrap_or_default();
        form::default_argv(self.defaults.as_ref(), harness, &cwd)
    }

    fn argv_now(&self) -> Vec<String> {
        let Some(idx) = self.row_index("argv") else {
            return Vec::new();
        };
        let typed =
            parse_launch_argv(self.rows[idx].control.value().trim(), "edit").unwrap_or_default();
        if typed.is_empty() {
            self.default_argv(&self.harness_text())
        } else {
            typed
        }
    }

    fn set_argv(&mut self, argv: &[String]) {
        if let Some(idx) = self.row_index("argv") {
            self.rows[idx].control.replace(display_argv(argv));
            self.rows[idx].cleared = false;
        }
    }

    fn toggle_option(&mut self, delta: i32) {
        let harness = self.harness_text();
        let options = launch_options_for(&harness);
        if options.is_empty() {
            return;
        }
        let argv = self.argv_now();
        let selected: Vec<usize> = options
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                crate::discovery::harness::argv_contains_fragment(&argv, option.argv)
            })
            .map(|(idx, _)| idx)
            .collect();
        let idx = match (selected.as_slice(), delta) {
            ([only], -1 | 1) if options.len() > 1 => {
                crate::tui::cursor::wrap_step(*only, options.len(), delta)
            }
            ([first, ..], _) => *first,
            ([], _) => 0,
        };
        let option = options[idx];
        let next = if crate::discovery::harness::argv_contains_fragment(&argv, option.argv) {
            argv_without_fragment(argv, option.argv)
        } else {
            crate::discovery::harness::argv_with_fragment(argv, option.argv)
        };
        self.set_argv(&next);
    }

    fn cycle_harness(&mut self, delta: i32) {
        if self.known_harness_keys.is_empty() {
            return;
        }
        let Some(idx) = self.row_index("harness") else {
            return;
        };
        let old = self.harness_text();
        let old_default = self.default_argv(&old);
        let typed = parse_launch_argv(
            self.row_index("argv")
                .map_or("", |i| self.rows[i].control.value())
                .trim(),
            "edit",
        )
        .unwrap_or_default();
        let keys = &self.known_harness_keys;
        let next_idx = match keys.iter().position(|key| *key == old) {
            Some(pos) => crate::tui::cursor::wrap_step(pos, keys.len(), delta),
            None if delta < 0 => keys.len() - 1,
            None => 0,
        };
        let next = keys[next_idx].clone();
        self.rows[idx].control.replace(next.clone());
        self.rows[idx].cleared = false;
        if !typed.is_empty() {
            let stripped = strip_known_launch_option_fragments(typed);
            if stripped == old_default {
                let new_default = self.default_argv(&next);
                self.set_argv(&new_default);
            } else {
                self.set_argv(&stripped);
            }
        }
    }

    fn inherit(&mut self) {
        let Some(Stop::Row(idx)) = self.stops().get(self.focus).copied() else {
            return;
        };
        let row = &mut self.rows[idx];
        if row.which == Which::PinName {
            let initial = row.initial.clone();
            row.control.replace(initial);
            return;
        }
        let value = row.inherited.0.clone();
        row.control.replace(value);
        row.cleared = true;
    }

    /// The request the form would save, or why it can't.
    pub fn request(&self) -> Result<PinEditRequest, String> {
        let name = self.rows[0].control.value().trim().to_string();
        let rename = if name == self.pin {
            None
        } else {
            crate::pin_slots::validate_id(&name).map_err(|err| err.to_string())?;
            if self.known_pin_names.contains(&name) {
                return Err(format!("a pin named `{name}` already exists"));
            }
            Some(name)
        };
        let mut set = Vec::new();
        let mut unset = Vec::new();
        for row in &self.rows[1..] {
            let Which::Field(field) = row.which else {
                continue;
            };
            let text = row.control.value().trim().to_string();
            if row.cleared {
                if row.overridden {
                    unset.push(field.to_string());
                }
                continue;
            }
            if !row.edited() {
                continue;
            }
            if text.is_empty() {
                if row.overridden {
                    unset.push(field.to_string());
                }
                continue;
            }
            let (name, value) =
                crate::pin_ops::parse_override(self.kind, &format!("{field}={text}"))
                    .map_err(|err| err.to_string())?;
            set.push((name, value));
        }
        Ok(PinEditRequest {
            pin: self.pin.clone(),
            rename,
            set,
            unset,
        })
    }

    fn try_commit(&mut self) -> OverlayOutcome {
        match self.request() {
            Ok(request) => {
                if request.rename.is_none() && request.set.is_empty() && request.unset.is_empty() {
                    return OverlayOutcome::Close;
                }
                OverlayOutcome::Commit(Box::new(Msg::CommitPinEdit(request)))
            }
            Err(message) => {
                self.error = Some(message);
                OverlayOutcome::Consumed
            }
        }
    }

    fn typed(&mut self, idx: usize, key: KeyEvent) -> OverlayOutcome {
        let before = self.rows[idx].control.value().to_string();
        let outcome = match &mut self.rows[idx].control {
            Control::Text(input) | Control::Harness(input) => input.handle_key(key),
            Control::Path(path) => {
                path.handle_key(key);
                InputOutcome::Continue
            }
        };
        if self.rows[idx].control.value() != before {
            self.rows[idx].cleared = false;
            self.error = None;
        }
        match outcome {
            InputOutcome::Continue => OverlayOutcome::Consumed,
            InputOutcome::Cancel => OverlayOutcome::Close,
            InputOutcome::Confirm(_) => self.try_commit(),
        }
    }
}

impl Overlay for PinEditFormState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: KeyEvent) -> OverlayOutcome {
        use crate::tui::keybindings::overlay::{FormKey, PIN_EDIT_KEYS, resolve};
        let stop = self
            .stops()
            .get(self.focus)
            .copied()
            .unwrap_or(Stop::Row(0));
        let option_delta = match resolve(PIN_EDIT_KEYS, key) {
            Some(FormKey::Abort | FormKey::Cancel) => return OverlayOutcome::Close,
            Some(FormKey::Up | FormKey::Back) => {
                self.move_focus(-1);
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Down) => {
                self.move_focus(1);
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Tab) => {
                if let Stop::Row(idx) = stop
                    && let Control::Path(path) = &mut self.rows[idx].control
                    && matches!(
                        path.handle_key(key),
                        PathOmniboxOutcome::Completed | PathOmniboxOutcome::Changed
                    )
                {
                    self.rows[idx].cleared = false;
                    return OverlayOutcome::Consumed;
                }
                self.move_focus(1);
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Inherit) => {
                self.inherit();
                return OverlayOutcome::Consumed;
            }
            Some(FormKey::Submit) => return self.try_commit(),
            Some(FormKey::OptionPrev) => Some(-1),
            Some(FormKey::OptionNext) => Some(1),
            None => None,
        };
        match stop {
            Stop::Options => {
                if let Some(delta) = option_delta {
                    self.toggle_option(delta);
                }
                OverlayOutcome::Consumed
            }
            Stop::Row(idx) => {
                if let (Control::Harness(_), Some(delta)) = (&self.rows[idx].control, option_delta)
                {
                    self.cycle_harness(delta);
                    return OverlayOutcome::Consumed;
                }
                self.typed(idx, key)
            }
        }
    }
}

/// Popup renderer for the form.
pub struct PinEditFormWidget<'a> {
    state: &'a PinEditFormState,
    theme: &'a Theme,
}

impl<'a> PinEditFormWidget<'a> {
    pub fn new(state: &'a PinEditFormState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for PinEditFormWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let state = self.state;
        let modal_width = 84;
        let inner_width =
            usize::from(std::cmp::min(modal_width, area.width.saturating_sub(4)).max(44)) - 2;
        let label_width = state
            .rows
            .iter()
            .map(|row| row.label.chars().count())
            .max()
            .unwrap_or(11)
            .max(11);
        let cols = Cols::new(inner_width).with_label(label_width);
        let stops = state.stops();
        let focus = stops.get(state.focus).copied().unwrap_or(Stop::Row(0));

        let mut lines: Vec<Line<'static>> = vec![
            note_row(
                "pin",
                &format!("{} · {} · {}", state.pin, state.kind.as_str(), state.status),
                cols,
            ),
            line![""],
        ];
        let mut cursor_line = 0;
        for (idx, row) in state.rows.iter().enumerate() {
            let active = focus == Stop::Row(idx);
            if active {
                cursor_line = lines.len();
            }
            lines.push(render_row(state, row, active, cols));
            if row.which == Which::PinName && state.live_mux {
                lines.push(note_row("", "renames the tmux session too", cols));
            }
            if row.which == Which::Field("harness") && state.has_options() {
                let active = focus == Stop::Options;
                if active {
                    cursor_line = lines.len();
                }
                let harness = state.harness_text();
                lines.push(options_row(
                    "options",
                    launch_options_for(&harness),
                    &state.argv_now(),
                    active,
                    cols,
                ));
            }
        }
        if let Some(err) = state.error() {
            lines.push(line![""]);
            lines.push(line![Span::styled(
                err.to_string(),
                Style::default().fg(self.theme.error)
            )]);
        }
        lines.push(line![""]);
        lines.extend(wrap_footer(
            &crate::tui::keybindings::overlay::footer(
                crate::tui::keybindings::overlay::PIN_EDIT_KEYS,
            ),
            inner_width,
        ));

        let modal = form::modal_rect(area, modal_width, lines.len(), 10);
        let inner_height = usize::from(modal.height.saturating_sub(2));
        let body = ScrollLinesBody {
            scroll_offset: scroll_offset_for_cursor(cursor_line, inner_height, lines.len()),
            lines,
            inner_width: usize::from(modal.width.saturating_sub(2)),
            inner_height,
        };
        crate::tui::widgets::popup_frame::themed_popup(body, line![" Edit pin "], self.theme)
            .render(area, buf);
    }
}

fn render_row(state: &PinEditFormState, row: &Row, active: bool, cols: Cols) -> Line<'static> {
    let tag = row.tag();
    let tag_text = format!("  {tag}");
    let tag_span = Span::styled(
        tag_text.clone(),
        Style::default().add_modifier(Modifier::DIM),
    );
    match &row.control {
        Control::Harness(input) => {
            let mut line = harness_row(&row.label, input, &state.known_harness_keys, active, cols);
            line.spans.push(tag_span);
            line
        }
        Control::Path(path) => {
            let mut line = form::path_row(&row.label, path, active, cols);
            line.spans.push(tag_span);
            line
        }
        Control::Text(input) => value_row(
            &row.label,
            input.value(),
            input.cursor(),
            active,
            cols,
            Some(form::Suffix {
                width: tag_text.chars().count(),
                spans: vec![tag_span],
            }),
        ),
    }
}

#[cfg(test)]
#[path = "pin_edit_tests.rs"]
mod tests;
