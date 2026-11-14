use std::collections::BTreeSet;

use super::*;
use crate::discovery::live_state::AgentPane;
use crate::live_store::{LiveEntry, PidProbe, PidStatus};
use crate::model::{RelationKind, RuntimeProcessId, RuntimeProcessNode, RuntimeProcessRole};

const NOW_MS: i64 = 1_790_769_600_000;

fn file(entries: &[(&str, &str, i64)]) -> LiveFile {
    let mut file = LiveFile::default();
    for (harness, session, observed_ms) in entries {
        let entry = LiveEntry::new(harness, session, *observed_ms);
        file.entries.insert(entry.key(), entry);
    }
    file
}

fn never_asked() -> Vec<RunningAgent> {
    panic!("the running agents are only needed for a hook that reports live state")
}

/// Running agents with these pids, tied to no session.
fn agents(pids: &[i64]) -> Vec<RunningAgent> {
    pids.iter()
        .map(|pid| RunningAgent {
            pid: *pid,
            sessions: BTreeSet::new(),
            panes: BTreeSet::new(),
        })
        .collect()
}

#[test]
fn entries_for_the_harness_mean_its_hooks_report_whatever_the_install_says() {
    let store = file(&[
        ("claude-code", "a", NOW_MS - 90_000),
        ("claude-code", "b", NOW_MS - 12_000),
        ("codex", "c", NOW_MS),
    ]);
    let receiving = Health::Receiving {
        sessions: 2,
        newest_ms: NOW_MS - 12_000,
        unreported: 0,
    };
    for installed in [true, false] {
        assert_eq!(
            Health::assess("claude-code", true, installed, &store, &NOBODY, Vec::new),
            receiving
        );
    }
    assert_eq!(receiving.label(), "receiving");
    assert_eq!(
        receiving.message(NOW_MS, "an install hint"),
        "live state: 2 sessions, newest event 12s ago"
    );
    let single = Health::Receiving {
        sessions: 1,
        newest_ms: NOW_MS + 5_000,
        unreported: 0,
    };
    assert_eq!(
        single.message(NOW_MS, ""),
        "live state: 1 session, newest event just now"
    );
}

#[test]
fn an_installed_hook_with_a_running_agent_and_no_entries_is_silent() {
    let store = file(&[("codex", "c", NOW_MS)]);
    let silent = Health::assess("claude-code", true, true, &store, &NOBODY, || agents(&[10]));
    assert_eq!(silent, Health::Silent);
    assert_eq!(silent.label(), "silent");
    assert_eq!(
        silent.message(NOW_MS, ""),
        "live state: active but no events: hooks may be untrusted, disabled, or the agent \
         was started before they were installed"
    );

    let idle = Health::assess("claude-code", true, true, &store, &NOBODY, Vec::new);
    assert_eq!(idle, Health::NoEvents);
    assert_eq!(idle.label(), "no-events");
}

#[test]
fn a_hook_not_installed_says_how_to_install_it() {
    let off = Health::assess(
        "claude-code",
        true,
        false,
        &LiveFile::default(),
        &NOBODY,
        never_asked,
    );
    assert_eq!(off, Health::Off);
    assert_eq!(off.label(), "off");
    assert_eq!(
        off.message(
            NOW_MS,
            "`csp hook init claude-code --scope project` installs the hooks that report it"
        ),
        "live state: off; `csp hook init claude-code --scope project` installs the hooks \
         that report it"
    );
}

#[test]
fn a_hook_that_reports_no_live_state_is_never_called_silent() {
    for installed in [true, false] {
        let health = Health::assess(
            "aider",
            false,
            installed,
            &LiveFile::default(),
            &NOBODY,
            never_asked,
        );
        assert_eq!(health, Health::Unsupported);
        assert_eq!(health.label(), "unsupported");
    }
}

