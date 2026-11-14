//! A poll's word on an agent with no hook entry (ADR 0222): who gets it,
//! where it binds, and how it ages once polls stop. Every listing here is
//! synthetic.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::*;
use crate::discovery::harness::claude_code::agents;
use crate::discovery::live_poller::{LivePoller, PollRunner, RunOutcome};
use crate::discovery::live_state::{apply_live_state, bind_live};
use crate::model::live::{LiveConfidence, LiveDetail, PolledAgent};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, LinkEndpoint, Provenance, RuntimeProcessId,
    RuntimeProcessNode,
};

const NOW_MS: i64 = 1_794_546_100_000;
const SECOND: i64 = 1_000;
const MINUTE: i64 = 60 * SECOND;
const CLAUDE: &str = "claude-code";
const SESSION: &str = "8c1d2f0e-5b7a-4c39-9e21-3a6f0b9d4e17";

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

/// A process table with no process in it.
struct Gone;

impl PidProbe for Gone {
    fn status(&self, _pid: i64) -> PidStatus {
        PidStatus::Gone
    }
}

/// A process table with 4242 running, started at the time it holds.
struct StartedAt(u64);

impl PidProbe for StartedAt {
    fn status(&self, pid: i64) -> PidStatus {
        if pid == 4242 {
            PidStatus::Running {
                start: Some(self.0),
            }
        } else {
            PidStatus::Gone
        }
    }
}

fn session_id(key: &str) -> AgentSessionId {
    AgentSessionId::new(CLAUDE, "/home/me/.claude", key)
}

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(session_id(key))
}

fn session_node(key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(session_id(key), CLAUDE))
}

/// Claude Code's main agent process 4242, as the process tree records it.
fn agent_process(role: RuntimeProcessRole) -> GraphNode {
    GraphNode::RuntimeProcess(RuntimeProcessNode {
        id: RuntimeProcessId::new("proc:4242"),
        observation_key: "proc:4242".to_string(),
        pid: Some(4242),
        parent_pid: None,
        root_pane_pid: None,
        command: None,
        cwd: None,
        harness_key: Some(CLAUDE.to_string()),
        role: Some(role),
        depth: None,
        observed_epoch: None,
    })
}

/// The process tree's word that process 4242 runs session `key`.
fn runs_session(key: &str) -> GraphLink {
    GraphLink::new(
        format!("test:proc:4242:{key}"),
        NodeId::RuntimeProcess(RuntimeProcessId::new("proc:4242")),
        LinkEndpoint::Node { id: session(key) },
        RelationKind::ProcessIdentifiesSession,
        Provenance::Discovered,
    )
}

/// Process 4242 (started at 77), session `key`, listed in `state`.
fn listed(state: LiveState, detail: Option<LiveDetail>, key: &str) -> PolledAgent {
    PolledAgent {
        pid: Some(4242),
        pid_start: Some(77),
        session_id: Some(key.to_string()),
        since_ms: NOW_MS - MINUTE,
        ..PolledAgent::new(state, detail, NOW_MS)
    }
}

/// A graph of `nodes` and `links` carrying a Claude Code poll at
/// `NOW_MS` that listed `agents`.
fn graph(nodes: Vec<GraphNode>, links: Vec<GraphLink>, agents: Vec<PolledAgent>) -> GraphSnapshot {
    GraphSnapshot {
        nodes,
        candidate_links: links,
        live_polls: BTreeMap::from([(
            CLAUDE.to_string(),
            LivePoll {
                observed_ms: NOW_MS,
                agents,
            },
        )]),
        ..GraphSnapshot::empty()
    }
}

/// The usual world: session `SESSION`, process 4242 running it, and a
/// poll that listed `agents`.
fn known(agents: Vec<PolledAgent>) -> GraphSnapshot {
    graph(
        vec![
            session_node(SESSION),
            agent_process(RuntimeProcessRole::HumanAgent),
        ],
        vec![runs_session(SESSION)],
        agents,
    )
}

