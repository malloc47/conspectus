//! Plans for running entities, the exact commands a plan prints, and the
//! guarantee that those commands are the ones a run executes.

use std::ffi::OsString;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::test_support::{FakeWorktrees, Harness, attach, titles};
use super::*;
use crate::discovery::tmux::{FakeTmux, MuxBackend, SystemTmux, TmuxPaneStatus};
use crate::model::{
    ArgvSource, CheckoutId, CheckoutNode, CheckoutRecipe, CheckoutRecord, GraphNode, GraphSnapshot,
    HistorySnapshot, HostedSession, MuxIncarnation, MuxRecipe, MuxRecord, MuxSessionId,
    MuxSessionNode, NodeId, RecordMeta, RecordedRemote, RepoId, RepoNode, RepoRecipe, RepoRecord,
    WorktreeMeta,
};

const NOW: i64 = 1_790_000_000;

fn meta(id: &str) -> RecordMeta {
    RecordMeta {
        id: id.into(),
        key: "mux_session:tmux:work".into(),
        first_seen: NOW - 9_000,
        last_seen: NOW - 60,
        origin: Default::default(),
        display: Default::default(),
        rebuilt_from: None,
    }
}

fn mux_record(cwd: &str, socket: Option<&str>, argv: &[&str]) -> MuxRecord {
    MuxRecord {
        meta: meta("h_work"),
        recipe: MuxRecipe {
            backend: "tmux".into(),
            socket: socket.map(str::to_string),
            name: "work".into(),
            cwd: Some(cwd.into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some("$1".into()),
                created: Some(NOW - 10_000),
            }),
            argv: Some(argv.iter().map(|a| (*a).to_string()).collect()),
            argv_source: Some(ArgvSource::Conspectus),
            harness: Some("claude-code".into()),
            hosted: vec![HostedSession {
                harness: "claude-code".into(),
                session_id: "e44d01cb".into(),
                first_seen: NOW - 9_000,
                last_seen: NOW - 100,
            }],
            ..Default::default()
        },
        extra: Default::default(),
    }
}

/// A running mux `work` whose record says how it was started.
fn running_mux(cwd: &str, socket: Option<&str>, argv: &[&str]) -> GraphSnapshot {
    let mut node = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    node.cwd = Some(cwd.into());
    node.backend_session_id = Some("$1".into());
    node.created_epoch = Some(NOW - 10_000);
    node.socket = socket.map(str::to_string);
    // The agent is running in the pane, so a real rebuild only attaches.
    let process = crate::model::RuntimeProcessId::new("p1");
    let link = crate::model::GraphLink::new(
        "mux-proc",
        NodeId::MuxSession(MuxSessionId::new("tmux:work")),
        crate::model::LinkEndpoint::Node {
            id: NodeId::RuntimeProcess(process.clone()),
        },
        crate::model::RelationKind::MuxContainsProcess,
        crate::model::Provenance::Discovered,
    );
    GraphSnapshot {
        nodes: vec![
            GraphNode::MuxSession(node),
            GraphNode::AgentSession(crate::model::AgentSessionNode::new(
                crate::model::AgentSessionId::new("claude-code", "global", "e44d01cb"),
                "claude-code",
            )),
            GraphNode::RuntimeProcess(crate::model::RuntimeProcessNode {
                id: process,
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
            }),
        ],
        candidate_links: vec![link],
        history: HistorySnapshot {
            muxes: vec![mux_record(cwd, socket, argv)],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    }
}

fn stopped() -> Options {
    Options {
        attach: true,
        assume_stopped: true,
        ..Default::default()
    }
}

fn plan_mux(snapshot: &GraphSnapshot, options: &Options) -> Result<Plan, RebuildError> {
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let subject = resolve_subject(snapshot, &[], "h_work").unwrap();
    harness.plan(snapshot, &subject, options)
}

#[test]
fn a_running_mux_is_planned_as_if_it_had_stopped() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let snapshot = running_mux(cwd, None, &["atelier", "exec", "claude"]);

    let real = plan_mux(&snapshot, &attach()).unwrap();
    assert_eq!(titles(&real), ["attach"], "a real rebuild only attaches");

    let plan = plan_mux(&snapshot, &stopped()).unwrap();

    assert_eq!(plan.subject, "mux `work` (running; planned as if stopped)");
    assert_eq!(
        titles(&plan),
        ["new session, resume e44d01cb", "attach"],
        "{}",
        plan.render()
    );
    assert!(plan.warnings[0].starts_with("hypothetical: `work` is running"));
    assert_eq!(
        plan.commands(),
        [
            format!(
                "tmux new-session -d -s work -c {cwd} atelier exec claude --resume e44d01cb \
                 \\; set-option -t work remain-on-exit failed"
            ),
            "tmux attach-session -t work".to_string(),
        ]
    );
}

