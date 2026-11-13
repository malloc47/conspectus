//! What an action is for, and the modal key convention checked by it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::matcher::ctrl;
use super::table::{KeyRow, rows};

/// What an action is for, whatever its name: the classifier the
/// convention checks read instead of matching action names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyRole {
    /// Dismisses the whole overlay at once, whatever level of it is
    /// open: what `Ctrl-C` does.
    Abort,
    /// Leaves the current level without applying anything still pending:
    /// close, cancel, no, back.
    Leave,
    /// Applies, submits, or activates: pick, confirm, yes, save, choose.
    Apply,
    /// Changes the focused item in place: toggles it, steps its option,
    /// selects a choice.
    Change,
    /// Moves the cursor or the view by a row, a field, or to an end.
    Move,
    /// Moves the view by a page or half a page.
    Page,
    /// Anything else: copy, complete, switch a scope, open a pane.
    Other,
}

/// An action enum whose variants say what they are for. A table's
/// actions implement it so the convention checks can ask an action its
/// [`KeyRole`].
pub trait KeyAction: Copy {
    /// What this action is for.
    fn role(self) -> KeyRole;
}

/// Whether `table` starts with `Ctrl-C`: its first bound row (included
/// tables spliced in) is `Ctrl-C`, bound to an action that dismisses
/// ([`KeyRole::Abort`], or [`KeyRole::Leave`] in a table with one level
/// to leave). Every table an overlay resolves with starts this way, so
/// `Ctrl-C` gets out of any overlay and no later row can take it.
pub fn ctrl_c_first<A: KeyAction>(table: &[KeyRow<A>]) -> bool {
    rows(table)
        .find_map(|row| match row {
            KeyRow::Bind { key, action, .. } => Some((*key, action.role())),
            _ => None,
        })
        .is_some_and(|(key, role)| {
            key == ctrl('c') && matches!(role, KeyRole::Abort | KeyRole::Leave)
        })
}

/// What `Enter` does in a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnterMeans {
    /// Applies or submits the whole modal.
    Submits,
    /// Activates the focused item (a menu entry, a search hit, a choice).
    Activates,
    /// Is not bound: a pager, whose `Enter` does nothing special.
    Nothing,
}

/// What `Space` does in a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceMeans {
    /// Toggles, steps, or selects the focused item.
    Toggles,
    /// Pages down, as in any pager.
    Pages,
    /// Is not bound: it types into the focused text field, or does
    /// nothing in a modal with none.
    Types,
}

/// One modal against the modal key convention: `Esc` leaves without
/// applying what is pending, `Enter` applies or activates, `Space`
/// changes the focused item. A modal that departs from it says why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Convention {
    /// The modal's name, for messages.
    pub modal: &'static str,
    /// What `Enter` does in it.
    pub enter: EnterMeans,
    /// What `Space` does in it.
    pub space: SpaceMeans,
    /// Why the modal departs from the convention, if it does; empty
    /// otherwise.
    pub exception: &'static str,
}

impl Convention {
    /// A modal's row of the convention.
    pub const fn new(
        modal: &'static str,
        enter: EnterMeans,
        space: SpaceMeans,
        exception: &'static str,
    ) -> Self {
        Self {
            modal,
            enter,
            space,
            exception,
        }
    }

    /// Where the modal's keys break its row of the convention, as
    /// messages; empty when they keep it. `roles` gives, for a key
    /// event, the role of the action each table the modal consults binds
    /// it to (a modal may consult several tables, one per level).
    pub fn breaks(&self, roles: &dyn Fn(KeyEvent) -> Vec<KeyRole>) -> Vec<String> {
        let press = |code| roles(KeyEvent::new(code, KeyModifiers::NONE));
        let mut out = Vec::new();
        let esc = press(KeyCode::Esc);
        if !esc.contains(&KeyRole::Leave) {
            out.push(format!(
                "{}: Esc is bound to {esc:?}, not to leaving",
                self.modal
            ));
        }
        let enter = press(KeyCode::Enter);
        match self.enter {
            EnterMeans::Submits | EnterMeans::Activates if !enter.contains(&KeyRole::Apply) => {
                out.push(format!(
                    "{}: Enter is bound to {enter:?}, not to applying",
                    self.modal
                ));
            }
            EnterMeans::Nothing if !enter.is_empty() => out.push(format!(
                "{}: Enter is bound to {enter:?} where it should do nothing special",
                self.modal
            )),
            _ => {}
        }
        if enter.contains(&KeyRole::Leave) {
            out.push(format!(
                "{}: Enter leaves; Esc is the key that leaves without applying",
                self.modal
            ));
        }
        let space = press(KeyCode::Char(' '));
        let wanted = match self.space {
            SpaceMeans::Toggles => Some(KeyRole::Change),
            SpaceMeans::Pages => Some(KeyRole::Page),
            SpaceMeans::Types => None,
        };
        match wanted {
            Some(role) if !space.contains(&role) => out.push(format!(
                "{}: Space is bound to {space:?}, not to {role:?}",
                self.modal
            )),
            None if !space.is_empty() => out.push(format!(
                "{}: Space is bound to {space:?}; it should reach the text field",
                self.modal
            )),
            _ => {}
        }
        out
    }
}
