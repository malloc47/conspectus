use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

use super::*;
use crate::model::{GraphNode, MuxSessionNode};

fn mux(name: &str) -> MuxSessionId {
    MuxSessionId::new(format!("tmux:{name}"))
}

fn target(session_id: &str, created_epoch: i64) -> MuxIncarnationTarget {
    MuxIncarnationTarget {
        backend: "tmux".to_string(),
        socket: None,
        session_id: session_id.to_string(),
        created_epoch,
    }
}

/// Answers existence checks from a script, then `false` once it runs out.
struct ScriptedProbe {
    answers: Mutex<VecDeque<bool>>,
    asked: Mutex<usize>,
}

impl ScriptedProbe {
    fn new(answers: &[bool]) -> Self {
        Self {
            answers: Mutex::new(answers.iter().copied().collect()),
            asked: Mutex::new(0),
        }
    }

    fn asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}

impl IncarnationProbe for ScriptedProbe {
    fn still_there(&self, _target: &MuxIncarnationTarget) -> bool {
        *self.asked.lock().unwrap() += 1;
        self.answers.lock().unwrap().pop_front().unwrap_or(false)
    }
}

mod check {
    use super::*;

    /// Run [`confirm`] against `answers`: whether it confirmed, how many
    /// answers it asked for, and the pauses it took.
    fn run(answers: &[bool]) -> (bool, usize, Vec<Duration>) {
        let probe = ScriptedProbe::new(answers);
        let mut pauses = Vec::new();
        let confirmed = confirm(&probe, &target("$3", 100), |pause| pauses.push(pause));
        (confirmed, probe.asked(), pauses)
    }

    #[test]
    fn a_mux_still_there_is_confirmed_by_the_first_answer() {
        assert_eq!(run(&[true]), (true, 1, vec![]));
    }

    #[test]
    fn a_gone_mux_is_asked_about_once_more_and_stays_unconfirmed() {
        assert_eq!(run(&[false, false]), (false, 2, vec![RECHECK_DELAY]));
    }

    #[test]
    fn an_unclear_first_answer_is_settled_by_the_recheck() {
        assert_eq!(run(&[false, true]), (true, 2, vec![RECHECK_DELAY]));
    }

    #[test]
    fn each_mux_is_reported_as_soon_as_it_is_confirmed() {
        // `a` is confirmed at once, `b` never, `c` on its re-check.
        let probe = ScriptedProbe::new(&[true, false, false, false, true]);
        let reported = RefCell::new(Vec::new());
        let unconfirmed = check_all(
            &probe,
            vec![
                (mux("a"), target("$1", 1)),
                (mux("b"), target("$2", 2)),
                (mux("c"), target("$3", 3)),
            ],
            |_| {},
            |mux| reported.borrow_mut().push(mux),
        );
        assert_eq!(reported.into_inner(), vec![mux("a"), mux("c")]);
        assert_eq!(unconfirmed, BTreeSet::from([mux("b")]));
    }

    #[test]
    fn only_the_same_session_id_and_creation_time_name_the_target() {
        let expected = target("$2", 1_794_441_156);
        assert!(answer_names("$2 1794441156\n", &expected));
        // tmux answers a target it can't find with empty fields.
        assert!(!answer_names(" \n", &expected));
        assert!(!answer_names("", &expected));
        // An answer about another session.
        assert!(!answer_names("$1 1794441156\n", &expected));
        // The same id on a restarted server is another incarnation.
        assert!(!answer_names("$2 1794449999\n", &expected));
        assert!(!answer_names("$2 1794441156 extra\n", &expected));
        assert!(!answer_names("$2 soon\n", &expected));
    }

    #[test]
    fn a_mux_of_another_backend_is_never_confirmed() {
        let mut other = target("$2", 1);
        other.backend = "zellij".to_string();
        assert!(!TmuxIncarnation::default().still_there(&other));
    }
}

/// The tmux probe against a private server: its own socket directory,
/// `TMUX` unset, killed at the end, so the server the tests run under is
/// never touched.
mod private_server {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    fn tmux(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-f", "/dev/null"])
            .args(args)
            .env_remove("TMUX")
            .env("TMUX_TMPDIR", dir)
            .output()
            .expect("tmux")
    }

