use super::super::resolve_subject;
use super::super::test_support::{Harness, attach, titles};
use super::*;
use crate::discovery::tmux::{FakeTmux, TmuxPaneStatus};
use crate::model::{
    AgentSessionId, AgentSessionNode, ArgvSource, FieldValue, HistorySnapshot, HostedSession,
    MuxIncarnation, MuxRecipe, MuxRecord, NodePresence, PinKind, PinSlot, RecordMeta, SlotFields,
};

const NOW: i64 = 1_790_000_000;

fn meta(id: &str) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: "mux_session:tmux:work".into(),
        first_seen: NOW - 9_000,
        last_seen: NOW - 86_400,
        origin: Default::default(),
        display: Default::default(),
        rebuilt_from: None,
    }
}

fn record(cwd: &str, argv: &[&str], hosted: &[&str]) -> MuxRecord {
    MuxRecord {
        meta: meta("h_work"),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: "work".into(),
            cwd: Some(cwd.into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some("$1".into()),
                created: Some(NOW - 10_000),
            }),
            argv: (!argv.is_empty()).then(|| argv.iter().map(|a| (*a).to_string()).collect()),
            argv_source: Some(ArgvSource::Conspectus),
            harness: argv.contains(&"claude").then(|| "claude-code".to_string()),
            hosted: hosted
                .iter()
                .enumerate()
                .map(|(i, id)| HostedSession {
                    harness: "claude-code".into(),
                    session_id: (*id).into(),
                    first_seen: NOW - 20_000,
                    last_seen: NOW - 100_000 - i as i64,
                })
                .collect(),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn session(key: &str) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode::new(
        AgentSessionId::new("claude-code", "global", key),
        "claude-code",
    ))
}

fn gone_world(record: MuxRecord, sessions: &[&str]) -> GraphSnapshot {
    let mut history = HistorySnapshot {
        muxes: vec![record],
        ..Default::default()
    };
    history.presence.insert(
        "h_work".into(),
        NodePresence::Gone {
            last_seen: NOW - 86_400,
        },
    );
    GraphSnapshot {
        nodes: sessions.iter().map(|key| session(key)).collect(),
        history,
        ..GraphSnapshot::empty()
    }
}

fn subject(snapshot: &GraphSnapshot) -> Subject {
    resolve_subject(snapshot, &[], "h_work").unwrap()
}

fn args(argv: &[OsString]) -> Vec<String> {
    argv.iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_gone_mux_comes_back_with_its_argv_and_resumes_its_last_session() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(
        record(
            &cwd,
            &[
                "atelier",
                "exec",
                "claude",
                "--dangerously-skip-permissions",
            ],
            &["e44d01cb", "old"],
        ),
        &["e44d01cb", "old"],
    );
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();

    assert_eq!(plan.subject, "mux `work` (gone, last seen 1d ago)");
    assert_eq!(titles(&plan), ["new session, resume e44d01cb", "attach"]);
    // The resume lands after the binary, inside the wrapper and ahead of
    // the options (ADR 0098), and the plan shows exactly what will run.
    assert!(
        plan.steps[0]
            .command
            .ends_with("atelier exec claude --resume e44d01cb --dangerously-skip-permissions"),
        "{}",
        plan.steps[0].command
    );
    assert!(plan.render().contains("1. new session, resume e44d01cb"));

    let executed = harness.run(&plan).unwrap();
    assert_eq!(executed.created.as_deref(), Some("work"));
    let calls = harness.tmux.new_session_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        args(&calls[0].3),
        [
            "atelier",
            "exec",
            "claude",
            "--resume",
            "e44d01cb",
            "--dangerously-skip-permissions"
        ]
    );
    assert_eq!(
        harness.tmux.attach_calls(),
        vec![(None, "work".to_string())]
    );

    // The record keeps the fresh argv, not the resume spliced into it,
    // and is marked launched.
    let load = harness.writer.store.load();
    let launched = load
        .history
        .muxes
        .iter()
        .find(|r| r.meta.origin == crate::model::RecordOrigin::Launched)
        .unwrap();
    assert_eq!(
        launched.recipe.argv.as_deref(),
        Some(
            &[
                "atelier",
                "exec",
                "claude",
                "--dangerously-skip-permissions"
            ]
            .map(String::from)[..]
        )
    );
}