#[test]
fn the_claude_code_codex_opencode_and_pi_hooks_report_live_state() {
    let reports = |harness: &str| {
        crate::discovery::harness::registered_adapters()
            .find(|adapter| adapter.harness_key() == harness)
            .is_some_and(crate::discovery::harness::HarnessAdapter::reports_live_state)
    };
    assert!(reports("claude-code"));
    assert!(reports("codex"));
    assert!(reports("opencode"));
    assert!(reports("pi"));
    assert!(!reports("aider"));
}

struct Table(&'static [i64]);

/// A process table in which nothing runs.
static NOBODY: Table = Table(&[]);

impl PidProbe for Table {
    fn status(&self, pid: i64) -> PidStatus {
        if self.0.contains(&pid) {
            PidStatus::Running { start: None }
        } else {
            PidStatus::Gone
        }
    }
}

fn process(key: &str, harness: &str, pid: i64, role: RuntimeProcessRole) -> GraphNode {
    GraphNode::RuntimeProcess(RuntimeProcessNode {
        id: RuntimeProcessId::new(key),
        observation_key: key.to_string(),
        pid: Some(pid),
        parent_pid: None,
        root_pane_pid: None,
        command: None,
        cwd: None,
        harness_key: Some(harness.to_string()),
        role: Some(role),
        depth: None,
        observed_epoch: None,
    })
}

#[test]
fn running_agents_counts_main_agents_of_the_harness_still_running() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            process("main", "claude-code", 10, RuntimeProcessRole::HumanAgent),
            // The same process seen through a second pane.
            process("again", "claude-code", 10, RuntimeProcessRole::HumanAgent),
            process("child", "claude-code", 11, RuntimeProcessRole::Subagent),
            process("other", "codex", 12, RuntimeProcessRole::HumanAgent),
            process("exited", "claude-code", 13, RuntimeProcessRole::HumanAgent),
        ],
        ..GraphSnapshot::empty()
    };
    let probe = Table(&[10, 11, 12]);
    assert_eq!(
        running_agents(&snapshot, "claude-code", &probe),
        agents(&[10])
    );
    assert_eq!(running_agents(&snapshot, "codex", &probe), agents(&[12]));
    assert_eq!(running_agents(&snapshot, "pi", &probe), agents(&[]));
    assert_eq!(
        running_agents(&snapshot, "claude-code", &Table(&[])),
        agents(&[])
    );
}

#[test]
fn only_codex_pane_titles_are_read_for_now() {
    let reads = |harness: &str| {
        crate::discovery::harness::registered_adapters()
            .find(|adapter| adapter.harness_key() == harness)
            .is_some_and(crate::discovery::harness::reads_pane_titles)
    };
    assert!(reads("codex"));
    for harness in ["claude-code", "opencode", "pi", "aider"] {
        assert!(!reads(harness), "{harness}");
    }
}

#[test]
fn title_agents_counts_the_harnesss_bindings_read_from_pane_titles() {
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
    use crate::model::{AgentSessionId, LivePane, MuxSessionId, MuxSessionNode};

    let mux = |name: &str, command: &str| {
        let mut node = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
        node.active_pane_command = Some(command.to_string());
        GraphNode::MuxSession(node)
    };
    let binding = |source| {
        LiveBinding::new(
            LiveObservation::new(LiveState::Waiting, source, NOW_MS),
            Liveness::Unknown,
        )
    };
    let pane = |name: &str| LivePane::new(MuxSessionId::new(format!("tmux:{name}")), None);
    let session = |harness: &str, key: &str| {
        NodeId::AgentSession(AgentSessionId::new(harness, "/scope", key))
    };
    let mut snapshot = GraphSnapshot {
        nodes: vec![mux("cx", "codex"), mux("hooked", "codex"), mux("sh", "zsh")],
        ..GraphSnapshot::empty()
    };
    snapshot
        .pane_live
        .insert(pane("cx"), binding(LiveSource::PaneTitle));
    snapshot
        .pane_live
        .insert(pane("hooked"), binding(LiveSource::Hook));
    snapshot
        .pane_live
        .insert(pane("sh"), binding(LiveSource::PaneTitle));
    snapshot
        .node_live
        .insert(session("codex", "a"), binding(LiveSource::PaneTitle));
    snapshot
        .node_live
        .insert(session("codex", "b"), binding(LiveSource::Hook));
    snapshot
        .node_live
        .insert(session("pi", "c"), binding(LiveSource::PaneTitle));

    assert_eq!(title_agents(&snapshot, "codex"), 2);
    assert_eq!(title_agents(&snapshot, "claude-code"), 0);
    assert_eq!(title_agents(&GraphSnapshot::empty(), "codex"), 0);
}

