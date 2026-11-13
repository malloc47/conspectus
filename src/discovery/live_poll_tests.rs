use std::collections::BTreeMap;

use super::*;
use crate::live_store::{LiveTmux, PidStatus};
use crate::model::live::{LiveConfidence, LiveDetail, PolledAgent};
use crate::model::{AgentSessionId, AgentSessionNode, GraphNode, MuxSessionId, MuxSessionNode};

use crate::discovery::live_state::bind_live;

const NOW_MS: i64 = 1_794_546_100_000;
const SECOND: i64 = 1_000;
const MINUTE: i64 = 60 * SECOND;
const CLAUDE: &str = "claude-code";
const SESSION: &str = "8c1d2f0e-5b7a-4c39-9e21-3a6f0b9d4e17";

// --- the rule, one binding at a time ---------------------------------

/// A Claude Code hook binding in `state`, last heard of `age_ms` before
/// the poll, its process 4242 (started at 77) alive, two subagents.
fn hook(state: LiveState, age_ms: i64) -> LiveBinding {
    LiveBinding::new(
        LiveObservation::new(state, LiveSource::Hook, NOW_MS - age_ms)
            .with_since_ms(NOW_MS - age_ms - MINUTE)
            .with_pid(4242, Some(77))
            .with_subagents(2),
        Liveness::Alive,
    )
}

/// Process 4242 (started at 77) listed in `state`, read since
/// `since_ms`.
fn listed(state: LiveState, detail: Option<LiveDetail>, since_ms: i64) -> PolledAgent {
    PolledAgent {
        pid: Some(4242),
        pid_start: Some(77),
        session_id: Some(SESSION.to_string()),
        since_ms,
        ..PolledAgent::new(state, detail, NOW_MS)
    }
}

/// A poll at `NOW_MS` that listed `agents`.
fn poll(agents: Vec<PolledAgent>) -> LivePoll {
    LivePoll {
        observed_ms: NOW_MS,
        agents,
    }
}

fn idle() -> LivePoll {
    poll(vec![listed(LiveState::Ready, None, NOW_MS)])
}

fn busy() -> LivePoll {
    poll(vec![listed(
        LiveState::Working,
        Some(LiveDetail::Generating),
        NOW_MS,
    )])
}

#[test]
fn a_silent_hook_working_reads_ready_when_the_poll_says_idle() {
    let repaired = combine(&hook(LiveState::Working, MINUTE), SESSION, &idle()).expect("repaired");
    let observation = repaired.observation;
    assert_eq!(observation.state, LiveState::Ready);
    assert_eq!(observation.detail, None);
    assert_eq!(observation.source, LiveSource::HarnessPoll);
    assert_eq!(observation.observed_ms, NOW_MS);
    assert_eq!(observation.since_ms, NOW_MS);
    // The hook entry's process, its liveness, and its subagents stay.
    assert_eq!(
        (
            observation.pid,
            observation.pid_start,
            observation.subagents
        ),
        (Some(4242), Some(77), 2)
    );
    assert!(!observation.via_subagent);
    assert_eq!(repaired.liveness, Liveness::Alive);
    // A ready prompt does not age while the process lives.
    assert_eq!(
        repaired.effective(NOW_MS + 6 * 60 * MINUTE).state,
        LiveState::Ready
    );
}

#[test]
fn a_silent_hook_wait_reads_working_when_the_poll_says_busy() {
    let repaired = combine(&hook(LiveState::Waiting, MINUTE), SESSION, &busy()).expect("repaired");
    assert_eq!(
        (
            repaired.observation.state,
            repaired.observation.detail,
            repaired.observation.source
        ),
        (
            LiveState::Working,
            Some(LiveDetail::Generating),
            LiveSource::HarnessPoll
        )
    );
}

