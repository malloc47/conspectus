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
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::discovery::harness::{
    argv_without_fragment, launch_options_for, strip_known_launch_option_fragments,
};
use crate::model::{FieldSource, FieldValue, PinKind, RecipeField, SlotFields};
use crate::tui::Msg;
use crate::tui::theme::Theme;
use crate::tui::widgets::form::{self, Cols, display_argv, footer_lines, note_row, options_row};
use crate::tui::widgets::launch_spec_form::parse_launch_argv;
use tuikit::form::FormBody;
use tuikit::overlay::Outcome;
use tuikit::path_field::{Candidate, PathField};
use tuikit::text_edit::LineEdit;

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
    pub cwd_candidates: Vec<Candidate>,
    pub defaults: Option<crate::launch_default::LaunchDefaults>,
}

#[derive(Debug, Clone)]
enum Control {
    Text(LineEdit),
    Path(PathField),
    Harness(LineEdit),
}

impl Control {
    fn value(&self) -> &str {
        match self {
            Self::Text(input) | Self::Harness(input) => input.value(),
            Self::Path(path) => path.value(),
        }
    }

    /// The value saving commits: trimmed, and with `~` expanded in a
    /// path, which the commands that read the pin take as given.
    fn committed(&self) -> String {
        match self {
            Self::Text(input) | Self::Harness(input) => input.value().trim().to_string(),
            Self::Path(path) => path.expanded_value(),
        }
    }

    fn replace(&mut self, value: String) {
        match self {
            Self::Text(input) | Self::Harness(input) => *input = LineEdit::new(value),
            Self::Path(path) => {
                path.set_value(value);
            }
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

    /// The word at the end of the row: where the value comes from, and
    /// what saving will do. At most [`TAG_WIDTH`] cells.
    fn tag(&self) -> &'static str {
        match self.which {
            Which::PinName => "",
            Which::Field(_) => {
                if self.cleared {
                    "will follow"
                } else if self.edited() {
                    "will keep"
                } else if self.overridden {
                    "kept"
                } else {
                    layer_word(self.source)
                }
            }
        }
    }

    /// What the focused row says about itself, in two lines: its state,
    /// and what following would give.
    fn hint(&self, kind: PinKind) -> (String, String) {
        let Which::Field(_) = self.which else {
            return (
                "the name this pin is listed under".to_string(),
                String::new(),
            );
        };
        let value = self.control.value().trim();
        let (inherited_value, inherited_layer) = &self.inherited;
        let follows = |value: &str, layer: Option<FieldSource>| match layer {
            Some(FieldSource::Live) => format!("following the running {}: {value}", kind.as_str()),
            Some(FieldSource::History) => format!("following the last seen value: {value}"),
            Some(FieldSource::Snapshot) => {
                format!("following the value saved with the pin: {value}")
            }
            Some(FieldSource::Default) => format!("following the default: {value}"),
            _ => "nothing to follow yet; type a value to keep with this pin".to_string(),
        };
        let back = || {
            let again = follows(inherited_value, *inherited_layer);
            format!(
                "Ctrl-R goes back to {}",
                again.replacen("following", "following —", 1)
            )
        };
        if self.cleared {
            return (
                "will follow again when saved".to_string(),
                follows(inherited_value, *inherited_layer),
            );
        }
        if self.edited() {
            let second = if inherited_layer.is_some() {
                back()
            } else {
                String::new()
            };
            return (format!("will be kept with this pin: {value}"), second);
        }
        if self.overridden {
            return (format!("kept with this pin: {value}"), back());
        }
        (follows(value, self.source), String::new())
    }
}

/// The word for where a value comes from.
fn layer_word(layer: Option<FieldSource>) -> &'static str {
    match layer {
        Some(FieldSource::Live) => "running",
        Some(FieldSource::History) => "last seen",
        Some(FieldSource::Snapshot) => "saved",
        Some(FieldSource::Default) => "default",
        Some(FieldSource::Override) => "kept",
        None => "unset",
    }
}