#[test]
fn title_agents_reads_each_panes_own_program_from_the_pane_listing() {
    use crate::model::live::{LiveBinding, LiveObservation, LiveSource, LiveState, Liveness};
    use crate::model::{
        LivePane, MuxProbe, MuxProbeStatus, MuxSessionId, MuxSessionNode, ProbedPane,
    };

    // The session's active pane runs a shell; Codex runs in %1 and an
    // npm-installed Codex (`node`) in %2.
    let mut node = MuxSessionNode::new(MuxSessionId::new("tmux:cx"), "tmux", "cx");
    node.active_pane_command = Some("zsh".to_string());
    let mut probe = MuxProbe::new(MuxProbeStatus::Listed, NOW_MS / 1000);
    for (pane, command) in [("%0", "zsh"), ("%1", "codex"), ("%2", "node")] {
        probe.panes.insert(
            pane.to_string(),
            ProbedPane {
                session: "cx".to_string(),
                command: Some(command.to_string()),
                active: pane == "%0",
            },
        );
    }
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(node)],
        ..GraphSnapshot::empty()
    };
    snapshot.mux_probes.insert("tmux".to_string(), probe);
    for pane in ["%1", "%2"] {
        snapshot.pane_live.insert(
            LivePane::new(MuxSessionId::new("tmux:cx"), Some(pane.to_string())),
            LiveBinding::new(
                LiveObservation::new(LiveState::Working, LiveSource::PaneTitle, NOW_MS),
                Liveness::Unknown,
            ),
        );
    }
    assert_eq!(title_agents(&snapshot, "codex"), 2);
}

#[test]
fn poll_agents_counts_the_harnesss_bindings_read_from_its_agent_list() {
    use crate::model::live::{
        LiveBinding, LiveObservation, LivePoll, LiveSource, LiveState, Liveness,
    };
    use crate::model::{AgentSessionId, LivePane, MuxSessionId};

    let binding = |source| {
        LiveBinding::new(
            LiveObservation::new(LiveState::Ready, source, NOW_MS),
            Liveness::Unknown,
        )
    };
    let session = |harness: &str, key: &str| {
        NodeId::AgentSession(AgentSessionId::new(harness, "/scope", key))
    };
    let mut snapshot = GraphSnapshot::empty();
    snapshot.node_live.insert(
        session("claude-code", "a"),
        binding(LiveSource::HarnessPoll),
    );
    snapshot
        .node_live
        .insert(session("claude-code", "b"), binding(LiveSource::Hook));
    snapshot
        .node_live
        .insert(session("codex", "c"), binding(LiveSource::HarnessPoll));
    snapshot.pane_live.insert(
        LivePane::new(MuxSessionId::new("tmux:w"), Some("%1".to_string())),
        binding(LiveSource::HarnessPoll),
    );
    // A pane binding names no harness: it counts only while one harness's
    // poll is on the graph.
    assert_eq!(poll_agents(&snapshot, "claude-code"), 1);
    snapshot
        .live_polls
        .insert("claude-code".to_string(), LivePoll::default());
    assert_eq!(poll_agents(&snapshot, "claude-code"), 2);
    assert_eq!(poll_agents(&snapshot, "pi"), 0);
}

#[test]
fn the_line_says_how_many_agents_their_agent_list_shows() {
    assert_eq!(poll_note(0), None);
    assert_eq!(
        poll_note(1).as_deref(),
        Some("1 agent shown from its agent list")
    );
    assert_eq!(
        poll_note(2).as_deref(),
        Some("2 agents shown from their agent list")
    );
}

