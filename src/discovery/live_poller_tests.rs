use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, Weak};

use super::*;
use crate::discovery::harness::claude_code::agents;
use crate::model::live::PolledAgent;

const NOW_MS: i64 = 1_794_546_100_000;
const SECOND: i64 = 1_000;

/// A runner that answers from a script and counts its runs; past the
/// script it prints an empty list.
#[derive(Clone, Default)]
struct Script {
    outcomes: Arc<Mutex<VecDeque<RunOutcome>>>,
    runs: Arc<AtomicUsize>,
}

impl Script {
    fn then(self, outcome: RunOutcome) -> Self {
        self.outcomes.lock().unwrap().push_back(outcome);
        self
    }

    fn listing(self, json: serde_json::Value) -> Self {
        self.then(RunOutcome::Output(serde_json::to_vec(&json).unwrap()))
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
            .unwrap_or_else(|| RunOutcome::Output(b"[]".to_vec()))
    }
}

/// A clock the test moves.
#[derive(Clone)]
struct Clock(Arc<AtomicI64>);

impl Clock {
    fn at(ms: i64) -> Self {
        Self(Arc::new(AtomicI64::new(ms)))
    }

    fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}

/// A process table where every pid runs, started at `pid * 10`.
struct EveryPid;

impl PidProbe for EveryPid {
    fn status(&self, pid: i64) -> PidStatus {
        PidStatus::Running {
            start: u64::try_from(pid * 10).ok(),
        }
    }
}

fn poller(script: &Script, clock: &Clock) -> LivePoller {
    let clock = clock.clone();
    LivePoller::new("claude-code", Box::new(script.clone()), agents::parse)
        .with_clock(move || clock.0.load(Ordering::SeqCst))
}

/// A Claude Code hook entry of process 4242 (started at 42420) in
/// `state`, last heard of at `observed_ms`.
fn candidate(state: LiveState, observed_ms: i64) -> PollCandidate {
    PollCandidate {
        session_id: "s-1".to_string(),
        pid: Some(4242),
        pid_start: Some(42_420),
        state,
        observed_ms,
    }
}

fn listing(status: &str) -> serde_json::Value {
    serde_json::json!([{"pid": 4242, "sessionId": "s-1", "status": status}])
}

fn read(poll: Option<&LivePoll>) -> Option<Vec<(Option<i64>, LiveState, i64)>> {
    poll.map(|poll| {
        poll.agents
            .iter()
            .map(|agent| (agent.pid, agent.state, agent.since_ms))
            .collect()
    })
}

// --- when a poll runs -------------------------------------------------

#[test]
fn nothing_runs_without_a_candidate_or_before_the_settle_window() {
    let script = Script::default();
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    assert_eq!(poller.poll(&[], 0, &EveryPid), None);
    let just_heard = candidate(LiveState::Working, NOW_MS - SETTLE_MS + 1);
    assert_eq!(poller.poll(&[just_heard], 0, &EveryPid), None);
    assert_eq!(script.runs(), 0);
}

