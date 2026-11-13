//! A node's live-state fields in the detail pane (ADR 0213): what a
//! session's agent is doing, and what a mux's agents are doing. They are
//! read from the values the left pane's rows were filled with
//! ([`LiveValues`]), so a field says what the node's row's mark says, and
//! a node with no evidence has none of them.

use tuikit::glyphs::marks;

use super::{CoreField, FieldValue};
use crate::model::MuxSessionId;
use crate::model::live::{LiveConfidence, LiveCounts, LiveDetail, LiveState};
use crate::tui::rows::{LiveCell, LiveSign, LiveValues};

/// The words of `cell`'s state: its name, then its detail's when it has
/// one (`waiting`, `permission`).
fn state_words(cell: LiveCell) -> Vec<&'static str> {
    std::iter::once(cell.state.name())
        .chain(cell.detail.map(LiveDetail::name))
        .collect()
}

/// What a session's agent is doing, from the cell its row's mark is drawn
/// from: `state` (the mark, then the state and its detail in words),
/// `since` (when that state began), `source` (what reported it), and
/// `evidence` (whether it is fresh, has gone quiet, or is too old to
/// trust; the last two recede, with the state), then `subagents` while
/// the agent runs any, by the rule the row's chip follows.
pub(super) fn session_fields(cell: LiveCell, now: Option<i64>) -> Vec<CoreField> {
    let receding = cell.confidence != LiveConfidence::Fresh;
    let words = state_words(cell);
    let mut fields = vec![
        CoreField::plain("state", words.join(marks::SPACED_SEPARATOR))
            .with_live_mark(LiveSign::of_session(Some(cell)))
            .receding_when(receding)
            .previewing(FieldValue::Glossary {
                field: "state",
                values: words.into_iter().map(str::to_string).collect(),
            }),
        CoreField::timestamp("since", cell.since_ms.div_euclid(1000), now),
        CoreField::glossary("source", cell.source.name()),
        CoreField::glossary("evidence", cell.confidence.name()).receding_when(receding),
    ];
    let subagents = cell.running_subagents();
    if subagents > 0 {
        fields.push(CoreField::plain("subagents", format!("{subagents} active")));
    }
    fields
}

/// The states `counts` counts, most urgent first by the rollup's
/// precedence, so the first is the one a mux's mark shows.
fn counted_states(counts: LiveCounts) -> Vec<LiveState> {
    let mut states: Vec<LiveState> = LiveState::ALL
        .into_iter()
        .filter(|state| counts.get(*state) > 0)
        .collect();
    states.sort_by_key(|state| state.default_precedence());
    states
}

/// What a mux's agents are doing: `agents`, its counts led by the mark
/// its row shows (`1 waiting · 1 error`), and `pane_agents`, the agents in
/// its panes that no session matches, each with its pane and state (they
/// count in `agents` too but have no row or node of their own). None
/// without evidence.
pub(super) fn mux_fields(live: &LiveValues, mux: &MuxSessionId) -> Vec<CoreField> {
    let mut fields = Vec::new();
    let counts = live.mux(mux);
    let states = counted_states(counts);
    if !states.is_empty() {
        let value = states
            .iter()
            .map(|state| format!("{} {}", counts.get(*state), state.name()))
            .collect::<Vec<_>>()
            .join(marks::SPACED_SEPARATOR);
        fields.push(
            CoreField::plain("agents", value)
                .with_live_mark(LiveSign::of_mux(counts))
                .previewing(FieldValue::Glossary {
                    field: "state",
                    values: states
                        .iter()
                        .map(|state| state.name().to_string())
                        .collect(),
                }),
        );
    }
    let panes: Vec<String> = live
        .panes_of(mux)
        .map(|(pane, cell)| {
            format!(
                "{} {}",
                pane.pane.as_deref().unwrap_or(marks::NO_VALUE),
                state_words(cell).join(marks::SPACED_SEPARATOR)
            )
        })
        .collect();
    if let Some(first) = panes.first() {
        // The first and how many more, as a list field shows, so the
        // row stays one line; the Preview lists them all.
        let value = if panes.len() > 1 {
            format!("{first}  (+{})", panes.len() - 1)
        } else {
            first.clone()
        };
        fields.push(CoreField::plain("pane_agents", value).previewing(FieldValue::List(panes)));
    }
    fields
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
