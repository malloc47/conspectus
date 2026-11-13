use std::collections::BTreeMap;

use super::*;
use crate::model::live::{LiveBinding, LiveDetail, LiveObservation, LiveSource, Liveness};
use crate::model::{
    AgentSessionId, AgentSessionNode, GraphLink, GraphNode, GraphSnapshot, LinkEndpoint, LinkState,
    LivePane, MuxSessionNode, NodeId, Provenance, RelationKind,
};
use crate::resolve::resolve_snapshot;
use crate::tui::explorer::grid::field_row;
use crate::tui::explorer::{ExplorerInputs, NodeView, build_node_view};
use crate::tui::rows::LiveMark;
use crate::tui::theme::Theme;

/// The time the views are read at, in Unix seconds.
const NOW: i64 = 1_700_003_600;
const NOW_MS: i64 = NOW * 1000;
const MINUTE_MS: i64 = 60_000;

fn session(key: &str) -> NodeId {
    NodeId::AgentSession(AgentSessionId::new("claude-code", "/state", key))
}

fn mux_id() -> MuxSessionId {
    MuxSessionId::new("tmux:work")
}

/// `state` with `detail`, begun five minutes ago and last heard from a
/// minute ago, its process found running.
fn running(state: LiveState, detail: Option<LiveDetail>) -> LiveBinding {
    let mut observation = LiveObservation::new(state, LiveSource::Hook, NOW_MS - MINUTE_MS)
        .with_since_ms(NOW_MS - 5 * MINUTE_MS);
    if let Some(detail) = detail {
        observation = observation.with_detail(detail);
    }
    LiveBinding::new(observation, Liveness::Alive)
}

/// Two sessions in the mux `work`, and a third, `idle`, with no mux.
fn world() -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    for key in ["asks", "fails", "idle"] {
        snapshot
            .nodes
            .push(GraphNode::AgentSession(AgentSessionNode::new(
                AgentSessionId::new("claude-code", "/state", key),
                "claude-code",
            )));
    }
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            mux_id(),
            "tmux",
            "work",
        )));
    for key in ["asks", "fails"] {
        let mut hosted = GraphLink::new(
            format!("fixture:{key}-in-work"),
            session(key),
            LinkEndpoint::Node {
                id: NodeId::MuxSession(mux_id()),
            },
            RelationKind::LinkedToMux,
            Provenance::Discovered,
        );
        hosted.state = LinkState::Active;
        snapshot.candidate_links.push(hosted);
    }
    resolve_snapshot(snapshot)
}

/// [`world`] with the hosted sessions waiting and failed, and two agents
/// in panes of `work` that no session matches.
fn live_world() -> GraphSnapshot {
    let mut snapshot = world();
    snapshot.node_live = BTreeMap::from([
        (
            session("asks"),
            running(LiveState::Waiting, Some(LiveDetail::Permission)),
        ),
        (
            session("fails"),
            running(LiveState::Error, Some(LiveDetail::RateLimited)),
        ),
    ]);
    snapshot.pane_live = BTreeMap::from([
        (
            LivePane::new(mux_id(), Some("%4".into())),
            running(LiveState::Waiting, Some(LiveDetail::Input)),
        ),
        (
            LivePane::new(mux_id(), Some("%5".into())),
            running(LiveState::Starting, None),
        ),
    ]);
    snapshot
}

/// The view of `target` with the live values `snapshot`'s tables give at
/// [`NOW_MS`].
fn view(snapshot: &GraphSnapshot, target: &NodeId) -> NodeView {
    let live = LiveValues::at(snapshot, snapshot.live_tables(), NOW_MS);
    build_node_view(ExplorerInputs {
        snapshot,
        target,
        home: None,
        now: Some(NOW),
        live: &live,
    })
    .expect("the node is in the snapshot")
}

fn labels(fields: &[CoreField]) -> Vec<&'static str> {
    fields.iter().map(|field| field.label).collect()
}

fn field<'a>(view: &'a NodeView, label: &str) -> &'a CoreField {
    view.core_fields
        .iter()
        .find(|field| field.label == label)
        .unwrap_or_else(|| panic!("no `{label}` among {:?}", labels(&view.core_fields)))
}

