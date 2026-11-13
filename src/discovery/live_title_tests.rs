use std::collections::BTreeMap;

use super::*;
use crate::live_store::{LiveFile, LiveTmux, PidProbe, PidStatus};
use crate::model::live::{LiveCounts, LiveDetail};
use crate::model::{
    AgentSessionId, AgentSessionNode, MuxProbe, MuxProbeStatus, MuxSessionId, MuxSessionNode,
    NodePresence,
};

use crate::discovery::live_state::bind_live;

const NOW_MS: i64 = 1_790_769_600_000;
const SECOND: i64 = 1_000;
const MINUTE: i64 = 60 * SECOND;
const DEFAULT_SOCKET: &str = "/tmp/tmux-1000/default";

/// A process table that holds exactly the pids given, with their start
/// times.
struct FakePids(BTreeMap<i64, u64>);

impl FakePids {
    fn running(pids: &[(i64, u64)]) -> Self {
        Self(pids.iter().copied().collect())
    }
}

impl PidProbe for FakePids {
    fn status(&self, pid: i64) -> PidStatus {
        match self.0.get(&pid) {
            Some(start) => PidStatus::Running {
                start: Some(*start),
            },
            None => PidStatus::Gone,
        }
    }
}

// --- the rules, one reading at a time --------------------------------

/// A title read at `NOW_MS` that has read `mark` since `since_ms`, the
/// probe before this one at `previous_ms`.
fn title(mark: TitleMark, since_ms: i64, previous_ms: Option<i64>) -> PaneTitle {
    PaneTitle {
        pane: Some("%0".to_string()),
        mark,
        observed_ms: NOW_MS,
        since_ms,
        previous_ms,
    }
}

/// The first probe to read `mark`.
fn first(mark: TitleMark) -> PaneTitle {
    title(mark, NOW_MS, None)
}

/// The second of two probes 5 s apart that read `mark`.
fn twice(mark: TitleMark) -> PaneTitle {
    title(mark, NOW_MS - 5 * SECOND, Some(NOW_MS - 5 * SECOND))
}

/// A hook entry's binding: `state` since and last heard of `age_ms`
/// before the probe, its process alive.
fn hook(state: LiveState, age_ms: i64) -> LiveBinding {
    LiveBinding::new(
        LiveObservation::new(state, LiveSource::Hook, NOW_MS - age_ms)
            .with_pid(4242, Some(77))
            .with_subagents(2),
        Liveness::Alive,
    )
}

#[test]
fn with_no_hook_entry_a_wait_or_work_binds_from_the_title_alone() {
    let waiting = combine(None, &first(TitleMark::Waiting)).expect("binds");
    let observation = waiting.observation;
    assert_eq!(observation.state, LiveState::Waiting);
    assert_eq!(observation.detail, Some(LiveDetail::Input));
    assert_eq!(observation.source, LiveSource::PaneTitle);
    assert_eq!(observation.observed_ms, NOW_MS);
    assert_eq!((observation.pid, observation.heartbeat_s), (None, 0));
    // No process to check: the 15-minute rule ages it out if probes
    // stop.
    assert_eq!(waiting.liveness, Liveness::Unknown);
    assert_eq!(
        waiting.effective(NOW_MS + 16 * MINUTE).state,
        LiveState::Unknown
    );

    let held = title(
        TitleMark::Working,
        NOW_MS - 3 * MINUTE,
        Some(NOW_MS - 5 * SECOND),
    );
    let working = combine(None, &held).expect("binds");
    assert_eq!(working.observation.state, LiveState::Working);
    assert_eq!(working.observation.detail, None);
    assert_eq!(working.observation.since_ms, NOW_MS - 3 * MINUTE);

    assert_eq!(combine(None, &twice(TitleMark::Neither)), None);
}

