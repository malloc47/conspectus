//! Live agent state in the text output: the `live` cells of `conspectus
//! table` and the `live state` section of `conspectus node show`.
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
/// session reads `exited` or `unknown`, whatever was last recorded
/// ([`LiveState::shows_subagents`]).
fn shown_subagents(binding: &LiveBinding, effective: &EffectiveLive) -> u16 {
    if effective.state.shows_subagents() {
        binding.observation.subagents
    } else {
        0
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

/// The body of `node show`'s `live state` section for `id`, one line per
/// field, or nothing when the node has no live state. A session lists its
/// own entry. A mux lists the counts over the agents it hosts and the
/// entry of each agent in one of its panes that no session matched yet,
/// with the pane's id.
pub(crate) fn node_show_lines(snapshot: &GraphSnapshot, id: &NodeId, now_ms: i64) -> Vec<String> {
    let mut lines = Vec::new();
    match id {
        NodeId::AgentSession(_) => {
            if let Some(binding) = snapshot.node_live.get(id) {
                binding_lines(&mut lines, binding, now_ms, "  ");
            }
        }
        NodeId::MuxSession(mux) => {
            if let Some(counts) = counts_text(&snapshot.live_counts_of_mux(mux, now_ms)) {
                lines.push(format!("  counts:     {counts}"));
            }
            for (pane, binding) in snapshot.live_panes_of(mux) {
                lines.push(match &pane.pane {
                    Some(pane) => format!("  pane agent with no session ({pane}):"),
                    None => "  pane agent with no session:".to_string(),
                });
                binding_lines(&mut lines, binding, now_ms, "    ");
            }
        }
        _ => {}
    }
    lines
}

/// The fields of one entry as `node show` lists them, each line led by
/// `indent`. Lines for a detail, a subagent origin, a recorded state the
/// read rules replaced, subagents, and a pid appear only when there is
/// one.
fn binding_lines(lines: &mut Vec<String>, binding: &LiveBinding, now_ms: i64, indent: &str) {
    let observation = &binding.observation;
    let effective = binding.effective(now_ms);
    let mut field = |name: &str, value: String| {
        lines.push(format!("{indent}{:<12}{value}", format!("{name}:")));
    };
    field("state", effective.state.name().to_string());
    if let Some(detail) = effective.detail {
        field("detail", detail.name().to_string());
    }
    if observation.via_subagent {
        field("via", "a subagent".to_string());
    }
    if effective.state != observation.state {
        let recorded = EffectiveLive {
            state: observation.state,
            detail: observation
                .detail
                .filter(|detail| observation.state.accepts(*detail)),
            confidence: LiveConfidence::Fresh,
        };
        field("recorded", state_label(&recorded));
    }
    let since_ms = shown_since_ms(binding, &effective);
    field("since", timestamp(since_ms));
    field("age", age(since_ms, now_ms));
    field("last event", timestamp(observation.observed_ms));
    field("confidence", effective.confidence.name().to_string());
    let subagents = shown_subagents(binding, &effective);
    if subagents > 0 {
        field("subagents", subagents.to_string());
    }
    field("source", observation.source.name().to_string());
    field(
        "process",
        match observation.pid {
            Some(pid) => format!("pid {pid}, {}", binding.liveness.name()),
            None => binding.liveness.name().to_string(),
        },
    );
}

/// `ms` as an RFC 3339 UTC time to the second, or the raw milliseconds
/// when the value is out of range.
fn timestamp(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms).map_or_else(
        || ms.to_string(),
        |time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    )
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