    fn incarnation(dir: &Path, socket: Option<&str>, name: &str) -> MuxIncarnationTarget {
        let mut args = Vec::new();
        if let Some(socket) = socket {
            args.extend(["-L", socket]);
        }
        let filter = format!("#{{==:#{{session_name}},{name}}}");
        args.extend([
            "list-sessions",
            "-f",
            &filter,
            "-F",
            "#{session_id} #{session_created}",
        ]);
        let out = String::from_utf8(tmux(dir, &args).stdout).unwrap();
        let (session_id, created) = out.trim().split_once(' ').expect("session listed");
        MuxIncarnationTarget {
            backend: "tmux".to_string(),
            socket: socket.map(str::to_string),
            session_id: session_id.to_string(),
            created_epoch: created.parse().unwrap(),
        }
    }

    struct Server(tempfile::TempDir);

    impl Drop for Server {
        fn drop(&mut self) {
            let _ = tmux(self.0.path(), &["kill-server"]);
            let _ = tmux(self.0.path(), &["-L", "lab", "kill-server"]);
        }
    }

    #[test]
    fn the_probe_tells_a_running_session_from_a_gone_one_and_a_new_one() {
        if Command::new("tmux").arg("-V").output().is_err() {
            eprintln!("tmux not installed; skipping");
            return;
        }
        // tmux socket paths must stay short.
        let server = Server(
            tempfile::Builder::new()
                .prefix("csp")
                .tempdir_in("/tmp")
                .unwrap(),
        );
        let dir = server.0.path();
        let probe = TmuxIncarnation {
            socket_dir: Some(dir.to_path_buf()),
        };
        for name in ["keep", "work"] {
            assert!(
                tmux(dir, &["new-session", "-d", "-s", name, "sleep 60"])
                    .status
                    .success()
            );
        }
        assert!(
            tmux(
                dir,
                &["-L", "lab", "new-session", "-d", "-s", "work", "sleep 60"]
            )
            .status
            .success()
        );
        let work = incarnation(dir, None, "work");
        let lab = incarnation(dir, Some("lab"), "work");
        assert!(probe.still_there(&work), "a running session answers");
        assert!(probe.still_there(&lab), "on its own server");

        assert!(
            tmux(dir, &["kill-session", "-t", &work.session_id])
                .status
                .success()
        );
        assert!(!probe.still_there(&work), "a gone session doesn't");
        assert!(
            tmux(dir, &["new-session", "-d", "-s", "work", "sleep 60"])
                .status
                .success()
        );
        assert!(
            !probe.still_there(&work),
            "a new session with the name is another incarnation"
        );

        assert!(tmux(dir, &["-L", "lab", "kill-server"]).status.success());
        assert!(
            !probe.still_there(&lab),
            "nor does one whose server is gone"
        );
    }
}

mod settle {
    use super::*;

    fn world(names: &[&str]) -> GraphSnapshot {
        GraphSnapshot {
            nodes: names
                .iter()
                .map(|name| GraphNode::MuxSession(MuxSessionNode::new(mux(name), "tmux", *name)))
                .collect(),
            ..GraphSnapshot::empty()
        }
    }

    fn ids(names: &[&str]) -> BTreeSet<NodeId> {
        names
            .iter()
            .map(|name| NodeId::MuxSession(mux(name)))
            .collect()
    }

    /// Probe results in order (the first is the worker's first probe);
    /// the last repeats. Returns the probes run, the pauses, and the
    /// snapshot taken.
    fn run(settle: &Settle, results: &[&[&str]]) -> (usize, usize, GraphSnapshot) {
        let calls = RefCell::new(1usize);
        let sleeps = RefCell::new(0usize);
        let (snapshot, ()) = probe_until_settled(
            (Ok(world(results[0])), ()),
            settle,
            2,
            Duration::from_millis(150),
            |_| *sleeps.borrow_mut() += 1,
            || {
                let i = (*calls.borrow()).min(results.len() - 1);
                *calls.borrow_mut() += 1;
                (Ok(world(results[i])), ())
            },
        );
        (calls.into_inner(), sleeps.into_inner(), snapshot.unwrap())
    }