#[test]
fn a_fresh_hook_entry_wins_over_the_title() {
    for state in [
        LiveState::Ready,
        LiveState::Error,
        LiveState::Working,
        LiveState::Compacting,
        LiveState::Starting,
    ] {
        for mark in [TitleMark::Waiting, TitleMark::Working, TitleMark::Neither] {
            assert_eq!(
                combine(Some(&hook(state, MINUTE)), &twice(mark)),
                None,
                "{state:?} against {mark:?}"
            );
        }
    }
    assert_eq!(
        combine(
            Some(&hook(LiveState::Waiting, MINUTE)),
            &twice(TitleMark::Waiting)
        ),
        None
    );
}

#[test]
fn a_hook_wait_turns_working_once_two_probes_after_it_lose_the_marker() {
    let wait = hook(LiveState::Waiting, MINUTE);
    for mark in [TitleMark::Working, TitleMark::Neither] {
        let demoted = combine(Some(&wait), &twice(mark)).expect("demoted");
        let observation = demoted.observation;
        assert_eq!(observation.state, LiveState::Working, "{mark:?}");
        assert_eq!(observation.detail, None);
        assert_eq!(observation.source, LiveSource::PaneTitle);
        assert_eq!(observation.since_ms, NOW_MS - 5 * SECOND);
        assert_eq!(observation.observed_ms, NOW_MS);
        // Still the same agent: its process, the check of it, and its
        // subagents carry over.
        assert_eq!(
            (observation.pid, observation.pid_start),
            (Some(4242), Some(77))
        );
        assert_eq!(observation.subagents, 2);
        assert_eq!(demoted.liveness, Liveness::Alive);
    }
}

#[test]
fn one_probe_or_a_probe_before_the_wait_does_not_clear_it() {
    let wait = hook(LiveState::Waiting, 7 * SECOND);
    // The first probe without the marker, which may have landed before
    // Codex redrew its title.
    assert_eq!(combine(Some(&wait), &first(TitleMark::Working)), None);
    // Two probes, but the earlier ran before the hook's write.
    let straddling = title(
        TitleMark::Working,
        NOW_MS - MINUTE,
        Some(NOW_MS - 8 * SECOND),
    );
    assert_eq!(combine(Some(&wait), &straddling), None);
}

#[test]
fn an_approval_answered_before_any_probe_saw_the_marker_still_clears() {
    // The spinner ran before the wait began and no probe caught the
    // marker in between: the run started before the hook, but its last
    // two probes came after it.
    let wait = hook(LiveState::Waiting, 7 * SECOND);
    let run = title(
        TitleMark::Working,
        NOW_MS - 2 * MINUTE,
        Some(NOW_MS - 5 * SECOND),
    );
    let demoted = combine(Some(&wait), &run).expect("demoted");
    assert_eq!(demoted.observation.state, LiveState::Working);
    assert_eq!(demoted.observation.since_ms, NOW_MS - 5 * SECOND);
}

#[test]
fn a_quiet_or_stale_hook_entry_gives_way_to_a_title_that_says_something() {
    // Working with no hook event for eleven minutes reads quiet.
    let quiet = hook(LiveState::Working, 11 * MINUTE);
    let refreshed = combine(Some(&quiet), &first(TitleMark::Working)).expect("refreshed");
    assert_eq!(refreshed.observation.source, LiveSource::PaneTitle);
    assert_eq!(
        refreshed.observation.since_ms,
        NOW_MS - 11 * MINUTE,
        "the same state keeps its start"
    );
    assert_eq!(
        refreshed.effective(NOW_MS).confidence,
        LiveConfidence::Fresh
    );
    assert_eq!(refreshed.observation.pid, Some(4242));

    let waits = combine(Some(&quiet), &first(TitleMark::Waiting)).expect("waits");
    assert_eq!(waits.observation.state, LiveState::Waiting);
    assert_eq!(waits.observation.since_ms, NOW_MS);

    assert_eq!(combine(Some(&quiet), &first(TitleMark::Neither)), None);

    // A process that can't be checked and has been silent past the
    // 15-minute rule reads stale.
    let mut stale = hook(LiveState::Ready, 16 * MINUTE);
    stale.liveness = Liveness::Unknown;
    let taken = combine(Some(&stale), &first(TitleMark::Waiting)).expect("taken");
    assert_eq!(taken.observation.state, LiveState::Waiting);
    assert_eq!(taken.liveness, Liveness::Unknown);

    // An entry that knows the session but not its state is no evidence.
    let unknown = hook(LiveState::Unknown, SECOND);
    assert_eq!(
        combine(Some(&unknown), &first(TitleMark::Working))
            .expect("taken")
            .observation
            .state,
        LiveState::Working
    );
}