/// Cells of the right-hand column that names where a value comes from.
const TAG_WIDTH: usize = 11;

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
            control: Control::Text(LineEdit::new(init.pin.clone())),
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
                | (PinKind::Repo, "path") => Control::Path(crate::tui::paths::directory_field(
                    effective.clone(),
                    init.cwd_candidates.clone(),
                )),
                (PinKind::Mux, "harness") => Control::Harness(LineEdit::new(effective.clone())),
                _ => Control::Text(LineEdit::new(effective.clone())),
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
        self.focus = tuikit::selection::Ends::Wrap.step(self.focus.min(count - 1), count, delta);
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
                tuikit::scroll::wrap_step(*only, options.len(), delta)
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
            Some(pos) => tuikit::scroll::wrap_step(pos, keys.len(), delta),
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
            let text = row.control.committed();
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

    fn try_commit(&mut self) -> Outcome<Msg> {
        match self.request() {
            Ok(request) => {
                if request.rename.is_none() && request.set.is_empty() && request.unset.is_empty() {
                    return Outcome::Close;
                }
                Outcome::Commit(Msg::CommitPinEdit(request))
            }
            Err(message) => {
                self.error = Some(message);
                Outcome::Consumed
            }
        }
    }

    /// A key on a text row: an edit that changes the value takes back
    /// a pending follow and clears the error.
    fn typed(&mut self, idx: usize, key: KeyEvent) -> Outcome<Msg> {
        let changed = match &mut self.rows[idx].control {
            Control::Text(input) | Control::Harness(input) => input.handle(key).changed,
            Control::Path(path) => path.handle(key).changed,
        };
        if changed {
            self.rows[idx].cleared = false;
            self.error = None;
        }
        Outcome::Consumed
    }
}

impl PinEditFormState {
    /// One key: edit, move between the rows, change an option, save, or close.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        use crate::tui::keybindings::overlay::PIN_EDIT_KEYS;
        use tuikit::keys::{FormKey, resolve};
        let stop = self
            .stops()
            .get(self.focus)
            .copied()
            .unwrap_or(Stop::Row(0));
        let option_delta = match resolve(PIN_EDIT_KEYS, key) {
            Some(FormKey::Abort | FormKey::Cancel) => return Outcome::Close,
            Some(FormKey::Up | FormKey::Back) => {
                self.move_focus(-1);
                return Outcome::Consumed;
            }
            Some(FormKey::Down) => {
                self.move_focus(1);
                return Outcome::Consumed;
            }
            Some(FormKey::Tab) => {
                if let Stop::Row(idx) = stop
                    && let Control::Path(path) = &mut self.rows[idx].control
                    && path.complete()
                {
                    self.rows[idx].cleared = false;
                    return Outcome::Consumed;
                }
                self.move_focus(1);
                return Outcome::Consumed;
            }
            Some(FormKey::Inherit) => {
                self.inherit();
                return Outcome::Consumed;
            }
            Some(FormKey::Submit) => return self.try_commit(),
            Some(FormKey::OptionPrev) => Some(-1),
            Some(FormKey::OptionNext | FormKey::Toggle) => Some(1),
            None => None,
        };
        match stop {
            Stop::Options => {
                if let Some(delta) = option_delta {
                    self.toggle_option(delta);
                }
                Outcome::Consumed
            }
            Stop::Row(idx) => {
                if let (Control::Harness(_), Some(delta)) = (&self.rows[idx].control, option_delta)
                {
                    self.cycle_harness(delta);
                    return Outcome::Consumed;
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
        let frame = form::modal_frame(self.theme, "Edit pin");
        let inner_width = usize::from(frame.inner_width(area));
        let label_width = state
            .rows
            .iter()
            .map(|row| tuikit::text::width(&row.label))
            .max()
            .unwrap_or(11)
            .clamp(11, 16);
        let cols = Cols::new(inner_width).with_label(label_width);
        let grid = Grid::new(inner_width, label_width);
        let stops = state.stops();
        let focus = stops.get(state.focus).copied().unwrap_or(Stop::Row(0));
        let dim = Style::default().add_modifier(self.theme.muted);

        let mut top: Vec<Line<'static>> = vec![note_row(
            "pin",
            &format!("{} · {} · {}", state.pin, state.kind.as_str(), state.status),
            cols,
            self.theme,
        )];
        let help = format!(
            "A blank field follows the running {}, then its last seen value, then what was \
             saved with the pin. Type to keep a value of your own with this pin; Ctrl-R goes \
             back to following.",
            state.kind.as_str()
        );
        top.extend(
            tuikit::text::wrap(&help, inner_width.saturating_sub(2))
                .into_iter()
                .map(|text| Line::styled(format!("  {}", text.trim_end()), dim)),
        );
        top.push(line![""]);

        let mut rows: Vec<Line<'static>> = Vec::new();
        let mut cursor_row = 0;
        let mut detail = (String::new(), String::new());
        for (idx, row) in state.rows.iter().enumerate() {
            let active = focus == Stop::Row(idx);
            if active {
                cursor_row = rows.len();
                detail = row.hint(state.kind);
                if detail.1.is_empty() {
                    detail.1 = row_help(row);
                }
            }
            rows.push(render_row(state, row, active, grid, cols, self.theme));
            if row.which == Which::PinName && state.live_mux {
                rows.push(note_row(
                    "",
                    "renames the tmux session too",
                    cols,
                    self.theme,
                ));
            }
            if row.which == Which::Field("harness") && state.has_options() {
                let active = focus == Stop::Options;
                if active {
                    cursor_row = rows.len();
                    detail = (
                        "launch options for this harness".to_string(),
                        "Space or ←/→ turns an option on or off".to_string(),
                    );
                }
                let harness = state.harness_text();
                rows.push(options_row(
                    "options",
                    launch_options_for(&harness),
                    &state.argv_now(),
                    active,
                    cols,
                    self.theme,
                ));
            }
        }

        // The focused row's explanation always takes the same two lines,
        // so the form never changes height as the cursor moves.
        let detail_width = inner_width.saturating_sub(2);
        let mut bottom: Vec<Line<'static>> = vec![
            line![""],
            Line::styled(
                format!("  {}", tuikit::text::fit(&detail.0, detail_width)),
                dim,
            ),
            Line::styled(
                format!("  {}", tuikit::text::fit(&detail.1, detail_width)),
                dim,
            ),
        ];
        if let Some(err) = state.error() {
            bottom.push(Line::styled(
                format!("  {}", tuikit::text::fit(err, detail_width)),
                Style::default().fg(self.theme.error),
            ));
        }
        bottom.push(line![""]);
        bottom.extend(footer_lines(
            crate::tui::keybindings::overlay::PIN_EDIT_KEYS,
            inner_width,
            self.theme,
        ));