    fn suspects(names: &[&str]) -> Settle {
        Settle {
            suspects: ids(names),
            unchanged_from: None,
        }
    }

    #[test]
    fn with_every_mux_confirmed_it_probes_once() {
        let (calls, sleeps, _) = run(&Settle::default(), &[&["a", "b"]]);
        assert_eq!((calls, sleeps), (1, 0));
    }

    #[test]
    fn a_suspect_no_longer_listed_is_taken_at_once() {
        let (calls, sleeps, snapshot) = run(&suspects(&["b"]), &[&["a"]]);
        assert_eq!((calls, sleeps), (1, 0));
        assert_eq!(live_mux_ids(&snapshot), ids(&["a"]));
    }

    #[test]
    fn a_suspect_still_listed_is_probed_again_until_it_goes() {
        // The first probe still sees the session that just ended, even
        // though another mux appeared meanwhile.
        let (calls, sleeps, snapshot) = run(&suspects(&["b"]), &[&["a", "b", "c"], &["a", "c"]]);
        assert_eq!((calls, sleeps), (2, 1));
        assert_eq!(live_mux_ids(&snapshot), ids(&["a", "c"]));
    }

    #[test]
    fn a_suspect_that_stays_listed_stops_after_the_bounded_retries() {
        let (calls, sleeps, _) = run(&suspects(&["a"]), &[&["a"]]);
        assert_eq!((calls, sleeps), (3, 2));
    }

    #[test]
    fn after_the_pane_comes_back_an_unchanged_set_is_probed_again() {
        let settle = Settle {
            suspects: BTreeSet::new(),
            unchanged_from: Some(ids(&["a", "b"])),
        };
        let (calls, sleeps, snapshot) = run(&settle, &[&["a", "b"], &["a"]]);
        assert_eq!((calls, sleeps), (2, 1));
        assert_eq!(live_mux_ids(&snapshot), ids(&["a"]));
        let (calls, _, _) = run(&settle, &[&["a"]]);
        assert_eq!(calls, 1, "a changed set is taken at once");
    }

    #[test]
    fn a_failed_probe_is_returned_without_retrying() {
        let calls = RefCell::new(0);
        let (result, ()) = probe_until_settled(
            (Err(anyhow::anyhow!("boom")), ()),
            &suspects(&["a"]),
            2,
            Duration::ZERO,
            |_| {},
            || {
                *calls.borrow_mut() += 1;
                (Ok(world(&[])), ())
            },
        );
        assert!(result.is_err());
        assert_eq!(calls.into_inner(), 0);
    }
}

/// The live loop end to end, with a scripted existence check and a
/// discovery that waits for the test to hand it each snapshot.
mod live {
    use super::*;
    use crate::tui::app::{App, Msg, PendingHandoff, SnapshotHandle};
    use crate::tui::runtime::{
        Discovered, ForgeWorker, LiveMode, LoopMode, WorkerLedger, content, visibility,
    };

    /// How long a test waits for the loop before it fails.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// A discovery whose every call hands the test a sender and waits for
    /// the snapshot to return.
    fn gated_discovery() -> (Arc<Discover>, mpsc::Receiver<mpsc::Sender<GraphSnapshot>>) {
        let (calls_tx, calls_rx) = mpsc::channel();
        let discover = move |_: &mut RunConfig, _: DaemonNudge| {
            let (gate_tx, gate_rx) = mpsc::channel();
            let _ = calls_tx.send(gate_tx);
            Discovered {
                snapshot: Ok(gate_rx.recv().unwrap_or_default()),
                from_daemon: true,
                warning: None,
            }
        };
        (Arc::new(discover), calls_rx)
    }

    struct Rig {
        mode: LiveMode,
        app: App,
        calls: mpsc::Receiver<mpsc::Sender<GraphSnapshot>>,
        regained: mpsc::Sender<()>,
        probe: Arc<ScriptedProbe>,
    }