#[test]
fn a_session_with_live_state_says_what_since_when_from_what_and_how_fresh() {
    let snapshot = live_world();
    let view = view(&snapshot, &session("asks"));
    assert_eq!(
        labels(&view.core_fields),
        [
            "id", "harness", "alias", "cwd", "status", "state", "since", "source", "evidence"
        ]
    );
    let state = field(&view, "state");
    assert_eq!(state.value, "waiting · permission");
    assert_eq!(
        state.live_mark.map(|sign| (sign.mark, sign.quiet)),
        Some((LiveMark::Waiting, false)),
        "the field leads with the row's mark"
    );
    assert!(!state.receding && !state.placeholder);
    assert_eq!(
        state.preview,
        Some(FieldValue::Glossary {
            field: "state",
            values: vec!["waiting".into(), "permission".into()],
        })
    );
    let since = field(&view, "since");
    assert_eq!(since.value, "5m ago");
    assert_eq!(
        since.preview,
        Some(FieldValue::Timestamp(NOW - 5 * 60)),
        "since previews when the state began"
    );
    assert_eq!(field(&view, "source").value, "hook");
    let evidence = field(&view, "evidence");
    assert_eq!(evidence.value, "fresh");
    assert!(!evidence.receding);
    assert_eq!(
        evidence.preview,
        Some(FieldValue::Glossary {
            field: "evidence",
            values: vec!["fresh".into()],
        })
    );
    // The full view lists them once, in the same place.
    let all = labels(view.fields(true));
    assert_eq!(all.iter().filter(|label| **label == "state").count(), 1);
}

#[test]
fn quiet_and_stale_evidence_recedes_and_stale_reads_as_the_cant_tell_mark() {
    let mut snapshot = world();
    let quiet = LiveObservation::new(
        LiveState::Working,
        LiveSource::Hook,
        NOW_MS - 11 * MINUTE_MS,
    )
    .with_detail(LiveDetail::Tool)
    .with_since_ms(NOW_MS - 30 * MINUTE_MS);
    let stale = LiveObservation::new(
        LiveState::Waiting,
        LiveSource::Hook,
        NOW_MS - 20 * MINUTE_MS,
    )
    .with_since_ms(NOW_MS - 40 * MINUTE_MS);
    snapshot.node_live = BTreeMap::from([
        (session("asks"), LiveBinding::new(quiet, Liveness::Alive)),
        (session("idle"), LiveBinding::new(stale, Liveness::Unknown)),
    ]);

    let quiet = view(&snapshot, &session("asks"));
    let state = field(&quiet, "state");
    assert_eq!(state.value, "working · tool");
    assert_eq!(
        state.live_mark.map(|sign| (sign.glyph(), sign.quiet)),
        Some(("»", true)),
        "a quiet state keeps its shape, dimmed"
    );
    assert!(state.receding, "a quiet state recedes");
    assert_eq!(field(&quiet, "since").value, "30m ago");
    assert_eq!(field(&quiet, "evidence").value, "quiet");
    assert!(field(&quiet, "evidence").receding);

    let stale = view(&snapshot, &session("idle"));
    let state = field(&stale, "state");
    assert_eq!(state.value, "unknown · stale");
    assert_eq!(state.live_mark.map(LiveSign::glyph), Some("?"));
    assert!(state.receding);
    // The read rules replaced the recorded wait, so the shown state
    // began no earlier than the latest evidence, as the CLI says.
    assert_eq!(field(&stale, "since").value, "20m ago");
    assert_eq!(field(&stale, "evidence").value, "stale");
    assert!(field(&stale, "evidence").receding);
}

#[test]
fn subagents_show_while_the_agent_runs_them_and_its_state_can_be_told() {
    let mut snapshot = world();
    let with_subagents = |state: LiveState, detail: Option<LiveDetail>| {
        let mut binding = running(state, detail);
        binding.observation = binding.observation.with_subagents(3);
        binding
    };
    snapshot.node_live = BTreeMap::from([
        (
            session("asks"),
            with_subagents(LiveState::Working, Some(LiveDetail::Generating)),
        ),
        (
            session("fails"),
            with_subagents(LiveState::Exited, Some(LiveDetail::Ended)),
        ),
        (session("idle"), running(LiveState::Ready, None)),
    ]);
    assert_eq!(
        field(&view(&snapshot, &session("asks")), "subagents").value,
        "3 active"
    );
    for key in ["fails", "idle"] {
        let view = view(&snapshot, &session(key));
        assert!(
            !labels(&view.core_fields).contains(&"subagents"),
            "{key}: {:?}",
            labels(&view.core_fields)
        );
    }
    let exited = view(&snapshot, &session("fails"));
    assert_eq!(field(&exited, "state").value, "exited · ended");
    assert_eq!(
        field(&exited, "state").live_mark.map(LiveSign::glyph),
        Some("·")
    );
    let ready = view(&snapshot, &session("idle"));
    assert_eq!(
        field(&ready, "state").value,
        "ready",
        "a state without a detail is its name alone"
    );
}

