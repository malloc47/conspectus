//! One item of a checklist.

/// One item in a [`Checklist`](super::Checklist): what its row says,
/// whether it starts checked, and whether it can change.
///
/// The fields are plain data, and the builders fill the optional ones:
///
/// ```
/// use tuikit::checklist::Item;
///
/// let item = Item::new("lettuce").checked(true);
/// assert!(item.checked && item.enabled);
/// let fixed = Item::new("salt").checked(true).detail("always added").disabled();
/// assert_eq!(fixed.detail.as_deref(), Some("always added"));
/// assert!(!fixed.enabled);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// The item's name, on its row after the cursor gutter and the check
    /// mark. Cut with `…` when the row is too narrow.
    pub label: String,
    /// Secondary text after the label, past the separator mark: why an
    /// item cannot change, or a note about it. Cut before the label is.
    pub detail: Option<String>,
    /// Whether the item is checked.
    pub checked: bool,
    /// Whether the item can change. One that cannot is drawn receding
    /// and keeps its check: the cursor steps over it, and neither `Space`
    /// nor select all or none changes it.
    pub enabled: bool,
}

impl Item {
    /// An unchecked item labeled `label` that can change, with no detail.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            checked: false,
            enabled: true,
        }
    }

    /// Sets whether the item starts checked.
    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Sets the secondary text after the label.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Marks the item as one that cannot change: it keeps the check it
    /// has.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}
