//! The rows of a settings panel, and the sections they sit in.

/// A titled group of rows in a [`SettingsPanel`](super::SettingsPanel):
/// a heading the cursor never lands on, and the rows under it.
///
/// ```
/// use tuikit::settings_panel::{Row, Section};
///
/// let section: Section<&str, &str> = Section::new(
///     "Wrap",
///     vec![
///         Row::choice("smart", "smart wrap", true, None),
///         Row::choice("none", "no wrap", false, Some("wrap off")),
///     ],
/// );
/// assert_eq!(section.title, "Wrap");
/// assert_eq!(section.rows.len(), 2);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section<Id, M> {
    /// The heading, drawn bold above the rows.
    pub title: String,
    /// The rows, in their order.
    pub rows: Vec<Row<Id, M>>,
}

impl<Id, M> Section<Id, M> {
    /// A section titled `title` over `rows`.
    pub fn new(title: impl Into<String>, rows: Vec<Row<Id, M>>) -> Self {
        Self {
            title: title.into(),
            rows,
        }
    }
}

/// What kind of setting a row is, and the state it shows: how it is
/// drawn, and what `Enter` does on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One of several choices in its section, drawn bold with `active`
    /// after it when it is the one in force. `Space` selects it; `Enter`
    /// selects it and closes the panel.
    Choice {
        /// Whether this choice is the one in force.
        active: bool,
    },
    /// A setting that is on or off, drawn `[x]` or `[ ]` before its
    /// label (bold when on). `Space` toggles it; `Enter` closes the
    /// panel.
    Checkbox {
        /// Whether the setting is on.
        checked: bool,
    },
    /// A value with an editor of its own, drawn as its name, the value,
    /// and `[ Edit ]`. `Space` and `Enter` both open the editor.
    Editor {
        /// The value as it stands, as the row shows it.
        value: String,
    },
    /// A value that steps through a few choices in place, drawn as its
    /// name, the value, and `[ Cycle ]`. `Space` steps it; `Enter`
    /// closes the panel.
    Cycle {
        /// The value as it stands, as the row shows it.
        value: String,
    },
    /// An action, drawn as its label. `Space` and `Enter` both press it.
    Button,
}

/// One row of a [`SettingsPanel`](super::SettingsPanel): the host's id
/// for it, what it shows, and the host's message for acting on it.
///
/// The fields are plain data, and a constructor per kind fills them.
/// The panel reports `action` at once when the row is acted on, as
/// [`Outcome::Stay`](crate::overlay::Outcome::Stay), so the change
/// applies while the panel stays open; a row that hands over to another
/// overlay is marked with [`leaves`](Self::leaves), and its action is
/// reported as [`Outcome::Commit`](crate::overlay::Outcome::Commit),
/// which closes the panel.
///
/// ```
/// use tuikit::settings_panel::{Kind, Row};
///
/// let row = Row::checkbox("float", "Float starred rows to top", false, "toggle float");
/// assert_eq!(row.kind, Kind::Checkbox { checked: false });
/// assert_eq!(row.action, Some("toggle float"));
/// assert!(!row.leaves);
/// let search = Row::editor("find", "find", "Enter searches", "open search").leaves();
/// assert!(search.leaves);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<Id, M> {
    /// The host's identity for the row, which the cursor keeps across
    /// the rebuilds of the rows: on every key and every draw the panel
    /// asks the host for them again.
    pub id: Id,
    /// What kind of setting the row is, with the state it shows.
    pub kind: Kind,
    /// The row's label: a choice's or a checkbox's text, an editor's or a
    /// cycle's name, a button's words.
    pub label: String,
    /// What acting on the row reports: the host's message that selects
    /// the choice, toggles the checkbox, steps the cycle, opens the
    /// editor, or presses the button. `None` when acting on it changes
    /// nothing (the choice already in force, a button with nothing to
    /// do).
    pub action: Option<M>,
    /// Whether acting on the row closes the panel (the action hands over
    /// to an overlay of its own) instead of keeping it open.
    pub leaves: bool,
}

impl<Id, M> Row<Id, M> {
    fn new(id: Id, kind: Kind, label: impl Into<String>, action: Option<M>) -> Self {
        Self {
            id,
            kind,
            label: label.into(),
            action,
            leaves: false,
        }
    }

    /// One of several choices, `active` when it is the one in force.
    /// `select` is what choosing it reports, or `None` when choosing it
    /// changes nothing.
    pub fn choice(id: Id, label: impl Into<String>, active: bool, select: Option<M>) -> Self {
        Self::new(id, Kind::Choice { active }, label, select)
    }

    /// An on or off setting, `checked` when on; `toggle` is the message
    /// that flips it.
    pub fn checkbox(id: Id, label: impl Into<String>, checked: bool, toggle: M) -> Self {
        Self::new(id, Kind::Checkbox { checked }, label, Some(toggle))
    }

    /// A value named `name`, shown as `value`, with an editor of its own;
    /// `open` is the message that asks the host to open the editor, as a
    /// step over the panel.
    pub fn editor(id: Id, name: impl Into<String>, value: impl Into<String>, open: M) -> Self {
        let value = value.into();
        Self::new(id, Kind::Editor { value }, name, Some(open))
    }

    /// A value named `name`, shown as `value`, that steps to its next
    /// choice in place; `next` is the message that steps it.
    pub fn cycle(id: Id, name: impl Into<String>, value: impl Into<String>, next: M) -> Self {
        let value = value.into();
        Self::new(id, Kind::Cycle { value }, name, Some(next))
    }

    /// An action labeled `label`; `press` is what pressing it reports, or
    /// `None` when there is nothing for it to do now.
    pub fn button(id: Id, label: impl Into<String>, press: Option<M>) -> Self {
        Self::new(id, Kind::Button, label, press)
    }

    /// Marks the row as one whose action closes the panel: an editor or a
    /// button that hands over to an overlay of its own (a search, a
    /// picker) rather than one opened over the panel.
    #[must_use]
    pub fn leaves(mut self) -> Self {
        self.leaves = true;
        self
    }
}