fn waiting() -> PolledAgent {
    listed(LiveState::Waiting, Some(LiveDetail::Permission), SESSION)
}

fn read(binding: Option<&LiveBinding>) -> Option<(LiveState, Option<LiveDetail>, LiveSource)> {
    binding.map(|binding| {
        (
            binding.observation.state,
            binding.observation.detail,
            binding.observation.source,
        )
    })
}

#[test]
fn a_known_agent_with_no_hook_entry_shows_the_polls_state_on_its_session() {
    let bound = bind_live(&known(vec![waiting()]), &LiveFile::default(), &Alive);
    let binding = bound.nodes.get(&session(SESSION)).expect("bound");
    assert_eq!(
        read(Some(binding)),
        Some((
            LiveState::Waiting,
            Some(LiveDetail::Permission),
            LiveSource::HarnessPoll
        ))
    );
    let observation = binding.observation;
    assert_eq!(observation.observed_ms, NOW_MS);
    assert_eq!(observation.since_ms, NOW_MS - MINUTE);
    // No pid is recorded, so the read rule treats its process as
    // unchecked and ages it once polls stop.
    assert_eq!((observation.pid, observation.pid_start), (None, None));
    assert_eq!(binding.liveness, Liveness::Unknown);
    assert!(bound.panes.is_empty(), "a poll binds no pane");
}

#[test]
fn the_session_comes_from_the_poll_when_the_graph_names_none() {
    let snapshot = graph(
        vec![
            session_node(SESSION),
            agent_process(RuntimeProcessRole::HumanAgent),
        ],
        Vec::new(),
        vec![waiting()],
    );
    let bound = bind_live(&snapshot, &LiveFile::default(), &Alive);
    assert_eq!(
        read(bound.nodes.get(&session(SESSION))).map(|read| read.2),
        Some(LiveSource::HarnessPoll)
    );

    // A session the graph does not have takes nothing, and nothing else
    // does either.
    let snapshot = graph(
        vec![agent_process(RuntimeProcessRole::HumanAgent)],
        Vec::new(),
        vec![waiting()],
    );
    assert!(bind_live(&snapshot, &LiveFile::default(), &Alive).is_empty());
}

#[test]
fn an_agent_discovery_does_not_know_as_a_running_main_agent_gets_nothing() {
    // No process: the agent would show only because another one made the
    // poller run.
    let snapshot = graph(vec![session_node(SESSION)], Vec::new(), vec![waiting()]);
    assert!(bind_live(&snapshot, &LiveFile::default(), &Alive).is_empty());

    // A background helper or a subagent is no main agent.
    for role in [RuntimeProcessRole::Background, RuntimeProcessRole::Subagent] {
        let snapshot = graph(
            vec![session_node(SESSION), agent_process(role)],
            vec![runs_session(SESSION)],
            vec![waiting()],
        );
        assert!(bind_live(&snapshot, &LiveFile::default(), &Alive).is_empty());
    }

    // A listed session with no pid cannot be tied to the process.
    let mut no_pid = waiting();
    no_pid.pid = None;
    assert!(bind_live(&known(vec![no_pid]), &LiveFile::default(), &Alive).is_empty());
}

