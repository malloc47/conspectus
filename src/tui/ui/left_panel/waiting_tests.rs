//! A group header counts the agents beneath it that wait on the
//! operator: each entity once, through the rows of its view's own kind
//! (sessions, or in the Mux view its muxes), drawn as `! N` after the
//! session count in one column across the tree's headers.

use super::*;
use crate::model::live::{LiveConfidence, LiveCounts, LiveState};
use crate::model::{AgentSessionId, MuxSessionId};
use crate::tui::rows::{
    AgentSessionRow, GroupRow, LiveCell, MixedChip, MixedLive, MixedRow, MixedTone, MuxSessionRow,
    PinMarker, RowTree, ViewLabel,
};

fn header(path: &str, depth: u8) -> Row {
    Row {
        id: RowId::Bucket(path.to_string()),
        depth,
        expandable: true,
        kind: RowKind::Group(GroupRow::at_path(path, None)),
    }
}

fn cell(state: LiveState) -> LiveCell {
    LiveCell {
        state,
        detail: None,
        confidence: LiveConfidence::Fresh,
        since_ms: 0,
        subagents: 0,
        source: crate::model::live::LiveSource::Hook,
        unseen: false,
    }
}

/// A session row for `key` at `depth`, in `state` (`None`: no evidence).
fn session(key: &str, depth: u8, state: Option<LiveState>) -> Row {
    let id = AgentSessionId::new("claude-code", "/state", key);
    Row {
        id: RowId::AgentSession(NodeId::AgentSession(id.clone())),
        depth,
        expandable: false,
        kind: RowKind::AgentSession(AgentSessionRow {
            session: id.clone(),
            short_id: key.into(),
            harness_label: "claude".into(),
            cwd_display: None,
            project_display: None,
            recency: None,
            activity_epoch: None,
            mux_state: MuxIndicator::Unmuxed,
            preview: None,
            title: None,
            alias: None,
            title_disambiguates: false,
            mux_host: None,
            primary_node: NodeId::AgentSession(id),
            pin_id: None,
            content_hit: None,
            live: state.map(cell),
        }),
    }
}

fn counts(states: &[LiveState]) -> LiveCounts {
    states.iter().copied().collect()
}

/// A running mux row named `name` at `depth` whose agents are in `states`.
fn mux(name: &str, depth: u8, states: &[LiveState]) -> Row {
    let id = MuxSessionId::new(format!("tmux:{name}"));
    Row {
        id: RowId::MuxSession(NodeId::MuxSession(id.clone())),
        depth,
        expandable: false,
        kind: RowKind::MuxSession(MuxSessionRow {
            mux: id.clone(),
            backend: "tmux".into(),
            native_id: name.into(),
            client_attached: Some(false),
            cwd_display: None,
            attached_count: states.len(),
            recency: Some("1m".into()),
            activity_epoch: None,
            created_epoch: None,
            last_attached_epoch: None,
            agent_labels: vec!["claude".into()],
            program: None,
            program_harness: None,
            single_session_preview: None,
            pin_id: None,
            primary_node: NodeId::MuxSession(id),
            content_hit: None,
            live: counts(states).into(),
        }),
    }
}

/// A stopped mux's row in the Mux view, still counting `states`.
fn stopped_mux(name: &str, depth: u8, states: &[LiveState]) -> Row {
    let id = NodeId::MuxSession(MuxSessionId::new(format!("tmux:{name}")));
    Row {
        id: RowId::MuxSession(id.clone()),
        depth,
        expandable: false,
        kind: RowKind::Mixed(MixedRow {
            node_kind: crate::model::NodeKind::MuxSession,
            label: name.into(),
            status: "stopped".into(),
            tone: MixedTone::Dim,
            age: Some("2d".into()),
            age_epoch: None,
            state: Some(crate::tui::rows::EntityState::Stopped),
            chip: Some(MixedChip::agent("claude")),
            content: "~/src".into(),
            primary_node: id,
            pr_status: None,
            marker: PinMarker::None,
            live: MixedLive::Mux(counts(states).into()),
        }),
    }
}

fn summaries(view: ViewLabel, rows: Vec<Row>) -> Vec<GroupSummary> {
    let tree = RowTree {
        rows,
        view,
        ..RowTree::default()
    };
    let summaries = compute_group_summaries(&tree);
    tree.rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::Group(_)))
        .map(|row| summaries[&row.id])
        .collect()
}