#[test]
fn the_line_says_how_many_agents_their_pane_titles_show() {
    assert_eq!(title_note(0), None);
    assert_eq!(
        title_note(1).as_deref(),
        Some("1 agent shown from its pane title")
    );
    assert_eq!(
        title_note(3).as_deref(),
        Some("3 agents shown from their pane titles")
    );
}

/// A store entry of `harness`'s `session` that recorded `pid`.
fn entry_with_pid(harness: &str, session: &str, pid: Option<i64>) -> LiveEntry {
    let mut entry = LiveEntry::new(harness, session, NOW_MS - 3_000);
    entry.observation.pid = pid;
    entry
}

fn store_of(entries: Vec<LiveEntry>) -> LiveFile {
    let mut file = LiveFile::default();
    for entry in entries {
        file.entries.insert(entry.key(), entry);
    }
    file
}

/// A link from the process node `key` to `harness`'s session `session`.
fn runs(
    key: &str,
    harness: &str,
    session: &str,
    relation: RelationKind,
) -> crate::model::GraphLink {
    use crate::model::{AgentSessionId, LinkEndpoint, Provenance};
    crate::model::GraphLink::new(
        format!("test:{key}:{session}"),
        NodeId::RuntimeProcess(RuntimeProcessId::new(key)),
        LinkEndpoint::Node {
            id: NodeId::AgentSession(AgentSessionId::new(harness, "/scope", session)),
        },
        relation,
        Provenance::Discovered,
    )
}

#[test]
fn a_running_agent_with_no_entry_is_counted_beside_one_that_reports() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            process("reports", "claude-code", 10, RuntimeProcessRole::HumanAgent),
            process("silent", "claude-code", 11, RuntimeProcessRole::HumanAgent),
            process("exited", "claude-code", 12, RuntimeProcessRole::HumanAgent),
            process("child", "claude-code", 13, RuntimeProcessRole::Subagent),
        ],
        ..GraphSnapshot::empty()
    };
    let probe = Table(&[10, 11, 13]);
    let store = store_of(vec![
        entry_with_pid("claude-code", "a", Some(10)),
        // An exited agent's entry describes no running one.
        entry_with_pid("claude-code", "b", Some(12)),
    ]);
    let health = Health::assess("claude-code", true, true, &store, &probe, || {
        running_agents(&snapshot, "claude-code", &probe)
    });
    assert_eq!(
        health,
        Health::Receiving {
            sessions: 2,
            newest_ms: NOW_MS - 3_000,
            unreported: 1,
        }
    );
    assert_eq!(health.label(), "receiving");
    assert_eq!(
        health.message(NOW_MS, ""),
        "live state: 2 sessions, newest event 3s ago; 1 running agent has no live state"
    );
    let two = Health::Receiving {
        sessions: 1,
        newest_ms: NOW_MS,
        unreported: 2,
    };
    assert_eq!(
        two.message(NOW_MS, ""),
        "live state: 1 session, newest event 0s ago; 2 running agents have no live state"
    );
}

