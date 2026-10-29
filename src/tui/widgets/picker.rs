//! The picker: a pick-one list with type-to-filter, for choosing among
//! known candidates instead of typing a name. Where a pin should point
//! next and which remembered search to show are its two uses.
//!
//! It is the framework's palette over items (`tuikit::palette`): the
//! query filters the list as it is typed (every word must appear in the
//! label or detail, case-insensitively), the arrows and
//! `Ctrl-N`/`Ctrl-P` move, wrapping, `PgUp`/`PgDn` page, `Enter` picks,
//! `Esc` closes, and the footer comes from the standard palette table.
//! This module keeps what each picker is for: its title, its note when
//! there is nothing to list, and the message a pick commits. The
//! candidates come from `actions::retarget_candidates` and the search
//! history.

use tuikit::overlay::Outcome;
use tuikit::palette::{Item, Palette, every_word};

use crate::tui::Msg;

/// One candidate: its label, its context (where it lives, its state),
/// and what choosing it commits (a node id, a history record id, or a
/// remembered search's position).
pub type PickerItem = Item<String>;

/// A picker: the framework's palette over [`PickerItem`]s, committing a
/// [`Msg`].
pub type Picker = Palette<PickerItem, Msg>;

/// What the retarget picker says when there is nothing to list.
const NOTHING_TO_RETARGET: &str = "(nothing else of this kind to point at)";

/// The picker that points `pin` at the chosen candidate.
pub fn retarget(pin: String, candidates: Vec<PickerItem>) -> Picker {
    Palette::new(
        format!("Retarget {pin}"),
        candidates,
        move |item: &PickerItem| {
            Outcome::Commit(Msg::CommitRetarget {
                pin: pin.clone(),
                target: item.value.clone(),
            })
        },
    )
    .ranked_by(every_word)
    .empty_note(NOTHING_TO_RETARGET)
}

/// The picker that shows a remembered search in the Search view. Each
/// item's value is the search's position, oldest first; the cursor starts
/// on the item whose value is `viewed`, the search the view shows now.
pub fn search_history(items: Vec<PickerItem>, viewed: Option<&str>) -> Picker {
    let at = viewed.and_then(|viewed| items.iter().position(|item| item.value == viewed));
    let picker = Palette::new("Searches", items, |item: &PickerItem| {
        Outcome::Commit(Msg::ShowSearchHistory(
            item.value.parse().unwrap_or_default(),
        ))
    })
    .ranked_by(every_word);
    match at {
        Some(at) => picker.at(at),
        None => picker,
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