#[test]
fn an_agent_whose_hooks_report_keeps_the_hook_rules() {
    // An entry for its session: the hook's ready stands.
    let mut ready = LiveEntry::new(CLAUDE, SESSION, NOW_MS - MINUTE);
    ready.observation = LiveObservation::new(LiveState::Ready, LiveSource::Hook, NOW_MS - MINUTE);
    ready.rev = 1;
    let mut file = LiveFile::default();
    file.entries.insert(ready.key(), ready);
    let bound = bind_live(&known(vec![waiting()]), &file, &Alive);
    assert_eq!(
        read(bound.nodes.get(&session(SESSION))),
        Some((LiveState::Ready, None, LiveSource::Hook))
    );

    // An entry that recorded its pid under another session id, as before
    // a `/clear`, still says its hooks report: the poll adds nothing.
    let mut other = LiveEntry::new(CLAUDE, "other-session", NOW_MS - MINUTE);
    other.observation = LiveObservation::new(LiveState::Ready, LiveSource::Hook, NOW_MS - MINUTE)
        .with_pid(4242, Some(77));
    let mut file = LiveFile::default();
    file.entries.insert(other.key(), other);
    assert!(
        bind_live(&known(vec![waiting()]), &file, &Alive)
            .nodes
            .is_empty()
    );
}

#[test]
fn it_is_gone_when_its_process_ends_or_the_poll_no_longer_lists_it() {
    // The process ended.
    assert!(bind_live(&known(vec![waiting()]), &LiveFile::default(), &Gone).is_empty());

    // Its pid now belongs to another process than the one the poll saw.
    let mut seen = waiting();
    seen.pid_start = Some(99);
    assert!(bind_live(&known(vec![seen]), &LiveFile::default(), &Alive).is_empty());

    // A newer poll that lists only another process.
    let mut someone_else = waiting();
    someone_else.pid = Some(5151);
    assert!(bind_live(&known(vec![someone_else]), &LiveFile::default(), &Alive).is_empty());
}

#[test]
fn unreported_agents_are_the_running_main_agents_no_entry_reports() {
    let snapshot = known(Vec::new());
    assert_eq!(
        unreported(&snapshot, &LiveFile::default(), &Alive, CLAUDE)
            .iter()
            .map(|agent| agent.pid)
            .collect::<Vec<_>>(),
        [4242]
    );
    // Another harness's agents are not this harness's.
    assert!(unreported(&snapshot, &LiveFile::default(), &Alive, "codex").is_empty());
    // An entry of its session reports it.
    let mut file = LiveFile::default();
    let entry = LiveEntry::new(CLAUDE, SESSION, NOW_MS);
    file.entries.insert(entry.key(), entry);
    assert!(unreported(&snapshot, &file, &Alive, CLAUDE).is_empty());
    // A process that is not running is not an agent.
    assert!(unreported(&snapshot, &LiveFile::default(), &Gone, CLAUDE).is_empty());
}

#[test]
fn an_entry_naming_the_agents_pane_reports_it_whatever_pid_it_recorded() {
    use crate::model::{MuxProbe, MuxProbeStatus, MuxSessionId, MuxSessionNode, ProbedPane};
    // Process 4242 was found under the active pane %1 of tmux session
    // `editor`, tied to no session yet (a new one with no transcript).
    let mux = MuxSessionId::new("tmux:editor");
    let mut contains = GraphLink::new(
        "test:editor:contains:4242",
        NodeId::MuxSession(mux.clone()),
        LinkEndpoint::Node {
            id: NodeId::RuntimeProcess(RuntimeProcessId::new("proc:4242")),
        },
        RelationKind::MuxContainsProcess,
        Provenance::StrongDiscovered,
    );
    contains.source_metadata.evidence = Some("active_pane_process_observation".to_string());
    let mut snapshot = graph(
        vec![
            GraphNode::MuxSession(MuxSessionNode::new(mux, "tmux", "editor")),
            agent_process(RuntimeProcessRole::HumanAgent),
        ],
        vec![contains],
        Vec::new(),
    );
    let mut probe = MuxProbe::new(MuxProbeStatus::Listed, NOW_MS / SECOND);
    probe.panes.insert(
        "%1".to_string(),
        ProbedPane {
            session: "editor".to_string(),
            command: Some("claude".to_string()),
            active: true,
        },
    );
    snapshot.mux_probes.insert("tmux".to_string(), probe);
    assert_eq!(
        unreported(&snapshot, &LiveFile::default(), &Alive, CLAUDE).len(),
        1
    );

    // The hook recorded no pid it could check, and names the pane.
    let mut entry = LiveEntry::new(CLAUDE, SESSION, NOW_MS);
    entry.tmux = Some(crate::live_store::LiveTmux {
        socket: Some("/tmp/tmux-1000/default".to_string()),
        session: Some("editor".to_string()),
        pane: Some("%1".to_string()),
    });
    let mut file = LiveFile::default();
    file.entries.insert(entry.key(), entry.clone());
    assert!(unreported(&snapshot, &file, &Alive, CLAUDE).is_empty());

    // An entry for pane %2 is another agent's.
    entry.tmux.as_mut().expect("tmux").pane = Some("%2".to_string());
    let mut file = LiveFile::default();
    file.entries.insert(entry.key(), entry);
    assert_eq!(unreported(&snapshot, &file, &Alive, CLAUDE).len(), 1);
}