#[test]
fn a_candidate_silent_for_the_settle_window_runs_a_poll() {
    let script = Script::default().listing(listing("idle"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let stuck = candidate(LiveState::Working, NOW_MS - SETTLE_MS);
    let poll = poller.poll(&[stuck], 0, &EveryPid).expect("polled");
    assert_eq!(script.runs(), 1);
    assert_eq!(poll.observed_ms, NOW_MS);
    assert_eq!(
        read(Some(&poll)),
        Some(vec![(Some(4242), LiveState::Ready, NOW_MS)])
    );
    // The listed pid's start time is read when the poll runs.
    assert_eq!(poll.agents[0].pid_start, Some(42_420));
}

#[test]
fn at_most_one_attempt_runs_per_interval_whatever_it_came_to() {
    let script = Script::default()
        .then(RunOutcome::Failed)
        .listing(listing("busy"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let stuck = [candidate(LiveState::Working, NOW_MS - 60 * SECOND)];

    // A failed attempt asserts nothing, and still counts.
    assert_eq!(poller.poll(&stuck, 0, &EveryPid), None);
    clock.set(NOW_MS + POLL_INTERVAL_MS - 1);
    assert_eq!(poller.poll(&stuck, 0, &EveryPid), None);
    assert_eq!(script.runs(), 1);

    clock.set(NOW_MS + POLL_INTERVAL_MS);
    let poll = poller.poll(&stuck, 0, &EveryPid).expect("polled");
    assert_eq!(script.runs(), 2);
    assert_eq!(poll.observed_ms, NOW_MS + POLL_INTERVAL_MS);
}

#[test]
fn a_failed_poll_keeps_the_last_one_that_succeeded() {
    for failure in [
        RunOutcome::Unavailable,
        RunOutcome::Failed,
        RunOutcome::TimedOut,
        RunOutcome::Output(b"not json".to_vec()),
        RunOutcome::Output(br#"{"sessions": []}"#.to_vec()),
    ] {
        let script = Script::default()
            .listing(listing("busy"))
            .then(failure.clone());
        let clock = Clock::at(NOW_MS);
        let poller = poller(&script, &clock);
        // Busy agrees with working, so the entry stays due.
        let stuck = [candidate(LiveState::Working, NOW_MS - 60 * SECOND)];
        let first = poller.poll(&stuck, 0, &EveryPid).expect("polled");
        clock.set(NOW_MS + POLL_INTERVAL_MS);
        let after = poller.poll(&stuck, 0, &EveryPid);
        assert_eq!(script.runs(), 2, "{failure:?}");
        assert_eq!(after.as_ref(), Some(&first), "{failure:?}");
    }
}

#[test]
fn a_poll_that_reads_ready_answers_the_entry_until_its_next_event() {
    let script = Script::default().listing(listing("idle"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let stuck = candidate(LiveState::Working, NOW_MS - 60 * SECOND);
    poller.poll(std::slice::from_ref(&stuck), 0, &EveryPid);
    assert_eq!(script.runs(), 1);

    // An interrupted agent left idle costs one poll.
    for later in [1, 10, 100] {
        clock.set(NOW_MS + later * POLL_INTERVAL_MS);
        poller.poll(std::slice::from_ref(&stuck), 0, &EveryPid);
    }
    assert_eq!(script.runs(), 1);

    // Its next event, once silent again, makes it due.
    let heard_again = candidate(LiveState::Working, NOW_MS + 200 * POLL_INTERVAL_MS);
    clock.set(heard_again.observed_ms + SETTLE_MS);
    poller.poll(&[heard_again], 0, &EveryPid);
    assert_eq!(script.runs(), 2);
}

#[test]
fn an_agent_the_poll_does_not_list_is_answered_too() {
    let script = Script::default().listing(serde_json::json!([]));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let stuck = [candidate(LiveState::Waiting, NOW_MS - 60 * SECOND)];
    poller.poll(&stuck, 0, &EveryPid);
    clock.set(NOW_MS + 10 * POLL_INTERVAL_MS);
    poller.poll(&stuck, 0, &EveryPid);
    assert_eq!(script.runs(), 1);
}

#[test]
fn a_poll_that_reads_work_or_a_wait_leaves_the_entry_due() {
    for (state, status) in [
        (LiveState::Working, "busy"),
        (LiveState::Waiting, "waiting"),
        (LiveState::Waiting, "busy"),
        (LiveState::Working, "waiting"),
    ] {
        let script = Script::default()
            .listing(listing(status))
            .listing(listing(status));
        let clock = Clock::at(NOW_MS);
        let poller = poller(&script, &clock);
        let stuck = [candidate(state, NOW_MS - 60 * SECOND)];
        poller.poll(&stuck, 0, &EveryPid);
        clock.set(NOW_MS + POLL_INTERVAL_MS);
        poller.poll(&stuck, 0, &EveryPid);
        assert_eq!(script.runs(), 2, "{state:?} read {status}");
    }
}

#[test]
fn a_poll_too_soon_after_an_entrys_event_does_not_answer_it() {
    let script = Script::default()
        .listing(serde_json::json!([
            {"pid": 4242, "status": "idle"},
            {"pid": 5151, "status": "idle"}
        ]))
        .listing(listing("idle"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let due = candidate(LiveState::Working, NOW_MS - 60 * SECOND);
    let fresh = PollCandidate {
        pid: Some(5151),
        pid_start: Some(51_510),
        session_id: "s-2".to_string(),
        ..candidate(LiveState::Working, NOW_MS - 5 * SECOND)
    };
    poller.poll(&[due.clone(), fresh.clone()], 0, &EveryPid);
    assert_eq!(script.runs(), 1);
    // The first poll answered `due`; it ran too soon after `fresh`'s
    // event for the binding to use, so `fresh` is still due.
    clock.set(NOW_MS + POLL_INTERVAL_MS);
    poller.poll(&[due, fresh], 0, &EveryPid);
    assert_eq!(script.runs(), 2);
}

#[test]
fn a_run_of_one_state_keeps_the_time_it_began() {
    let script = Script::default()
        .listing(listing("busy"))
        .listing(listing("busy"))
        .listing(listing("waiting"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    let stuck = [candidate(LiveState::Working, NOW_MS - 60 * SECOND)];
    poller.poll(&stuck, 0, &EveryPid);
    clock.set(NOW_MS + POLL_INTERVAL_MS);
    let second = poller.poll(&stuck, 0, &EveryPid);
    assert_eq!(
        read(second.as_ref()),
        Some(vec![(Some(4242), LiveState::Working, NOW_MS)])
    );
    clock.set(NOW_MS + 2 * POLL_INTERVAL_MS);
    let third = poller.poll(&stuck, 0, &EveryPid);
    assert_eq!(
        read(third.as_ref()),
        Some(vec![(
            Some(4242),
            LiveState::Waiting,
            NOW_MS + 2 * POLL_INTERVAL_MS
        )])
    );
}

#[test]
fn a_clock_that_went_back_does_not_stop_the_polls() {
    // Busy agrees with working, so the entry stays due.
    let script = Script::default()
        .listing(listing("busy"))
        .listing(listing("busy"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    poller.poll(
        &[candidate(LiveState::Working, NOW_MS - 60 * SECOND)],
        0,
        &EveryPid,
    );
    clock.set(NOW_MS - 3_600 * SECOND);
    poller.poll(
        &[candidate(LiveState::Working, NOW_MS - 3_700 * SECOND)],
        0,
        &EveryPid,
    );
    assert_eq!(script.runs(), 2);
}

/// A runner that, while it runs, has another discovery ask the same
/// poller for a poll, as a second daemon class cycle would.
struct Reentrant {
    poller: OnceLock<Weak<LivePoller>>,
    inner: Mutex<Option<Option<LivePoll>>>,
    runs: AtomicUsize,
}

impl PollRunner for Arc<Reentrant> {
    fn run(&self) -> RunOutcome {
        self.runs.fetch_add(1, Ordering::SeqCst);
        if let Some(poller) = self.poller.get().and_then(Weak::upgrade) {
            let stuck = [candidate(LiveState::Working, NOW_MS - 60 * SECOND)];
            *self.inner.lock().unwrap() = Some(poller.poll(&stuck, 0, &EveryPid));
        }
        RunOutcome::Output(serde_json::to_vec(&listing("idle")).unwrap())
    }
}

#[test]
fn a_discovery_beside_a_running_poll_does_not_start_another() {
    let runner = Arc::new(Reentrant {
        poller: OnceLock::new(),
        inner: Mutex::new(None),
        runs: AtomicUsize::new(0),
    });
    let poller = Arc::new(
        LivePoller::new("claude-code", Box::new(Arc::clone(&runner)), agents::parse)
            .with_clock(|| NOW_MS),
    );
    runner.poller.set(Arc::downgrade(&poller)).unwrap();
    let stuck = [candidate(LiveState::Working, NOW_MS - 60 * SECOND)];
    let outer = poller.poll(&stuck, 0, &EveryPid);
    assert_eq!(runner.runs.load(Ordering::SeqCst), 1);
    // The discovery beside it saw the attempt claimed and got the last
    // poll, of which there was none yet.
    assert_eq!(*runner.inner.lock().unwrap(), Some(None));
    assert!(outer.is_some());
}

// --- answered and due -------------------------------------------------

#[test]
fn answered_needs_the_settle_window_and_an_answer_only_a_hook_event_ends() {
    let entry = candidate(LiveState::Working, NOW_MS - SETTLE_MS);
    let poll = |state: LiveState| LivePoll {
        observed_ms: NOW_MS,
        agents: vec![PolledAgent {
            pid: Some(4242),
            pid_start: Some(42_420),
            ..PolledAgent::new(state, None, NOW_MS)
        }],
    };
    for (state, settled) in [
        (LiveState::Ready, true),
        (LiveState::Error, true),
        (LiveState::Exited, true),
        (LiveState::Working, false),
        (LiveState::Waiting, false),
    ] {
        assert_eq!(answered(&entry, &poll(state)), settled, "{state:?}");
    }
    let early = candidate(LiveState::Working, NOW_MS - SETTLE_MS + 1);
    assert!(!answered(&early, &poll(LiveState::Ready)));
    let unlisted = LivePoll {
        observed_ms: NOW_MS,
        agents: Vec::new(),
    };
    assert!(answered(&entry, &unlisted));

    assert!(due(&entry, None, NOW_MS));
    assert!(!due(&entry, Some(&poll(LiveState::Ready)), NOW_MS));
    assert!(due(&entry, Some(&poll(LiveState::Working)), NOW_MS));
    assert!(!due(&early, None, NOW_MS));
}

// --- the program ------------------------------------------------------

/// A directory with a stand-in `claude` that runs `body`.
struct FakeClaude {
    dir: tempfile::TempDir,
}

impl FakeClaude {
    fn new(body: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the stand-in");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("claude")
    }

    /// Run it with `timeout`. A freshly written script can fail to start
    /// while another test's child still holds it open for writing, so a
    /// start-up failure is retried when `expected` is something else.
    fn run(&self, timeout: Duration, expected: fn(&RunOutcome) -> bool) -> (RunOutcome, Duration) {
        let runner = agents::AgentList::at(self.path()).with_timeout(timeout);
        let mut tries = 0;
        loop {
            let start = Instant::now();
            let outcome = runner.run();
            tries += 1;
            if expected(&outcome) || tries == 5 || outcome != RunOutcome::Failed {
                return (outcome, start.elapsed());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[test]
fn the_program_s_output_is_read_and_its_arguments_are_agents_json() {
    let fake = FakeClaude::new(
        r#"[ "$1 $2" = "agents --json" ] || exit 9
[ "$CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC" = 1 ] || exit 8
echo '[{"pid": 4242, "status": "idle"}]'"#,
    );
    let (outcome, _) = fake.run(Duration::from_secs(10), |outcome| {
        matches!(outcome, RunOutcome::Output(_))
    });
    let RunOutcome::Output(bytes) = outcome else {
        panic!("ran: {outcome:?}");
    };
    let poll = agents::parse(&bytes, NOW_MS).expect("parses");
    assert_eq!(
        read(Some(&poll)),
        Some(vec![(Some(4242), LiveState::Ready, NOW_MS)])
    );
}

#[test]
fn a_hung_program_is_killed_at_its_timeout() {
    let fake = FakeClaude::new(r#"echo $$ > "$(dirname "$0")/pid"; exec sleep 30"#);
    let (outcome, took) = fake.run(Duration::from_millis(300), |outcome| {
        *outcome == RunOutcome::TimedOut
    });
    assert_eq!(outcome, RunOutcome::TimedOut);
    assert!(took < Duration::from_secs(5), "took {took:?}");
    let pid: i64 = std::fs::read_to_string(fake.dir.path().join("pid"))
        .expect("the stand-in wrote its pid")
        .trim()
        .parse()
        .expect("a pid");
    assert_eq!(
        crate::live_store::ProcProbe::system().status(pid),
        PidStatus::Gone,
        "the hung child was killed and reaped"
    );
}

#[test]
fn a_failing_program_says_nothing_of_what_it_printed() {
    let fake = FakeClaude::new("echo 'sentinel-fail-91b2 on stderr' >&2; echo '[]'; exit 3");
    let (outcome, _) = fake.run(Duration::from_secs(10), |_| false);
    assert_eq!(outcome, RunOutcome::Failed);
    assert!(!format!("{outcome:?}").contains("sentinel-fail-91b2"));
}

#[test]
fn output_past_the_cap_fails_without_waiting_for_the_timeout() {
    let fake = FakeClaude::new("head -c 200000 /dev/zero");
    let runner_cap = 1024;
    let mut command = crate::exec::command(crate::exec::Program::Harness, fake.path());
    command.args(["agents", "--json"]);
    let start = Instant::now();
    let outcome = run_bounded(command, Duration::from_secs(10), runner_cap);
    assert_eq!(outcome, RunOutcome::Failed);
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_missing_program_is_unavailable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runner = agents::AgentList::at(dir.path().join("claude"));
    assert_eq!(runner.run(), RunOutcome::Unavailable);
}

#[test]
fn a_program_that_leaves_its_output_open_fails_at_the_timeout() {
    let fake = FakeClaude::new("sleep 2 & echo '[]'");
    let (outcome, took) = fake.run(Duration::from_millis(300), |outcome| {
        *outcome == RunOutcome::Failed
    });
    assert_eq!(outcome, RunOutcome::Failed);
    assert!(took < Duration::from_secs(5), "took {took:?}");
}

#[test]
fn a_running_agent_with_no_hook_entry_keeps_the_poll_running_every_interval() {
    let script = Script::default()
        .listing(listing("idle"))
        .listing(listing("idle"));
    let clock = Clock::at(NOW_MS);
    let poller = poller(&script, &clock);
    // No candidate and no unreported agent: nothing runs.
    assert_eq!(poller.poll(&[], 0, &EveryPid), None);
    assert_eq!(script.runs(), 0);
    // One unreported agent runs it, at most once per interval.
    assert!(poller.poll(&[], 1, &EveryPid).is_some());
    assert_eq!(script.runs(), 1);
    clock.set(NOW_MS + POLL_INTERVAL_MS - 1);
    poller.poll(&[], 1, &EveryPid);
    assert_eq!(script.runs(), 1);
    clock.set(NOW_MS + POLL_INTERVAL_MS);
    poller.poll(&[], 1, &EveryPid);
    assert_eq!(script.runs(), 2);
    // Once it is gone, the poll rests again.
    clock.set(NOW_MS + 3 * POLL_INTERVAL_MS);
    poller.poll(&[], 0, &EveryPid);
    assert_eq!(script.runs(), 2);
}