#[test]
fn a_rejected_resume_falls_back_to_a_fresh_launch_and_says_so() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record(&cwd, &["claude"], &["e44d01cb"]), &["e44d01cb"]);
    let tmux = FakeTmux::with_sessions("")
        .with_pane_statuses("work", [TmuxPaneStatus::Dead { status: Some(1) }]);
    let harness = Harness::new(tmux);
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();
    let executed = harness.run(&plan).unwrap();

    let calls = harness.tmux.new_session_calls();
    assert_eq!(calls.len(), 2, "resume, then fresh");
    assert_eq!(args(&calls[0].3), ["claude", "--resume", "e44d01cb"]);
    assert_eq!(args(&calls[1].3), ["claude"]);
    assert_eq!(
        harness.tmux.kill_calls().len(),
        1,
        "the dead session was removed first"
    );
    assert!(
        executed
            .notes
            .iter()
            .any(|n| n.contains("launched fresh instead")),
        "{:?}",
        executed.notes
    );
}

#[test]
fn an_explicit_session_overrides_the_most_recent_one() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(
        record(&cwd, &["claude"], &["new", "older"]),
        &["new", "older"],
    );
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let options = Options {
        session: Some("older".into()),
        attach: false,
    };
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &options)
        .unwrap();
    assert_eq!(titles(&plan), ["new session, resume older"]);
}

#[test]
fn a_resume_that_cannot_be_spliced_launches_fresh_with_a_note() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let mut rec = record(&cwd, &["./start-agent.sh"], &["abc"]);
    rec.recipe.harness = Some("claude-code".into());
    let snapshot = gone_world(rec, &["abc"]);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();
    assert_eq!(titles(&plan), ["new session"]);
    assert!(
        plan.warnings[0].contains("doesn't run `claude`"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_session_that_no_longer_exists_launches_fresh_with_a_note() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record(&cwd, &["claude"], &["vanished"]), &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();
    assert_eq!(titles(&plan), ["new session"]);
    assert!(plan.warnings[0].contains("no longer exists"));
}

#[test]
fn a_mux_with_no_harness_comes_back_as_a_shell_or_its_recorded_command() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let harness = Harness::new(FakeTmux::with_sessions(""));

    let shell = gone_world(record(&cwd, &[], &[]), &[]);
    let plan = harness
        .plan(&shell, &subject(&shell), &Options::default())
        .unwrap();
    let Action::NewSession { argv, resume, .. } = &plan.steps[0].action else {
        panic!("{plan:?}")
    };
    assert!(argv.is_empty() && resume.is_none(), "a login shell");

    let serve = gone_world(record(&cwd, &["conspectus", "serve"], &[]), &[]);
    let plan = harness
        .plan(&serve, &subject(&serve), &Options::default())
        .unwrap();
    let Action::NewSession { argv, .. } = &plan.steps[0].action else {
        panic!("{plan:?}")
    };
    assert_eq!(args(argv), ["conspectus", "serve"]);
}

#[test]
fn no_attach_leaves_the_attach_step_out() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record(&cwd, &["claude"], &[]), &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();
    assert_eq!(titles(&plan), ["new session"]);
}