fn text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn a_header_counts_the_sessions_beneath_it_that_wait_each_once() {
    use LiveState::{Ready, Waiting, Working};
    // The same waiting session listed twice beneath one header (under two
    // pull requests of one checkout) counts once; the session count, which
    // counts rows, is unchanged by this rule.
    let rows = vec![
        header("~/src/project", 0),
        session("aaaa", 2, Some(Waiting)),
        session("bbbb", 2, Some(Ready)),
        session("aaaa", 2, Some(Waiting)),
        session("cccc", 2, Some(Waiting)),
        session("dddd", 2, Some(Working)),
        session("eeee", 2, None),
    ];
    for view in [ViewLabel::Sessions, ViewLabel::Prs, ViewLabel::Search] {
        let [summary] = summaries(view, rows.clone())[..] else {
            panic!("one header");
        };
        assert_eq!(summary.waiting, 2, "{view:?}");
        assert_eq!(summary.agents, 6, "{view:?}");
    }
}

#[test]
fn nested_headers_each_count_what_is_beneath_them() {
    let waiting = Some(LiveState::Waiting);
    let rows = vec![
        header("~/src/project", 0),
        header("~/src/project/a", 1),
        session("aaaa", 2, waiting),
        header("~/src/project/b", 1),
        session("bbbb", 2, waiting),
        session("cccc", 2, Some(LiveState::Error)),
        header("~/src/other", 0),
        session("dddd", 1, None),
    ];
    let waiting: Vec<usize> = summaries(ViewLabel::Sessions, rows)
        .iter()
        .map(|summary| summary.waiting)
        .collect();
    assert_eq!(waiting, [2, 1, 1, 0]);
}

#[test]
fn a_mux_view_header_counts_the_agents_its_muxes_host() {
    use LiveState::{Error, Ready, Waiting, Working};
    // A mux counts every agent it hosts, its nested agents among them, so
    // the nested rows add nothing; a mux whose one agent waits, and a
    // stopped mux that still counts a waiting agent, count without a row
    // for the agent.
    let rows = vec![
        header("~/src/project", 0),
        mux("main", 1, &[Waiting, Waiting, Working]),
        session("aaaa", 2, Some(Waiting)),
        session("bbbb", 2, Some(Waiting)),
        session("cccc", 2, Some(Working)),
        mux("solo", 1, &[Waiting]),
        mux("idle", 1, &[Ready, Error]),
        stopped_mux("old", 1, &[Waiting]),
        header("~/src/other", 0),
        mux("single", 1, &[Waiting]),
    ];
    let found = summaries(ViewLabel::Mux, rows);
    assert_eq!(found[0].waiting, 4);
    assert_eq!(found[0].agents, 3, "the session count still counts rows");
    assert_eq!(found[1].waiting, 1);
    assert_eq!(found[1].agents, 0, "no session row, no session count");
}

#[test]
fn pinned_and_history_buckets_count_nothing() {
    let pinned_session = Row {
        kind: RowKind::Mixed(MixedRow {
            node_kind: crate::model::NodeKind::AgentSession,
            label: "parser".into(),
            status: "in mux".into(),
            tone: MixedTone::Normal,
            age: None,
            age_epoch: None,
            state: None,
            chip: None,
            content: String::new(),
            primary_node: NodeId::AgentSession(AgentSessionId::new("claude-code", "/s", "p")),
            pr_status: None,
            marker: PinMarker::Running,
            live: MixedLive::Session(cell(LiveState::Waiting)),
        }),
        ..session("pppp", 1, None)
    };
    for view in [ViewLabel::Pinned, ViewLabel::History] {
        let rows = vec![
            header("Sessions", 0),
            pinned_session.clone(),
            stopped_mux("old", 1, &[LiveState::Waiting]),
        ];
        assert_eq!(summaries(view, rows)[0].waiting, 0, "{view:?}");
    }
}