#[test]
fn an_entry_of_the_agents_session_reports_for_it_whatever_pid_it_recorded() {
    // A wrapper: the graph's process is the wrapper, while the hook
    // recorded the harness process beneath it.
    let snapshot = GraphSnapshot {
        nodes: vec![
            process("wrapped", "claude-code", 20, RuntimeProcessRole::HumanAgent),
            process(
                "candidate",
                "claude-code",
                21,
                RuntimeProcessRole::HumanAgent,
            ),
            process(
                "ambiguous",
                "claude-code",
                22,
                RuntimeProcessRole::HumanAgent,
            ),
        ],
        candidate_links: vec![
            runs(
                "wrapped",
                "claude-code",
                "s-wrapped",
                RelationKind::ProcessIdentifiesSession,
            ),
            runs(
                "candidate",
                "claude-code",
                "s-candidate",
                RelationKind::ProcessCandidatesSession,
            ),
            // Two candidates name no session for the process.
            runs(
                "ambiguous",
                "claude-code",
                "s-one",
                RelationKind::ProcessCandidatesSession,
            ),
            runs(
                "ambiguous",
                "claude-code",
                "s-two",
                RelationKind::ProcessCandidatesSession,
            ),
        ],
        ..GraphSnapshot::empty()
    };
    let probe = Table(&[20, 21, 22]);
    let running = running_agents(&snapshot, "claude-code", &probe);
    assert_eq!(
        running
            .iter()
            .map(|agent| agent.sessions.iter().cloned().collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [
            vec!["s-wrapped".to_string()],
            vec!["s-candidate".to_string()],
            vec![]
        ]
    );
    let store = store_of(vec![
        entry_with_pid("claude-code", "s-wrapped", Some(9020)),
        entry_with_pid("claude-code", "s-candidate", None),
        entry_with_pid("claude-code", "s-one", Some(9022)),
    ]);
    let health = Health::assess("claude-code", true, true, &store, &NOBODY, || {
        running.clone()
    });
    assert!(
        matches!(health, Health::Receiving { unreported: 1, .. }),
        "only the agent with two candidate sessions is unreported: {health:?}"
    );
}

#[test]
fn entries_of_a_harness_that_reports_no_live_state_count_no_agent() {
    let store = store_of(vec![entry_with_pid("aider", "a", Some(10))]);
    let health = Health::assess("aider", false, true, &store, &NOBODY, never_asked);
    assert!(matches!(health, Health::Receiving { unreported: 0, .. }));
}

const DEFAULT_SOCKET: &str = "/tmp/tmux-1000/default";

/// Tmux session `editor` on the default server, whose probe read the
/// socket path and listed panes `%1` (active) and `%2`, and Claude Code
/// processes found under its active pane (`active`) or seen only by a
/// hook (`hooked`), each a main agent.
fn editor_with_agents(active: &[(&str, i64)], hooked: &[(&str, i64)]) -> GraphSnapshot {
    use crate::model::{
        LinkEndpoint, MuxProbe, MuxProbeStatus, MuxSessionId, MuxSessionNode, ProbedPane,
        Provenance,
    };
    let mux_id = MuxSessionId::new("tmux:editor");
    let mut nodes = vec![GraphNode::MuxSession(MuxSessionNode::new(
        mux_id.clone(),
        "tmux",
        "editor",
    ))];
    let mut candidate_links = Vec::new();
    let observed = active
        .iter()
        .map(|agent| (agent, "active_pane_process_observation"))
        .chain(
            hooked
                .iter()
                .map(|agent| (agent, "hook_process_observation")),
        );
    for ((key, pid), evidence) in observed {
        nodes.push(process(
            key,
            "claude-code",
            *pid,
            RuntimeProcessRole::HumanAgent,
        ));
        let mut link = crate::model::GraphLink::new(
            format!("test:contains:{key}"),
            NodeId::MuxSession(mux_id.clone()),
            LinkEndpoint::Node {
                id: NodeId::RuntimeProcess(RuntimeProcessId::new(*key)),
            },
            RelationKind::MuxContainsProcess,
            Provenance::StrongDiscovered,
        );
        link.source_metadata.evidence = Some(evidence.to_string());
        candidate_links.push(link);
    }
    let mut probe = MuxProbe::new(MuxProbeStatus::Listed, NOW_MS / 1000);
    probe.socket_path = Some(DEFAULT_SOCKET.to_string());
    for (pane, active) in [("%1", true), ("%2", false)] {
        probe.panes.insert(
            pane.to_string(),
            ProbedPane {
                session: "editor".to_string(),
                command: Some("claude".to_string()),
                active,
            },
        );
    }
    GraphSnapshot {
        nodes,
        candidate_links,
        mux_probes: std::collections::BTreeMap::from([("tmux".to_string(), probe)]),
        ..GraphSnapshot::empty()
    }
}

/// A store entry of Claude Code session `session` that recorded `pid`
/// and names pane `pane` of tmux session `editor` on the server whose
/// socket is at `socket`.
fn entry_in_pane(session: &str, pid: i64, socket: &str, pane: &str) -> LiveEntry {
    let mut entry = entry_with_pid("claude-code", session, Some(pid));
    entry.tmux = Some(crate::live_store::LiveTmux {
        socket: Some(socket.to_string()),
        session: Some("editor".to_string()),
        pane: Some(pane.to_string()),
    });
    entry
}

#[test]
fn running_agents_names_the_active_pane_its_mux_found_the_process_under() {
    let snapshot = editor_with_agents(&[("tree", 30)], &[("hook", 31)]);
    let running = running_agents(&snapshot, "claude-code", &Table(&[30, 31]));
    let panes: Vec<Vec<AgentPane>> = running
        .iter()
        .map(|agent| agent.panes.iter().cloned().collect())
        .collect();
    assert_eq!(
        panes,
        [
            vec![AgentPane {
                server: None,
                socket_path: Some(DEFAULT_SOCKET.to_string()),
                session: "editor".to_string(),
                pane: "%1".to_string(),
            }],
            // A hook's observation does not say which pane.
            vec![],
        ]
    );
}

#[test]
fn an_entry_naming_the_agents_pane_reports_for_it_whatever_pid_it_recorded() {
    // The graph found a wrapper (pid 30) under pane %1 and ties it to no
    // session yet; the hook recorded the harness process beneath it.
    let snapshot = editor_with_agents(&[("wrapper", 30)], &[]);
    let probe = Table(&[30, 9030, 9031]);
    let running = running_agents(&snapshot, "claude-code", &probe);
    let [agent] = running.as_slice() else {
        panic!("one running agent: {running:?}")
    };
    let in_pane = entry_in_pane("new", 9030, DEFAULT_SOCKET, "%1");
    assert!(agent.reported_by(&in_pane, &probe));

    // Another pane of the same mux runs another agent.
    assert!(!agent.reported_by(&entry_in_pane("beside", 9031, DEFAULT_SOCKET, "%2"), &probe));
    // The same session and pane on another server.
    assert!(!agent.reported_by(
        &entry_in_pane("elsewhere", 9031, "/run/other/tmux-1000/default", "%1"),
        &probe
    ));
    // An agent that ran in the pane before: exited, or its process gone.
    let mut exited = entry_in_pane("before", 9031, DEFAULT_SOCKET, "%1");
    exited.observation.state = crate::model::live::LiveState::Exited;
    assert!(!agent.reported_by(&exited, &probe));
    assert!(!agent.reported_by(&entry_in_pane("gone", 9032, DEFAULT_SOCKET, "%1"), &probe));
    // An entry that names no session or no pane names no pane.
    let mut no_session = in_pane.clone();
    no_session.tmux.as_mut().expect("tmux").session = None;
    assert!(!agent.reported_by(&no_session, &probe));
    let mut no_pane = in_pane.clone();
    no_pane.tmux.as_mut().expect("tmux").pane = None;
    assert!(!agent.reported_by(&no_pane, &probe));

    let store = store_of(vec![in_pane]);
    let health = Health::assess("claude-code", true, true, &store, &probe, || {
        running.clone()
    });
    assert!(
        matches!(health, Health::Receiving { unreported: 0, .. }),
        "{health:?}"
    );
}

#[test]
fn an_entry_for_another_pane_of_the_mux_leaves_its_agent_unreported() {
    let snapshot = editor_with_agents(&[("left", 30)], &[]);
    let probe = Table(&[30, 9031]);
    let store = store_of(vec![entry_in_pane("right", 9031, DEFAULT_SOCKET, "%2")]);
    let health = Health::assess("claude-code", true, true, &store, &probe, || {
        running_agents(&snapshot, "claude-code", &probe)
    });
    assert!(
        matches!(health, Health::Receiving { unreported: 1, .. }),
        "{health:?}"
    );
}