#[test]
fn a_missing_cwd_is_an_error_that_names_the_fix() {
    let snapshot = gone_world(record("/definitely/not/here", &["claude"], &[]), &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let err = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap_err();
    assert!(err.to_string().contains("doesn't exist"), "{err}");

    let mut rec = record("/x", &["claude"], &[]);
    rec.recipe.cwd = None;
    let snapshot = gone_world(rec, &[]);
    let err = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap_err();
    assert!(
        err.to_string().contains("pin edit work --set cwd="),
        "{err}"
    );
}

#[test]
fn an_orchestrator_owned_mux_refuses_naming_the_owner() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let mut rec = record(&cwd, &["claude"], &[]);
    rec.recipe.name = "agentdeck_repo_ab12".into();
    let snapshot = gone_world(rec, &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let err = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap_err();
    assert!(
        matches!(err, RebuildError::Owned { ref owner, .. } if owner == "agent-deck"),
        "{err}"
    );
    assert!(err.to_string().contains("rebuild it there"));
}

fn live_world(command: &str, running_process: bool) -> GraphSnapshot {
    let mut node = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    node.cwd = Some("/tmp".into());
    node.backend_session_id = Some("$1".into());
    node.created_epoch = Some(NOW - 10_000);
    node.active_pane_command = Some(command.into());
    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode::MuxSession(node)],
        history: HistorySnapshot {
            muxes: vec![record("/tmp", &["claude"], &[])],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    };
    if running_process {
        use crate::model::{RuntimeProcessId, RuntimeProcessNode};
        let id = RuntimeProcessId::new("p1");
        snapshot
            .nodes
            .push(GraphNode::RuntimeProcess(RuntimeProcessNode {
                id: id.clone(),
                observation_key: "p1".into(),
                pid: Some(11),
                parent_pid: None,
                root_pane_pid: None,
                command: Some("claude".into()),
                cwd: None,
                harness_key: Some("claude-code".into()),
                role: None,
                depth: None,
                observed_epoch: None,
            }));
        snapshot.candidate_links.push(crate::model::GraphLink::new(
            "mux-proc",
            NodeId::MuxSession(MuxSessionId::new("tmux:work")),
            crate::model::LinkEndpoint::Node {
                id: NodeId::RuntimeProcess(id),
            },
            RelationKind::MuxContainsProcess,
            crate::model::Provenance::Discovered,
        ));
    }
    snapshot
}

#[test]
fn a_live_mux_running_its_agent_is_just_attached() {
    let snapshot = live_world("zsh", true);
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["attach"]);
    assert!(!plan.creates_something());
    harness.run(&plan).unwrap();
    assert!(
        harness.tmux.send_keys_calls().is_empty(),
        "never types into a live agent"
    );
}

#[test]
fn a_live_mux_whose_foreground_command_is_the_harness_is_running_it() {
    let snapshot = live_world("claude", false);
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["attach"]);
}

#[test]
fn a_live_mux_whose_agent_exited_gets_the_argv_typed_in_and_is_attached() {
    let snapshot = live_world("zsh", false);
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["relaunch the agent", "attach"]);
    harness.run(&plan).unwrap();
    let keys = harness.tmux.send_keys_calls();
    assert_eq!(
        keys,
        vec![(None, "work".to_string(), "claude".to_string(), true)]
    );
    assert!(harness.tmux.new_session_calls().is_empty());
}

#[test]
fn a_live_mux_with_a_dead_pane_is_replaced() {
    let snapshot = live_world("zsh", false);
    let harness = Harness::new(FakeTmux::with_sessions("").with_pane_statuses(
        "work",
        [
            TmuxPaneStatus::Dead { status: Some(1) },
            TmuxPaneStatus::Alive,
        ],
    ));
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    assert_eq!(
        titles(&plan),
        ["end the dead session", "new session", "attach"]
    );
    assert!(plan.warnings[0].contains("has died"));
}

#[test]
fn a_live_shell_mux_is_attached() {
    let mut snapshot = live_world("zsh", false);
    snapshot.history.muxes[0].recipe.harness = None;
    snapshot.history.muxes[0].recipe.argv = None;
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["attach"]);
}