// --- folded into the tables ------------------------------------------

/// A tmux mux `name` on the default server whose active pane runs
/// `command`.
fn mux_node(name: &str, command: &str) -> GraphNode {
    let mut node = MuxSessionNode::new(MuxSessionId::new(format!("tmux:{name}")), "tmux", name);
    node.active_pane_command = Some(command.to_string());
    GraphNode::MuxSession(node)
}

fn mux_id(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

fn pane(name: &str, pane: &str) -> LivePane {
    LivePane::new(mux_id(name), Some(pane.to_string()))
}

fn session_id(key: &str) -> AgentSessionId {
    AgentSessionId::new("codex", "/home/me/.codex", key)
}

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(session_id(key))
}

/// A graph of `nodes` whose default-server probe read `titles`.
fn graph(nodes: Vec<GraphNode>, titles: &[(&str, PaneTitle)]) -> GraphSnapshot {
    let probe = MuxProbe {
        titles: titles
            .iter()
            .map(|(name, title)| (name.to_string(), title.clone()))
            .collect(),
        ..MuxProbe::new(MuxProbeStatus::Listed, NOW_MS / 1000)
    };
    GraphSnapshot {
        nodes,
        mux_probes: BTreeMap::from([("tmux".to_string(), probe)]),
        ..GraphSnapshot::empty()
    }
}

/// A store entry for the Codex session `key` in `state`, heard from
/// `age_ms` before the probe, its process 4242 running in pane `pane` of
/// tmux session `name`.
fn entry(key: &str, state: LiveState, age_ms: i64, name: &str, pane: &str) -> LiveEntry {
    let mut entry = LiveEntry::new("codex", key, NOW_MS - age_ms);
    entry.observation =
        LiveObservation::new(state, LiveSource::Hook, NOW_MS - age_ms).with_pid(4242, Some(77));
    entry.tmux = Some(LiveTmux {
        socket: Some(DEFAULT_SOCKET.to_string()),
        session: Some(name.to_string()),
        pane: Some(pane.to_string()),
    });
    entry.rev = 1;
    entry
}

fn file(entries: impl IntoIterator<Item = LiveEntry>) -> LiveFile {
    let mut file = LiveFile::default();
    for entry in entries {
        file.rev = file.rev.max(entry.rev);
        file.entries.insert(entry.key(), entry);
    }
    file
}

fn alive() -> FakePids {
    FakePids::running(&[(4242, 77)])
}

fn state_and_source(binding: Option<&LiveBinding>) -> Option<(LiveState, LiveSource)> {
    binding.map(|binding| (binding.observation.state, binding.observation.source))
}

#[test]
fn a_codex_pane_with_no_hooks_counts_on_its_mux_from_its_title() {
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", first(TitleMark::Waiting))],
    );
    let bound = bind_live(&snapshot, &LiveFile::default(), &alive());
    assert!(bound.nodes.is_empty());
    assert_eq!(
        state_and_source(bound.panes.get(&pane("cx", "%0"))),
        Some((LiveState::Waiting, LiveSource::PaneTitle))
    );
    let counts = snapshot.live_counts_by_mux(bound.tables(), NOW_MS);
    assert_eq!(
        counts.get(&mux_id("cx")).copied(),
        Some(LiveCounts {
            waiting: 1,
            ..LiveCounts::default()
        })
    );
}

