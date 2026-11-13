//! The live-state health line of `conspectus hook status`: whether a
//! harness's hooks are feeding the live-state store.
//!
//! It goes by what Conspectus already holds: the store's entries for the
//! harness, whether the hooks are installed, and the harness's agent
//! processes in the graph the last discovery saved (`graph.bin`, which a
//! running `conspectus serve` rewrites every cycle), each looked up in
//! the process table so that an agent that has since exited does not
//! count. It reads those two files and `/proc`, starts no process, and
//! asks no daemon, so `hook status` stays fast and read-only. When the
//! store holds entries for the harness, the line also counts its running
//! agents that none of them describes, so one agent whose hooks are
//! silent is not hidden by another that reports. For a harness whose
//! pane titles the mux probe reads (ADR 0217), the line also says how
//! many of its agents that graph shows from their titles, and for one
//! whose own list of its sessions a poll reads (ADRs 0218, 0222), how
//! many it shows from that list: either is how a harness whose hooks are
//! silent can still show state.

use std::path::PathBuf;

use crate::live_store::{LiveFile, LiveStore, ProcProbe};
use crate::model::live::LiveSource;
use crate::model::{GraphNode, GraphSnapshot, NodeId};

/// What the health line reports for a harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Health {
    /// The store holds entries for the harness: its hooks report.
    Receiving {
        sessions: usize,
        /// The latest evidence among them, Unix epoch milliseconds.
        newest_ms: i64,
        /// Running agents of the harness that no entry describes.
        unreported: usize,
    },
    /// The hooks are installed and an agent of the harness runs, but the
    /// store holds nothing from it.
    Silent,
    /// The hooks are installed, the store holds nothing, and no running
    /// agent of the harness is known.
    NoEvents,
    /// The hooks are not installed and the store holds nothing.
    Off,
    /// The harness's hooks report sessions but no live state.
    Unsupported,
}

impl Health {
    /// The health of `harness`'s live state, deciding in this order:
    /// entries in the store (however the hooks got there), a hook that
    /// reports no live state, hooks not installed, a running agent with
    /// no entries, and no entries. `running_agents` is asked when the
    /// store holds entries of a harness whose hooks report live state,
    /// to count the agents none of them describes, and when the hooks
    /// are installed and the store holds nothing.
    pub(super) fn assess(
        harness: &str,
        reports_live_state: bool,
        installed: bool,
        file: &LiveFile,
        running_agents: impl FnOnce() -> Vec<RunningAgent>,
    ) -> Self {
        let entries: Vec<_> = file
            .entries
            .values()
            .filter(|entry| entry.harness == harness)
            .collect();
        if let Some(newest_ms) = entries
            .iter()
            .map(|entry| entry.observation.observed_ms)
            .max()
        {
            let unreported = if reports_live_state {
                running_agents()
                    .iter()
                    .filter(|agent| !entries.iter().any(|entry| agent.reported_by(entry)))
                    .count()
            } else {
                0
            };
            return Health::Receiving {
                sessions: entries.len(),
                newest_ms,
                unreported,
            };
        }
        if !reports_live_state {
            Health::Unsupported
        } else if !installed {
            Health::Off
        } else if !running_agents().is_empty() {
            Health::Silent
        } else {
            Health::NoEvents
        }
    }

    /// The line's first field.
    pub(super) fn label(self) -> &'static str {
        match self {
            Health::Receiving { .. } => "receiving",
            Health::Silent => "silent",
            Health::NoEvents => "no-events",
            Health::Off => "off",
            Health::Unsupported => "unsupported",
        }
    }

    /// The line's last field, for a reader. `init_command` is the command
    /// that installs the hooks.
    pub(super) fn message(self, now_ms: i64, init_command: &str) -> String {
        match self {
            Health::Receiving {
                sessions,
                newest_ms,
                unreported,
            } => {
                let plural = if sessions == 1 { "" } else { "s" };
                let newest = if newest_ms > now_ms {
                    "just now".to_string()
                } else {
                    format!(
                        "{} ago",
                        crate::output::render::format_relative_age(
                            newest_ms.div_euclid(1000),
                            now_ms.div_euclid(1000),
                        )
                    )
                };
                let unreported = match unreported {
                    0 => String::new(),
                    1 => "; 1 running agent has no live state".to_string(),
                    count => format!("; {count} running agents have no live state"),
                };
                format!("live state: {sessions} session{plural}, newest event {newest}{unreported}")
            }
            Health::Silent => "live state: active but no events: hooks may be untrusted, \
                 disabled, or the agent was started before they were installed"
                .to_string(),
            Health::NoEvents => "live state: no events yet, and no running agent seen".to_string(),
            Health::Off => {
                format!("live state: off; `{init_command}` installs the hooks that report it")
            }
            Health::Unsupported => "live state: not reported by this harness's hook".to_string(),
        }
    }
}

pub(super) use crate::discovery::live_state::{RunningAgent, running_agents};