#[test]
fn a_slot_overrides_win_over_what_history_recorded() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record("/definitely/not/here", &["claude"], &[]), &[]);
    let slot = PinSlot {
        id: "daily".into(),
        kind: PinKind::Mux,
        target: Some("history:h_work".into()),
        pinned_at: None,
        overrides: SlotFields::from([
            ("cwd".to_string(), FieldValue::Text(cwd)),
            (
                "argv".to_string(),
                FieldValue::List(vec!["claude".into(), "--model".into(), "opus".into()]),
            ),
        ]),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    };
    let subject = resolve_subject(&snapshot, std::slice::from_ref(&slot), "daily").unwrap();
    assert_eq!(subject.source, crate::rebuild::SubjectSource::Slot);
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let plan = harness
        .plan(&snapshot, &subject, &Options::default())
        .unwrap();
    let Action::NewSession {
        argv,
        cwd: planned_cwd,
        ..
    } = &plan.steps[0].action
    else {
        panic!("{plan:?}")
    };
    assert_eq!(args(argv), ["claude", "--model", "opus"]);
    assert!(planned_cwd.is_dir());
}

#[test]
fn a_failed_step_reports_what_already_ran() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record(&cwd, &["claude"], &[]), &[]);
    let tmux = FakeTmux::with_sessions("")
        .with_attach("work", crate::discovery::tmux::TmuxAttachOutcome::NoTarget);
    let harness = Harness::new(tmux);
    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &attach())
        .unwrap();
    let failure = harness.run(&plan).unwrap_err();
    assert_eq!(failure.step, 2);
    assert_eq!(failure.done, ["created `work`"]);
    assert!(failure.to_string().contains("already done: created `work`"));
}

#[test]
fn refs_resolve_by_slot_history_id_node_id_or_mux_name() {
    use crate::rebuild::SubjectSource::{History, Node, Slot};
    let snapshot = live_world("zsh", false);
    let slot = PinSlot {
        id: "daily".into(),
        kind: PinKind::Mux,
        target: Some("history:h_work".into()),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    };
    for (input, source) in [
        ("daily", Slot),
        ("h_work", History),
        ("work", Node),
        ("tmux:work", Node),
        ("mux_session:tmux:work", Node),
    ] {
        let subject = resolve_subject(&snapshot, std::slice::from_ref(&slot), input).unwrap();
        assert_eq!(subject.source, source, "{input}");
    }
    // A node with a history record rebuilds from the record.
    let subject = resolve_subject(&snapshot, &[], "work").unwrap();
    assert_eq!(subject.slot.target.as_deref(), Some("history:h_work"));
    assert!(matches!(
        resolve_subject(&snapshot, &[], "nothing"),
        Err(RebuildError::NotFound(_))
    ));
}

// --- Chaining (ADR 0142): a mux whose working directory is a worktree
// that is gone brings the worktree back first.

mod chain {
    use super::*;
    use crate::model::{
        BranchId, BranchNode, CheckoutId, CheckoutNode, CheckoutRecipe, CheckoutRecord, RepoId,
        WorktreeMeta,
    };
    use crate::rebuild::test_support::FakeWorktrees;

    struct Setup {
        tmp: tempfile::TempDir,
        snapshot: GraphSnapshot,
    }