#[test]
fn a_silent_hook_working_reads_a_wait_the_hooks_did_not_report() {
    let waiting = poll(vec![listed(
        LiveState::Waiting,
        Some(LiveDetail::Permission),
        NOW_MS,
    )]);
    let repaired = combine(&hook(LiveState::Working, MINUTE), SESSION, &waiting).expect("repaired");
    assert_eq!(
        (repaired.observation.state, repaired.observation.detail),
        (LiveState::Waiting, Some(LiveDetail::Permission))
    );
}

#[test]
fn a_hook_event_newer_than_the_poll_or_inside_the_settle_window_wins() {
    for age_ms in [SETTLE_MS - 1, 1, 0, -SECOND, -MINUTE] {
        for state in [LiveState::Working, LiveState::Waiting] {
            for read in [idle(), busy()] {
                assert_eq!(
                    combine(&hook(state, age_ms), SESSION, &read),
                    None,
                    "{state:?} heard {age_ms} ms before the poll"
                );
            }
        }
    }
    // Silent for exactly the settle window is silent enough.
    assert!(combine(&hook(LiveState::Working, SETTLE_MS), SESSION, &idle()).is_some());
}

#[test]
fn a_poll_never_changes_a_hook_state_other_than_working_or_waiting() {
    for state in [
        LiveState::Ready,
        LiveState::Error,
        LiveState::Compacting,
        LiveState::Starting,
        LiveState::Unknown,
        LiveState::Exited,
    ] {
        for read in [idle(), busy()] {
            assert_eq!(
                combine(&hook(state, 10 * MINUTE), SESSION, &read),
                None,
                "{state:?}"
            );
        }
    }
}

#[test]
fn a_poll_that_agrees_changes_nothing() {
    assert_eq!(
        combine(&hook(LiveState::Working, MINUTE), SESSION, &busy()),
        None
    );
    let waiting = poll(vec![listed(
        LiveState::Waiting,
        Some(LiveDetail::Input),
        NOW_MS,
    )]);
    // The hook's own detail stays.
    assert_eq!(
        combine(&hook(LiveState::Waiting, MINUTE), SESSION, &waiting),
        None
    );
}

#[test]
fn only_the_same_process_is_repaired() {
    let stuck = hook(LiveState::Working, MINUTE);
    let other_pid = poll(vec![PolledAgent {
        pid: Some(5151),
        ..listed(LiveState::Ready, None, NOW_MS)
    }]);
    assert_eq!(combine(&stuck, SESSION, &other_pid), None, "pid mismatch");
    let reused_pid = poll(vec![PolledAgent {
        pid_start: Some(78),
        ..listed(LiveState::Ready, None, NOW_MS)
    }]);
    assert_eq!(combine(&stuck, SESSION, &reused_pid), None, "pid reused");
    let unknown_start = poll(vec![PolledAgent {
        pid_start: None,
        ..listed(LiveState::Ready, None, NOW_MS)
    }]);
    assert!(
        combine(&stuck, SESSION, &unknown_start).is_some(),
        "an unread start time does not tell them apart"
    );
    // Two known pids decide, whatever the session ids say: the same pid
    // under another session id is the same process, and another pid under
    // the same session id (`other_pid` above) is not.
    assert!(combine(&stuck, "another-session", &idle()).is_some());

    // Without a pid on one side, the session ids decide.
    let mut no_pid = stuck;
    no_pid.observation.pid = None;
    no_pid.observation.pid_start = None;
    assert!(combine(&no_pid, SESSION, &idle()).is_some());
    assert_eq!(combine(&no_pid, "another-session", &idle()), None);
    let unnamed = poll(vec![PolledAgent {
        session_id: None,
        ..listed(LiveState::Ready, None, NOW_MS)
    }]);
    assert_eq!(combine(&no_pid, SESSION, &unnamed), None);
    let pidless_listing = poll(vec![PolledAgent {
        pid: None,
        pid_start: None,
        ..listed(LiveState::Ready, None, NOW_MS)
    }]);
    assert!(combine(&stuck, SESSION, &pidless_listing).is_some());
    assert_eq!(combine(&stuck, "another-session", &pidless_listing), None);

    // An empty listing names nobody.
    assert_eq!(combine(&stuck, SESSION, &poll(Vec::new())), None);
}

