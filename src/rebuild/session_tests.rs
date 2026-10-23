use super::super::resolve_subject;
use super::super::test_support::{Harness, attach, titles};
use super::*;
use crate::discovery::tmux::{FakeTmux, TmuxPaneStatus};
use crate::model::{
    AgentSessionNode, ArgvSource, GraphLink, HistorySnapshot, HostedSession, LinkEndpoint,
    MuxIncarnation, MuxRecipe, MuxSessionId, MuxSessionNode, NodePresence, Provenance, RecordMeta,
};

const NOW: i64 = 1_790_000_000;

fn session_node(key: &str, cwd: &str) -> GraphNode {
    GraphNode::AgentSession(
        crate::model::AgentSessionNode::new(
            AgentSessionId::new("claude-code", "global", key),
            "claude-code",
        )
        .with_cwd(cwd),
    )
}

fn mux_record(id: &str, name: &str, cwd: &str, hosted: &[(&str, i64)]) -> MuxRecord {
    MuxRecord {
        meta: RecordMeta {
            id: id.into(),
            key: format!("mux_session:tmux:{name}"),
            first_seen: 1,
            last_seen: NOW - 86_400,
            origin: Default::default(),
            display: Default::default(),
            rebuilt_from: None,
        },
        recipe: MuxRecipe {
            backend: "tmux".into(),
            name: name.into(),
            cwd: Some(cwd.into()),
            incarnation: Some(MuxIncarnation {
                session_id: Some(format!("${id}")),
                created: Some(NOW - 50_000),
            }),
            argv: Some(vec!["claude".into()]),
            argv_source: Some(ArgvSource::Conspectus),
            harness: Some("claude-code".into()),
            hosted: hosted
                .iter()
                .map(|(session, seen)| HostedSession {
                    harness: "claude-code".into(),
                    session_id: (*session).into(),
                    first_seen: 1,
                    last_seen: *seen,
                })
                .collect(),
            ..Default::default()
        },
        extra: Default::default(),
    }
}

fn parent_link(child: &str, parent: &str) -> GraphLink {
    GraphLink::new(
        format!("lineage:{child}"),
        NodeId::AgentSession(AgentSessionId::new("claude-code", "global", child)),
        LinkEndpoint::Node {
            id: NodeId::AgentSession(AgentSessionId::new("claude-code", "global", parent)),
        },
        RelationKind::ParentSession,
        Provenance::Discovered,
    )
}

fn subject_for(snapshot: &GraphSnapshot, key: &str) -> crate::rebuild::Subject {
    let id = NodeId::AgentSession(AgentSessionId::new("claude-code", "global", key));
    resolve_subject(snapshot, &[], &id.to_string()).unwrap()
}

fn ghost(snapshot: &mut GraphSnapshot, id: &str) {
    snapshot.history.presence.insert(
        id.into(),
        NodePresence::Gone {
            last_seen: NOW - 86_400,
        },
    );
}

#[test]
fn a_session_resumes_into_the_ghost_mux_it_last_ran_in_at_its_lineage_head() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    // `old` compacted into `head`, but the mux only ever hosted `old`.
    let mut snapshot = GraphSnapshot {
        nodes: vec![session_node("old", cwd), session_node("head", cwd)],
        candidate_links: vec![parent_link("head", "old")],
        history: HistorySnapshot {
            muxes: vec![mux_record("h_a", "work", cwd, &[("old", NOW - 100_000)])],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    };
    ghost(&mut snapshot, "h_a");
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = h
        .plan(&snapshot, &subject_for(&snapshot, "old"), &attach())
        .unwrap();

    assert!(
        plan.subject
            .starts_with("session `old` (live) into mux `work` (stopped"),
        "{}",
        plan.subject
    );
    assert_eq!(titles(&plan), ["new session, resume head", "attach"]);
    h.run(&plan).unwrap();
    let calls = h.tmux.new_session_calls();
    assert_eq!(calls[0].1, "work");
    assert_eq!(
        calls[0]
            .3
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["claude", "--resume", "head"]
    );
}

fn busy_mux_world(cwd: &str) -> GraphSnapshot {
    let mut mux = MuxSessionNode::new(MuxSessionId::new("tmux:work"), "tmux", "work");
    mux.cwd = Some(cwd.into());
    mux.backend_session_id = Some("$h_a".into());
    mux.created_epoch = Some(NOW - 50_000);
    mux.active_pane_command = Some("claude".into());
    GraphSnapshot {
        nodes: vec![
            session_node("mine", cwd),
            session_node("other", cwd),
            GraphNode::MuxSession(mux),
        ],
        history: HistorySnapshot {
            muxes: vec![mux_record("h_a", "work", cwd, &[("mine", NOW - 100_000)])],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    }
}

#[test]
fn a_busy_last_mux_gets_a_new_mux_instead_of_typing_into_a_live_agent() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let snapshot = busy_mux_world(cwd);
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = h
        .plan(&snapshot, &subject_for(&snapshot, "mine"), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["new session, resume mine", "attach"]);
    h.run(&plan).unwrap();
    assert_eq!(
        h.tmux.new_session_calls()[0].1,
        "work-2",
        "a new, distinct name"
    );
    assert!(h.tmux.send_keys_calls().is_empty());
}