    fn setup(worktree_exists: bool) -> Setup {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("app/.git")).unwrap();
        let repo = RepoId::new(tmp.path().join("app/.git").display().to_string());
        let wt_root = tmp.path().join("wt-feature");
        if worktree_exists {
            std::fs::create_dir_all(&wt_root).unwrap();
        }
        let mut history = HistorySnapshot {
            muxes: vec![{
                let mut rec = record(wt_root.join("crates").to_str().unwrap(), &["claude"], &[]);
                rec.recipe.name = "work".into();
                rec
            }],
            checkouts: vec![CheckoutRecord {
                meta: meta("h_wt"),
                recipe: CheckoutRecipe {
                    repo: format!("repo:{}", tmp.path().join("app/.git").display()),
                    root: wt_root.display().to_string(),
                    linked: true,
                    branch: Some("feature/x".into()),
                    head: Some("abc123".into()),
                    upstream: None,
                    extra: Default::default(),
                },
                extra: Default::default(),
            }],
            ..Default::default()
        };
        history.presence.insert(
            "h_work".into(),
            NodePresence::Gone {
                last_seen: NOW - 86_400,
            },
        );
        history.presence.insert(
            "h_wt".into(),
            NodePresence::Gone {
                last_seen: NOW - 86_400,
            },
        );
        let snapshot = GraphSnapshot {
            nodes: vec![
                GraphNode::Checkout(
                    CheckoutNode::new(
                        CheckoutId::new(repo.clone(), tmp.path().join("app").display().to_string()),
                        tmp.path().join("app").display().to_string(),
                    )
                    .with_worktree(WorktreeMeta::primary()),
                ),
                GraphNode::Branch(BranchNode {
                    id: BranchId::new(repo, "refs/heads/feature/x"),
                    refname: "refs/heads/feature/x".into(),
                    current_commit: None,
                    upstream: None,
                }),
            ],
            history,
            ..GraphSnapshot::empty()
        };
        Setup { tmp, snapshot }
    }

    #[test]
    fn a_ghost_mux_on_a_gone_worktree_rebuilds_the_worktree_then_the_mux() {
        let s = setup(false);
        // Worktrunk puts it somewhere other than where it was recorded.
        let landed = s.tmp.path().join("elsewhere/app.feature-x");
        let h = Harness::new(FakeTmux::with_sessions(""))
            .with_worktrees(FakeWorktrees::landing_at(&landed));
        let subject = resolve_subject(&s.snapshot, &[], "h_work").unwrap();
        let plan = h.plan(&s.snapshot, &subject, &attach()).unwrap();
        assert_eq!(
            titles(&plan),
            ["rebuild worktree feature/x", "new session", "attach"]
        );

        h.run(&plan).unwrap();
        // The session starts inside the worktree where it really landed,
        // at the same place relative to its root as before.
        let calls = h.tmux.new_session_calls();
        assert_eq!(calls[0].2, landed.join("crates"));
        assert_eq!(
            h.worktree.as_ref().unwrap().switches.lock().unwrap().len(),
            1
        );
    }

    #[test]
    fn a_failed_prerequisite_stops_the_plan_before_the_mux() {
        let s = setup(false);
        let h = Harness::new(FakeTmux::with_sessions(""))
            .with_worktrees(FakeWorktrees::landing_at(&s.tmp.path().join("wt-feature")));
        *h.worktree.as_ref().unwrap().fail_with.lock().unwrap() = Some("boom".into());
        let subject = resolve_subject(&s.snapshot, &[], "h_work").unwrap();
        let plan = h.plan(&s.snapshot, &subject, &attach()).unwrap();
        let failure = h.run(&plan).unwrap_err();
        assert_eq!(failure.step, 1);
        assert!(failure.done.is_empty());
        assert!(
            h.tmux.new_session_calls().is_empty(),
            "the mux never started"
        );
        assert!(
            failure.to_string().contains("nothing had run yet"),
            "{failure}"
        );
    }

    #[test]
    fn a_planned_mux_with_a_worktree_branch_builds_the_worktree_first() {
        let s = setup(false);
        let landed = s.tmp.path().join("wt-new");
        let h = Harness::new(FakeTmux::with_sessions(""))
            .with_worktrees(FakeWorktrees::landing_at(&landed));
        let slot = PinSlot {
            id: "stream".into(),
            kind: PinKind::Mux,
            target: None,
            pinned_at: None,
            overrides: SlotFields::from([
                ("name".to_string(), FieldValue::Text("stream".into())),
                (
                    "cwd".to_string(),
                    FieldValue::Text(s.tmp.path().join("app").display().to_string()),
                ),
                (
                    "harness".to_string(),
                    FieldValue::Text("claude-code".into()),
                ),
                (
                    "worktree_branch".to_string(),
                    FieldValue::Text("feature/new".into()),
                ),
            ]),
            snapshot: SlotFields::new(),
            order: 0,
            legacy: false,
        };
        let subject = resolve_subject(&s.snapshot, std::slice::from_ref(&slot), "stream").unwrap();
        let plan = h.plan(&s.snapshot, &subject, &attach()).unwrap();
        assert_eq!(
            titles(&plan),
            ["recreate branch feature/new", "new session", "attach"]
        );
        h.run(&plan).unwrap();
        assert_eq!(h.tmux.new_session_calls()[0].2, landed);
    }

    #[test]
    fn a_worktree_branch_that_already_has_a_worktree_is_used_as_is() {
        let mut s = setup(true);
        let existing = s.tmp.path().join("wt-feature");
        let repo = RepoId::new(s.tmp.path().join("app/.git").display().to_string());
        let mut node = CheckoutNode::new(
            CheckoutId::new(repo.clone(), existing.display().to_string()),
            existing.display().to_string(),
        )
        .with_worktree(WorktreeMeta::linked());
        node.current_branch = Some(BranchId::new(repo, "refs/heads/feature/x"));
        s.snapshot.nodes.push(GraphNode::Checkout(node));
        let h = Harness::new(FakeTmux::with_sessions(""));
        let slot = PinSlot {
            id: "stream".into(),
            kind: PinKind::Mux,
            target: None,
            pinned_at: None,
            overrides: SlotFields::from([
                ("name".to_string(), FieldValue::Text("stream".into())),
                (
                    "cwd".to_string(),
                    FieldValue::Text(s.tmp.path().join("app").display().to_string()),
                ),
                (
                    "harness".to_string(),
                    FieldValue::Text("claude-code".into()),
                ),
                (
                    "worktree_branch".to_string(),
                    FieldValue::Text("feature/x".into()),
                ),
            ]),
            snapshot: SlotFields::new(),
            order: 0,
            legacy: false,
        };
        let subject = resolve_subject(&s.snapshot, std::slice::from_ref(&slot), "stream").unwrap();
        let plan = h.plan(&s.snapshot, &subject, &Options::default()).unwrap();
        assert_eq!(titles(&plan), ["new session"]);
        h.run(&plan).unwrap();
        assert_eq!(h.tmux.new_session_calls()[0].2, existing);
    }
}