/// How many of `harness`'s agents the graph `snapshot` shows from their
/// pane titles: its live bindings whose source is the pane title, on a
/// session of the harness or on a pane whose program shows the harness's
/// titles (by the probe's pane listing, else the mux's active pane).
pub(super) fn title_agents(snapshot: &GraphSnapshot, harness: &str) -> usize {
    use crate::discovery::harness::pane_title_reader;
    let from_title = |binding: &crate::model::live::LiveBinding| {
        binding.observation.source == LiveSource::PaneTitle
    };
    let sessions = snapshot
        .node_live
        .iter()
        .filter(|(id, binding)| {
            from_title(binding)
                && matches!(id, NodeId::AgentSession(session) if session.harness_key == harness)
        })
        .count();
    let shows_harness = |pane: &crate::model::LivePane| {
        snapshot.nodes.iter().any(|node| match node {
            GraphNode::MuxSession(mux) if mux.id == pane.mux => {
                let listed = pane.pane.as_deref().and_then(|id| {
                    snapshot
                        .mux_probes
                        .get(&crate::model::probe_key(
                            &mux.backend,
                            mux.socket.as_deref(),
                        ))?
                        .pane_command(mux.session_name(), id)
                });
                listed
                    .unwrap_or(mux.active_pane_command.as_deref())
                    .and_then(pane_title_reader)
                    .is_some_and(|reader| reader.adapter().harness_key() == harness)
            }
            _ => false,
        })
    };
    let panes = snapshot
        .pane_live
        .iter()
        .filter(|(pane, binding)| from_title(binding) && shows_harness(pane))
        .count();
    sessions + panes
}

/// How many of `harness`'s agents the graph `snapshot` shows from a poll
/// of the harness's own list of its sessions: its live bindings whose
/// source is that poll, on a session of the harness, or on a pane when
/// the harness is the only one the snapshot carries a poll of (a pane
/// binding names no harness).
pub(super) fn poll_agents(snapshot: &GraphSnapshot, harness: &str) -> usize {
    let from_poll = |binding: &crate::model::live::LiveBinding| {
        binding.observation.source == LiveSource::HarnessPoll
    };
    let sessions = snapshot
        .node_live
        .iter()
        .filter(|(id, binding)| {
            from_poll(binding)
                && matches!(id, NodeId::AgentSession(session) if session.harness_key == harness)
        })
        .count();
    let only_poll = snapshot.live_polls.len() == 1 && snapshot.live_polls.contains_key(harness);
    let panes = if only_poll {
        snapshot
            .pane_live
            .values()
            .filter(|binding| from_poll(binding))
            .count()
    } else {
        0
    };
    sessions + panes
}

/// What the health line adds when the graph shows `count` agents from
/// the harness's own list of its sessions; nothing for none.
pub(super) fn poll_note(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 agent shown from its agent list".to_string()),
        count => Some(format!("{count} agents shown from their agent list")),
    }
}

/// What the health line adds when the graph shows `count` agents from
/// their pane titles; nothing for none.
pub(super) fn title_note(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 agent shown from its pane title".to_string()),
        count => Some(format!("{count} agents shown from their pane titles")),
    }
}

/// Print the health line for `harness`:
/// `<label>\t<harness>\t<store file>\t<message>`. `installed` says
/// whether the hooks are installed in the scope `hook status` looked at,
/// and `scope_flag` is that scope's flag for the install hint (empty for
/// the user scope).
pub(super) fn print_health(harness: &str, installed: bool, scope_flag: &str) {
    let adapter = crate::discovery::harness::registered_adapters()
        .find(|adapter| adapter.harness_key() == harness);
    let reports_live_state =
        adapter.is_some_and(crate::discovery::harness::HarnessAdapter::reports_live_state);
    let reads_titles = adapter.is_some_and(crate::discovery::harness::reads_pane_titles);
    let store = LiveStore::from_env();
    let file = store.as_ref().map(LiveStore::read).unwrap_or_default();
    let graph = std::cell::OnceCell::new();
    let saved = || graph.get_or_init(saved_graph).as_ref();
    let health = Health::assess(harness, reports_live_state, installed, &file, || {
        saved().map_or_else(Vec::new, |snapshot| {
            running_agents(snapshot, harness, &ProcProbe::system())
        })
    });
    let titles = if reads_titles {
        saved().map_or(0, |snapshot| title_agents(snapshot, harness))
    } else {
        0
    };
    let init_command = format!(
        "{} hook init {harness}{scope_flag}",
        crate::invocation::invoked_name()
    );
    let path = store
        .as_ref()
        .map_or_else(|| PathBuf::from("-"), LiveStore::path);
    // Building a poller runs nothing; it only says the harness has one.
    let polls = adapter.is_some_and(|adapter| adapter.live_poller().is_some());
    let polled = if polls {
        saved().map_or(0, |snapshot| poll_agents(snapshot, harness))
    } else {
        0
    };
    let mut message = health.message(crate::live_store::now_ms(), &init_command);
    for note in [title_note(titles), poll_note(polled)]
        .into_iter()
        .flatten()
    {
        message.push_str("; ");
        message.push_str(&note);
    }
    println!(
        "{}\t{harness}\t{}\t{message}",
        health.label(),
        path.display()
    );
}

/// The graph the last discovery saved, or `None` when there is none this
/// version can read.
fn saved_graph() -> Option<GraphSnapshot> {
    let handle = crate::snapshot::open_mmap(&crate::snapshot::graph_bin_path()).ok()?;
    crate::snapshot::deserialize_owned(&handle).ok()
}

#[cfg(test)]
#[path = "hook_health_tests.rs"]
mod tests;
