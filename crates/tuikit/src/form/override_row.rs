//! The override row: a field that follows an inherited value until the
//! user gives it one of their own, says where its value comes from, and
//! goes back to following on the form's reset key.

use std::fmt;
use std::sync::Arc;

use crate::text;

/// The word an override row shows while it holds a value of its own that
/// it held when the form opened.
pub const KEPT: &str = "kept";
/// The word an override row shows once its value was changed: a submit
/// keeps the new value as the field's own.
pub const WILL_KEEP: &str = "will keep";
/// The word an override row shows once it was reset: a submit drops the
/// field's own value, so it follows the inherited one again.
pub const WILL_FOLLOW: &str = "will follow";
/// Where an override row's value stands: following the inherited value,
/// holding one of its own, or changed in this form and waiting for a
/// submit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Standing {
    /// The field follows the inherited value, and nothing is pending.
    Follows,
    /// The field held a value of its own when the form opened and still
    /// holds it.
    Kept,
    /// The value changed since the form opened: a submit keeps it as the
    /// field's own.
    Changed,
    /// The field was reset to the inherited value: a submit drops the
    /// value it held of its own.
    Reset,
}

impl Standing {
    /// Whether a submit would change what the field holds (the value was
    /// changed or reset).
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Changed | Self::Reset)
    }
}

/// What an override row's hint is told about the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverrideView<'a> {
    /// Where the value stands.
    pub standing: Standing,
    /// The field's text as it is now.
    pub value: &'a str,
    /// The value the field follows without one of its own.
    pub inherited: &'a str,
    /// The source word: where the inherited value comes from.
    pub source: &'a str,
}

/// The lines an override row's hint gives for the row as it is now.
type Hint = Arc<dyn Fn(&OverrideView<'_>) -> Vec<String> + Send + Sync>;

/// A decorator for a text, path, or choice field
/// ([`Field::with_override`](super::Field::with_override)): the field
/// follows an inherited value until the user types, steps, or completes a
/// value of their own, and the form's reset key
/// ([`FormKey::Inherit`](crate::keys::FormKey::Inherit), bound in the
/// host's key table) puts the inherited value back.
///
/// The row shows the value in effect and, in a column at its right edge,
/// a word for where that value stands: the **source word** while it
/// follows (where the inherited value comes from, in the host's words),
/// [`KEPT`] while it holds a value of its own, and, while a change waits
/// for a submit, [`WILL_KEEP`] or [`WILL_FOLLOW`] in `warning`. A
/// [`hint`](Self::hint) explains the row in the form's detail lines while
/// it has focus.
///
/// What a submit means is the host's: `on_submit` reads each override
/// row's [`Standing`] (through [`Values::standing`](super::Values::standing))
/// and keeps a changed value, drops a reset one, and leaves the others
/// alone.
///
/// ```
/// use tuikit::form::{Field, OverrideRow, Standing};
///
/// let field = Field::text("color", "color", "blue")
///     .with_override(OverrideRow::new("blue", "default"));
/// assert_eq!(field.standing(), Some(Standing::Follows));
/// assert_eq!(field.override_row().map(|row| row.word("blue")), Some("default"));
/// ```
#[derive(Clone)]
pub struct OverrideRow {
    inherited: String,
    source: String,
    overridden: bool,
    opened: String,
    reset: bool,
    hint: Option<Hint>,
}

impl OverrideRow {
    /// A row that follows `inherited`, which comes from where `source`
    /// says (a short word such as `default`). The field holds no value of
    /// its own until [`overridden`](Self::overridden) says it does.
    pub fn new(inherited: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            inherited: inherited.into(),
            source: source.into(),
            overridden: false,
            opened: String::new(),
            reset: false,
            hint: None,
        }
    }

    /// Whether the field holds a value of its own as the form opens (the
    /// field's text is that value).
    #[must_use]
    pub fn overridden(mut self, overridden: bool) -> Self {
        self.overridden = overridden;
        self
    }

    /// Explains the row in the form's detail lines while it has focus:
    /// `hint` gets the row as it is now and gives the lines to show,
    /// before the field's own [`hint`](super::Field::hint) lines.
    #[must_use]
    pub fn hint(
        mut self,
        hint: impl Fn(&OverrideView<'_>) -> Vec<String> + Send + Sync + 'static,
    ) -> Self {
        self.hint = Some(Arc::new(hint));
        self
    }

    /// The value the field follows without one of its own.
    pub fn inherited(&self) -> &str {
        &self.inherited
    }

    /// The source word: where the inherited value comes from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the field held a value of its own when the form opened.
    pub fn is_overridden(&self) -> bool {
        self.overridden
    }

    /// The field's text when the form opened.
    pub fn opened(&self) -> &str {
        &self.opened
    }

    /// Where a field holding `value` stands: reset while the last thing
    /// done to it was a reset and it still holds the inherited value
    /// (following, when it had no value of its own to drop); changed once
    /// its text differs from what the form opened with; else kept or
    /// following, as it opened.
    pub fn standing(&self, value: &str) -> Standing {
        if self.reset && value == self.inherited {
            if self.overridden {
                Standing::Reset
            } else {
                Standing::Follows
            }
        } else if value != self.opened {
            Standing::Changed
        } else if self.overridden {
            Standing::Kept
        } else {
            Standing::Follows
        }
    }

    /// The word the row's source column shows for a field holding
    /// `value`.
    pub fn word(&self, value: &str) -> &str {
        match self.standing(value) {
            Standing::Follows => &self.source,
            Standing::Kept => KEPT,
            Standing::Changed => WILL_KEEP,
            Standing::Reset => WILL_FOLLOW,
        }
    }

    /// The cells the row's source column needs for any word it may show.
    pub fn column_width(&self) -> usize {
        [self.source.as_str(), KEPT, WILL_KEEP, WILL_FOLLOW]
            .into_iter()
            .map(text::width)
            .max()
            .unwrap_or(0)
    }

    /// What the hint says about a field holding `value`; nothing without
    /// a hint.
    pub fn hints(&self, value: &str) -> Vec<String> {
        self.hint.as_ref().map_or_else(Vec::new, |hint| {
            hint(&OverrideView {
                standing: self.standing(value),
                value,
                inherited: &self.inherited,
                source: &self.source,
            })
        })
    }

    /// Records the text the field opened with.
    pub(super) fn open(&mut self, value: &str) {
        value.clone_into(&mut self.opened);
    }

    /// Records whether the last change to the field was a reset.
    pub(super) fn set_reset(&mut self, reset: bool) {
        self.reset = reset;
    }
}

impl fmt::Debug for OverrideRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OverrideRow")
            .field("inherited", &self.inherited)
            .field("source", &self.source)
            .field("overridden", &self.overridden)
            .field("opened", &self.opened)
            .field("reset", &self.reset)
            .field("hinted", &self.hint.is_some())
            .finish()
    }
}

#[cfg(test)]
#[path = "override_row_tests.rs"]
mod tests;