    impl Rig {
        /// The loop showing `snapshot`, with `answers` scripted for the
        /// existence check.
        fn new(snapshot: GraphSnapshot, answers: &[bool]) -> Self {
            let (result_tx, result_rx) = mpsc::channel();
            let (preview_tx, preview_rx) = mpsc::channel();
            let (confirmed_tx, confirmed_rx) = mpsc::channel();
            let (regained, regained_rx) = mpsc::channel();
            let (discover, calls) = gated_discovery();
            let probe = Arc::new(ScriptedProbe::new(answers));
            let mode = LiveMode {
                tmux: Box::new(crate::discovery::tmux::FakeTmux::with_sessions("")),
                result_tx,
                result_rx,
                preview_tx,
                preview_rx,
                confirmed_tx,
                confirmed_rx,
                probe: Arc::clone(&probe) as Arc<dyn IncarnationProbe>,
                discover,
                workers: WorkerLedger::default(),
                last_refresh: Instant::now(),
                refresh_interval: Duration::from_secs(3600),
                forge: ForgeWorker::new(),
                visibility: Some(visibility::VisibilityWatcher { rx: regained_rx }),
                content: content::ContentDriver::default(),
            };
            let mut app = App::new(RunConfig::defaults());
            app.update(Msg::SwitchView(crate::tui::View::Mux));
            let tree = crate::tui::rows::build_tree_for_view(
                crate::tui::rows::TreeInputs::from_app(&snapshot, &app),
            );
            app.update(Msg::SetData {
                snapshot: SnapshotHandle::new(snapshot),
                tree,
                loaded_at_epoch: 1_700_000_200,
                initial_selection_hint: None,
            });
            Self {
                mode,
                app,
                calls,
                regained,
                probe,
            }
        }

        /// The operator came back from an attach to `name`, and the loop
        /// started its refresh. Returns the gate of that refresh's first
        /// discovery.
        fn returned(&mut self, name: &str) -> mpsc::Sender<GraphSnapshot> {
            self.app.update(Msg::HandoffReturned(mux(name)));
            self.mode.start_requested_refreshes(&mut self.app);
            self.next_discovery()
        }

        fn next_discovery(&self) -> mpsc::Sender<GraphSnapshot> {
            self.calls
                .recv_timeout(PATIENCE)
                .expect("a refresh to start its discovery")
        }

        fn drain(&mut self) {
            let config = self.app.config().clone();
            self.mode.drain(&mut self.app, &config).unwrap();
        }

