//! Pane titles in the live tables (ADRs 0217, 0221): what a mux probe
//! read from the title of each pane of its server, folded into the
//! bindings of the store's entries.
//!
//! The probe keeps, for a pane whose harness reads its own titles, only
//! a mark ([`TitleMark`]) and the run of probes that read it
//! ([`PaneTitle`]). A pane that runs one of that harness's launchers
//! (`node` for an npm-installed Codex) has its mark kept too, and it
//! counts only where the harness's hook entry names the pane: rule 1
//! does not apply to it. Here each mark meets the pane's *hook entry*: the
//! bound store entry of the pane's harness whose tmux names the mux's
//! server and session and the pane (or no pane), that has not recorded
//! `exited` and whose process was not found dead. Read at the probe's
//! time ([`combine`]):
//!
//! 1. with no hook entry, `waiting` and `working` bind as a pane-only
//!    entry for the pane, with no pid;
//! 2. a fresh hook entry wins;
//! 3. a quiet, stale, or `unknown` hook entry gives way to `waiting` or
//!    `working` in its own slot;
//! 4. a fresh hook-derived `waiting` turns `working` once the title has
//!    read the same other mark on two consecutive probes, both after the
//!    hook entry's latest evidence: Codex fires no hook when the operator
//!    answers an approval.
//!
//! `neither` asserts nothing. A title never changes a hook entry's
//! `ready`, `error`, `compacting`, `starting`, or `exited`, and it never
//! binds to a session the store did not name.

use std::collections::BTreeMap;

use crate::live_store::LiveEntry;
use crate::model::live::{
    LiveBinding, LiveConfidence, LiveObservation, LiveSource, LiveState, Liveness, PaneTitle,
    TitleMark,
};
use crate::model::{GraphNode, GraphSnapshot, LivePane, NodeId, effective_socket, probe_key};

use super::{BoundLive, names_server, probed_socket, rank, server_of};

/// Fold every pane title `snapshot`'s probes recorded into `bound`, the
/// tables of the store's entries, whose winners per session and per pane
/// are `sessions` and `panes`.
pub(super) fn fold_titles(
    snapshot: &GraphSnapshot,
    sessions: &BTreeMap<NodeId, &LiveEntry>,
    panes: &BTreeMap<LivePane, &LiveEntry>,
    bound: &mut BoundLive,
) {
    for read in readings(snapshot) {
        let hook = hook_entry(&read, sessions, panes, bound);
        let (slot, binding) = match hook {
            Some((slot, binding)) => (slot, Some(binding)),
            // A launcher's pane is the harness's only on its hook's word.
            None if read.launched => continue,
            None => (
                Slot::Pane(LivePane::new(read.mux.clone(), read.title.pane.clone())),
                None,
            ),
        };
        let Some(next) = combine(binding.as_ref(), read.title) else {
            continue;
        };
        match slot {
            Slot::Session(node) => {
                bound.nodes.insert(node, next);
            }
            Slot::Pane(pane) => {
                bound.panes.insert(pane, next);
            }
        }
    }
}

/// What a pane's slot holds once its title is read, given the pane's
/// hook entry: `None` leaves the slot as it is. See the module's rules.
pub(crate) fn combine(hook: Option<&LiveBinding>, title: &PaneTitle) -> Option<LiveBinding> {
    let Some(hook) = hook else {
        return from_title(title, None);
    };
    let read = hook.effective(title.observed_ms);
    if read.confidence != LiveConfidence::Fresh || read.state == LiveState::Unknown {
        return from_title(title, Some(hook));
    }
    let recorded = &hook.observation;
    if recorded.state == LiveState::Waiting
        && title.mark != TitleMark::Waiting
        && title.held_twice_after(recorded.observed_ms)
    {
        let since_ms = if title.since_ms > recorded.observed_ms {
            title.since_ms
        } else {
            title.previous_ms.unwrap_or(title.observed_ms)
        };
        return Some(replacing(
            hook,
            LiveObservation::new(LiveState::Working, LiveSource::PaneTitle, title.observed_ms)
                .with_since_ms(since_ms),
        ));
    }
    None
}

/// The binding `title`'s mark asserts, in place of `hook` when there is
/// one: `None` for a mark that asserts nothing.
fn from_title(title: &PaneTitle, hook: Option<&LiveBinding>) -> Option<LiveBinding> {
    let (state, detail) = title.mark.asserts()?;
    let mut observation = LiveObservation::new(state, LiveSource::PaneTitle, title.observed_ms)
        .with_since_ms(title.since_ms);
    if let Some(detail) = detail {
        observation = observation.with_detail(detail);
    }
    Some(match hook {
        None => LiveBinding::new(observation, Liveness::Unknown),
        Some(hook) => {
            if hook.observation.state == state {
                observation.since_ms = hook.observation.since_ms;
            }
            replacing(hook, observation)
        }
    })
}