#[test]
fn a_node_without_live_evidence_lists_no_live_field() {
    let bare = world();
    let session_view = view(&bare, &session("asks"));
    assert_eq!(
        labels(&session_view.core_fields),
        ["id", "harness", "alias", "cwd", "status"]
    );
    let mux_view = view(&bare, &NodeId::MuxSession(mux_id()));
    assert_eq!(
        labels(&mux_view.core_fields),
        ["id", "backend", "cwd", "attached", "last_active"]
    );
    // Other sessions' evidence adds nothing to one without its own.
    let live = live_world();
    assert_eq!(
        labels(&view(&live, &session("idle")).core_fields),
        ["id", "harness", "alias", "cwd", "status"]
    );
}

#[test]
fn a_mux_counts_its_agents_in_rollup_order_and_lists_its_pane_agents() {
    let snapshot = live_world();
    let view = view(&snapshot, &NodeId::MuxSession(mux_id()));
    assert_eq!(
        labels(&view.core_fields),
        [
            "id",
            "backend",
            "cwd",
            "attached",
            "last_active",
            "agents",
            "pane_agents"
        ]
    );
    let agents = field(&view, "agents");
    assert_eq!(
        agents.value, "2 waiting · 1 error · 1 starting",
        "waiting, then error, then the rest, as the row's mark ranks them"
    );
    assert_eq!(
        agents.live_mark,
        Some(LiveSign::of_mux(
            snapshot.live_counts_of_mux(&mux_id(), NOW_MS)
        )),
        "the field leads with the mux row's mark"
    );
    assert_eq!(
        agents.preview,
        Some(FieldValue::Glossary {
            field: "state",
            values: vec!["waiting".into(), "error".into(), "starting".into()],
        })
    );
    let panes = field(&view, "pane_agents");
    assert_eq!(
        panes.value, "%4 waiting · input  (+1)",
        "the first and how many more, as a list field reads"
    );
    assert_eq!(
        panes.preview,
        Some(FieldValue::List(vec![
            "%4 waiting · input".into(),
            "%5 starting".into()
        ]))
    );
}

#[test]
fn a_mux_whose_agents_all_have_sessions_lists_no_pane_agents() {
    let mut snapshot = live_world();
    snapshot.pane_live.clear();
    let view = view(&snapshot, &NodeId::MuxSession(mux_id()));
    assert_eq!(field(&view, "agents").value, "1 waiting · 1 error");
    assert!(!labels(&view.core_fields).contains(&"pane_agents"));
}

#[test]
fn a_live_field_draws_the_rows_mark_and_recedes_when_its_evidence_is_not_fresh() {
    let theme = Theme::default();
    let mut snapshot = world();
    let quiet = LiveObservation::new(
        LiveState::Working,
        LiveSource::Hook,
        NOW_MS - 11 * MINUTE_MS,
    );
    snapshot.node_live = BTreeMap::from([
        (
            session("asks"),
            running(LiveState::Waiting, Some(LiveDetail::Question)),
        ),
        (session("fails"), LiveBinding::new(quiet, Liveness::Alive)),
    ]);
    let line = |key: &str| {
        let view = view(&snapshot, &session(key));
        let row = field_row(field(&view, "state"), &theme);
        crate::tui::explorer::grid::grid(&theme)
            .row(row)
            .lines(80, None, false, &theme)
            .lines
            .remove(0)
    };
    let waiting = line("asks");
    let text: String = waiting
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert!(
        text.contains("! waiting · question"),
        "the mark leads the value: {text:?}"
    );
    let mark = waiting
        .spans
        .iter()
        .find(|span| span.content == "!")
        .expect("the mark");
    assert_eq!(mark.style, LiveMark::Waiting.style(&theme));
    let value = waiting
        .spans
        .iter()
        .find(|span| span.content.contains("waiting"))
        .expect("the value");
    assert!(!value.style.add_modifier.contains(theme.muted));

    let quiet = line("fails");
    let value = quiet
        .spans
        .iter()
        .find(|span| span.content.contains("working"))
        .expect("the value");
    assert!(
        value.style.add_modifier.contains(theme.muted),
        "a quiet state's words recede"
    );
    let mark = quiet
        .spans
        .iter()
        .find(|span| span.content == "»")
        .expect("the mark");
    assert!(mark.style.add_modifier.contains(theme.muted));
}