        fn drain_until(&mut self, what: &str, done: impl Fn(&App) -> bool) {
            let start = Instant::now();
            loop {
                self.drain();
                if done(&self.app) {
                    return;
                }
                assert!(start.elapsed() < PATIENCE, "timed out waiting for {what}");
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        fn frozen(&self) -> BTreeSet<MuxSessionId> {
            self.app
                .handoff()
                .map(PendingHandoff::muxes)
                .unwrap_or_default()
        }
    }

    fn running(name: &str, session_id: &str) -> MuxSessionNode {
        let mut node =
            MuxSessionNode::new(mux(name), "tmux", name).with_created_epoch(1_700_000_100);
        node.backend_session_id = Some(session_id.to_string());
        node
    }

    fn snapshot(muxes: Vec<MuxSessionNode>) -> GraphSnapshot {
        GraphSnapshot {
            nodes: muxes.into_iter().map(GraphNode::MuxSession).collect(),
            ..GraphSnapshot::empty()
        }
    }

    fn holds(app: &App, name: &str) -> bool {
        app.handoff()
            .is_some_and(|handoff| handoff.holds_mux(&mux(name)))
    }

    #[test]
    fn a_detach_from_a_running_mux_releases_its_rows_before_the_refresh_lands() {
        let shown = snapshot(vec![running("a", "$1")]);
        let mut rig = Rig::new(shown.clone(), &[true]);

        let gate = rig.returned("a");
        assert!(holds(&rig.app, "a"), "frozen on return");
        rig.drain_until("the check to release `a`", |app| !holds(app, "a"));

        assert!(
            rig.app.handoff().is_some(),
            "the refresh is still on its way"
        );
        assert_eq!(rig.probe.asked(), 1);
        gate.send(shown).unwrap();
        rig.drain_until("the refresh to land", |app| app.handoff().is_none());
        assert!(
            rig.calls.try_recv().is_err(),
            "a confirmed mux leaves nothing to probe again"
        );
    }

    #[test]
    fn a_mux_that_exited_stays_frozen_until_a_refresh_shows_it_gone() {
        let mut rig = Rig::new(snapshot(vec![running("a", "$1"), running("b", "$2")]), &[]);

        let first = rig.returned("a");
        let start = Instant::now();
        while rig.probe.asked() < 2 {
            assert!(start.elapsed() < PATIENCE, "the check and its re-check");
            std::thread::sleep(Duration::from_millis(5));
        }
        rig.drain();
        assert!(holds(&rig.app, "a"), "neither answer confirmed it");

        // The listing still shows the session that just ended.
        first
            .send(snapshot(vec![running("a", "$1"), running("b", "$2")]))
            .unwrap();
        let again = rig.next_discovery();
        rig.drain();
        assert!(holds(&rig.app, "a"), "the refresh hasn't shown it gone yet");

        again.send(snapshot(vec![running("b", "$2")])).unwrap();
        rig.drain_until("the refresh to land", |app| app.handoff().is_none());
        assert!(
            rig.app
                .snapshot_handle()
                .unwrap()
                .snapshot()
                .find_node(&NodeId::MuxSession(mux("a")))
                .is_none()
        );
    }

    #[test]
    fn a_mux_confirmed_only_by_the_recheck_is_released_then() {
        let shown = snapshot(vec![running("a", "$1")]);
        let mut rig = Rig::new(shown.clone(), &[false, true]);

        let gate = rig.returned("a");
        rig.drain_until("the re-check to release `a`", |app| !holds(app, "a"));
        assert_eq!(rig.probe.asked(), 2);
        gate.send(shown).unwrap();
        rig.drain_until("the refresh to land", |app| app.handoff().is_none());
    }

    #[test]
    fn a_blocking_attach_to_b_does_not_hold_a() {
        // Outside tmux the attach blocks the loop: A's refresh finishes
        // while the operator is in B, and its result waits undrained
        // until the return from B has started B's refresh.
        let mut rig = Rig::new(snapshot(vec![]), &[]);
        let a = rig.returned("a");
        a.send(snapshot(vec![])).unwrap();
        let waiting = rig
            .mode
            .result_rx
            .recv_timeout(PATIENCE)
            .expect("A's refresh to finish");

        let b = rig.returned("b");
        rig.mode.result_tx.send(waiting).unwrap();
        rig.drain();
        assert_eq!(
            rig.frozen(),
            BTreeSet::from([mux("b")]),
            "the first drain after the return releases A"
        );

        b.send(snapshot(vec![])).unwrap();
        rig.drain_until("B's refresh to land", |app| app.handoff().is_none());
    }

    #[test]
    fn inside_tmux_each_mux_is_released_by_its_own_refresh() {
        // `switch-client` returns at once, so returns pile up, and the
        // pane coming back into view starts one more refresh.
        let mut rig = Rig::new(snapshot(vec![running("other", "$9")]), &[]);
        let a = rig.returned("a");
        let b = rig.returned("b");
        rig.regained.send(()).unwrap();
        rig.drain();
        let regained = rig.next_discovery();

        a.send(snapshot(vec![])).unwrap();
        rig.drain_until("A's refresh to release it", |app| !holds(app, "a"));
        assert_eq!(rig.frozen(), BTreeSet::from([mux("b")]));

        b.send(snapshot(vec![])).unwrap();
        rig.drain_until("B's refresh to release it", |app| !holds(app, "b"));
        assert!(
            rig.app.handoff().is_some(),
            "the pane's own refresh is still on its way"
        );

        regained.send(snapshot(vec![])).unwrap();
        rig.drain_until("the pane's refresh to land", |app| app.handoff().is_none());
    }
}