fn harness_only_record(cwd: &str) -> MuxRecord {
    let mut record = record(cwd, &[], &[]);
    record.recipe.harness = Some("claude-code".into());
    record
}

fn profile(
    origin: crate::config::ProfileOrigin,
    argv: &[&str],
) -> crate::launch_default::LaunchDefaults {
    let mut config = crate::config::LaunchConfig::default();
    config.profiles.insert(
        "claude-code".into(),
        crate::config::LaunchProfile {
            argv: argv.iter().map(|a| (*a).to_string()).collect(),
            origin,
        },
    );
    crate::launch_default::LaunchDefaults::new(config).with_atelier(false)
}

#[test]
fn a_mux_with_no_recorded_argv_starts_from_the_launch_profile_and_says_so() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(harness_only_record(&cwd), &[]);
    let harness = Harness::new(FakeTmux::with_sessions("")).with_launch(profile(
        crate::config::ProfileOrigin::Project,
        &["nix", "develop", "-c", "claude", "--flag"],
    ));

    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();

    let Action::NewSession { argv, .. } = &plan.steps[0].action else {
        panic!("{plan:?}")
    };
    assert_eq!(args(argv), ["nix", "develop", "-c", "claude", "--flag"]);
    assert!(
        plan.render()
            .contains("note: argv comes from the project launch profile"),
        "{}",
        plan.render()
    );
}

#[test]
fn a_recorded_argv_beats_the_launch_profile() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(record(&cwd, &["claude", "--recorded"], &[]), &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""))
        .with_launch(profile(crate::config::ProfileOrigin::User, &["other"]));

    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();

    let Action::NewSession { argv, .. } = &plan.steps[0].action else {
        panic!("{plan:?}")
    };
    assert_eq!(args(argv), ["claude", "--recorded"]);
    assert!(
        !plan.render().contains("argv comes from"),
        "{}",
        plan.render()
    );
}

#[test]
fn the_harness_default_adds_no_note() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap().to_string();
    let snapshot = gone_world(harness_only_record(&cwd), &[]);
    let harness = Harness::new(FakeTmux::with_sessions(""));

    let plan = harness
        .plan(&snapshot, &subject(&snapshot), &Options::default())
        .unwrap();

    assert!(
        !plan.render().contains("argv comes from"),
        "{}",
        plan.render()
    );
}