#[test]
fn a_dead_process_or_a_state_another_source_wrote_is_left_alone() {
    let mut dead = hook(LiveState::Working, MINUTE);
    dead.liveness = Liveness::Dead;
    assert_eq!(combine(&dead, SESSION, &idle()), None);
    assert_eq!(dead.effective(NOW_MS).state, LiveState::Exited);

    let mut titled = hook(LiveState::Working, MINUTE);
    titled.observation.source = LiveSource::PaneTitle;
    assert_eq!(combine(&titled, SESSION, &idle()), None);

    // Unchecked liveness is not dead: the repair applies and ages by the
    // fifteen-minute rule.
    let mut unchecked = hook(LiveState::Working, MINUTE);
    unchecked.liveness = Liveness::Unknown;
    let repaired = combine(&unchecked, SESSION, &idle()).expect("repaired");
    assert_eq!(repaired.liveness, Liveness::Unknown);
    assert_eq!(
        repaired.effective(NOW_MS + 16 * MINUTE).confidence,
        LiveConfidence::Stale
    );
}

#[test]
fn the_repair_dates_from_the_first_poll_after_the_hooks_last_event() {
    let stuck = hook(LiveState::Working, 2 * MINUTE);
    // Idle on every poll for the last minute, all after the event.
    let held = poll(vec![listed(LiveState::Ready, None, NOW_MS - MINUTE)]);
    let repaired = combine(&stuck, SESSION, &held).expect("repaired");
    assert_eq!(repaired.observation.since_ms, NOW_MS - MINUTE);
    // A run that began before the event says nothing of the time since.
    let older = poll(vec![listed(LiveState::Ready, None, NOW_MS - 5 * MINUTE)]);
    let repaired = combine(&stuck, SESSION, &older).expect("repaired");
    assert_eq!(repaired.observation.since_ms, NOW_MS);
}

// --- folded into the tables ------------------------------------------

/// A process table that runs 4242, started at 77.
struct Alive;

impl PidProbe for Alive {
    fn status(&self, pid: i64) -> PidStatus {
        if pid == 4242 {
            PidStatus::Running { start: Some(77) }
        } else {
            PidStatus::Gone
        }
    }
}

fn session_id(harness: &str, key: &str) -> AgentSessionId {
    AgentSessionId::new(harness, "/home/me/.claude", key)
}

fn session_node(harness: &str, key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(session_id(harness, key), harness))
}

fn session(harness: &str, key: &str) -> NodeId {
    NodeId::AgentSession(session_id(harness, key))
}

/// A store entry for `harness:key` in `state`, heard from `age_ms`
/// before `NOW_MS`, its process 4242.
fn entry(harness: &str, key: &str, state: LiveState, age_ms: i64) -> LiveEntry {
    let mut entry = LiveEntry::new(harness, key, NOW_MS - age_ms);
    entry.observation =
        LiveObservation::new(state, LiveSource::Hook, NOW_MS - age_ms).with_pid(4242, Some(77));
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

fn graph(nodes: Vec<GraphNode>, polls: &[(&str, LivePoll)]) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        live_polls: polls
            .iter()
            .map(|(harness, poll)| (harness.to_string(), poll.clone()))
            .collect(),
        ..GraphSnapshot::empty()
    }
}

fn state_and_source(binding: Option<&LiveBinding>) -> Option<(LiveState, LiveSource)> {
    binding.map(|binding| (binding.observation.state, binding.observation.source))
}

#[test]
fn the_binding_reads_the_poll_for_a_stuck_session() {
    let snapshot = graph(vec![session_node(CLAUDE, SESSION)], &[(CLAUDE, idle())]);
    let store = file([entry(CLAUDE, SESSION, LiveState::Working, MINUTE)]);
    let bound = bind_live(&snapshot, &store, &Alive);
    assert_eq!(
        state_and_source(bound.nodes.get(&session(CLAUDE, SESSION))),
        Some((LiveState::Ready, LiveSource::HarnessPoll))
    );

    // Without the poll the hook's state stands.
    let bare = graph(vec![session_node(CLAUDE, SESSION)], &[]);
    assert_eq!(
        state_and_source(
            bind_live(&bare, &store, &Alive)
                .nodes
                .get(&session(CLAUDE, SESSION))
        ),
        Some((LiveState::Working, LiveSource::Hook))
    );
}