#[test]
fn an_idle_last_mux_gets_the_resume_typed_into_its_pane() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let mut snapshot = busy_mux_world(cwd);
    if let GraphNode::MuxSession(mux) = &mut snapshot.nodes[2] {
        mux.active_pane_command = Some("zsh".into());
    }
    let h = Harness::new(
        FakeTmux::with_sessions("").with_pane_statuses("work", [TmuxPaneStatus::Alive]),
    );
    let plan = h
        .plan(&snapshot, &subject_for(&snapshot, "mine"), &attach())
        .unwrap();
    assert_eq!(titles(&plan), ["relaunch the agent", "attach"]);
    h.run(&plan).unwrap();
    assert_eq!(h.tmux.send_keys_calls()[0].2, "claude --resume mine");
}

#[test]
fn a_session_that_never_had_a_mux_gets_one_named_after_it_in_its_own_cwd() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let snapshot = GraphSnapshot {
        nodes: vec![session_node("e44d01cb-1111", cwd)],
        ..GraphSnapshot::empty()
    };
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = h
        .plan(
            &snapshot,
            &subject_for(&snapshot, "e44d01cb-1111"),
            &attach(),
        )
        .unwrap();
    assert_eq!(titles(&plan), ["new session, resume e44d01cb", "attach"]);
    h.run(&plan).unwrap();
    let calls = h.tmux.new_session_calls();
    assert_eq!(calls[0].1, "claude-code-e44d01cb");
    assert_eq!(calls[0].2.to_str().unwrap(), cwd);
}

#[test]
fn a_session_with_no_directory_cannot_get_a_mux() {
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::AgentSession(AgentSessionNode::new(
            AgentSessionId::new("claude-code", "global", "x"),
            "claude-code",
        ))],
        ..GraphSnapshot::empty()
    };
    let h = Harness::new(FakeTmux::with_sessions(""));
    let err = h
        .plan(&snapshot, &subject_for(&snapshot, "x"), &attach())
        .unwrap_err();
    assert!(
        err.to_string().contains("no recorded working directory"),
        "{err}"
    );
}

#[test]
fn a_slot_can_choose_the_mux_to_resume_into() {
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let mut snapshot = GraphSnapshot {
        nodes: vec![session_node("s1", cwd)],
        history: HistorySnapshot {
            muxes: vec![
                mux_record("h_a", "auto", cwd, &[("s1", NOW - 10)]),
                mux_record("h_b", "chosen", cwd, &[]),
            ],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    };
    ghost(&mut snapshot, "h_a");
    ghost(&mut snapshot, "h_b");
    let mut subject = subject_for(&snapshot, "s1");
    subject.slot.overrides.insert(
        "resume_into".into(),
        crate::model::FieldValue::Text("chosen".into()),
    );
    let h = Harness::new(FakeTmux::with_sessions(""));
    let plan = h.plan(&snapshot, &subject, &Options::default()).unwrap();
    h.run(&plan).unwrap();
    assert_eq!(h.tmux.new_session_calls()[0].1, "chosen");
}

#[test]
fn any_session_a_mux_hosted_can_be_rebuilt_from_the_mux() {
    // "Rebuild with this session": the mux rebuild with --session.
    let cwd = tempfile::tempdir().unwrap();
    let cwd = cwd.path().to_str().unwrap();
    let mut snapshot = GraphSnapshot {
        nodes: vec![session_node("a", cwd), session_node("b", cwd)],
        history: HistorySnapshot {
            muxes: vec![mux_record(
                "h_a",
                "work",
                cwd,
                &[("a", NOW - 200), ("b", NOW - 100)],
            )],
            ..Default::default()
        },
        ..GraphSnapshot::empty()
    };
    ghost(&mut snapshot, "h_a");
    let subject = resolve_subject(&snapshot, &[], "h_a").unwrap();
    let h = Harness::new(FakeTmux::with_sessions(""));
    let options = Options {
        session: Some("a".into()),
        attach: false,
        ..Default::default()
    };
    let plan = h.plan(&snapshot, &subject, &options).unwrap();
    assert_eq!(titles(&plan), ["new session, resume a"]);
}