#[test]
fn the_script_is_comments_and_exact_commands() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let snapshot = running_mux(cwd, Some("side"), &["claude"]);
    let options = Options {
        attach: false,
        ..stopped()
    };

    let script = plan_mux(&snapshot, &options).unwrap().script();

    assert_eq!(
        script,
        format!(
            "# rebuild mux `work` (running; planned as if stopped)\n\
             # note: hypothetical: `work` is running; this is what a rebuild would do if it had stopped\n\
             # new session, resume e44d01cb\n\
             tmux -L side new-session -d -s work -c {cwd} claude --resume e44d01cb \\; set-option -t work remain-on-exit failed\n"
        )
    );
}

#[test]
fn arguments_with_spaces_and_quotes_are_quoted_for_the_shell() {
    let cwd = tempfile::tempdir().unwrap();
    let dir = cwd.path().join("my project");
    std::fs::create_dir_all(&dir).unwrap();
    let cwd = dir.to_str().unwrap();
    let snapshot = running_mux(
        cwd,
        None,
        &["claude", "--append-system-prompt", "it's a $trap"],
    );

    let plan = plan_mux(&snapshot, &stopped()).unwrap();
    let command = plan.commands()[0].to_string();

    assert!(command.contains(&format!("-c '{cwd}'")), "{command}");
    assert!(command.contains(r"'it'\''s a $trap'"), "{command}");
    // A shell reads back exactly the argument list a run would use.
    let Action::NewSession {
        cwd, argv, resume, ..
    } = &plan.steps[0].action
    else {
        panic!("{plan:?}")
    };
    let launched = resume
        .as_ref()
        .map_or_else(|| argv.clone(), |r| r.argv.clone());
    let mut expected = vec!["tmux", "new-session", "-d", "-s", "work", "-c"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    expected.push(cwd.display().to_string());
    expected.extend(launched.iter().map(|a| a.to_string_lossy().into_owned()));
    expected.push(";".to_string());
    expected.extend(
        ["set-option", "-t", "work", "remain-on-exit", "failed"]
            .iter()
            .map(|s| (*s).to_string()),
    );
    assert_eq!(crate::argv::split_command_line(&command), expected);
}

#[test]
fn an_orchestrator_owned_mux_still_refuses() {
    let cwd = tempfile::tempdir().unwrap();
    let mut snapshot = running_mux(cwd.path().to_str().unwrap(), None, &["claude"]);
    snapshot.history.muxes[0].recipe.orchestrator = Some("agent-deck".into());

    let err = plan_mux(&snapshot, &stopped()).unwrap_err();

    assert!(matches!(err, RebuildError::Owned { .. }), "{err}");
}

fn live_worktree_world() -> (tempfile::TempDir, GraphSnapshot) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("app/.git")).unwrap();
    std::fs::create_dir_all(tmp.path().join("wt-feature")).unwrap();
    let common = tmp.path().join("app/.git").display().to_string();
    let repo = RepoId::new(common.clone());
    let root = tmp.path().join("wt-feature").display().to_string();
    let mut nodes = vec![
        GraphNode::Repo(RepoNode::new(repo.clone())),
        GraphNode::Checkout(
            CheckoutNode::new(CheckoutId::new(repo.clone(), root.clone()), root.clone())
                .with_worktree(WorktreeMeta::linked()),
        ),
        GraphNode::Branch(crate::model::BranchNode {
            id: crate::model::BranchId::new(repo, "refs/heads/feature/x"),
            refname: "refs/heads/feature/x".into(),
            current_commit: None,
            upstream: None,
        }),
    ];
    nodes.push(GraphNode::Checkout(
        CheckoutNode::new(
            CheckoutId::new(
                RepoId::new(common.clone()),
                tmp.path().join("app").display().to_string(),
            ),
            tmp.path().join("app").display().to_string(),
        )
        .with_worktree(WorktreeMeta::primary()),
    ));
    let history = HistorySnapshot {
        checkouts: vec![CheckoutRecord {
            meta: meta("h_wt"),
            recipe: CheckoutRecipe {
                repo: format!("repo:{common}"),
                root,
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
    (
        tmp,
        GraphSnapshot {
            nodes,
            history,
            ..GraphSnapshot::empty()
        },
    )
}

#[test]
fn a_running_worktree_is_planned_as_if_it_were_gone() {
    let (tmp, snapshot) = live_worktree_world();
    let harness = Harness::new(FakeTmux::with_sessions(""))
        .with_worktrees(FakeWorktrees::landing_at(&tmp.path().join("wt-feature")));
    let subject = resolve_subject(&snapshot, &[], "h_wt").unwrap();

    let real = harness
        .plan(&snapshot, &subject, &Options::default())
        .unwrap();
    assert!(real.steps.is_empty(), "nothing to rebuild when it exists");

    let plan = harness.plan(&snapshot, &subject, &stopped()).unwrap();

    assert_eq!(
        plan.subject,
        "worktree `wt-feature` (exists; planned as if removed)"
    );
    assert_eq!(
        plan.commands(),
        [format!(
            "wt -C {} switch --no-cd feature/x",
            tmp.path().join("wt-feature").display()
        )]
    );
    assert!(
        plan.warnings[0].starts_with("hypothetical:"),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn a_running_repo_is_planned_as_if_it_were_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("app");
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    let common = dir.join(".git").display().to_string();
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Repo(RepoNode::new(RepoId::new(common.clone())))],
        history: HistorySnapshot {
            repos: vec![RepoRecord {
                meta: meta("h_repo"),
                recipe: RepoRecipe {
                    common_dir: common,
                    remotes: vec![RecordedRemote {
                        name: "origin".into(),
                        url: "git@github.com:me/app.git".into(),
                    }],
                    primary_root: Some(dir.display().to_string()),
                    default_branch: None,
                    extra: Default::default(),
                },
                extra: Default::default(),
            }],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    };
    let harness = Harness::new(FakeTmux::with_sessions(""));
    let subject = resolve_subject(&snapshot, &[], "h_repo").unwrap();

    let plan = harness.plan(&snapshot, &subject, &stopped()).unwrap();

    assert_eq!(
        plan.commands(),
        [format!(
            "git clone git@github.com:me/app.git {}",
            dir.display()
        )]
    );
    assert!(
        plan.warnings[0].contains("git would refuse to clone over it"),
        "{:?}",
        plan.warnings
    );
}

// --- the commands are the commands a run executes ---------------------

/// A `tmux` that records its arguments, one per line, and succeeds.
fn recording_binary(dir: &Path, log: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("tmux");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn recorded(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn displayed(action: &Action) -> Vec<String> {
    action
        .argv_for_display()
        .expect("a tmux action shows a command")
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn the_tmux_commands_a_plan_shows_are_the_arguments_the_runner_executes() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("args.log");
    let tmux = SystemTmux::with_binary(recording_binary(dir.path(), &log));
    let cwd = dir.path().join("a dir");
    let argv: Vec<OsString> = ["claude", "--resume", "s 1", "it's"]
        .map(OsString::from)
        .into();

    for socket in [None, Some("side")] {
        let new_session = Action::NewSession {
            backend: "tmux".into(),
            socket: socket.map(str::to_string),
            name: "work".into(),
            cwd: cwd.clone(),
            cwd_after: None,
            argv: argv.clone(),
            resume: None,
            harness: None,
        };
        tmux.new_session(socket, "work", &cwd, &argv).unwrap();
        assert_eq!(recorded(&log), displayed(&new_session), "new-session");

        let keys = Action::SendKeys {
            backend: "tmux".into(),
            socket: socket.map(str::to_string),
            name: "work".into(),
            argv: argv.clone(),
        };
        let literal = launch::format_argv_for_send_keys(&argv);
        tmux.send_keys(socket, "work", &literal, true).unwrap();
        assert_eq!(recorded(&log), displayed(&keys), "send-keys");

        let kill = Action::KillSession {
            backend: "tmux".into(),
            socket: socket.map(str::to_string),
            name: "work".into(),
        };
        tmux.kill_session(socket, "work").unwrap();
        assert_eq!(recorded(&log), displayed(&kill), "kill-session");

        // Outside tmux the runner attaches; inside it switches clients.
        if std::env::var_os("TMUX").is_none() {
            let attach = Action::Attach {
                backend: "tmux".into(),
                socket: socket.map(str::to_string),
                name: "work".into(),
            };
            tmux.attach_session(socket, "work").unwrap();
            assert_eq!(recorded(&log), displayed(&attach), "attach-session");
        }
    }
}

#[test]
fn the_worktree_command_a_plan_shows_is_the_one_the_backend_runs() {
    use crate::discovery::worktree::{
        WorktreeBackend, WorktreeSwitchRequest, WorktrunkBackend, WtRunner,
    };
    struct Recording(Arc<Mutex<Vec<String>>>);
    impl WtRunner for Recording {
        fn run(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
            *self.0.lock().unwrap() = args.iter().map(|a| (*a).to_string()).collect();
            Ok(std::process::Output {
                status: std::process::ExitStatus::default(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }
    for (create, base) in [(false, None), (true, Some("abc123".to_string()))] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let backend = WorktrunkBackend::with_runner(Box::new(Recording(seen.clone())));
        backend
            .switch(&WorktreeSwitchRequest {
                repo_root: "/src/app".into(),
                branch: "feature/x".into(),
                create,
                base: base.clone(),
            })
            .unwrap();
        let action = Action::WorktreeSwitch {
            key: "k".into(),
            repo_root: "/src/app".into(),
            repo: "repo:/src/app/.git".into(),
            branch: "feature/x".into(),
            create,
            base,
            recorded_root: None,
            old_record: None,
        };
        assert_eq!(*seen.lock().unwrap(), displayed(&action));
    }
}

#[test]
fn every_step_command_splits_back_into_its_argv() {
    let cwd = tempfile::tempdir().unwrap();
    let snapshot = running_mux(
        cwd.path().to_str().unwrap(),
        Some("side"),
        &["nix", "develop", "-c", "claude", "--flag=a b"],
    );
    let plan = plan_mux(&snapshot, &stopped()).unwrap();
    assert!(!plan.steps.is_empty());
    for step in &plan.steps {
        let argv: Vec<String> = step
            .action
            .argv_for_display()
            .unwrap()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let parsed = crate::argv::split_command_line(&step.command);
        assert_eq!(parsed, argv, "{}", step.command);
    }
}

#[test]
fn a_running_session_is_planned_as_resumed_into_a_mux_that_had_stopped() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let mut snapshot = running_mux(cwd, None, &["claude"]);
    // The agent session knows where it ran.
    for node in &mut snapshot.nodes {
        if let GraphNode::AgentSession(session) = node {
            session.cwd = Some(cwd.to_string());
        }
    }
    let reference = NodeId::AgentSession(crate::model::AgentSessionId::new(
        "claude-code",
        "global",
        "e44d01cb",
    ))
    .to_string();
    let harness = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let subject = resolve_subject(&snapshot, &[], &reference).unwrap();

    let plan = harness.plan(&snapshot, &subject, &stopped()).unwrap();

    assert_eq!(
        plan.commands()[0],
        format!(
            "tmux new-session -d -s work -c {cwd} claude --resume e44d01cb \\; set-option -t work remain-on-exit failed"
        )
    );
}