// --- with a poller, a scripted runner, and a clock --------------------

/// A runner that answers with its outcomes in turn, then fails.
#[derive(Clone, Default)]
struct Script {
    outcomes: Arc<Mutex<VecDeque<RunOutcome>>>,
    runs: Arc<AtomicUsize>,
}

impl Script {
    fn listing(self, json: serde_json::Value) -> Self {
        self.outcomes
            .lock()
            .unwrap()
            .push_back(RunOutcome::Output(serde_json::to_vec(&json).unwrap()));
        self
    }

    fn runs(&self) -> usize {
        self.runs.load(Ordering::SeqCst)
    }
}

impl PollRunner for Script {
    fn run(&self) -> RunOutcome {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(RunOutcome::Failed)
    }
}

/// `claude agents --json` listing process 4242's session in `status`.
fn agent_list(status: &str) -> serde_json::Value {
    serde_json::json!([{ "pid": 4242, "sessionId": SESSION, "status": status }])
}

/// A pass world: the known agent, no hook entry (no store at all), and
/// a Claude Code poller on `clock` running `script`.
struct PassWorld {
    root: tempfile::TempDir,
    snapshot: GraphSnapshot,
    poller: Arc<LivePoller>,
    clock: Arc<AtomicI64>,
}

impl PassWorld {
    fn new(script: &Script) -> Self {
        let clock = Arc::new(AtomicI64::new(NOW_MS));
        let read = Arc::clone(&clock);
        let poller = Arc::new(
            LivePoller::new(CLAUDE, Box::new(script.clone()), agents::parse)
                .with_clock(move || read.load(Ordering::SeqCst)),
        );
        let mut snapshot = known(Vec::new());
        snapshot.live_polls.clear();
        Self {
            root: tempfile::tempdir().expect("tempdir"),
            snapshot,
            poller,
            clock,
        }
    }

    /// One live-state pass `after_ms` past `NOW_MS`, with the poller.
    fn pass(&mut self, after_ms: i64) -> Option<LiveBinding> {
        self.clock.store(NOW_MS + after_ms, Ordering::SeqCst);
        apply_live_state(
            &mut self.snapshot,
            self.root.path(),
            &StartedAt(0),
            &[Arc::clone(&self.poller)],
        );
        self.snapshot.node_live.get(&session(SESSION)).copied()
    }
}

#[test]
fn a_known_agent_with_no_hook_entry_is_polled_every_interval() {
    let script = Script::default()
        .listing(agent_list("waiting"))
        .listing(agent_list("busy"));
    let mut world = PassWorld::new(&script);

    let first = world.pass(0).expect("bound");
    assert_eq!(script.runs(), 1);
    assert_eq!(
        (first.observation.state, first.observation.source),
        (LiveState::Waiting, LiveSource::HarnessPoll)
    );
    // Within the interval the last poll serves.
    world.pass(10 * SECOND);
    assert_eq!(script.runs(), 1);
    // Past it the agent is polled again, and its state follows.
    let second = world.pass(30 * SECOND).expect("bound");
    assert_eq!(script.runs(), 2);
    assert_eq!(second.observation.state, LiveState::Working);
    assert_eq!(second.observation.observed_ms, NOW_MS + 30 * SECOND);

    // Once its process is gone it shows nothing, and no poll runs for it.
    world
        .snapshot
        .nodes
        .retain(|node| !matches!(node, GraphNode::RuntimeProcess(_)));
    assert_eq!(world.pass(70 * SECOND), None);
    assert_eq!(script.runs(), 2);
}

