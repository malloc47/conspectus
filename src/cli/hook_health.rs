//! The live-state health line of `conspectus hook status`: whether a
//! harness's hooks are feeding the live-state store.
//!
//! It goes by what Conspectus already holds: the store's entries for the
//! harness, whether the hooks are installed, and the harness's agent
//! processes in the graph the last discovery saved (`graph.bin`, which a
//! running `conspectus serve` rewrites every cycle), each looked up in
//! the process table so that an agent that has since exited does not
//! count. It reads those two files and `/proc`, starts no process, and
//! asks no daemon, so `hook status` stays fast and read-only. For a
//! harness whose pane titles the mux probe reads (ADR 0217), the line
//! also says how many of its agents that graph shows from their titles,
//! which is how a harness whose hooks are silent can still show state.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::live_store::{LiveFile, LiveStore, PidProbe, PidStatus, ProcProbe};
use crate::model::live::LiveSource;
use crate::model::{GraphNode, GraphSnapshot, NodeId, RuntimeProcessRole};

/// What the health line reports for a harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Health {
    /// The store holds entries for the harness: its hooks report.
    Receiving {
        sessions: usize,
        /// The latest evidence among them, Unix epoch milliseconds.
        newest_ms: i64,
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
    /// no entries, and no entries. `running_agents` is asked only when
    /// the hooks are installed and the store holds nothing.
    pub(super) fn assess(
        harness: &str,
        reports_live_state: bool,
        installed: bool,
        file: &LiveFile,
        running_agents: impl FnOnce() -> usize,
    ) -> Self {
        let entries: Vec<i64> = file
            .entries
            .values()
            .filter(|entry| entry.harness == harness)
            .map(|entry| entry.observation.observed_ms)
            .collect();
        if let Some(newest_ms) = entries.iter().copied().max() {
            return Health::Receiving {
                sessions: entries.len(),
                newest_ms,
            };
        }
        if !reports_live_state {
            Health::Unsupported
        } else if !installed {
            Health::Off
        } else if running_agents() > 0 {
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
                format!("live state: {sessions} session{plural}, newest event {newest}")
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

/// How many distinct agent processes of `harness` the graph `snapshot`
/// recorded that `probe` finds running now. Only main agents count, not
/// their subagents, background helpers, or shells.
pub(super) fn running_agents(
    snapshot: &GraphSnapshot,
    harness: &str,
    probe: &dyn PidProbe,
) -> usize {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::RuntimeProcess(process)
                if process.harness_key.as_deref() == Some(harness)
                    && process.role == Some(RuntimeProcessRole::HumanAgent) =>
            {
                process.pid
            }
            _ => None,
        })
        .collect::<BTreeSet<i64>>()
        .into_iter()
        .filter(|pid| matches!(probe.status(*pid), PidStatus::Running { .. }))
        .count()
}

/// How many of `harness`'s agents the graph `snapshot` shows from their
/// pane titles: its live bindings whose source is the pane title, on a
/// session of the harness or on a pane of a mux whose active pane runs
/// it.
pub(super) fn title_agents(snapshot: &GraphSnapshot, harness: &str) -> usize {
    use crate::discovery::harness::adapters_for_program;
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
    let runs_harness = |mux: &crate::model::MuxSessionId| {
        snapshot.nodes.iter().any(|node| match node {
            GraphNode::MuxSession(node) if node.id == *mux => node
                .active_pane_command
                .as_deref()
                .and_then(|command| adapters_for_program(command).next())
                .is_some_and(|adapter| adapter.harness_key() == harness),
            _ => false,
        })
    };
    let panes = snapshot
        .pane_live
        .iter()
        .filter(|(pane, binding)| from_title(binding) && runs_harness(&pane.mux))
        .count();
    sessions + panes
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
        saved().map_or(0, |snapshot| {
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
    let mut message = health.message(crate::live_store::now_ms(), &init_command);
    if let Some(note) = title_note(titles) {
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