#[test]
fn a_poll_applies_to_its_own_harness_only() {
    let snapshot = graph(vec![session_node("codex", SESSION)], &[(CLAUDE, idle())]);
    let store = file([entry("codex", SESSION, LiveState::Working, MINUTE)]);
    assert_eq!(
        state_and_source(
            bind_live(&snapshot, &store, &Alive)
                .nodes
                .get(&session("codex", SESSION))
        ),
        Some((LiveState::Working, LiveSource::Hook))
    );
}

#[test]
fn a_session_with_no_hook_entry_and_no_known_process_gets_nothing_from_the_poll() {
    // Only an agent whose process discovery knows can show the poll's
    // state on its own (see `live_poll_only_tests`).
    let snapshot = graph(vec![session_node(CLAUDE, SESSION)], &[(CLAUDE, idle())]);
    let bound = bind_live(&snapshot, &file([]), &Alive);
    assert!(bound.is_empty(), "{bound:?}");
}

#[test]
fn a_pane_entry_is_repaired_in_its_own_slot() {
    let mux = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    let snapshot = graph(vec![GraphNode::MuxSession(mux)], &[(CLAUDE, idle())]);
    let mut stuck = entry(CLAUDE, SESSION, LiveState::Working, MINUTE);
    stuck.tmux = Some(LiveTmux {
        socket: Some("/tmp/tmux-1000/default".to_string()),
        session: Some("work".to_string()),
        pane: Some("%3".to_string()),
    });
    let bound = bind_live(&snapshot, &file([stuck]), &Alive);
    let pane = LivePane::new(MuxSessionId::new("tmux:work"), Some("%3".to_string()));
    assert_eq!(
        state_and_source(bound.panes.get(&pane)),
        Some((LiveState::Ready, LiveSource::HarnessPoll))
    );
    assert!(bound.nodes.is_empty());
}

#[test]
fn a_hook_ready_or_a_fresh_event_holds_in_the_tables() {
    let snapshot = graph(vec![session_node(CLAUDE, SESSION)], &[(CLAUDE, busy())]);
    let ready = file([entry(CLAUDE, SESSION, LiveState::Ready, 10 * MINUTE)]);
    assert_eq!(
        state_and_source(
            bind_live(&snapshot, &ready, &Alive)
                .nodes
                .get(&session(CLAUDE, SESSION))
        ),
        Some((LiveState::Ready, LiveSource::Hook))
    );
    let fresh = file([entry(CLAUDE, SESSION, LiveState::Waiting, SECOND)]);
    assert_eq!(
        state_and_source(
            bind_live(&snapshot, &fresh, &Alive)
                .nodes
                .get(&session(CLAUDE, SESSION))
        ),
        Some((LiveState::Waiting, LiveSource::Hook))
    );
    let newer = file([entry(CLAUDE, SESSION, LiveState::Waiting, -SECOND)]);
    assert_eq!(
        state_and_source(
            bind_live(&snapshot, &newer, &Alive)
                .nodes
                .get(&session(CLAUDE, SESSION))
        ),
        Some((LiveState::Waiting, LiveSource::Hook))
    );
}

