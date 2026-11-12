//! Live state in the text output: `conspectus table`'s `live` columns
//! over a scratch live-state store that discovery binds, read at a fixed
//! time, and `conspectus node show`'s `live state` section over a fixture
//! graph. Every store is populated with synthetic events, and every table
//! is rendered once with them and once with an empty store.

mod support;

use conspectus::config::Projection;
use conspectus::live_store::{LiveContext, LiveEvent, LiveEventKind, WaitKind};
use conspectus::model::{GraphSnapshot, NodeId};
use conspectus::output::node_show::render_node_show_at;
use conspectus::output::render::RenderOptions;
use conspectus::output::table::render_with;
use support::fixtures;
use support::replay::{ReplayWorld, TmuxReplayRow};

const MINUTE_MS: i64 = 60_000;

/// The sessions the worlds hold, every one in tmux session `editor`.
const SESSIONS: [&str; 5] = ["builder", "gone", "lost", "unheard", "waiter"];

/// A world with the sessions in [`SESSIONS`], the tmux sessions `editor`
/// (where they work) and `spare` (empty), and no live state.
fn world() -> ReplayWorld {
    let mut world = ReplayWorld::new();
    let work = world.mkdir("work");
    let spare = world.mkdir("spare");
    world.add_tmux_row(TmuxReplayRow::new("editor").with_cwd(&work));
    world.add_tmux_row(TmuxReplayRow::new("spare").with_cwd(&spare));
    for session in SESSIONS {
        world.write_claude_code_session(session, &work);
    }
    world
}

/// The read time of a test, on a whole minute so ages are whole minutes.
/// The store's writers check pids against this machine's processes and
/// collect entries by the real clock, so the events are stamped minutes
/// before the real time, and the tables are read at it.
fn read_time_ms() -> i64 {
    conspectus::live_store::now_ms().div_euclid(MINUTE_MS) * MINUTE_MS
}

/// `kind` for `session`, `minutes_ago` minutes before `now_ms`, from the
/// process `pid` (none when `None`).
fn event(
    session: &str,
    now_ms: i64,
    minutes_ago: i64,
    pid: Option<i64>,
    kind: LiveEventKind,
) -> LiveEvent {
    LiveEvent::new(
        "claude-code",
        session,
        now_ms - minutes_ago * MINUTE_MS,
        kind,
    )
    .with_context(LiveContext {
        pid,
        pid_start: pid.and_then(conspectus::live_store::process_start_time),
        ..Default::default()
    })
}

/// Live state for four of the sessions: `waiter` waits for a permission
/// with a subagent running, `builder` has worked for 12 minutes without a
/// word, nobody can check `lost`'s process, and `gone`'s process is gone.
/// `unheard` has none.
fn write_live_state(world: &ReplayWorld, now_ms: i64) {
    // One process holds one live session, so each agent is another
    // process that outlives the test: this one, and its parent.
    let this = Some(i64::from(std::process::id()));
    let parent = Some(i64::from(std::os::unix::process::parent_id()));
    for live in [
        event("waiter", now_ms, 5, this, LiveEventKind::TurnStarted),
        event(
            "waiter",
            now_ms,
            4,
            this,
            LiveEventKind::SubagentStarted {
                id: "agent-1".to_string(),
            },
        ),
        event(
            "waiter",
            now_ms,
            3,
            this,
            LiveEventKind::WaitStarted {
                agent: None,
                kind: WaitKind::Permission,
            },
        ),
        event("builder", now_ms, 12, parent, LiveEventKind::TurnStarted),
        event("lost", now_ms, 20, None, LiveEventKind::TurnStarted),
        // No process can have this pid: it is above every pid limit.
        event(
            "gone",
            now_ms,
            4,
            Some(i64::from(u32::MAX)),
            LiveEventKind::TurnStarted,
        ),
    ] {
        world.write_live_event(&live);
    }
}

fn table(
    snapshot: &GraphSnapshot,
    projection: Projection,
    columns: &[&'static str],
    now_ms: i64,
) -> String {
    render_with(
        snapshot,
        projection,
        &RenderOptions::wide()
            .with_columns(columns.to_vec())
            .with_now_epoch(Some(now_ms / 1000)),
    )
}

#[test]
fn the_live_columns_show_a_populated_store() {
    let world = world();
    let now_ms = read_time_ms();
    write_live_state(&world, now_ms);
    let snapshot = world.run().resolved;

    insta::assert_snapshot!(
        "live_sessions_table_populated_store",
        table(&snapshot, Projection::Agent, &["agent", "live"], now_ms)
    );
    insta::assert_snapshot!(
        "live_mux_table_populated_store",
        table(
            &snapshot,
            Projection::Mux,
            &["mux", "agents", "live"],
            now_ms
        )
    );
}

#[test]
fn the_live_columns_show_nothing_for_an_empty_store() {
    let world = world();
    let now_ms = read_time_ms();
    let snapshot = world.run().resolved;
    assert!(snapshot.node_live.is_empty());

    insta::assert_snapshot!(
        "live_sessions_table_empty_store",
        table(&snapshot, Projection::Agent, &["agent", "live"], now_ms)
    );
    insta::assert_snapshot!(
        "live_mux_table_empty_store",
        table(
            &snapshot,
            Projection::Mux,
            &["mux", "agents", "live"],
            now_ms
        )
    );
}

/// The fixture's evidence is from 2026-09-30; read it at noon that day.
const FIXTURE_NOW_MS: i64 = 1_790_769_600_000;

fn fixture_node(snapshot: &GraphSnapshot, display: &str) -> NodeId {
    conspectus::output::node_show::resolve_node_id(display, snapshot).expect("fixture node")
}

#[test]
fn node_show_prints_a_session_s_live_state() {
    let snapshot = fixtures::live_state_graph();
    let id = fixture_node(&snapshot, "codex:session-a");
    insta::assert_snapshot!(
        "live_node_show_session",
        render_node_show_at(&snapshot, &id, false, FIXTURE_NOW_MS)
    );
}

#[test]
fn node_show_prints_a_mux_s_counts_and_its_pane_agent() {
    let snapshot = fixtures::live_state_graph();
    let id = fixture_node(&snapshot, "tmux:main");
    insta::assert_snapshot!(
        "live_node_show_mux",
        render_node_show_at(&snapshot, &id, false, FIXTURE_NOW_MS)
    );
}

#[test]
fn node_show_prints_no_live_state_without_evidence() {
    let snapshot = fixtures::live_state_graph();
    let id = fixture_node(&snapshot, "codex:session-quiet");
    let rendered = render_node_show_at(&snapshot, &id, false, FIXTURE_NOW_MS);
    assert!(!rendered.contains("live state"), "{rendered}");
    insta::assert_snapshot!("live_node_show_no_evidence", rendered);

    let mut empty = snapshot;
    empty.node_live.clear();
    let mux = fixture_node(&empty, "tmux:main");
    let rendered = render_node_show_at(&empty, &mux, false, FIXTURE_NOW_MS);
    assert!(!rendered.contains("live state"), "{rendered}");
}
