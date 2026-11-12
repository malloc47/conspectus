//! Live agent state in the text output: the `live` cells of `conspectus
//! table`.
//!
//! Every value is read through the snapshot's accessors
//! ([`GraphSnapshot::live_state_of`], [`GraphSnapshot::live_counts_of_mux`])
//! at the time the caller gives, so the read-time rules apply: a long
//! silence marks a working agent quiet, a gone process reads `exited`, and
//! old evidence about a process nobody could check reads `unknown`. A node
//! without an entry shows nothing, because no evidence is not idle.
//!
//! Only states, details, counts, ids, and times are shown.

use std::fmt::Write as _;

use super::render::format_relative_age;
use crate::model::live::{EffectiveLive, LiveBinding, LiveConfidence, LiveCounts, LiveState};
use crate::model::{GraphSnapshot, MuxSessionId, NodeId};

/// The time, in Unix epoch milliseconds, that live state is read at:
/// `now_epoch` (Unix seconds) when the caller pins one, else the clock.
pub(crate) fn read_time_ms(now_epoch: Option<i64>) -> i64 {
    now_epoch.map_or_else(crate::live_store::now_ms, |epoch| {
        epoch.saturating_mul(1000)
    })
}

/// `state`, or `state:detail` when the state has a detail, in the
/// vocabulary's own names (`waiting:permission`, `ready:finished`).
pub(crate) fn state_label(effective: &EffectiveLive) -> String {
    match effective.detail {
        Some(detail) => format!("{}:{}", effective.state.name(), detail.name()),
        None => effective.state.name().to_string(),
    }
}

/// When the state shown began. That is the recorded start while the state
/// reads as recorded. When the read rules put another state in its place
/// (a gone process reads `exited`, old evidence `unknown`), it is the
/// latest evidence instead: the shown state began no earlier than that.
pub(crate) fn shown_since_ms(binding: &LiveBinding, effective: &EffectiveLive) -> i64 {
    if effective.state == binding.observation.state {
        binding.observation.since_ms
    } else {
        binding.observation.observed_ms
    }
}

/// How long before `now_ms` the time `then_ms` was, on the table's age
/// ladder (`12s`, `5m`, `2h`); `now` for a time after `now_ms`.
pub(crate) fn age(then_ms: i64, now_ms: i64) -> String {
    format_relative_age(then_ms.div_euclid(1000), now_ms.div_euclid(1000))
}

/// The `live` cell of a session row, or `None` when the session has no
/// live state. The state and its detail, `+N` for running subagents,
/// `quiet` once a working agent has been silent past its quiet window,
/// and how long ago the state began: `waiting:permission +1 3m`,
/// `working:generating quiet 25m`, `unknown:stale 40m`.
pub(crate) fn session_cell(snapshot: &GraphSnapshot, id: &NodeId, now_ms: i64) -> Option<String> {
    let effective = snapshot.live_state_of(id, now_ms)?;
    let binding = snapshot.node_live.get(id)?;
    let mut cell = state_label(&effective);
    let subagents = shown_subagents(binding, &effective);
    if subagents > 0 {
        let _ = write!(cell, " +{subagents}");
    }
    if effective.confidence == LiveConfidence::Quiet {
        cell.push_str(" quiet");
    }
    let _ = write!(
        cell,
        " {}",
        age(shown_since_ms(binding, &effective), now_ms)
    );
    Some(cell)
}

/// The running subagents worth showing beside the state: none once the
/// session reads `exited` or `unknown`, whatever was last recorded.
fn shown_subagents(binding: &LiveBinding, effective: &EffectiveLive) -> u16 {
    match effective.state {
        LiveState::Exited | LiveState::Unknown => 0,
        _ => binding.observation.subagents,
    }
}

/// The `live` cell of a mux row: [`counts_text`] over the agents the mux
/// hosts, or `None` when none of them has live state.
pub(crate) fn mux_cell(
    snapshot: &GraphSnapshot,
    mux: &MuxSessionId,
    now_ms: i64,
) -> Option<String> {
    counts_text(&snapshot.live_counts_of_mux(mux, now_ms))
}

/// The counted states as `1 waiting, 2 working`, most urgent first by
/// [`LiveState::default_precedence`], or `None` when nothing is counted.
pub(crate) fn counts_text(counts: &LiveCounts) -> Option<String> {
    let mut states: Vec<LiveState> = LiveState::ALL
        .into_iter()
        .filter(|state| counts.get(*state) > 0)
        .collect();
    if states.is_empty() {
        return None;
    }
    states.sort_by_key(|state| state.default_precedence());
    Some(
        states
            .iter()
            .map(|state| format!("{} {}", counts.get(*state), state.name()))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