/// `observation` in place of `hook`'s, for the same agent: it keeps the
/// hook entry's process, what its check found, and its subagents.
fn replacing(hook: &LiveBinding, mut observation: LiveObservation) -> LiveBinding {
    let recorded = &hook.observation;
    observation.subagents = recorded.subagents;
    observation.pid = recorded.pid;
    observation.pid_start = recorded.pid_start;
    LiveBinding::new(observation, hook.liveness)
}

/// Where a binding sits.
enum Slot {
    Session(NodeId),
    Pane(LivePane),
}

/// One pane title a probe recorded, on the live tmux mux it describes.
struct Reading<'a> {
    mux: crate::model::MuxSessionId,
    /// The mux's server by the name tmux gives it; `None` is the default.
    server: Option<&'a str>,
    /// The full path of the server's socket, when its probe read one.
    socket_path: Option<&'a str>,
    session: &'a str,
    /// The harness whose titles the pane shows, by the program the same
    /// listing reported for it.
    harness: &'static str,
    /// The pane runs a launcher of that harness, not the harness itself.
    launched: bool,
    /// The pane is its session's active pane.
    active: bool,
    title: &'a PaneTitle,
}

/// Every pane title the snapshot's probes recorded for a pane of one of
/// its live tmux muxes whose program shows a harness's titles.
fn readings(snapshot: &GraphSnapshot) -> Vec<Reading<'_>> {
    use crate::discovery::harness::{PaneTitleReader, pane_title_reader};
    let mut readings = Vec::new();
    for node in snapshot.live_nodes() {
        let GraphNode::MuxSession(mux) = node else {
            continue;
        };
        if mux.backend != crate::discovery::tmux::TMUX_BACKEND {
            continue;
        }
        let Some(probe) = snapshot
            .mux_probes
            .get(&probe_key(&mux.backend, mux.socket.as_deref()))
        else {
            continue;
        };
        for (pane, title) in &probe.titles {
            let Some(listed) = probe
                .panes
                .get(pane)
                .filter(|listed| listed.session == mux.session_name())
            else {
                continue;
            };
            let Some(reader) = listed.command.as_deref().and_then(pane_title_reader) else {
                continue;
            };
            readings.push(Reading {
                mux: mux.id.clone(),
                server: effective_socket(mux.socket.as_deref()),
                socket_path: probed_socket(snapshot, mux),
                session: mux.session_name(),
                harness: reader.adapter().harness_key(),
                launched: matches!(reader, PaneTitleReader::Launched(_)),
                active: listed.active,
                title,
            });
        }
    }
    readings
}

/// The pane's hook entry, with its slot and its binding as `bound` holds
/// it: of the entries bound to a session or to the pane (or, for the
/// session's active pane, to the mux with no pane) that belong to the
/// pane's agent, the one the store wrote last.
fn hook_entry(
    read: &Reading<'_>,
    sessions: &BTreeMap<NodeId, &LiveEntry>,
    panes: &BTreeMap<LivePane, &LiveEntry>,
    bound: &BoundLive,
) -> Option<(Slot, LiveBinding)> {
    let mut in_pane = vec![LivePane::new(read.mux.clone(), read.title.pane.clone())];
    if read.active {
        in_pane.push(LivePane::new(read.mux.clone(), None));
    }
    let pane_entries = in_pane.into_iter().filter_map(|pane| {
        let entry = panes.get(&pane)?;
        let binding = bound.panes.get(&pane)?;
        Some((Slot::Pane(pane), *entry, *binding))
    });
    let session_entries = sessions
        .iter()
        .filter(|(_, entry)| names_pane(entry, read))
        .filter_map(|(node, entry)| {
            let binding = bound.nodes.get(node)?;
            Some((Slot::Session(node.clone()), *entry, *binding))
        });
    pane_entries
        .chain(session_entries)
        .filter(|(_, entry, binding)| {
            entry.harness == read.harness
                && binding.observation.state != LiveState::Exited
                && binding.liveness != Liveness::Dead
        })
        .max_by_key(|(_, entry, _)| rank(entry))
        .map(|(slot, _, binding)| (slot, binding))
}

/// Whether `entry`'s tmux names the reading's mux and its pane, or, for
/// the session's active pane, the mux and no pane: an entry that names no
/// pane came from a hook tmux answered for the session's active pane.
/// The server is matched as the binding matches it.
fn names_pane(entry: &LiveEntry, read: &Reading<'_>) -> bool {
    let Some(tmux) = &entry.tmux else {
        return false;
    };
    let Some(socket) = tmux.socket.as_deref() else {
        return false;
    };
    let Some(server) = server_of(socket) else {
        return false;
    };
    let pane = tmux.pane.as_deref().filter(|pane| !pane.is_empty());
    server == read.server
        && names_server(socket, read.socket_path)
        && tmux.session.as_deref() == Some(read.session)
        && match pane {
            None => read.active,
            Some(pane) => Some(pane) == read.title.pane.as_deref(),
        }
}

#[cfg(test)]
#[path = "live_title_tests.rs"]
mod tests;