#[test]
fn a_title_binds_only_where_its_pane_still_runs_a_harness() {
    // The probe records marks only for a harness that reads its titles;
    // a mux whose pane no longer runs one takes nothing from it.
    for command in ["zsh", "node", "sleep"] {
        let snapshot = graph(
            vec![mux_node("cx", command)],
            &[("cx", first(TitleMark::Waiting))],
        );
        assert!(
            bind_live(&snapshot, &LiveFile::default(), &alive()).is_empty(),
            "{command}"
        );
    }
    // A mux history keeps as gone takes no live state.
    let mut gone = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", first(TitleMark::Waiting))],
    );
    gone.node_presence.insert(
        NodeId::MuxSession(mux_id("cx")),
        NodePresence::Gone { last_seen: 1 },
    );
    assert!(bind_live(&gone, &LiveFile::default(), &alive()).is_empty());
    // Nor does a mux the probe recorded no title for.
    let other = graph(
        vec![mux_node("cx", "codex")],
        &[("elsewhere", first(TitleMark::Waiting))],
    );
    assert!(bind_live(&other, &LiveFile::default(), &alive()).is_empty());
}

#[test]
fn a_hook_wait_of_the_same_pane_is_cleared_in_its_own_slot() {
    let nodes = vec![
        mux_node("cx", "codex"),
        GraphNode::AgentSession(AgentSessionNode::new(session_id("s-1"), "codex")),
    ];
    let snapshot = graph(nodes, &[("cx", twice(TitleMark::Working))]);
    let store = file([entry("s-1", LiveState::Waiting, MINUTE, "cx", "%0")]);
    let bound = bind_live(&snapshot, &store, &alive());
    assert_eq!(
        state_and_source(bound.nodes.get(&session("s-1"))),
        Some((LiveState::Working, LiveSource::PaneTitle))
    );
    assert!(bound.panes.is_empty(), "one agent, counted once");

    // While the title still shows the marker, the hook's wait stands.
    let snapshot = graph(
        vec![
            mux_node("cx", "codex"),
            GraphNode::AgentSession(AgentSessionNode::new(session_id("s-1"), "codex")),
        ],
        &[("cx", twice(TitleMark::Waiting))],
    );
    let bound = bind_live(&snapshot, &store, &alive());
    assert_eq!(
        state_and_source(bound.nodes.get(&session("s-1"))),
        Some((LiveState::Waiting, LiveSource::Hook))
    );
    assert!(bound.panes.is_empty());
}

#[test]
fn a_fresh_hook_ready_holds_against_a_spinning_title() {
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", twice(TitleMark::Working))],
    );
    // A pane-only entry: no session node matches it yet.
    let store = file([entry("s-1", LiveState::Ready, MINUTE, "cx", "%0")]);
    let bound = bind_live(&snapshot, &store, &alive());
    assert_eq!(
        state_and_source(bound.panes.get(&pane("cx", "%0"))),
        Some((LiveState::Ready, LiveSource::Hook))
    );
    assert_eq!(bound.panes.len(), 1);
}

#[test]
fn a_stale_hook_entry_gives_way_to_the_title() {
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", first(TitleMark::Waiting))],
    );
    // No pid: its process can't be checked, and it has said nothing for
    // twenty minutes.
    let mut silent = entry("s-1", LiveState::Working, 20 * MINUTE, "cx", "%0");
    silent.observation.pid = None;
    silent.observation.pid_start = None;
    let bound = bind_live(&snapshot, &file([silent]), &alive());
    let binding = bound.panes.get(&pane("cx", "%0")).expect("bound");
    assert_eq!(
        (binding.observation.state, binding.observation.source),
        (LiveState::Waiting, LiveSource::PaneTitle)
    );
    assert_eq!(binding.effective(NOW_MS).confidence, LiveConfidence::Fresh);
}

