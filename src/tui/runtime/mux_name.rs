//! The name the new tmux session form and the launch form start with:
//! the next free name in the series of the mux the operator is looking
//! at, so with `worker-1` selected the form opens on `worker-2`.

use std::collections::BTreeSet;

use crate::model::{GraphSnapshot, MuxSessionId, NodeId, PinKind};
use crate::tui::actions::{Subject, action_subject};
use crate::tui::app::App;

/// The name both forms open with: the next free name after the mux the
/// operator is looking at (the right pane's entity while it has focus,
/// else the selected row), or empty when that is not a mux and runs in
/// none. A name is free when no mux, live or gone, no mux in history,
/// and no pin has it.
pub(crate) fn seed(app: &App) -> String {
    let Some(handle) = app.snapshot_handle() else {
        return String::new();
    };
    let snapshot = handle.snapshot();
    let Some(base) = base_name(snapshot, &action_subject(app)).filter(|base| !base.is_empty())
    else {
        return String::new();
    };
    let pins: BTreeSet<&str> = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .map(|pin| pin.id.id.as_str())
        .collect();
    next_in_series(&base, |name| {
        snapshot.mux_name_taken(name) || pins.contains(name)
    })
}

/// The name of the mux the subject is, or the one it runs in. A session
/// takes the mux its row shows: the one hosting it now, else the one
/// that hosted it last. Groups, PRs, and the other kinds have none, even
/// when a single mux is scoped to them.
fn base_name(snapshot: &GraphSnapshot, subject: &Subject) -> Option<String> {
    match &subject.node {
        Some(NodeId::MuxSession(mux)) => Some(mux_name(snapshot, mux, subject.record.as_deref())),
        Some(session @ NodeId::AgentSession(_)) => {
            snapshot.mux_host_of(session).map(|host| host.name)
        }
        Some(_) => None,
        // A mux pin that points at nothing yet: the name its mux will
        // get, which is the pin's id unless the pin overrides it.
        None => {
            let id = subject.pin.as_deref()?;
            let pin = crate::tui::rows::pin_nodes(snapshot)
                .into_iter()
                .find(|pin| pin.id.id == id)?;
            (pin.kind == PinKind::Mux).then(|| {
                pin.text("name")
                    .filter(|name| !name.is_empty())
                    .unwrap_or(id)
                    .to_string()
            })
        }
    }
}

/// The session name of `mux`: the live session's, else the name its
/// history record keeps (`record`, or the record of a mux with its id),
/// else its id without the backend and socket.
fn mux_name(snapshot: &GraphSnapshot, mux: &MuxSessionId, record: Option<&str>) -> String {
    let node = NodeId::MuxSession(mux.clone());
    if snapshot.is_live(&node)
        && let Some(crate::model::GraphNode::MuxSession(live)) = snapshot.find_node(&node)
    {
        return live.session_name().to_string();
    }
    let muxes = &snapshot.history.muxes;
    record
        .and_then(|record| muxes.iter().find(|r| r.meta.id == record))
        .or_else(|| muxes.iter().find(|r| r.recipe.native_id() == mux.native_id))
        .map_or_else(
            || super::strip_tmux_prefix(&mux.native_id),
            |r| r.recipe.name.clone(),
        )
}

/// The first name after `base` in its series that is not `taken`.
///
/// A `base` ending in ASCII digits bumps that number and keeps its zero
/// padding as a minimum width (`work-007` gives `work-008`, `work-099`
/// gives `work-100`). Any other `base`, or one whose number does not fit
/// a `u64` once bumped, gets `-2`, `-3`, and so on. The first candidate
/// is always the next one, even when `base` itself is free: the form
/// starts a new mux beside the one it was opened from.
pub(crate) fn next_in_series(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit());
    let digits = &base[stem.len()..];
    if let Ok(mut number) = digits.parse::<u64>() {
        let width = digits.len();
        while let Some(next) = number.checked_add(1) {
            let candidate = format!("{stem}{next:0width$}");
            if !taken(&candidate) {
                return candidate;
            }
            number = next;
        }
    }
    (2u64..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken(candidate))
        .expect("an unbounded range has a free name")
}

#[cfg(test)]
mod tests {
    use super::next_in_series;

    fn free(_: &str) -> bool {
        false
    }

    #[test]
    fn a_trailing_number_is_bumped() {
        assert_eq!(next_in_series("worker-1", free), "worker-2");
        assert_eq!(next_in_series("agent3", free), "agent4");
        assert_eq!(next_in_series("v1.9", free), "v1.10");
        assert_eq!(next_in_series("42", free), "43");
    }

    #[test]
    fn taken_names_are_skipped_and_gaps_filled() {
        let taken = ["work-2", "work-3", "work-5"];
        let is_taken = |name: &str| taken.contains(&name);
        assert_eq!(next_in_series("work-1", is_taken), "work-4");
        assert_eq!(next_in_series("work-4", is_taken), "work-6");
    }

    #[test]
    fn the_first_candidate_is_the_next_even_when_the_base_is_free() {
        assert_eq!(next_in_series("work-1", |name| name == "work-1"), "work-2");
        assert_eq!(next_in_series("work-1", free), "work-2");
        assert_eq!(next_in_series("work", free), "work-2");
    }

    #[test]
    fn zero_padding_keeps_its_width() {
        assert_eq!(next_in_series("work-007", free), "work-008");
        assert_eq!(next_in_series("work-099", free), "work-100");
        assert_eq!(next_in_series("work-999", free), "work-1000");
        assert_eq!(next_in_series("run-00", free), "run-01");
        assert_eq!(
            next_in_series("work-007", |name| name == "work-008"),
            "work-009"
        );
    }

    #[test]
    fn a_name_without_a_number_gets_a_dash_suffix() {
        assert_eq!(next_in_series("scratch", free), "scratch-2");
        assert_eq!(
            next_in_series("scratch", |name| name == "scratch-2"),
            "scratch-3"
        );
    }

    #[test]
    fn digits_too_large_for_a_number_count_as_no_number() {
        let huge = "build-99999999999999999999";
        assert_eq!(next_in_series(huge, free), format!("{huge}-2"));
        let max = format!("build-{}", u64::MAX);
        assert_eq!(next_in_series(&max, free), format!("{max}-2"));
    }
}