        // The rows scroll; the summary above them and the explanation,
        // error, and keys below stay in view.
        let body = FormBody::new(rows, cursor_row).top(top).notes(bottom);
        frame.render(area, buf, body);
    }
}

/// The second explanation line of a focused row: what its keys do.
fn row_help(row: &Row) -> String {
    match &row.control {
        Control::Path(_) => "Tab completes the path".to_string(),
        Control::Harness(_) => "←/→ choose a harness".to_string(),
        Control::Text(_) => String::new(),
    }
}

/// The columns of a row, in cells: the two-cell selection gutter, the
/// label, a gap, the value, a gap, and the right-aligned tag. Every row
/// uses the same widths, focused or not.
#[derive(Clone, Copy)]
struct Grid {
    label: usize,
    value: usize,
}

impl Grid {
    fn new(inner: usize, label: usize) -> Self {
        let fixed = 2 + label + 2 + 1 + TAG_WIDTH;
        Self {
            label,
            value: inner.saturating_sub(fixed).max(4),
        }
    }
}

fn render_row(
    state: &PinEditFormState,
    row: &Row,
    active: bool,
    grid: Grid,
    cols: Cols,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(form::prefix(&row.label, grid.label))];
    match &row.control {
        Control::Harness(input) => {
            let value = input.value().trim();
            let custom = !value.is_empty() && !state.known_harness_keys.iter().any(|k| k == value);
            if custom {
                let mark = " [custom]";
                let width = grid.value.saturating_sub(tuikit::text::width(mark));
                spans.extend(form::value_cells(input, width, active, theme));
                spans.push(Span::styled(mark, Style::default().fg(theme.warning)));
            } else {
                let slot = 18.min(grid.value);
                spans.extend(form::value_cells(input, slot, active, theme));
                let chips =
                    form::harness_chips(value, &state.known_harness_keys, grid.value - slot, theme);
                let used = tuikit::text::spans_width(&chips);
                spans.extend(chips);
                spans.push(Span::raw(" ".repeat(grid.value - slot - used)));
            }
        }
        Control::Path(path) => {
            spans.extend(form::path_cells(path, grid.value, active, theme));
        }
        Control::Text(input) => {
            spans.extend(form::value_cells(input, grid.value, active, theme));
        }
    }
    spans.push(Span::raw(" "));
    let pending = row.cleared || row.edited();
    let tag_style = if pending {
        Style::default().fg(theme.warning)
    } else {
        Style::default().add_modifier(theme.muted)
    };
    spans.push(Span::styled(
        format!("{:>width$}", row.tag(), width = TAG_WIDTH),
        tag_style,
    ));
    let _ = cols;
    form::row(spans, active, theme)
}

#[cfg(test)]
#[path = "pin_edit_tests.rs"]
mod tests;
