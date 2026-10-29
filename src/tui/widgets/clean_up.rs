//! The History view's Clean up modal: the records `history reap`
//! would remove, all preselected, each of which the operator can
//! deselect before confirming (ADR 0140). It is the framework's
//! checklist over the candidates' labels ([`checklist`]); confirming
//! maps the checked rows back to the records to forget. With nothing to
//! remove, Clean up is the framework's notice dialog instead
//! ([`empty_notice`]).

use tuikit::age::Age;
use tuikit::checklist::{Checklist, Item};
use tuikit::dialog::Dialog;
use tuikit::overlay::Outcome;

use crate::history::reap::Candidate;
use crate::tui::Msg;

/// The checklist's title.
const TITLE: &str = "Clean up: forget these records";

/// The row a candidate gets: its kind, its label, and when it was last
/// seen, dated by `now`.
fn label(candidate: &Candidate, now: i64) -> String {
    format!(
        "{:<8} {} · last seen {}",
        candidate.kind.as_str(),
        candidate.label,
        Age::between(candidate.last_seen, now).ago()
    )
}

/// The Clean up checklist over `candidates`, every one checked. Enter
/// forgets exactly the checked records (`Msg::ReapRecords`), and with
/// none checked it closes without forgetting anything. `now` dates the
/// "last seen" ages.
pub fn checklist(candidates: &[Candidate], now: i64) -> Checklist<Msg> {
    let items = candidates
        .iter()
        .map(|candidate| Item::new(label(candidate, now)).checked(true))
        .collect();
    let ids: Vec<String> = candidates.iter().map(|c| c.id.clone()).collect();
    Checklist::new(TITLE, items, move |indices: Vec<usize>| {
        let chosen: Vec<String> = indices
            .into_iter()
            .filter_map(|index| ids.get(index).cloned())
            .collect();
        if chosen.is_empty() {
            Outcome::Close
        } else {
            Outcome::Commit(Msg::ReapRecords(chosen))
        }
    })
}

/// What Clean up shows when nothing is past its reap age: a notice that
/// says why the list is empty, which any of the dialog keys dismisses.
pub fn empty_notice() -> Dialog<Msg> {
    Dialog::notice("Clean up").body([
        "Nothing to clean up.",
        "Stopped and removed records are kept until they pass their \
         `[history.reap_after]` age, and records a pin targets are never listed.",
    ])
}

#[cfg(test)]
#[path = "clean_up_tests.rs"]
mod tests;
