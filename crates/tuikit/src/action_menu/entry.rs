//! One entry of an action menu, and how it confirms.

use crate::keys::KeyMatcher;

/// Whether picking an entry needs a second `Enter`.
///
/// House rule 3 has two confirmation idioms: an inline arm on a menu
/// entry, when there is nothing more to say than "are you sure", and a
/// dialog when a body helps. A menu entry can only ask for the first; a
/// host that has more to say opens a dialog from the entry instead. A
/// reversible action asks for neither.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Confirm {
    /// The first `Enter` commits the entry.
    #[default]
    None,
    /// The first `Enter` arms the entry and its row says what the next
    /// `Enter` will do; the second commits it. Moving the cursor disarms
    /// it.
    Inline,
}

/// One action in an [`ActionMenu`](super::ActionMenu): what its row says,
/// how it is reached, and what picking it reports.
///
/// The fields are plain data, and the builders fill the optional ones:
///
/// ```
/// use tuikit::action_menu::{Confirm, Entry};
/// use tuikit::keys::ch;
///
/// let entry = Entry::new("Remove", "remove")
///     .detail("delete the file from the list")
///     .shortcut(ch('x'))
///     .confirm(Confirm::Inline);
/// assert_eq!(entry.label, "Remove");
/// assert!(entry.enabled);
/// assert_eq!(entry.outcome, "remove");
/// assert!(!Entry::new("Rename", ()).disabled().enabled);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<M> {
    /// The entry's name, on its row after the cursor gutter (and the key
    /// column, when the menu has one). Cut with `…` when the row is too
    /// narrow.
    pub label: String,
    /// Secondary text, wrapped on the rows under the label and indented
    /// past it: what the entry does, or why it cannot run now.
    pub detail: Option<String>,
    /// A key that picks the entry from anywhere in the menu, shown in the
    /// menu's key column. A key the menu's own table binds (the arrows,
    /// `j`, `k`, `q`, `Enter`, `Esc`) never reaches a shortcut;
    /// [`ActionMenu::shadowed_shortcuts`](super::ActionMenu::shadowed_shortcuts)
    /// finds one.
    pub shortcut: Option<KeyMatcher>,
    /// Whether the entry can run now. A disabled entry is drawn receding,
    /// the cursor steps over it, and neither `Enter` nor its shortcut
    /// commits it; its detail is where the host says why.
    pub enabled: bool,
    /// Whether picking it needs a second `Enter`.
    pub confirm: Confirm,
    /// What committing the entry reports: the host's message.
    pub outcome: M,
}

impl<M> Entry<M> {
    /// An enabled entry labeled `label` that commits `outcome` on the
    /// first `Enter`, with no detail and no shortcut.
    pub fn new(label: impl Into<String>, outcome: M) -> Self {
        Self {
            label: label.into(),
            detail: None,
            shortcut: None,
            enabled: true,
            confirm: Confirm::None,
            outcome,
        }
    }

    /// Sets the secondary text under the label.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Sets the key that picks the entry from anywhere in the menu.
    #[must_use]
    pub fn shortcut(mut self, key: KeyMatcher) -> Self {
        self.shortcut = Some(key);
        self
    }

    /// Marks the entry as one that cannot run now.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Sets how picking the entry confirms.
    #[must_use]
    pub fn confirm(mut self, confirm: Confirm) -> Self {
        self.confirm = confirm;
        self
    }
}