#[test]
fn an_agent_in_another_pane_keeps_its_own_entry() {
    let nodes = vec![
        mux_node("cx", "codex"),
        GraphNode::AgentSession(AgentSessionNode::new(session_id("s-1"), "codex")),
    ];
    let snapshot = graph(nodes, &[("cx", first(TitleMark::Waiting))]);
    let store = file([entry("s-1", LiveState::Working, MINUTE, "cx", "%1")]);
    let bound = bind_live(&snapshot, &store, &alive());
    assert_eq!(
        state_and_source(bound.nodes.get(&session("s-1"))),
        Some((LiveState::Working, LiveSource::Hook))
    );
    assert_eq!(
        state_and_source(bound.panes.get(&pane("cx", "%0"))),
        Some((LiveState::Waiting, LiveSource::PaneTitle))
    );
}

#[test]
fn an_entry_whose_agent_is_gone_is_not_the_panes_agent() {
    // The session's entry ended; a new Codex runs in the pane.
    let nodes = vec![
        mux_node("cx", "codex"),
        GraphNode::AgentSession(AgentSessionNode::new(session_id("s-1"), "codex")),
    ];
    let snapshot = graph(nodes, &[("cx", first(TitleMark::Working))]);
    let store = file([entry("s-1", LiveState::Exited, MINUTE, "cx", "%0")]);
    let bound = bind_live(&snapshot, &store, &alive());
    assert_eq!(
        state_and_source(bound.nodes.get(&session("s-1"))),
        Some((LiveState::Exited, LiveSource::Hook)),
        "a recorded exit is never changed"
    );
    assert_eq!(
        state_and_source(bound.panes.get(&pane("cx", "%0"))),
        Some((LiveState::Working, LiveSource::PaneTitle))
    );

    // A pane-only entry whose process is dead gives up its slot to the
    // agent the pane runs now.
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", first(TitleMark::Waiting))],
    );
    let store = file([entry("s-1", LiveState::Working, MINUTE, "cx", "%0")]);
    let bound = bind_live(&snapshot, &store, &FakePids::running(&[]));
    let binding = bound.panes.get(&pane("cx", "%0")).expect("bound");
    assert_eq!(
        (binding.observation.state, binding.observation.source),
        (LiveState::Waiting, LiveSource::PaneTitle)
    );
    assert_eq!(binding.liveness, Liveness::Unknown);
}

#[test]
fn another_harnesss_entry_is_not_the_panes_agent() {
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", twice(TitleMark::Working))],
    );
    let mut claude = entry("c-1", LiveState::Waiting, MINUTE, "cx", "%0");
    claude.harness = "claude-code".to_string();
    let bound = bind_live(&snapshot, &file([claude]), &alive());
    // The wait is not cleared as Codex's would be: the title asserts
    // working in the pane on its own.
    assert_eq!(
        state_and_source(bound.panes.get(&pane("cx", "%0"))),
        Some((LiveState::Working, LiveSource::PaneTitle))
    );
}

#[test]
fn an_entry_with_no_pane_is_the_agent_of_its_muxs_active_pane() {
    let snapshot = graph(
        vec![mux_node("cx", "codex")],
        &[("cx", twice(TitleMark::Neither))],
    );
    let mut wait = entry("s-1", LiveState::Waiting, MINUTE, "cx", "%0");
    if let Some(tmux) = wait.tmux.as_mut() {
        tmux.pane = None;
    }
    let bound = bind_live(&snapshot, &file([wait]), &alive());
    let mux_slot = LivePane::new(mux_id("cx"), None);
    assert_eq!(
        state_and_source(bound.panes.get(&mux_slot)),
        Some((LiveState::Working, LiveSource::PaneTitle))
    );
    assert_eq!(bound.panes.len(), 1);
}