#[test]
fn the_count_follows_the_session_count_in_the_waiting_mark_style() {
    let theme = Theme::default();
    let rows = vec![
        header("~/src/project", 0),
        session("aaaa", 1, Some(LiveState::Waiting)),
        session("bbbb", 1, Some(LiveState::Waiting)),
        session("cccc", 1, Some(LiveState::Ready)),
    ];
    let line = lay_out(&rows, &theme, 0, 120, None).remove(0);
    let drawn = text(&line);
    assert!(drawn.trim_end().ends_with("(3)  ! 2"), "{drawn:?}");
    let chip = line
        .spans
        .iter()
        .find(|span| span.content.contains('!'))
        .expect("the count");
    assert_eq!(chip.content, "! 2", "the mark and the count in one chip");
    assert_eq!(
        chip.style,
        crate::tui::rows::LiveMark::Waiting.style(&theme),
        "the waiting mark style"
    );
}

#[test]
fn a_tree_where_nothing_waits_draws_its_headers_as_before() {
    let theme = Theme::default();
    let quiet = vec![
        header("~/src/project", 0),
        session("aaaa", 1, Some(LiveState::Ready)),
        header("~/src/other", 0),
        session("bbbb", 1, None),
    ];
    let without_state: Vec<Row> = quiet
        .iter()
        .cloned()
        .map(|mut row| {
            if let RowKind::AgentSession(session) = &mut row.kind {
                session.live = None;
            }
            row
        })
        .collect();
    let draw = |rows: &[Row]| -> Vec<String> {
        lay_out(rows, &theme, 0, 120, None)
            .iter()
            .step_by(2)
            .map(text)
            .collect()
    };
    assert_eq!(
        draw(&quiet),
        draw(&without_state),
        "no header reserves the column"
    );
}

#[test]
fn every_headers_count_and_warning_line_up() {
    let theme = Theme::default();
    let ambiguous = MuxIndicator::Ambiguous { candidate_count: 2 };
    let mut rows = vec![header("~/src/project", 0)];
    rows.extend((0..12).map(|n| session(&format!("w{n:03}"), 1, Some(LiveState::Waiting))));
    rows.push(header("~/src/b", 0));
    rows.push(session("x001", 1, Some(LiveState::Waiting)));
    rows.push(header("~/src/c", 0));
    let mut odd = session("x002", 1, None);
    if let RowKind::AgentSession(session) = &mut odd.kind {
        session.mux_state = ambiguous;
    }
    rows.push(odd);
    let lines: Vec<String> = lay_out(&rows, &theme, 0, 120, None)
        .iter()
        .zip(&rows)
        .filter(|(_, row)| matches!(row.kind, RowKind::Group(_)))
        .map(|(line, _)| text(line))
        .collect();
    let column = |line: &str, needle: char| {
        line.chars()
            .position(|c| c == needle)
            .unwrap_or_else(|| panic!("no {needle} in {line:?}"))
    };
    assert_eq!(column(&lines[0], '!'), column(&lines[1], '!'), "{lines:#?}");
    assert!(lines[0].contains("(12)  ! 12"), "{lines:#?}");
    assert!(lines[1].contains("(1)  ! 1"), "{lines:#?}");
    // A header with no waiting agent keeps the column blank, so its
    // warning stands where every warning does.
    assert!(!lines[2].contains('!'), "{lines:#?}");
    assert!(
        column(&lines[2], '▲') > column(&lines[0], '!') + "! 12".len(),
        "{lines:#?}"
    );
}

#[test]
fn the_count_never_gives_way_on_a_narrow_pane() {
    let theme = Theme::default();
    let rows = vec![
        header("~/a/rather/long/path/to/the/project", 0),
        session("aaaa", 1, Some(LiveState::Waiting)),
    ];
    for width in [60, 40, 30, 24] {
        let drawn = text(&lay_out(&rows, &theme, 0, width, None)[0]);
        assert!(
            drawn.trim_end().ends_with("(1)  ! 1"),
            "at {width}: {drawn:?}"
        );
    }
}

#[test]
fn a_mux_view_header_shows_its_count_without_a_session_count() {
    let theme = Theme::default();
    let rows = vec![
        header("~/src/project", 0),
        mux("solo", 1, &[LiveState::Waiting]),
    ];
    let drawn = text(&lay_out_in(ViewLabel::Mux, &rows, &theme, 0, 120, None)[0]);
    assert!(
        drawn.trim_end().ends_with("~/src/project  ! 1"),
        "{drawn:?}"
    );
    assert!(!drawn.contains('('), "{drawn:?}");
}