#[test]
fn a_poll_only_state_ages_out_once_polls_stop() {
    // One poll reads `idle`; every later one fails, as when `claude` is
    // gone or times out.
    let script = Script::default().listing(agent_list("idle"));
    let mut world = PassWorld::new(&script);
    let ready = world.pass(0).expect("bound");
    assert_eq!(ready.effective(NOW_MS).state, LiveState::Ready);

    let later = world.pass(5 * MINUTE).expect("the last poll still serves");
    assert!(script.runs() > 1, "the failing polls were tried");
    assert_eq!(later.observation.observed_ms, NOW_MS);
    assert_eq!(
        later.effective(NOW_MS + 5 * MINUTE).confidence,
        LiveConfidence::Fresh
    );
    // A ready prompt from the poll alone goes stale fifteen minutes after
    // the last poll that listed it.
    let stale = world.pass(16 * MINUTE).expect("bound");
    assert_eq!(
        stale.effective(NOW_MS + 16 * MINUTE),
        crate::model::live::EffectiveLive::STALE
    );

    // Work read once goes quiet after ten minutes, then stale.
    let script = Script::default().listing(agent_list("busy"));
    let mut world = PassWorld::new(&script);
    world.pass(0).expect("bound");
    let working = world.pass(11 * MINUTE).expect("bound");
    let read = working.effective(NOW_MS + 11 * MINUTE);
    assert_eq!(
        (read.state, read.confidence),
        (LiveState::Working, LiveConfidence::Quiet)
    );
    assert_eq!(
        working.effective(NOW_MS + 16 * MINUTE),
        crate::model::live::EffectiveLive::STALE
    );

    // A pass with no poller at all, as after a restart with the switch
    // set, shows nothing.
    apply_live_state(&mut world.snapshot, world.root.path(), &StartedAt(0), &[]);
    assert!(world.snapshot.node_live.is_empty());
}

#[test]
fn a_poll_only_state_keeps_nothing_of_the_listings_text() {
    const SENTINEL: &str = "sentinel-agents-poll-only-3b7e10";
    let script = Script::default().listing(serde_json::json!([{
        "pid": 4242,
        "sessionId": SESSION,
        "name": format!("{SENTINEL} fix the login bug"),
        "cwd": format!("/home/user/{SENTINEL}"),
        "kind": "interactive",
        "status": "waiting",
        "waitingFor": format!("permission prompt {SENTINEL}"),
        "unknownField": SENTINEL,
    }]));
    let mut world = PassWorld::new(&script);
    let binding = world.pass(0).expect("bound");
    assert_eq!(binding.observation.source, LiveSource::HarnessPoll);

    let json = serde_json::to_string(&world.snapshot).unwrap();
    assert!(!json.contains(SENTINEL), "JSON");
    assert!(!format!("{:?}", world.snapshot).contains(SENTINEL), "Debug");
    let bytes = crate::snapshot::serialize_to_bytes(&world.snapshot).unwrap();
    assert!(
        !bytes
            .windows(SENTINEL.len())
            .any(|window| window == SENTINEL.as_bytes()),
        "graph.bin bytes"
    );
    // Nothing was written: there is still no store.
    assert!(
        std::fs::read_dir(world.root.path())
            .unwrap()
            .next()
            .is_none()
    );
}