#[test]
fn candidates_are_the_harnesss_bound_working_and_waiting_hook_entries() {
    let snapshot = graph(
        vec![
            session_node(CLAUDE, "s-working"),
            session_node(CLAUDE, "s-waiting"),
            session_node(CLAUDE, "s-ready"),
            session_node(CLAUDE, "s-dead"),
            session_node("codex", "s-codex"),
        ],
        &[],
    );
    let mut dead = entry(CLAUDE, "s-dead", LiveState::Working, MINUTE);
    dead.observation.pid = Some(5151);
    let store = file([
        entry(CLAUDE, "s-working", LiveState::Working, MINUTE),
        entry(CLAUDE, "s-waiting", LiveState::Waiting, 2 * MINUTE),
        entry(CLAUDE, "s-ready", LiveState::Ready, MINUTE),
        dead,
        entry("codex", "s-codex", LiveState::Working, MINUTE),
        // Binds nowhere: no node, no tmux.
        entry(CLAUDE, "s-unbound", LiveState::Working, MINUTE),
    ]);
    let found: BTreeMap<String, (LiveState, i64)> = candidates(&snapshot, &store, &Alive, CLAUDE)
        .into_iter()
        .map(|candidate| {
            assert_eq!((candidate.pid, candidate.pid_start), (Some(4242), Some(77)));
            (
                candidate.session_id,
                (candidate.state, candidate.observed_ms),
            )
        })
        .collect();
    assert_eq!(
        found,
        BTreeMap::from([
            (
                "s-waiting".to_string(),
                (LiveState::Waiting, NOW_MS - 2 * MINUTE)
            ),
            (
                "s-working".to_string(),
                (LiveState::Working, NOW_MS - MINUTE)
            ),
        ])
    );
}

/// A runner that prints `listing` and counts its runs.
struct Listing {
    listing: serde_json::Value,
    runs: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl crate::discovery::live_poller::PollRunner for Listing {
    fn run(&self) -> crate::discovery::live_poller::RunOutcome {
        self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        crate::discovery::live_poller::RunOutcome::Output(
            serde_json::to_vec(&self.listing).unwrap(),
        )
    }
}

#[test]
fn the_pass_runs_its_pollers_and_without_them_leaves_no_poll() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::discovery::harness::claude_code::agents;
    use crate::discovery::live_poller::LivePoller;
    use crate::discovery::live_state::apply_live_state;
    use crate::live_store::{LiveEvent, LiveEventKind, LiveStore};

    let root = tempfile::tempdir().expect("tempdir");
    let now = crate::live_store::now_ms();
    let started = LiveEvent::new(CLAUDE, SESSION, now - MINUTE, LiveEventKind::TurnStarted)
        .with_context(crate::live_store::LiveContext {
            pid: Some(4242),
            pid_start: Some(77),
            ..Default::default()
        });
    LiveStore::at(root.path())
        .apply_event(&started)
        .expect("write the store");
    let runs = Arc::new(AtomicUsize::new(0));
    let poller = Arc::new(
        LivePoller::new(
            CLAUDE,
            Box::new(Listing {
                listing: serde_json::json!([{"pid": 4242, "status": "idle"}]),
                runs: Arc::clone(&runs),
            }),
            agents::parse,
        )
        .with_clock(move || now),
    );

    let mut snapshot = graph(vec![session_node(CLAUDE, SESSION)], &[]);
    apply_live_state(&mut snapshot, root.path(), &Alive, &[Arc::clone(&poller)]);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(snapshot.live_polls[CLAUDE].observed_ms, now);
    assert_eq!(
        state_and_source(snapshot.node_live.get(&session(CLAUDE, SESSION))),
        Some((LiveState::Ready, LiveSource::HarnessPoll))
    );

    // The next pass within the interval runs nothing and keeps the poll.
    apply_live_state(&mut snapshot, root.path(), &Alive, &[Arc::clone(&poller)]);
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    assert_eq!(
        state_and_source(snapshot.node_live.get(&session(CLAUDE, SESSION))),
        Some((LiveState::Ready, LiveSource::HarnessPoll))
    );

    // A pass with no poller, as a one-shot command runs, leaves none.
    apply_live_state(&mut snapshot, root.path(), &Alive, &[]);
    assert!(snapshot.live_polls.is_empty());
    assert_eq!(
        state_and_source(snapshot.node_live.get(&session(CLAUDE, SESSION))),
        Some((LiveState::Working, LiveSource::Hook))
    );
}
