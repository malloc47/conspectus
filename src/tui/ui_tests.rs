use super::*;
use crate::filter::RowFilter;
use crate::model::{
    AgentSessionId, AgentSessionNode, CheckoutId, CheckoutNode, GraphNode, GraphSnapshot, RepoId,
    RepoNode,
};
use crate::resolve::resolve_snapshot;
use crate::tui::SessionsGrouping;
use crate::tui::app::Msg;
use crate::tui::preview::PreviewSource;
use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
use crate::tui::{RunConfig, View};
use ratatui::layout::{Constraint, Direction, Layout};

fn seeded_app() -> App {
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_title("Phase 8 walkthrough".to_string())
        .with_last_message_preview("could you give me a bit more context?".to_string()),
    ));
    let snapshot = resolve_snapshot(snapshot);

    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn muxed_app(native_id: &str, capture: Option<&str>) -> App {
    muxed_app_with_links(native_id, capture, Vec::new())
}

/// `muxed_app` plus extra candidate links, added before resolution.
fn muxed_app_with_links(
    native_id: &str,
    capture: Option<&str>,
    extra_links: Vec<crate::model::GraphLink>,
) -> App {
    // These rows preview their pane; sessions default to transcripts.
    muxed_app_previewing(
        native_id,
        capture,
        extra_links,
        crate::tui::SessionPreview::Pane,
    )
}

/// A codex session (`codex:/state:abc`) in the tmux session
/// `native_id`, selected, with `session_preview` chosen.
fn muxed_app_previewing(
    native_id: &str,
    capture: Option<&str>,
    extra_links: Vec<crate::model::GraphLink>,
    session_preview: crate::tui::SessionPreview,
) -> App {
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind,
    };

    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_last_message_preview("stale msg".to_string()),
    ));
    let mux_graph_id = MuxSessionId::new(native_id);
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            mux_graph_id.clone(),
            "tmux".to_string(),
            native_id.to_string(),
        )));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = NodeId::MuxSession(mux_graph_id.clone());
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: session_id,
        target: LinkEndpoint::Node { id: mux_id },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    });
    snapshot.candidate_links.extend(extra_links);
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    config.default_session_preview = session_preview;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::NavDown);
    if let Some(capture) = capture {
        app.update(Msg::SetPreview {
            source: PreviewSource::Pane(mux_graph_id),
            content: PreviewContent::text(capture),
        });
    }
    app
}

/// The slot `ingest`: a mux slot with harness and cwd overrides,
/// pointing at `target` (or planned when `None`).
fn ingest_slot(target: Option<NodeId>) -> crate::model::PinSlot {
    use crate::model::{FieldValue, PinKind, PinSlot, SlotFields};
    PinSlot {
        id: "ingest".to_string(),
        kind: PinKind::Mux,
        target: target.map(|node| node.to_string()),
        pinned_at: None,
        overrides: SlotFields::from([
            ("name".to_string(), FieldValue::Text("ingest".to_string())),
            ("harness".to_string(), FieldValue::Text("codex".to_string())),
            (
                "cwd".to_string(),
                FieldValue::Text("/home/op/src/proj".to_string()),
            ),
        ]),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }
}

fn pinned_app(target: Option<NodeId>) -> App {
    pinned_app_with(target, |_| {})
}

fn pinned_app_with(target: Option<NodeId>, extend: impl FnOnce(&mut GraphSnapshot)) -> App {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(ingest_slot(target));
    extend(&mut snapshot);
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let tree =
        crate::tui::rows::pinned::build_pinned_tree(crate::tui::rows::pinned::PinnedBuildInputs {
            snapshot: &snapshot,
            home: Some(std::path::Path::new("/home/op")),
            now: None,
            grouping: crate::tui::PinnedGrouping::None,
            sort: crate::tui::Sort::Hierarchy,
        });
    let mut config = RunConfig::defaults();
    config.default_view = View::Pinned;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: crate::tui::app::SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    // Select the slot row.
    let pin_row = app
        .tree()
        .rows
        .iter()
        .find(|r| matches!(&r.id, crate::tui::rows::RowId::Pin { .. }))
        .map(|r| r.id.clone())
        .expect("pin row emitted");
    app.set_selection(pin_row);
    app
}

#[test]
fn status_hint_for_a_planned_pin_points_at_rebuild() {
    let app = pinned_app(None);
    let hint = default_action_status_hint(&app);
    assert!(
        hint.contains("rebuild ingest") && hint.contains("isn't built yet"),
        "unexpected hint for planned pin: {hint}"
    );
}

#[test]
fn status_hint_for_a_pin_with_a_missing_target_names_the_fixes() {
    let app = pinned_app(Some(NodeId::MuxSession(crate::model::MuxSessionId::new(
        "tmux:gone",
    ))));
    let hint = default_action_status_hint(&app);
    assert!(
        hint.contains("target not found") && hint.contains("pin retarget ingest"),
        "unexpected hint for missing target: {hint}"
    );
}

#[test]
fn live_pin_placeholder_previews_its_live_pane() {
    let mux = crate::model::MuxSessionId::new("tmux:ingest");
    let mut app = pinned_app_with(Some(NodeId::MuxSession(mux.clone())), |snapshot| {
        snapshot
            .nodes
            .push(GraphNode::MuxSession(crate::model::MuxSessionNode::new(
                mux.clone(),
                "tmux",
                "ingest",
            )));
    });

    let target = resolve_attach_target(&app).expect("pin row targets its live mux");
    assert_eq!(target.native_id, "ingest");
    app.update(Msg::SetPreview {
        source: PreviewSource::Pane(mux),
        content: PreviewContent::text("serving on :8080"),
    });
    let preview = preview_text_for_selection(&app, 80, 10).to_string();
    assert!(preview.contains("serving on :8080"), "{preview}");
}

#[test]
fn planned_pin_placeholder_previews_its_status() {
    let app = pinned_app(None);

    assert!(resolve_attach_target(&app).is_err());
    let preview = preview_text_for_selection(&app, 80, 10).to_string();
    assert!(
        preview.contains("Pin `ingest` (mux · planned)"),
        "{preview}"
    );
    assert!(preview.contains("rebuild ingest"), "{preview}");
}

#[test]
fn render_at_default_size_shows_header_tree_and_detail() {
    let mut app = seeded_app();
    // Auto-selection lands on the first visible row (the repo
    // group); step down once so the right panel shows the
    // session's detail, which is what the operator-facing
    // assertions below cover.
    app.update(Msg::NavDown);

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);

    assert!(
        text.contains("sessions"),
        "session count word missing: {text}"
    );
    // Count wording switched from "N agents" to
    // "N sessions" so the header vocabulary matches the rest
    // of the TUI.
    assert!(text.contains("1 sessions"), "session count missing: {text}");
    assert!(
        text.contains("~/src/proj"),
        "shortened path missing: {text}"
    );
    assert!(
        text.contains("codex"),
        "session harness label missing: {text}"
    );
    assert!(
        text.contains("could you give me a"),
        "same-line preview missing: {text}"
    );
    assert!(
        text.contains("Phase 8 walkthrough"),
        "right-panel title row missing: {text}"
    );
    assert!(text.contains(" Preview "), "preview chip missing: {text}");
}

#[test]
fn header_drops_brand_view_label_and_state_chips_by_default() {
    // The audit deleted the `Conspectus` brand and
    // `sessions` view-label words from the header prefix
    // (duplicated by the left-panel title strip), made the
    // harness chips opt-in (see the `show_harness_chips`
    // variant below), and collapsed the three-bucket mux chip
    // section to a single `▲ N` chip that only renders when
    // N > 0. With the showcase fixture having zero ambiguous
    // rows, the header should now read approximately
    // `updated Ns ago · N/M sessions · M mux` with no chips.
    let mut app = seeded_app();
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let header = text.lines().next().expect("header line");

    assert!(
        !header.contains("Conspectus"),
        "brand should be dropped: {header}",
    );
    assert!(
        header.contains("sessions"),
        "session count label present: {header}",
    );
    assert!(
        !header.contains("agents"),
        "Atelier-era `agents` word should be replaced with `sessions`: {header}",
    );
    assert!(header.contains("mux"), "mux count missing: {header}");
    // Old chip vocabulary should not render — both the harness
    // pill text and the three-bucket mux glyphs.
    assert!(
        !header.contains("◉") && !header.contains("◐") && !header.contains("◯"),
        "three-bucket mux chips should be gone post-audit: {header}",
    );
}

#[test]
fn header_fits_at_narrow_width_post_audit() {
    // The bare header (ADR 0078) is short enough to fit
    // comfortably at 80 cols (and even at 40 cols with
    // truncation).
    let mut app = seeded_app();
    let area = Rect::new(0, 0, 80, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let header = text.lines().next().expect("header line");
    assert!(
        header.contains("sessions") && header.contains("mux"),
        "narrow header keeps the count signals: {header}",
    );
    // The render area is 80 cells wide; the rendered header
    // (post-trim) should be far shorter than that.
    let rendered = header.trim_end();
    assert!(
        rendered.len() < 80,
        "narrow header should fit comfortably: {rendered:?}",
    );
}

#[test]
fn header_shows_harness_chips_when_opt_in_is_set() {
    // Per-harness count chips render
    // only when `[tui] show_harness_chips = true` is set in the
    // operator's config. Default-off seeded_app + a separately
    // seeded opt-in app exercise both paths.
    let mut opt_in = seeded_app_with_harness_chips();
    let area = Rect::new(0, 0, 200, 24);
    let buffer = render_to_buffer(&mut opt_in, area);
    let text = buffer_to_string(&buffer);
    let header = text.lines().next().expect("header line");
    assert!(
        header.contains("codex"),
        "harness chip should render when opt-in: {header}",
    );
}

/// Seed a test App with `[tui] show_harness_chips = true`.
fn seeded_app_with_harness_chips() -> App {
    // Reproduce the seeded_app data path but flip the opt-in.
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_title("Phase 8 walkthrough".to_string())
        .with_last_message_preview("could you give me a bit more context?".to_string()),
    ));
    let snapshot = resolve_snapshot(snapshot);

    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    config.show_harness_chips = true;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

#[test]
fn left_panel_title_renders_a_view_tab_strip() {
    // The left pane title lists the surfaced views as a tab strip
    // with the active one accented. Only Sessions and Mux are
    // surfaced (Union/PRs/Forks are hidden), so the
    // strip carries exactly those two.
    let mut app = seeded_app();
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let title_line = text
        .lines()
        .find(|l| l.contains("sessions") && l.contains("mux"))
        .expect("left pane tab strip line present");
    for label in ["sessions", "mux"] {
        assert!(
            title_line.contains(label),
            "tab strip missing `{label}`: {title_line}",
        );
    }
    for label in ["union", "prs", "forks"] {
        assert!(
            !title_line.contains(label),
            "tab strip should not list hidden view `{label}`: {title_line}",
        );
    }
}

#[test]
fn right_panel_title_names_the_selected_node_kind() {
    // Phase 11: instead of a constant `detail`, the right pane
    // title carries the kind of node currently being inspected
    // — `session` for an agent session, `repo` / `mux` / `pr` /
    // etc. for other kinds — so the operator can tell what
    // they're looking at without re-reading the body.
    let mut app = seeded_app();
    app.update(Msg::NavDown);
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let title_line = text
        .lines()
        .find(|l| l.contains("session ") && l.contains("───"))
        .expect("right-pane title with session kind");
    assert!(
        !title_line.contains("detail"),
        "right title should drop the generic `detail` label: {title_line}",
    );
}

#[test]
fn focus_marker_prefixes_only_the_active_pane_title() {
    // Phase 9 refinement: focus is signaled by a `► ` glyph on
    // the active pane's title rather than by holistically
    // styling the panel. The inactive pane gets two-space
    // padding so titles align column-wise and content colors
    // stay untouched between focus states. The right-pane label
    // varies by selection kind (Phase 11) so the assertion is
    // on the *count* of `►` markers, not on a specific suffix.
    let area = Rect::new(0, 0, 160, 24);
    let mut app = seeded_app();
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert_eq!(
        text.matches('►').count(),
        1,
        "exactly one focus marker should be visible: {text}",
    );
    let header_band = text.lines().take(3).collect::<Vec<_>>().join("\n");
    let left_border = header_band
        .lines()
        .nth(1)
        .map_or("", |l| l.split('│').next().unwrap_or(""));
    assert!(
        left_border.contains('►'),
        "marker should sit in the left pane's title when left is focused: \
             left border was `{left_border}` and full header was:\n{header_band}",
    );

    let mut app = seeded_app();
    app.update(Msg::CycleFocus);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert_eq!(
        text.matches('►').count(),
        1,
        "exactly one focus marker should still be visible after CycleFocus: {text}",
    );
    // After CycleFocus the marker moves from the left tab strip
    // to the right pane title — its column position shifts past
    // the panel split (well to the right of column 0).
    let header_line = text.lines().nth(1).expect("header line");
    let marker_col = header_line.find('►').expect("marker present");
    assert!(
        marker_col > 60,
        "marker should sit in the right pane after CycleFocus (col={marker_col}): {header_line}",
    );
}

fn two_repo_app() -> App {
    // Build two repos with very different name lengths so the
    // group-row body widths diverge. Each carries one session
    // so both rows participate in summary-chip rendering.
    let mut snapshot = GraphSnapshot::empty();
    for name in ["x", "very-long-project-name"] {
        let common = format!("/home/op/src/{name}");
        let repo_id = RepoId::new(common.clone());
        snapshot
            .nodes
            .push(GraphNode::Repo(RepoNode::new(repo_id.clone())));
        snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
            id: CheckoutId::new(repo_id, common.clone()),
            root: common.clone(),
            git_dir: None,
            current_branch: None,
            worktree: None,
            head_commit: None,
        }));
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", name),
                "codex".to_string(),
            )
            .with_cwd(common),
        ));
    }
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

#[test]
fn group_row_summary_chips_align_across_visible_groups() {
    // Two group rows with very different body widths must have
    // their `(N)` chips start at the same column so the eye can
    // scan summary state without zig-zagging across rows.
    let mut app = two_repo_app();
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);

    let short_line = text
        .lines()
        .find(|l| l.contains("~/src/x") && !l.contains("very-long"))
        .expect("short-named group row present");
    let long_line = text
        .lines()
        .find(|l| l.contains("~/src/very-long-project-name"))
        .expect("long-named group row present");
    let short_col = short_line.find("(1)").expect("short row has count chip");
    let long_col = long_line.find("(1)").expect("long row has count chip");
    assert_eq!(
        short_col, long_col,
        "(N) chips should start at the same column across group rows:\n  short: {short_line}\n  long:  {long_line}",
    );
}

#[test]
fn group_row_secondary_column_starts_at_same_column_across_groups() {
    // Label column anchor: the secondary segment (path or
    // `+`-delimited member list) must start at the same column
    // on every visible group row, even when the labels have
    // very different widths.
    let mut app = two_repo_app();
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);

    let short_line = text
        .lines()
        .find(|l| l.contains("~/src/x") && !l.contains("very-long"))
        .expect("short-named group row present");
    let long_line = text
        .lines()
        .find(|l| l.contains("~/src/very-long-project-name"))
        .expect("long-named group row present");
    let short_col = short_line
        .find("~/src/x")
        .expect("short row secondary segment present");
    let long_col = long_line
        .find("~/src/very-long-project-name")
        .expect("long row secondary segment present");
    assert_eq!(
        short_col, long_col,
        "secondary segment should start at the same column across group rows:\n  short: {short_line}\n  long:  {long_line}",
    );
}

#[test]
fn group_rows_carry_session_count_chip_only_when_no_ambiguity() {
    // ADR 0072: group rows show `(N)` total agents and nothing
    // else when none of their descendants are in the ambiguous
    // candidate-set state. The per-bucket muxed/unmuxed counts
    // from the original Phase 7 chip strip are gone; ambiguity
    // is surfaced only by the trailing `▲` glyph (asserted in
    // `group_rows_show_warning_glyph_when_descendant_is_ambiguous`).
    let mut app = seeded_app();
    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let group_line = text
        .lines()
        .find(|l| l.contains("~/src/proj"))
        .expect("group line present");
    assert!(
        group_line.contains("(1)"),
        "group should advertise its agent count: {group_line}",
    );
    assert!(
        !group_line.contains('◉') && !group_line.contains('◐') && !group_line.contains('▲'),
        "non-ambiguous group should carry no per-bucket glyphs and no warning: {group_line}",
    );
}

#[test]
fn truncate_to_width_middle_collapses_to_inline_ellipsis() {
    // Fits unchanged.
    assert_eq!(
        truncate_to_width_middle("/fixture/repos/project", 30),
        "/fixture/repos/project"
    );
    // Drops the middle and keeps both ends visible.
    let collapsed = truncate_to_width_middle("/fixture/atelier-demo/repo-a", 15);
    assert!(collapsed.contains('…'), "{collapsed}");
    assert!(collapsed.starts_with('/'), "{collapsed}");
    assert!(collapsed.ends_with("repo-a"), "{collapsed}");
    assert_eq!(UnicodeWidthStr::width(collapsed.as_str()), 15);
    // Degenerate widths render the ellipsis alone or nothing.
    assert_eq!(truncate_to_width_middle("abc", 1), "…");
    assert_eq!(truncate_to_width_middle("abc", 0), "");
}

#[test]
fn render_node_field_line_places_kind_chip_before_value() {
    // ADR 0073 §3 amendment: the cwd kind-chip glyph should sit
    // beside the label so the chip stays attached when the value
    // wraps onto a new terminal row. The pre-fix render emitted
    // `cwd  <path>  ▦`, which orphaned the chip past the wrap
    // boundary on the showcase fixture's deck-launcher session.
    let theme = Theme::default();
    let field = crate::tui::explorer::CoreField {
        label: "cwd",
        value: "/fixture/.agent-deck/multi-repo-worktrees/showcase-deck-c0debeef".to_string(),
        placeholder: false,
        annotation: None,
        long_value: None,
        kind_chip: Some(crate::model::NodeKind::Workspace),
        preview: None,
    };
    let line = render_node_field_line(&field, false, &theme);
    let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let label_idx = rendered.find("cwd").expect("label present");
    let chip_idx = rendered.find('▦').expect("workspace chip present");
    let value_idx = rendered.find("/fixture/").expect("value present");
    assert!(label_idx < chip_idx, "label before chip: {rendered}");
    assert!(chip_idx < value_idx, "chip before value: {rendered}");
}

#[test]
fn group_body_secondary_truncates_mid_string_to_preserve_summary() {
    // The dim canonical path collapses with an inline `…` so the
    // right-anchored `(N)` count and `▲` ambiguity glyph stay
    // visible in narrow panes. Before this, long fixture paths
    // pushed the summary chip off the right edge.
    let theme = Theme::default();
    let group = crate::tui::rows::GroupRow {
        display_path: "/fixture/atelier-demo/repo-a".to_string(),
        primary_node: None,
        is_launch_context: false,
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    append_group_body_spans(&mut spans, &group, &theme, 12, Some(14));
    let rendered: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(rendered.contains('…'), "{rendered}");
    assert!(rendered.contains("repo-a"), "tail preserved: {rendered}");
    assert!(rendered.contains("/fixtur"), "head preserved: {rendered}");

    // Wide budget leaves the path untouched.
    let mut spans: Vec<Span<'static>> = Vec::new();
    append_group_body_spans(&mut spans, &group, &theme, 12, Some(200));
    let rendered: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        rendered.contains("/fixture/atelier-demo/repo-a"),
        "wide budget keeps full path: {rendered}"
    );
    assert!(!rendered.contains('…'), "{rendered}");
}

#[test]
fn group_rows_show_warning_glyph_when_descendant_is_ambiguous() {
    // ADR 0072: when any descendant session is in the
    // `Ambiguous` candidate-set state, the group row gains a
    // single `▲` after the `(N)` count chip. The catalog of
    // ambiguous muxes lives on the group's detail pane
    // (ADR 0071); the row glyph is just the flag.
    let theme = Theme::default();
    let summary = GroupSummary {
        agents: 2,
        attached: 0,
        ambiguous: 2,
        unmuxed: 0,
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    append_group_summary_spans(&mut spans, summary, &theme, 3);
    let rendered: String = spans.iter().map(|s| s.content.as_ref()).collect();

    assert!(rendered.contains("(2)"), "count chip missing: {rendered}");
    assert!(rendered.contains('▲'), "warning glyph missing: {rendered}");
    assert!(
        !rendered.contains('◉') && !rendered.contains('◐') && !rendered.contains('◯'),
        "per-bucket mux glyphs should not appear on the group summary: {rendered}",
    );

    // The unambiguous case omits the warning glyph entirely.
    let clean_summary = GroupSummary {
        agents: 2,
        attached: 2,
        ambiguous: 0,
        unmuxed: 0,
    };
    let mut clean_spans: Vec<Span<'static>> = Vec::new();
    append_group_summary_spans(&mut clean_spans, clean_summary, &theme, 3);
    let clean_rendered: String = clean_spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        !clean_rendered.contains('▲'),
        "warning should be hidden when no descendant is ambiguous: {clean_rendered}",
    );
}

#[test]
fn detail_pane_omits_initial_node_divider() {
    // The right pane starts directly with the selected node's
    // fields. Later zones still render labeled dividers; empty
    // sections are suppressed entirely. ADR 0074 collapsed the
    // prior `Upstream` / `Downstream` chip dividers into one
    // `Related` chip; this test pins the new label.
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        !text.contains(" Node "),
        "unexpected Node section divider label: {text}",
    );
    assert!(
        text.contains(" Related "),
        "expected Related section divider label: {text}",
    );
    assert!(
        text.contains(" Preview "),
        "expected Preview section divider label: {text}",
    );
}

#[test]
fn related_row_keeps_verb_and_neighbor_label_on_one_line() {
    // ADR 0074 §3: the prior two-line composite (`relation
    // [kind]` row + indented neighbor-label row) collapses to a
    // single `<verb> <glyph> <neighbor_label>` line. The verb
    // carries the relation, the glyph carries the neighbor
    // kind, and the row is selectable as one cursor stop.
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let line = text
        .lines()
        .find(|line| line.contains("attached to") && line.contains("tmux:editor"))
        .expect("validated `attached to … tmux:editor` row");
    let verb_idx = line.find("attached to").expect("verb on row");
    let label_idx = line.find("tmux:editor").expect("neighbor label on row");
    assert!(
        verb_idx < label_idx,
        "verb should sit left of the neighbor label: {line}",
    );
}

#[test]
fn link_rows_hide_edge_meta_by_default_and_surface_it_after_toggle() {
    // By default the explorer's single-link composite
    // collapses to just its header row (no `provenance ·
    // confidence · state` trailing line). After Msg::ToggleEdgeMeta
    // the trailing meta surfaces.
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let default_text = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        !default_text.contains("discovered · "),
        "edge meta should be hidden by default: {default_text}",
    );
    // Toggle to opt-in.
    app.update(Msg::ToggleEdgeMeta);
    let toggled_text = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        toggled_text.contains("discovered · ") || toggled_text.contains("strong_discovered · "),
        "edge meta should surface after the toggle: {toggled_text}",
    );
}

#[test]
fn kind_chip_span_renders_per_kind_glyph_in_node_kind_color() {
    // ADR 0073 §3: the dim `[kind]` text chip is replaced by the
    // per-kind slate glyph in the node-kind color. A missing kind
    // falls back to a dim `?` so the chip slot stays visible
    // without misleading the operator.
    let theme = Theme::default();
    let cases = [
        (NodeKind::Workspace, "▦", theme.node_workspace),
        (NodeKind::MuxSession, "▣", theme.node_mux_session),
        (NodeKind::Fork, "⋎", theme.node_fork),
        (NodeKind::Checkout, "◇", theme.node_checkout),
    ];
    for (tag, glyph, expected_color) in cases {
        let span = kind_chip_span(Some(tag), &theme);
        assert_eq!(span.content.as_ref(), glyph, "wrong glyph for `{tag:?}`");
        assert_eq!(
            span.style.fg,
            Some(expected_color),
            "wrong color for `{tag:?}`"
        );
    }
    // ForgePr's kind glyph reuses `pr_open` at chip surfaces
    // because the chip layer doesn't carry PR state.
    let pr = kind_chip_span(Some(NodeKind::ForgePr), &theme);
    assert_eq!(pr.content.as_ref(), "⇄");
    assert_eq!(pr.style.fg, Some(theme.pr_open));
    // Missing kind → dim `?` fallback.
    let unknown = kind_chip_span(None, &theme);
    assert_eq!(unknown.content.as_ref(), "?");
    assert!(unknown.style.add_modifier.contains(theme.placeholder));
}

#[test]
fn right_panel_title_prefixes_kind_glyph_when_detail_resolves() {
    // ADR 0073 §3: `<glyph> <label>` in the right-panel title.
    // The glyph appears in the node-kind color; the label stays
    // bold. The muxed fixture selects an agent-session row on
    // the left, so the right-panel title reads
    // `► ● session ◄ …` (`AgentSession` glyph is preserved at
    // pill-less surfaces per the §3 amendment).
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let agent_glyph = crate::tui::icons::NodeKind::AgentSession.default_glyph();
    let pattern = format!("{agent_glyph} session");
    assert!(
        text.lines().any(|line| line.contains(&pattern)),
        "expected `{pattern}` in right-panel title; rendered:\n{text}",
    );
}

#[test]
fn related_row_orders_verb_glyph_then_neighbor_label() {
    // ADR 0073 §3 + ADR 0074 §3: each related-entities row reads
    // as `<verb 22w> <glyph> <neighbor_label>`. The glyph sits
    // between the verb column and the neighbor label so the
    // operator can scan kinds without reading the label first.
    // Pin glyph + ordering on the validated `attached to … ▣
    // tmux:editor` row that the muxed fixture produces.
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let mux_glyph = crate::tui::icons::NodeKind::MuxSession.default_glyph();
    let line = text
        .lines()
        .find(|line| line.contains("attached to") && line.contains(mux_glyph))
        .expect("validated row carrying the mux-kind glyph");
    let verb_idx = line.find("attached to").expect("verb on row");
    let glyph_idx = line.find(mux_glyph).expect("kind glyph on row");
    let label_idx = line.find("tmux:editor").expect("neighbor label on row");
    assert!(
        verb_idx < glyph_idx && glyph_idx < label_idx,
        "row order should be verb → glyph → label: {line}",
    );
}

#[test]
fn other_link_line_dispatches_per_edge_state() {
    // ADR 0075: Other-zone rows visually distinguish AltOf
    // (quiet) from Conflict (loud + `▲`) so an operator
    // skimming the zone reads the resolver state at a glance.
    // Test the three shapes by constructing fake links and
    // rendering directly through `render_other_link_line` — no
    // App needed.
    use crate::model::{Confidence, NodeId, Provenance, RelationKind};
    use crate::tui::explorer::{
        CoreField, Direction, EdgeStateLabel, LinkStateLabel, RelationshipGroup, RelationshipLink,
    };
    let theme = Theme::default();
    let group = RelationshipGroup {
        direction: Direction::Downstream,
        relation: RelationKind::LinkedToMux,
        neighbor_kind: "mux_session".into(),
        links: Vec::new(),
        unresolved: Vec::new(),
        ambiguous: false,
        unresolved_count: 0,
    };
    let link = |edge_state: EdgeStateLabel| RelationshipLink {
        link_id: "l".into(),
        neighbor_id: NodeId::MuxSession(crate::model::MuxSessionId::new("x")),
        neighbor_kind: Some(crate::model::NodeKind::MuxSession),
        neighbor_label: "tmux:x".into(),
        neighbor_short_id: "x".into(),
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        state: LinkStateLabel::Active,
        resolved_winner: false,
        edge_state,
        preview: Vec::<CoreField>::new(),
        evidence: Vec::new(),
    };

    // Conflict: prefix `▲`, label color = edge_conflict, BOLD.
    let conflict_line = render_other_link_line(
        &group,
        &link(EdgeStateLabel::Conflict),
        false,
        &theme,
        false,
        200,
    );
    let text: String = conflict_line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        text.starts_with("▲ "),
        "Conflict rows must lead with the `▲ ` prefix (NO_COLOR-safe signal): `{text}`",
    );
    let label_span = conflict_line
        .spans
        .iter()
        .find(|s| s.content.contains("tmux:x"))
        .expect("neighbor-label span present on conflict row");
    assert_eq!(label_span.style.fg, Some(theme.edge_conflict));
    assert!(label_span.style.add_modifier.contains(Modifier::BOLD));

    // AltOf: no prefix, label color = edge_alt_of, no BOLD.
    let alt_line = render_other_link_line(
        &group,
        &link(EdgeStateLabel::AltOf(RelationKind::LinkedToMux)),
        false,
        &theme,
        false,
        200,
    );
    let alt_text: String = alt_line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        !alt_text.starts_with("▲ "),
        "AltOf rows should not carry the conflict prefix: `{alt_text}`",
    );
    let alt_label = alt_line
        .spans
        .iter()
        .find(|s| s.content.contains("tmux:x"))
        .expect("neighbor-label span present on alt row");
    assert_eq!(alt_label.style.fg, Some(theme.edge_alt_of));
    assert!(!alt_label.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn validated_link_line_drops_the_legacy_winner_star() {
    // ADR 0075: the validated zone never carries `★` — every
    // row there is a resolver winner, so the marker no longer
    // earns its column. The same fact also gets enforced
    // structurally by `render_related_row`'s removal of the
    // `resolved_winner` branch, but the buffer-level assertion
    // pins it from the operator-visible angle.
    use crate::model::{Confidence, NodeId, Provenance, RelationKind};
    use crate::tui::explorer::{
        CoreField, Direction, EdgeStateLabel, LinkStateLabel, RelationshipGroup, RelationshipLink,
    };
    let theme = Theme::default();
    let group = RelationshipGroup {
        direction: Direction::Downstream,
        relation: RelationKind::LinkedToMux,
        neighbor_kind: "mux_session".into(),
        links: Vec::new(),
        unresolved: Vec::new(),
        ambiguous: false,
        unresolved_count: 0,
    };
    let link = RelationshipLink {
        link_id: "l".into(),
        neighbor_id: NodeId::MuxSession(crate::model::MuxSessionId::new("x")),
        neighbor_kind: Some(crate::model::NodeKind::MuxSession),
        neighbor_label: "tmux:x".into(),
        neighbor_short_id: "x".into(),
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        state: LinkStateLabel::Active,
        resolved_winner: true,
        edge_state: EdgeStateLabel::Resolves,
        preview: Vec::<CoreField>::new(),
        evidence: Vec::new(),
    };
    let line = render_validated_link_line(&group, &link, false, &theme, true, 200);
    let text: String = line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        !text.contains("★"),
        "validated row should no longer carry the `★` marker: `{text}`",
    );
}

#[test]
fn related_row_truncates_long_labels_instead_of_wrapping_them_away() {
    // The explorer paragraph word-wraps, so an unbreakable
    // path wider than the pane used to drop onto the next line and
    // leave `checked out at ◇` with no visible label.
    use crate::model::{CheckoutId, Confidence, NodeId, Provenance, RelationKind, RepoId};
    use crate::tui::explorer::{
        CoreField, Direction, EdgeStateLabel, LinkStateLabel, RelationshipGroup, RelationshipLink,
    };
    let theme = Theme::default();
    let group = RelationshipGroup {
        direction: Direction::Downstream,
        relation: RelationKind::BelongsToRepo,
        neighbor_kind: "checkout".into(),
        links: Vec::new(),
        unresolved: Vec::new(),
        ambiguous: false,
        unresolved_count: 0,
    };
    let path = "/fixture/checkouts/a-rather-long-directory-name/bare-project";
    let link = RelationshipLink {
        link_id: "l".into(),
        neighbor_id: NodeId::Checkout(CheckoutId::new(RepoId::new("/r.git"), path)),
        neighbor_kind: Some(crate::model::NodeKind::Checkout),
        neighbor_label: path.into(),
        neighbor_short_id: "c".into(),
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        state: LinkStateLabel::Active,
        resolved_winner: true,
        edge_state: EdgeStateLabel::Resolves,
        preview: Vec::<CoreField>::new(),
        evidence: Vec::new(),
    };
    let width = 56;
    let line = render_validated_link_line(&group, &link, false, &theme, false, width);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        UnicodeWidthStr::width(text.as_str()) <= width,
        "row must fit the pane so it never wraps: `{text}`"
    );
    assert!(
        text.contains('…'),
        "label should be middle-truncated: `{text}`"
    );
    assert!(
        text.ends_with("bare-project"),
        "truncation keeps the basename: `{text}`"
    );

    let wide = render_validated_link_line(&group, &link, false, &theme, false, 200);
    let wide_text: String = wide.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(wide_text.ends_with(path), "no truncation when it fits");
}

#[test]
fn related_zone_header_renders_aggregate_left_of_label() {
    // ADR 0074 §5: the bold zone label
    // anchors flush right and the summary segment (`N validated
    // · M other …`) sits to the left of the chip on the same
    // divider line. Pin the relative ordering plus the new
    // summary vocabulary.
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let line = text
        .lines()
        .find(|line| line.contains(" Related "))
        .expect("Related divider line");
    let validated_idx = line.find("validated").expect("`validated` segment on line");
    let label_idx = line.find(" Related ").expect("Related chip on line");
    assert!(
        validated_idx < label_idx,
        "summary `validated …` should render left of the Related chip; got: {line}",
    );
}

#[test]
fn detail_pane_shows_linked_to_mux_row_and_drills_into_mux() {
    use crate::tui::explorer::ExplorerRow;

    // Locked decision 8: instead of expanding linked
    // entity details in place, the explorer drills. Pressing
    // Enter on the cursor while it sits on the `linked_to_mux`
    // row should refocus the right pane on the mux node and
    // push a breadcrumb hop. The Node-zone fields then mirror
    // the mux summary (`backend · native_id`, etc.).
    let mut app = muxed_app("editor", None);
    let area = Rect::new(0, 0, 120, 24);
    let initial = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        initial.contains("attached to"),
        "session detail should expose the `attached to` row (ADR 0074 verb catalog): {initial}"
    );
    app.update(Msg::CycleFocus);
    // Walk the cursor onto the link row, then activate.
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);
    let drilled = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        drilled.contains("backend") && drilled.contains("tmux"),
        "after drilldown the Node zone should expose the mux fields: {drilled}"
    );
    assert!(
        drilled.contains("◄"),
        "breadcrumb back-hint should surface in the right-pane title: {drilled}"
    );
    // The breadcrumb chain renders each
    // hop as `<kind glyph> <tag>`, replacing the prior
    // `kind:short_tag` text form. The previous session hop
    // should render with the `AgentSession` glyph (●) so the
    // operator scans depth by symbol rather than reading
    // verbose kind prefixes.
    let agent_session_glyph = crate::tui::icons::NodeKind::AgentSession.default_glyph();
    assert!(
        drilled.contains(agent_session_glyph),
        "breadcrumb chain should carry the previous session's kind glyph ({agent_session_glyph}): {drilled}"
    );
    assert!(
        drilled.contains("depth 1"),
        "breadcrumb title should carry the depth marker: {drilled}"
    );
}

#[test]
fn agreeing_mux_evidence_renders_as_one_row_with_preview_evidence_list() {
    use crate::model::{
        AgentSessionId, Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, NodeId,
        Provenance, RelationKind, SourceMetadata,
    };
    use crate::tui::explorer::ExplorerRow;

    // ADR 0107: a second producer agreeing on the same mux folds into
    // the existing row and shows up in the Preview zone's evidence.
    let hook = GraphLink {
        id: "session-mux-hook".to_string(),
        source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(MuxSessionId::new("editor")),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "hook_sidecar".to_string(),
            evidence: Some("hook_session_path_match".to_string()),
            ..Default::default()
        },
        state: LinkState::Active,
    };
    let mut app = muxed_app_with_links("editor", None, vec![hook]);

    let area = Rect::new(0, 0, 140, 40);
    let initial = buffer_to_string(&render_to_buffer(&mut app, area));
    assert_eq!(
        initial.matches("attached to").count(),
        1,
        "two producers agreeing on one mux should render one row: {initial}"
    );
    assert!(
        !initial.contains("Other"),
        "agreeing evidence must not open an Other zone: {initial}"
    );

    app.update(Msg::CycleFocus);
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| matches!(row, ExplorerRow::ValidatedLink { .. }))
        .expect("validated row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    let focused = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        focused.contains("evidence"),
        "preview should carry an evidence block: {focused}"
    );
    assert!(
        focused.contains("hook_session_path_match · strong_discovered · high"),
        "hook evidence line expected: {focused}"
    );
    assert!(
        focused.contains("discovered · medium"),
        "the other producer's evidence line expected: {focused}"
    );
}

#[test]
fn mux_detail_session_section_shows_session_id_when_collapsed() {
    // Regression: with a long mux native_id, the right pane's
    // Session section originally vanished entirely. After the
    // wrap-aware budget fix the section divider returned, but
    // the operator reported the linked-session row still didn't
    // render its id until they pressed `e`. This test pins the
    // expectation that the collapsed Session row always carries
    // the `session  <harness>:<key>` line, even when the panel
    // is tall enough to need no clamp.
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance,
        RelationKind,
    };
    let long_native = "agentdeck_-local-command-caveat-Caveat-The-messages-\
                           below-were-generated-by-the-user-while-running-local-\
                           comm-Branch-_573ac208";
    let mux_graph_id = MuxSessionId::new(long_native);

    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string()),
    ));
    snapshot.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(
            mux_graph_id.clone(),
            "tmux".to_string(),
            long_native.to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_client_attached(true)
        .with_activity_epoch(1_700_000_000),
    ));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_graph_id),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Mux;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::NavDown);

    // Render a generously tall area so the height clamp never
    // kicks in — the bug should reproduce purely from the
    // section-content path, not from vertical clamping.
    let area = Rect::new(0, 0, 120, 40);
    let collapsed = buffer_to_string(&render_to_buffer(&mut app, area));
    // ADR 0074: the linked session surfaces in the
    // mux's `Related` zone via the inbound `attached session`
    // verb (the session is the link's source, the mux its
    // target). The row carries the session id by `harness:key`.
    assert!(
        collapsed.contains(" Related "),
        "Related section divider should render for the mux: {collapsed}"
    );
    assert!(
        collapsed.contains("attached session"),
        "inbound `attached session` verb should label the row: {collapsed}"
    );
    assert!(
        collapsed.contains("codex:abc"),
        "the related row should expose the session id: {collapsed}"
    );
}

#[test]
fn expanded_session_under_mux_matches_standalone_session_detail() {
    use crate::tui::explorer::ExplorerRow;

    // The expanded representation should reuse the same
    // section-divided, 10-char-bold-label rendering as a
    // standalone session detail — only indented. This pins the
    // per-row labels and the Mux divider so the two surfaces
    // can't drift visually.
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, Provenance,
        RelationKind,
    };
    let mux_graph_id = MuxSessionId::new("editor");

    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string()),
    ));
    snapshot.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(
            mux_graph_id.clone(),
            "tmux".to_string(),
            "editor".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_client_attached(true)
        .with_activity_epoch(1_700_000_000),
    ));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_graph_id),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);

    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Mux;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::NavDown);
    app.update(Msg::CycleFocus);
    // Walk the cursor onto the upstream `linked_to_mux` row,
    // then activate to drill into the linked session.
    let link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                ExplorerRow::ValidatedLink { .. } | ExplorerRow::OtherLink { .. }
            )
        })
        .expect("link row");
    for _ in 0..link_idx {
        app.update(Msg::ExplorerNavDown);
    }
    app.update(Msg::ExplorerActivate);

    let area = Rect::new(0, 0, 120, 40);
    let drilled = buffer_to_string(&render_to_buffer(&mut app, area));
    // Post-drill the right pane is now focused on the
    // session itself. Its Node zone exposes the standalone
    // session core fields (id, harness, alias, cwd, status).
    assert!(
        drilled.contains("id"),
        "drilled session Node zone should carry the id row: {drilled}"
    );
    assert!(
        drilled.contains("codex"),
        "drilled session Node zone should expose the harness: {drilled}"
    );
    // Breadcrumb back-hint surfaces in the right-pane title
    // after a drilldown.
    assert!(
        drilled.contains("◄"),
        "drilldown should add a breadcrumb back-hint to the title: {drilled}"
    );
}

#[test]
fn empty_app_renders_loading_placeholder() {
    let mut app = App::new(RunConfig::defaults());
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("Loading"),
        "expected loading placeholder: {text}"
    );
}

#[test]
fn session_harness_span_renders_as_filled_badge() {
    // Phase 4 + Phase 10 refinement: the harness label renders
    // as a single fixed-width pill — padding cells included —
    // styled REVERSED+BOLD over the harness color. Every badge
    // is the same visible width regardless of label length so
    // the recency column lands at the same column on every row.
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let row = AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abcdef".into(),
        harness_label: "codex".into(),
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
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };
    let spans = render_session_spans(&row, &theme, now, None);
    let badge = spans
        .iter()
        .find(|s| s.content.trim() == "codex")
        .expect("harness badge span present");
    assert_eq!(badge.content.chars().count(), theme.badge_width + 2);
    assert_eq!(badge.style.fg, Some(theme.harness_color("codex")));
    assert!(badge.style.add_modifier.contains(Modifier::REVERSED));
    assert!(badge.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn node_kind_glyph_span_uses_slate_glyph_and_theme_color() {
    // ADR 0073 §3: each row carries a prefix glyph in the kind
    // color. The helper returns `<glyph> ` (glyph + trailing
    // space) so callers can splice it before the row body
    // without per-call padding.
    let theme = Theme::default();
    let span = node_kind_glyph_span(NodeKind::Repo, &theme);
    assert_eq!(span.content, "◆ ");
    assert_eq!(span.style.fg, Some(theme.node_repo));
    let workspace = node_kind_glyph_span(NodeKind::Workspace, &theme);
    assert_eq!(workspace.content, "▦ ");
    assert_eq!(workspace.style.fg, Some(theme.node_workspace));
}

#[test]
fn forge_pr_glyph_span_picks_color_from_pr_state() {
    // The PR glyph color follows `theme.pr_*` based on PR state
    // (ADR 0073 §2). Draft overrides state.
    use crate::tui::rows::PrRow;
    let theme = Theme::default();
    let base = PrRow {
        pr_number: 1,
        repo_display: "owner/repo".into(),
        label: "owner/repo".into(),
        state: Some("open".into()),
        is_draft: false,
        branch_name: None,
        tracked: false,
        title: None,
        updated_recency: None,
        updated_epoch: None,
        url: None,
        primary_node: NodeId::ForgePr(crate::model::ForgePrId::new(
            "github",
            "github.com",
            "owner",
            "repo",
            1,
        )),
    };
    assert_eq!(
        forge_pr_glyph_span(&base, &theme).style.fg,
        Some(theme.pr_open)
    );
    let closed = PrRow {
        state: Some("closed".into()),
        ..base.clone()
    };
    assert_eq!(
        forge_pr_glyph_span(&closed, &theme).style.fg,
        Some(theme.pr_closed)
    );
    let merged = PrRow {
        state: Some("merged".into()),
        ..base.clone()
    };
    assert_eq!(
        forge_pr_glyph_span(&merged, &theme).style.fg,
        Some(theme.pr_merged)
    );
    let draft = PrRow {
        is_draft: true,
        state: Some("open".into()),
        ..base.clone()
    };
    assert_eq!(
        forge_pr_glyph_span(&draft, &theme).style.fg,
        Some(theme.pr_draft),
        "draft overrides state-based color",
    );
    // Glyph itself stays the `NodeKind::ForgePr` slate glyph
    // regardless of color.
    assert_eq!(forge_pr_glyph_span(&base, &theme).content, "⇄ ");
}

#[test]
fn pr_tracking_span_tells_a_local_branch_from_none() {
    // ADR 0129: the Branch glyph in the branch color for a PR with a
    // local branch, a dim hollow circle for one without. Both are one
    // cell so the columns after the marker stay aligned.
    use crate::tui::rows::PrRow;
    let theme = Theme::default();
    let untracked = PrRow {
        pr_number: 1,
        repo_display: "owner/repo#1".into(),
        label: "#1".into(),
        state: Some("open".into()),
        is_draft: false,
        branch_name: None,
        tracked: false,
        title: None,
        updated_recency: None,
        updated_epoch: None,
        url: None,
        primary_node: NodeId::ForgePr(crate::model::ForgePrId::new(
            "github",
            "github.com",
            "owner",
            "repo",
            1,
        )),
    };
    let tracked = PrRow {
        tracked: true,
        branch_name: Some("feature/x".into()),
        ..untracked.clone()
    };

    let none = pr_tracking_span(&untracked, &theme);
    assert_eq!(none.content, "◌");
    assert!(none.style.add_modifier.contains(theme.placeholder));

    let some = pr_tracking_span(&tracked, &theme);
    assert_eq!(some.content, "⌥");
    assert_eq!(some.style.fg, Some(theme.node_branch));
    assert_eq!(some.width(), none.width());
}

#[test]
fn row_kind_glyph_span_dispatches_per_row_kind() {
    use crate::tui::rows::{ForkRow, GroupRow};
    let theme = Theme::default();

    // Group rows derive their kind from `primary_node`; synthetic
    // group buckets without a backing node skip the glyph.
    let workspace_group = RowKind::Group(GroupRow {
        display_path: "/ws".into(),
        primary_node: Some(NodeId::Workspace(crate::model::WorkspaceId::new("/ws"))),
        is_launch_context: false,
    });
    assert_eq!(
        row_kind_glyph_span(&workspace_group, &theme)
            .map(|s| s.content.to_string())
            .as_deref(),
        Some("▦ "),
    );

    let synthetic_group = RowKind::Group(GroupRow {
        display_path: "(ungrouped)".into(),
        primary_node: None,
        is_launch_context: false,
    });
    assert!(
        row_kind_glyph_span(&synthetic_group, &theme).is_none(),
        "synthetic group buckets have no NodeKind",
    );

    // AgentSession rows skip the kind glyph in row contexts —
    // the colored harness pill already carries the identity
    // signal, so a stacked `●` would only repeat what the pill
    // already says (ADR 0073 amendment).
    let agent = RowKind::AgentSession(crate::tui::rows::AgentSessionRow {
        session: AgentSessionId::new("claude", "/state", "abc"),
        short_id: "abc".into(),
        harness_label: "claude".into(),
        cwd_display: None,
        project_display: None,
        recency: None,
        activity_epoch: None,
        mux_state: crate::tui::rows::MuxIndicator::Unmuxed,
        preview: None,
        title: None,
        alias: None,
        title_disambiguates: false,
        mux_host: None,
        primary_node: NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    });
    assert!(
        row_kind_glyph_span(&agent, &theme).is_some(),
        "a session wears its kind glyph in every view",
    );

    // A mux row gets `▣`.
    let mux = RowKind::MuxSession(crate::tui::rows::MuxSessionRow {
        mux: MuxSessionId::new("project"),
        backend: "tmux".into(),
        native_id: "project".into(),
        client_attached: Some(true),
        cwd_display: None,
        attached_count: 1,
        recency: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
        agent_labels: Vec::new(),
        program: None,
        program_harness: None,
        single_session_preview: None,
        pin_id: None,
        primary_node: NodeId::MuxSession(MuxSessionId::new("project")),
        content_hit: None,
    });
    assert_eq!(
        row_kind_glyph_span(&mux, &theme)
            .map(|s| s.content.to_string())
            .as_deref(),
        Some("▣ "),
    );
    // Fork rows get `⋎`.
    let fork = RowKind::Fork(ForkRow {
        fork_label: "alpha".into(),
        provider: "github".into(),
        scope: None,
        parent_label: None,
        child_count: 0,
        primary_node: NodeId::Fork(crate::model::ForkId::new("alpha")),
    });
    let fork_glyph = row_kind_glyph_span(&fork, &theme).unwrap();
    assert_eq!(fork_glyph.content, "⋎ ");
    assert_eq!(fork_glyph.style.fg, Some(theme.node_fork));
}

#[test]
fn session_alias_renders_after_mux_glyph_in_left_row() {
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let row = AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abcdef".into(),
        harness_label: "codex".into(),
        cwd_display: None,
        project_display: None,
        recency: None,
        activity_epoch: None,
        mux_state: MuxIndicator::Unmuxed,
        preview: None,
        title: Some("harness title".into()),
        alias: Some("ingest-refactor".into()),
        title_disambiguates: false,
        mux_host: None,
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };

    let spans = render_session_spans(&row, &theme, now, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert!(
        rendered.contains("∘  ingest-refactor"),
        "alias should render after the mux glyph: {rendered}"
    );
    assert!(
        rendered.starts_with("abc  "),
        "external session id should lead the row: {rendered}"
    );
    let alias = spans
        .iter()
        .find(|span| span.content.trim() == "ingest-refactor")
        .expect("alias span present");
    assert!(alias.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn a_session_last_hosted_by_a_gone_mux_carries_a_was_in_chip() {
    use crate::model::MuxHost;
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let row = |host: Option<MuxHost>| AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abc".into(),
        harness_label: "codex".into(),
        cwd_display: None,
        project_display: Some("conspectus".into()),
        recency: Some("2d".into()),
        activity_epoch: Some(now - 172_800),
        mux_state: MuxIndicator::Unmuxed,
        preview: None,
        title: None,
        alias: None,
        title_disambiguates: false,
        mux_host: host,
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };
    let text = |row: &AgentSessionRow| -> String {
        render_session_spans(row, &theme, now, None)
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    };

    let gone = text(&row(Some(MuxHost {
        name: "old-ledger".into(),
        live: false,
    })));
    let live = text(&row(Some(MuxHost {
        name: "ledger".into(),
        live: true,
    })));
    let none = text(&row(None));

    let mut titled = row(Some(MuxHost {
        name: "old-ledger".into(),
        live: false,
    }));
    titled.alias = Some("Nightly".into());
    let titled = text(&titled);

    insta::assert_snapshot!(
        "session_row_was_in_chip",
        format!("{}\n{}", gone.trim_end(), titled.trim_end())
    );
    assert!(gone.contains("old-ledger  (mux stopped)"), "{gone}");
    assert!(titled.contains("Nightly  was in old-ledger"), "{titled}");
    assert!(!live.contains("was in"), "a live mux needs no chip: {live}");
    assert!(
        live.contains("ledger"),
        "the live mux still names the session: {live}"
    );
    assert!(!none.contains("was in"), "{none}");
}

#[test]
fn session_id_is_strictly_truncated_in_left_row() {
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let long_id = "ffffffff-1111-2222-3333-444444444444";
    let row = AgentSessionRow {
        session: AgentSessionId::new("opencode", "/state", long_id),
        short_id: "abcdef".into(),
        harness_label: "opencode".into(),
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
        primary_node: NodeId::AgentSession(AgentSessionId::new("opencode", "/state", long_id)),
        pin_id: None,
        content_hit: None,
    };

    let spans = render_session_spans(&row, &theme, now, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
    assert!(
        rendered.starts_with("ffffffff  "),
        "left row should strictly truncate long session id without ellipsis: {rendered}"
    );
}

#[test]
fn session_display_label_is_truncated_in_left_row() {
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let long_title =
        "The conspectus TUI, fashioned after a long prompt, should not consume the row";
    let row = AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abcdef".into(),
        harness_label: "codex".into(),
        cwd_display: None,
        project_display: None,
        recency: None,
        activity_epoch: None,
        mux_state: MuxIndicator::Unmuxed,
        preview: None,
        title: Some(long_title.into()),
        alias: None,
        // The renderer only surfaces the title when the
        // builder flagged the row for disambiguation; the
        // truncation assertion is exercising the renderer's
        // width cap, so flip this on so the title actually
        // reaches the row.
        title_disambiguates: true,
        mux_host: None,
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };

    let spans = render_session_spans(&row, &theme, now, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
    let label = spans
        .iter()
        .find(|span| span.content.contains("The conspectus"))
        .expect("display label span present");
    assert!(
        rendered.contains("The conspectus TUI, fashioned a…"),
        "long display label should be capped: {rendered}"
    );
    assert_eq!(
        unicode_width::UnicodeWidthStr::width(label.content.trim()),
        32
    );
    assert!(!label.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn mux_session_row_mirrors_session_column_order() {
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let row = MuxSessionRow {
        mux: MuxSessionId::new("tmux:editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        client_attached: Some(true),
        cwd_display: Some("~/src/conspectus".into()),
        attached_count: 1,
        recency: Some("3s".into()),
        activity_epoch: Some(now - 3),
        created_epoch: None,
        last_attached_epoch: None,
        agent_labels: vec!["codex".into()],
        program: None,
        program_harness: None,
        single_session_preview: Some("running cargo test".into()),
        pin_id: None,
        primary_node: NodeId::MuxSession(MuxSessionId::new("tmux:editor")),
        content_hit: None,
    };

    let spans = render_mux_session_spans(&row, &theme, now, 100, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();

    assert!(
        UnicodeWidthStr::width(rendered.as_str()) <= 100,
        "mux row should fit the given width: {rendered:?}"
    );
    assert!(
        rendered.contains('⊸'),
        "attached glyph should still render: {rendered}"
    );
    assert!(
        rendered.contains("  3s"),
        "recency should render right-aligned: {rendered}"
    );
    assert!(
        rendered.contains(" codex "),
        "agent harness badge should label the mux row: {rendered}"
    );
    assert!(
        rendered.contains("running cargo test"),
        "preview should flow into the trailing column: {rendered}"
    );
    assert!(
        !rendered.contains("~/src/conspectus"),
        "cwd column was dropped from the mux row: {rendered}"
    );
    assert!(
        !rendered.contains("tmux:"),
        "backend prefix should be stripped from the mux label: {rendered}"
    );

    // Column order: native id label · harness chip · recency ·
    // attached glyph · preview. Probe by substring index since the
    // chip widget adds internal padding.
    let label_idx = rendered.find("editor").expect("label present");
    let chip_idx = rendered.find("codex").expect("harness chip present");
    let recency_idx = rendered.find("3s").expect("recency present");
    let glyph_idx = rendered.find('⊸').expect("glyph present");
    let preview_idx = rendered
        .find("running cargo test")
        .expect("preview present");
    assert!(label_idx < chip_idx, "label before chip: {rendered}");
    assert!(chip_idx < recency_idx, "chip before recency: {rendered}");
    assert!(recency_idx < glyph_idx, "recency before glyph: {rendered}");
    assert!(glyph_idx < preview_idx, "glyph before preview: {rendered}");
}

#[test]
fn session_project_column_renders_before_inline_preview() {
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;
    let row = AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abcdef".into(),
        harness_label: "codex".into(),
        cwd_display: None,
        project_display: Some("conspectus".into()),
        recency: None,
        activity_epoch: None,
        mux_state: MuxIndicator::Unmuxed,
        preview: Some("latest message".into()),
        title: None,
        alias: None,
        title_disambiguates: false,
        mux_host: None,
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };

    let mut spans = render_session_spans(&row, &theme, now, None);
    append_session_preview(&mut spans, &row, 120, &theme);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();
    let project_idx = rendered.find("conspectus").expect("project rendered");
    let preview_idx = rendered
        .find("latest message")
        .expect("inline preview rendered");
    assert!(
        project_idx < preview_idx,
        "project column should precede preview: {rendered}"
    );
}

#[test]
fn session_recency_span_picks_bucket_style_from_theme() {
    // Locate the recency span by its formatted content (4-cell
    // right-aligned tag). Index varies with harness label length
    // once the badge widget pads short labels — looking up by
    // content keeps the test resilient to badge layout changes.
    fn recency_span<'a>(spans: &'a [Span<'static>], rendered: &str) -> &'a Span<'static> {
        spans
            .iter()
            .find(|s| s.content.trim() == rendered.trim())
            .expect("recency span present")
    }

    // Build a minimal AgentSessionRow directly so we can pin the
    // activity_epoch and assert the recency span's style without
    // staging a full snapshot. The render_session_spans helper is
    // intentionally cheap to call from the test module.
    use crate::tui::rows::{AgentSessionRow, MuxIndicator};
    let theme = Theme::default();
    let now: i64 = 1_700_000_000;

    let make_row = |recency: Option<&str>, epoch: Option<i64>| AgentSessionRow {
        session: AgentSessionId::new("codex", "/state", "abc"),
        short_id: "abcdef".into(),
        harness_label: "codex".into(),
        cwd_display: None,
        project_display: None,
        recency: recency.map(std::string::ToString::to_string),
        activity_epoch: epoch,
        mux_state: MuxIndicator::Unmuxed,
        preview: None,
        title: None,
        alias: None,
        title_disambiguates: false,
        mux_host: None,
        primary_node: NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc")),
        pin_id: None,
        content_hit: None,
    };

    let fresh = make_row(Some("1m"), Some(now - 60));
    let spans = render_session_spans(&fresh, &theme, now, None);
    assert_eq!(
        recency_span(&spans, "1m").style,
        theme.recency_fresh.into_style(),
        "fresh row should inherit recency_fresh from the theme",
    );

    let cold = make_row(Some("3d"), Some(now - 3 * 24 * 60 * 60));
    let spans = render_session_spans(&cold, &theme, now, None);
    assert_eq!(
        recency_span(&spans, "3d").style,
        theme.recency_cold.into_style(),
    );

    let unknown = make_row(None, None);
    let spans = render_session_spans(&unknown, &theme, now, None);
    assert_eq!(
        recency_span(&spans, "—").style,
        Style::default().add_modifier(theme.placeholder),
        "missing activity_epoch falls back to placeholder dimming",
    );
}

#[test]
fn header_shows_updated_ns_ago_when_clock_is_ahead_of_load_epoch() {
    let mut app = seeded_app();
    // seeded_app sets loaded_at_epoch = 1_700_000_000.
    // Advance the rendering clock 12s to assert the freshness slot.
    test_clock::set(1_700_000_012);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("updated 12s ago"),
        "expected updated-ago slot, got: {text}"
    );
}

#[test]
fn narrow_terminal_stacks_the_two_panels_vertically() {
    let mut app = seeded_app();
    app.update(Msg::NavDown);
    // Width 60 is below the default narrow_layout_threshold (100).
    let area = Rect::new(0, 0, 60, 30);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);

    // In the stacked layout, only one panel border occupies
    // each row at any given column. The header still appears
    // at the top, and the right-panel content ("Phase 8
    // walkthrough" title) sits *below* the row tree content
    // ("~/src/proj") rather than beside it. Assert that
    // ordering.
    let proj_line = text
        .lines()
        .position(|l| l.contains("~/src/proj"))
        .expect("project path line present");
    let title_line = text
        .lines()
        .position(|l| l.contains("Phase 8 walkthrough"))
        .expect("right-panel title present");
    assert!(
        title_line > proj_line,
        "right panel should be below left in narrow mode (title={title_line}, proj={proj_line})"
    );
}

#[test]
fn no_live_preview_muxed_session_shows_privacy_banner() {
    // Build a session that's muxed (has one LinkedToMux candidate),
    // load with --no-live-preview, and ensure the preview block
    // shows the privacy banner rather than the graph snippet.
    use crate::model::{
        Confidence, GraphLink, GraphNode, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode,
        NodeId, Provenance, RelationKind,
    };

    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_last_message_preview("stale msg".to_string()),
    ));
    snapshot
        .nodes
        .push(GraphNode::MuxSession(MuxSessionNode::new(
            MuxSessionId::new("editor"),
            "tmux".to_string(),
            "editor".to_string(),
        )));
    let session_id = NodeId::AgentSession(AgentSessionId::new("codex", "/state", "abc"));
    let mux_id = NodeId::MuxSession(MuxSessionId::new("editor"));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: session_id,
        target: LinkEndpoint::Node { id: mux_id },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    config.live_preview_enabled = false;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::NavDown); // jump from repo group → session row

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("preview disabled"),
        "expected --no-live-preview banner in right panel, got: {text}"
    );
    // Per the locked mockup decision, `--no-live-preview` does
    // NOT suppress same-line previews in the row tree — only
    // live extras (pane capture + transcript-tail) in the
    // right panel. The graph-resident `stale msg` is allowed
    // to remain in the session row.
}

#[test]
fn right_focus_keeps_selected_row_highlighted_and_changes_status_scope() {
    let mut app = seeded_app();
    app.update(Msg::NavDown);
    app.update(Msg::NavDown);
    app.update(Msg::CycleFocus);

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        // Right-focus hint now describes the explorer
        // cursor instead of preview scroll.
        text.contains("j/k cursor"),
        "right focus status hint missing: {text}"
    );
    assert!(
        text.contains("[right]"),
        "right focus marker missing: {text}"
    );

    // Selected row should still carry the inactive-selection
    // indicator (BOLD without REVERSED) in the left pane while
    // focus is on the right pane.
    let selected_carries_inactive_indicator = (0..buffer.area.height).any(|y| {
        let left_width = buffer.area.width / 2;
        let any_bold =
            (0..left_width).any(|x| buffer[(x, y)].style().add_modifier.contains(Modifier::BOLD));
        let any_reversed = (0..left_width).any(|x| {
            buffer[(x, y)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        });
        any_bold && !any_reversed
    });
    assert!(
        selected_carries_inactive_indicator,
        "selected row should remain highlighted via BOLD when right pane has focus"
    );
}

#[test]
fn contextual_status_offers_enter_view_for_unmuxed_session() {
    let mut app = seeded_app();
    app.update(Msg::NavDown);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    // Un-muxed agent sessions now advertise Enter
    // (and `v`) as the primary default action rather than the
    // attach-disabled reason. Sessions backed by a harness that
    // exposes a resume command additionally surface `S` resume.
    assert!(
        text.contains("Enter/v view"),
        "expected Enter/v view hint for un-muxed session: {text}"
    );
    assert!(
        !text.contains("attach: session is not attached to any mux"),
        "Enter hint should replace the attach-disabled reason on viewable rows: {text}"
    );
    assert!(
        text.contains("S resume"),
        "codex sessions should advertise S resume: {text}"
    );
}

#[test]
fn contextual_status_advertises_enter_attach_for_muxed_session() {
    // Muxed session: Enter (and `a`) attach to the resolved mux.
    let app = muxed_app("editor", None);
    // muxed_app already navigates onto the session row.
    let mut app = app;
    app.update(Msg::NavDown);
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("Enter/a attach"),
        "expected Enter/a attach hint for muxed session: {text}"
    );
}

#[test]
fn status_bar_shows_group_filter_and_sort_settings() {
    let mut app = seeded_app();
    let _ = app.update(crate::tui::Msg::SetGrouping(
        crate::tui::Grouping::Sessions(crate::tui::SessionsGrouping::None),
    ));
    let _ = app.update(crate::tui::Msg::SetFilter(crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..crate::filter::RowFilter::default()
    }));

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("group:none"),
        "grouping chip missing from status bar: {text}"
    );
    assert!(
        text.contains("filter:harness:codex"),
        "filter chip missing from status bar: {text}"
    );
    assert!(
        text.contains("sort:recency"),
        "sort chip missing from status bar: {text}"
    );
}

#[test]
fn contextual_status_offers_ambiguous_attach_hint_with_inspect_affordance() {
    // When the selected agent-session row resolves to an
    // ambiguous mux candidate set, the status bar advertises the
    // preferred-target attach and points at the right pane, which lists
    // the competing candidates. (`m` belongs to the Mux menu, ADR 0096.)
    //
    // Forcing `MuxIndicator::Ambiguous` on a session row goes
    // through the cwd-suppression path in `resolve::mod`: two
    // distinct sessions claim the same mux via `exact_cwd_match`
    // evidence, which makes the resolver leave
    // `selected_link_id = None` on the `LinkedToMux` slot. The
    // sessions row-tree builder then surfaces every competing
    // candidate, so the row reports `candidate_count = 2`.
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionId, MuxSessionNode, NodeId,
        Provenance, RelationKind, SourceMetadata,
    };

    fn cwd_link(id: &str, session: AgentSessionId, mux: MuxSessionId) -> GraphLink {
        let mut metadata = SourceMetadata::default();
        metadata.fields.insert(
            "match_kind".to_string(),
            serde_json::json!("exact_cwd_match"),
        );
        GraphLink {
            id: id.to_string(),
            source: NodeId::AgentSession(session),
            target: LinkEndpoint::Node {
                id: NodeId::MuxSession(mux),
            },
            relation: RelationKind::LinkedToMux,
            provenance: Provenance::Discovered,
            confidence: Confidence::Medium,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: metadata,
            state: LinkState::Active,
        }
    }

    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(
            "/home/op/src/proj",
        ))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new("/home/op/src/proj"), "/home/op/src/proj"),
        root: "/home/op/src/proj".to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string()),
    ));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "def"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string()),
    ));
    let editor = MuxSessionId::new("tmux:editor");
    snapshot.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(editor.clone(), "tmux".to_string(), "editor".to_string())
            .with_cwd("/home/op/src/proj".to_string()),
    ));
    let scratch = MuxSessionId::new("tmux:scratch");
    snapshot.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(scratch.clone(), "tmux".to_string(), "scratch".to_string())
            .with_cwd("/home/op/src/proj".to_string()),
    ));
    // The first session has cwd-evidence links to two muxes; the
    // second session sits in the same cwd and pins each mux as
    // well. That gives both muxes "multiple distinct sessions"
    // and triggers the cwd-suppression path on both slots for
    // session `abc`, leaving `selected_link_id = None`.
    snapshot.candidate_links.push(cwd_link(
        "abc-editor",
        AgentSessionId::new("codex", "/state", "abc"),
        editor.clone(),
    ));
    snapshot.candidate_links.push(cwd_link(
        "abc-scratch",
        AgentSessionId::new("codex", "/state", "abc"),
        scratch.clone(),
    ));
    snapshot.candidate_links.push(cwd_link(
        "def-editor",
        AgentSessionId::new("codex", "/state", "def"),
        editor,
    ));
    snapshot.candidate_links.push(cwd_link(
        "def-scratch",
        AgentSessionId::new("codex", "/state", "def"),
        scratch,
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    // Step past the group row onto the first (ambiguous) session row.
    app.update(Msg::NavDown);

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("Enter/a attach preferred"),
        "ambiguous row should advertise preferred attach: {text}"
    );
    assert!(
        text.contains("Tab inspect candidates"),
        "ambiguous row should point at the candidate list: {text}"
    );
    assert!(
        !text.contains("m choose"),
        "`m` opens the Mux menu, not a candidate picker: {text}"
    );
}

#[test]
fn contextual_status_for_group_row_advertises_expand_collapse_folding() {
    // A group-row selection should surface the
    // expand/collapse fold bindings, not an attach hint.
    let mut app = seeded_app();
    // Auto-selection lands on the project group row, which is
    // exactly what we want to assert against.
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("Enter/l expand"),
        "group row should advertise expand: {text}"
    );
    assert!(
        text.contains("h collapse"),
        "group row should advertise collapse: {text}"
    );
}

#[test]
fn status_bar_renders_spinner_chip_for_in_flight_discovery() {
    // An in-flight discovery op surfaces an animated
    // Braille-spinner chip + "Discovering" label in the status bar.
    // The reducer stamps `started_at` so we can't assert on the
    // exact glyph frame (elapsed-derived), but every Braille
    // spinner glyph is in the braille spinner set — grep for the label
    // as the reliable presence check.
    let mut app = seeded_app();
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Discovery,
        label: "Discovering".to_string(),
    });
    app.update(Msg::SetStatus(None));

    let area = Rect::new(0, 0, 220, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("Discovering"),
        "in-flight discovery chip should render: {text}"
    );
    // Spot-check that at least one Braille glyph from the spinner
    // set appears in the buffer.
    let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"]
        .iter()
        .any(|g| text.contains(g));
    assert!(
        has_spinner,
        "in-flight chip should render a Braille spinner glyph: {text}"
    );
}

#[test]
fn status_bar_hides_spinner_after_in_flight_finish() {
    // Finish removes the tracker; the chip disappears.
    let mut app = seeded_app();
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Discovery,
        label: "Discovering".to_string(),
    });
    app.update(Msg::InFlightFinish(
        crate::tui::app::InFlightKind::Discovery,
    ));
    app.update(Msg::SetStatus(None));

    let area = Rect::new(0, 0, 220, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        !text.contains("Discovering"),
        "finished op should not render its chip: {text}"
    );
}

#[test]
fn status_bar_renders_stale_chip_when_refresh_failure_recorded() {
    // A recorded refresh failure surfaces a `stale` chip
    // ahead of any provider chips so the operator notices the
    // background data is older than expected. The test runs at
    // 220 columns to keep the contextual left-zone text from
    // cropping the chip suffix.
    let mut app = seeded_app();
    app.update(Msg::SetRefreshFailure("network unavailable".to_string()));
    app.update(Msg::SetStatus(None));

    let area = Rect::new(0, 0, 220, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("stale"),
        "stale chip should render once a refresh failure is recorded: {text}"
    );
}

#[test]
fn status_bar_renders_provider_error_chip_for_unavailable_tmux() {
    // A tmux provider error renders a right-zone chip
    // labelled `tmux:<reason>` so the operator sees why the mux
    // surface is empty.
    let mut app = seeded_app();
    app.update(Msg::SetProviderStatus(crate::tui::app::ProviderStatus {
        tmux_disabled: false,
        tmux_available: Some(false),
        tmux_reason: Some("missing binary".to_string()),
        forge_disabled: false,
        forge_available: None,
        forge_reason: None,
    }));

    let area = Rect::new(0, 0, 220, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("tmux:missing binary"),
        "provider error chip should surface the reason: {text}"
    );
}

#[test]
fn contextual_status_surfaces_disabled_attach_reason_for_current_tmux_session() {
    // When the selected row is muxed but the preferred
    // mux happens to be the operator's *current* tmux session,
    // `resolve_attach_target` returns `CurrentTmuxSession`. The
    // status hint should fall through to `attach_disabled_reason`
    // so the operator sees a "refusing to attach …" cue instead
    // of the default `Enter/a attach` text.
    let mut app = muxed_app("editor", None);
    app.update(Msg::NavDown);
    // Pretend conspectus was launched inside the same tmux
    // session the selected row is attached to. The RunConfig
    // field is normally populated from `$TMUX` at startup.
    app.config_mut().current_tmux_session = Some("editor".to_string());

    let area = Rect::new(0, 0, 160, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    assert!(
        text.contains("refusing to attach current tmux session `editor`"),
        "current-tmux row should surface the disabled-attach reason: {text}"
    );
    assert!(
        !text.contains("Enter/a attach"),
        "Enter/a attach hint must not render when attach is disabled: {text}"
    );
}

#[test]
fn mux_preview_renders_compact_header_and_bottom_cropped_capture() {
    let mut app = muxed_app(
        "agentdeck_conspectus-very-long-session-name-with-suffix_12345678",
        Some("line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7"),
    );
    app.update(Msg::ScrollPreviewBy(1));
    app.update(Msg::ScrollPreviewBy(-1));

    let area = Rect::new(0, 0, 100, 14);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);
    let preview_divider = text
        .lines()
        .find(|l| l.contains(" Preview "))
        .expect("preview divider line present");
    // The pane label + captured-time freshness now live in the
    // Mux detail section above, so they should NOT appear on
    // the preview divider itself — that was the duplication
    // the styling refresh removed.
    assert!(
        !preview_divider.contains("tmux:"),
        "preview divider should no longer duplicate the mux pane label: \
             {preview_divider}",
    );
    assert!(
        !preview_divider.contains("captured"),
        "preview divider should drop the captured-time tag (now in the Mux \
             section): {preview_divider}",
    );
    assert!(
        !text.contains("line 1"),
        "oldest capture lines should be cropped out: {text}"
    );
    assert!(
        text.contains("line 7"),
        "latest capture line should remain visible: {text}"
    );
}

#[test]
fn compact_path_helpers_keep_primary_label_short() {
    assert_eq!(compact_path_label("~/src/conspectus"), "conspectus");
    assert_eq!(
        compact_path_secondary("~/src/conspectus"),
        "~/src/conspectus"
    );
    assert_eq!(compact_path_label("Ungrouped"), "Ungrouped");
    assert_eq!(compact_path_secondary("Ungrouped"), "");
}

#[test]
fn compact_path_helpers_split_workspace_display_at_double_space() {
    // `format_workspace_display` joins label, members, and
    // provider with `  ` separators. The renderer bolds the
    // label and renders the rest with `theme.placeholder`, so
    // the helpers must split there to keep the member list and
    // provider chip out of the bold span — parallel to how a
    // repo header bolds the basename and leaves the CWD path
    // non-bold.
    let display = "nix-config  config+personal+work-config  (agent-deck)";
    assert_eq!(compact_path_label(display), "nix-config");
    assert_eq!(
        compact_path_secondary(display),
        "config+personal+work-config  (agent-deck)"
    );

    // Workspace with no members still splits cleanly because
    // `format_workspace_display` keeps the `  (<provider>)`
    // separator.
    let display = "nix-config  (agent-deck)";
    assert_eq!(compact_path_label(display), "nix-config");
    assert_eq!(compact_path_secondary(display), "(agent-deck)");
}

#[test]
fn mux_preview_skips_the_blank_rows_below_a_quiet_panes_output() {
    // A server pane: a few lines of output, then the rest of the screen
    // blank. The bottom rows are the output, not the empty screen.
    let capture = format!(
        "$ conspectus serve\nlistening on :7777\n{}",
        "\n".repeat(40)
    );
    let app = muxed_app("serve", Some(&capture));
    let preview = preview_text_for_selection(&app, 80, 5);
    let rows: Vec<String> = preview.lines.iter().map(ToString::to_string).collect();
    assert_eq!(rows, ["$ conspectus serve", "listening on :7777"]);
}

#[test]
fn mux_preview_keeps_the_newest_rows_that_fit() {
    let capture = (1..=30)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let app = muxed_app("busy", Some(&capture));
    let preview = preview_text_for_selection(&app, 80, 3);
    let rows: Vec<String> = preview.lines.iter().map(ToString::to_string).collect();
    assert_eq!(rows, ["line 28", "line 29", "line 30"]);
}

#[test]
fn mux_preview_follows_the_wrap_mode() {
    let capture = format!("{}\n> prompt", "─".repeat(60));
    let mut app = muxed_app("agent", Some(&capture));
    let rows = |app: &App| -> Vec<String> {
        preview_text_for_selection(app, 20, 10)
            .lines
            .iter()
            .map(ToString::to_string)
            .collect()
    };
    assert_eq!(rows(&app), ["─".repeat(20), "> prompt".to_string()]);
    app.update(Msg::SetPreviewWrap(crate::tui::PreviewWrap::Plain));
    assert_eq!(rows(&app).len(), 4, "the rule wraps onto three rows");
}

#[test]
fn mux_preview_of_an_all_blank_pane_says_empty() {
    let app = muxed_app("idle", Some("\n\n   \n"));
    let preview = preview_text_for_selection(&app, 80, 5).to_string();
    assert_eq!(preview, "(empty pane)");
}

#[test]
fn render_captured_pane_with_color_parses_ansi_into_styled_spans() {
    // ESC[31m makes "red", ESC[0m resets.
    let raw = "\x1b[31mred\x1b[0m  plain";
    let text = render_captured_pane(raw, true);
    // Flattened content matches the visible characters.
    assert_eq!(text.to_string(), "red  plain");
    // The first line's first span carries red foreground style.
    let first_line = text.lines.first().expect("at least one line");
    let first_span = first_line.spans.first().expect("at least one span");
    assert_eq!(first_span.content, "red");
    assert_eq!(
        first_span.style.fg,
        Some(ratatui::style::Color::Red),
        "expected red fg on the red span"
    );
}

#[test]
fn render_captured_pane_without_color_strips_styling() {
    let raw = "\x1b[31mred\x1b[0m  plain";
    let text = render_captured_pane(raw, false);
    assert_eq!(text.to_string(), "red  plain");
    // With colour disabled every span should land styled
    // identically to a plain `Text::raw` — i.e. default fg.
    for line in &text.lines {
        for span in &line.spans {
            assert_eq!(
                span.style.fg, None,
                "expected colour to be stripped, got span={span:?}"
            );
        }
    }
}

#[test]
fn render_captured_pane_falls_back_to_plain_text_on_malformed_input() {
    // Lone ESC byte — ansi-to-tui should either parse harmlessly
    // or fail; either way `render_captured_pane` returns
    // something printable rather than panicking.
    let raw = "before\x1bafter";
    let text = render_captured_pane(raw, true);
    let flattened = text.to_string();
    // The visible characters either side of the rogue ESC
    // must survive — operators don't lose pane content to a
    // single bad byte.
    assert!(
        flattened.contains("before"),
        "expected 'before' in output, got: {flattened:?}"
    );
    assert!(
        flattened.contains("after"),
        "expected 'after' in output, got: {flattened:?}"
    );
}

#[test]
fn left_panel_scrolls_to_keep_selected_row_visible_past_viewport() {
    // Build a snapshot with one repo and twenty sessions so
    // the rendered tree spills well past a small viewport. The
    // repo path is intentionally long: a past regression
    // wrapped that group row in the left tree but
    // computed scroll offsets as if every row occupied one
    // physical line. That put the selected row one line below
    // the viewport instead of on the bottom line.
    let repo_root =
        "/home/op/src/proj-with-a-very-long-display-path-that-would-wrap-before-clipping";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_root))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(repo_root), repo_root),
        root: repo_root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    for i in 0..20 {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                "codex".to_string(),
            )
            .with_cwd(repo_root.to_string()),
        ));
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });

    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    // Jump to the last visible row — it lives well below the
    // viewport for a 10-tall window.
    app.update(Msg::End);

    // Render into a side-by-side 120x24 window. The left panel
    // inner viewport is 20 rows tall, so the final selected row
    // should land exactly on y=21, the bottom content row.
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let text = buffer_to_string(&buffer);

    // The last row should be visible. Confirm via the external
    // session id of the last session pushed (s19).
    let visible = app.visible_rows();
    let last_session_id = match &visible.last().unwrap().kind {
        RowKind::AgentSession(s) => s.session.session_key.clone(),
        other => panic!("expected last row to be a session, got {other:?}"),
    };
    assert!(
        text.contains(&last_session_id),
        "selected row's external id ({last_session_id}) should be visible after End; got:\n{text}"
    );
    let bottom_left_line: String = (1..59).map(|x| buffer[(x, 21)].symbol()).collect();
    assert!(
        bottom_left_line.contains(&last_session_id),
        "selected row's external id ({last_session_id}) should land on the bottom visible left-panel line; got {bottom_left_line:?}\n{text}"
    );

    // The first row (the repo group) should now be scrolled
    // off the top.
    let top_left_line: String = (1..59).map(|x| buffer[(x, 2)].symbol()).collect();
    assert!(
        !top_left_line.contains("proj-with-a-very-long"),
        "top-of-tree group should be scrolled away when selection is at End; got top line {top_left_line:?}\n{text}"
    );
}

/// Build an app focused on a workspace whose detail pane has
/// `repo_count` validated `WorkspaceContainsRepo` rows. Used to
/// exercise the right-pane scroll + preview-floor invariants
/// when the Related list is taller than the available header.
fn workspace_app_with_repos(repo_count: usize) -> App {
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, NodeId, Provenance, RelationKind,
        WorkspaceId, WorkspaceNode,
    };

    let mut snapshot = GraphSnapshot::empty();
    let workspace_root = "/home/op/work/multi";
    snapshot.nodes.push(GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(workspace_root),
        root: workspace_root.to_string(),
        provider: None,
        name: Some("multi".to_string()),
    }));
    let workspace_id = NodeId::Workspace(WorkspaceId::new(workspace_root));
    for idx in 0..repo_count {
        let common_dir = format!("/srv/git/repo-{idx:02}.git");
        let repo_id = RepoId::new(&common_dir);
        snapshot.nodes.push(GraphNode::Repo(RepoNode {
            id: repo_id.clone(),
            common_dir: common_dir.clone(),
            source_paths: Vec::new(),
            remotes: Vec::new(),
        }));
        snapshot.candidate_links.push(GraphLink {
            id: format!("ws-repo-{idx:02}"),
            source: workspace_id.clone(),
            target: LinkEndpoint::Node {
                id: NodeId::Repo(repo_id),
            },
            relation: RelationKind::WorkspaceContainsRepo,
            provenance: Provenance::StrongDiscovered,
            confidence: Confidence::High,
            freshness: crate::model::Freshness::Fresh,
            source_metadata: crate::model::SourceMetadata::default(),
            state: LinkState::Active,
        });
    }
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Workspace,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    config.sessions_grouping = SessionsGrouping::Workspace;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let workspace_row = app
        .tree()
        .rows
        .iter()
        .find_map(|r| match &r.id {
            crate::tui::rows::RowId::Group(NodeId::Workspace(_)) => Some(r.id.clone()),
            _ => None,
        })
        .expect("workspace row");
    app.set_selection(workspace_row);
    app
}

#[test]
fn right_pane_scrolls_to_keep_explorer_cursor_visible() {
    // Walk the cursor onto the last validated link row.
    use crate::tui::explorer::ExplorerRow;

    // When the workspace detail pane has more Related rows than
    // the header zone can hold at a small terminal height, the
    // cursor must stay in the viewport as the operator navigates
    // down. Mirrors the left pane's `End`-scroll behavior.
    let mut app = workspace_app_with_repos(20);
    app.update(Msg::CycleFocus);

    // Small terminal: 100 columns wide, 20 rows tall. The right
    // pane is roughly half (~50 cols) and the explorer header is
    // capped to leave room for the preview, so 20 repo rows
    // cannot all fit at once.
    let area = Rect::new(0, 0, 100, 20);

    // Walk down a few rows from the top of the explorer. The
    // top validated rows should remain in view.
    for _ in 0..3 {
        app.update(Msg::ExplorerNavDown);
    }
    let initial = buffer_to_string(&render_to_buffer(&mut app, area));
    assert!(
        initial.contains("repo-00.git"),
        "early rows should be visible before scrolling: {initial}"
    );

    let last_link_idx = app
        .explorer()
        .expect("state")
        .rows()
        .iter()
        .enumerate()
        .filter_map(|(idx, row)| match row {
            ExplorerRow::ValidatedLink { .. } => Some(idx),
            _ => None,
        })
        .next_back()
        .expect("at least one validated link row");
    let current = app.explorer().expect("state").cursor;
    for _ in current..last_link_idx {
        app.update(Msg::ExplorerNavDown);
    }

    let buffer = render_to_buffer(&mut app, area);
    let scrolled = buffer_to_string(&buffer);
    assert!(
        scrolled.contains("repo-19.git"),
        "last validated row must stay in the viewport after navigating to it: {scrolled}"
    );
    assert!(
        !scrolled.contains("repo-00.git"),
        "early rows should have scrolled off the top once the cursor reaches the end: {scrolled}"
    );
    assert!(
        app.explorer_scroll() > 0,
        "scroll offset should have advanced past zero; got {}",
        app.explorer_scroll(),
    );

    // Regression: the chip divider above the validated rows
    // (`Related N validated · M other`) is sized to the
    // paragraph's render width. Pre-fix, the divider's width
    // was computed against the full inner width; once
    // `scrollbar_layout` reserved a gutter, the paragraph
    // rendered at one less column and the divider wrapped a
    // few characters onto a second row. Every cursor row
    // below the divider then landed one row lower than the
    // wrap math predicted, leaving the cursor visible off
    // the bottom of the viewport. The cursor row (`repo-19`)
    // must appear *strictly above* the Preview divider, never
    // at or past its row.
    let right_pane_x = 50u16..area.width.saturating_sub(1);
    let cursor_row_y = (0..area.height)
        .find(|&y| {
            let line: String = right_pane_x
                .clone()
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.contains("repo-19.git")
        })
        .expect("cursor row visible");
    let preview_row_y = (0..area.height)
        .find(|&y| {
            let line: String = right_pane_x
                .clone()
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.contains("Preview")
        })
        .expect("preview divider visible");
    assert!(
        cursor_row_y < preview_row_y,
        "cursor row (y={cursor_row_y}) must sit above the Preview divider (y={preview_row_y}); the off-by-one bug from the divider wrap would let it sit at or past the divider\n{scrolled}",
    );
}

#[test]
fn right_pane_preview_keeps_minimum_height_when_related_full() {
    // Regression: when the Related list is taller than the
    // right pane, the explorer header used to grow until the
    // preview zone collapsed to 2 rows. The renderer now caps
    // the header so the preview zone keeps a usable minimum.
    let mut app = workspace_app_with_repos(40);
    let area = Rect::new(0, 0, 100, 30);
    let buffer = render_to_buffer(&mut app, area);

    // Locate the 1-row Preview divider that separates the
    // explorer header from the preview body. It's the line that
    // carries the `Preview` chip; find it by scanning right-pane
    // columns for the divider chip text.
    let right_start = 50u16;
    let mut divider_row: Option<u16> = None;
    for y in 0..area.height {
        let line: String = (right_start..area.width.saturating_sub(1))
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if line.contains("Preview") {
            divider_row = Some(y);
            break;
        }
    }
    let divider_row = divider_row.expect("preview divider should be visible");
    // The preview body sits between the divider and the bottom
    // border. Assert it has at least MIN_PREVIEW_HEIGHT rows so
    // a full Related list cannot crowd it out.
    let bottom_border = area.height.saturating_sub(1);
    let preview_body_rows = bottom_border.saturating_sub(divider_row + 1);
    assert!(
        preview_body_rows >= 6,
        "preview zone should keep at least 6 body rows even when Related is full; got {preview_body_rows} (divider at row {divider_row})\n{}",
        buffer_to_string(&buffer)
    );
}

/// Set of glyphs `ratatui::widgets::Scrollbar` paints by default
/// for `ScrollbarOrientation::VerticalRight`. The exact symbol
/// set sits in `ratatui_core::symbols::scrollbar::DOUBLE_VERTICAL`.
/// Tests look for *any* of these in the rendered buffer so we
/// don't pin the precise glyph (ratatui may swap them later) but
/// can still assert "a scrollbar is present" robustly.
const SCROLLBAR_GLYPHS: &[&str] = &["█", "║", "▲", "▼"];

fn buffer_column(buffer: &ratatui::buffer::Buffer, x: u16) -> String {
    (0..buffer.area.height)
        .map(|y| buffer[(x, y)].symbol())
        .collect()
}

fn rightmost_inner_column(area: Rect) -> u16 {
    area.x + area.width - 2
}

#[test]
fn left_pane_renders_scrollbar_when_content_exceeds_viewport() {
    // Build the same overflowing tree the
    // `left_panel_scrolls_to_keep_selected_row_visible_past_viewport`
    // test uses, then assert that ADR 0076's scrollbar glyphs
    // appear in the rightmost column of the left pane's inner
    // area.
    let repo_root =
        "/home/op/src/proj-with-a-very-long-display-path-that-would-wrap-before-clipping";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_root))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(repo_root), repo_root),
        root: repo_root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    for i in 0..20 {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                "codex".to_string(),
            )
            .with_cwd(repo_root.to_string()),
        ));
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    // 120x24 — left pane spans roughly x=0..60. The inner area
    // (post-border) sits at x=1..59; the scrollbar rides the
    // rightmost inner column.
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let left_pane = split[0];
    let bar_col = rightmost_inner_column(left_pane);
    let column = buffer_column(&buffer, bar_col);
    assert!(
        SCROLLBAR_GLYPHS.iter().any(|g| column.contains(g)),
        "expected a scrollbar glyph in left-pane column {bar_col}; got {column:?}\n{}",
        buffer_to_string(&buffer)
    );
}

#[test]
fn left_pane_hides_scrollbar_when_content_fits() {
    // The default seeded app holds one repo + one checkout + one
    // session — three rows total. With a 24-row terminal there
    // is nothing to scroll, so the bar must stay hidden
    // (fade-on-fit, ADR 0076).
    let mut app = seeded_app();
    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let left_pane = split[0];
    let bar_col = rightmost_inner_column(left_pane);
    let column = buffer_column(&buffer, bar_col);
    for glyph in SCROLLBAR_GLYPHS {
        assert!(
            !column.contains(glyph),
            "left-pane column {bar_col} should not carry the `{glyph}` scrollbar glyph when content fits; got {column:?}\n{}",
            buffer_to_string(&buffer)
        );
    }
}

#[test]
fn right_pane_explorer_renders_scrollbar_when_related_list_overflows() {
    // Workspace with 20 repos focused → the validated zone is
    // taller than the right-pane header at this terminal size,
    // so the explorer scrollbar should be drawn on the
    // rightmost inner column of the right pane.
    let mut app = workspace_app_with_repos(20);
    app.update(Msg::CycleFocus);
    let area = Rect::new(0, 0, 120, 20);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let right_pane = split[1];
    let bar_col = rightmost_inner_column(right_pane);
    let column = buffer_column(&buffer, bar_col);
    assert!(
        SCROLLBAR_GLYPHS.iter().any(|g| column.contains(g)),
        "expected a scrollbar glyph in right-pane column {bar_col} when the related list overflows; got {column:?}\n{}",
        buffer_to_string(&buffer)
    );
}

#[test]
fn left_pane_scrollbar_thumb_reaches_bottom_at_max_scroll() {
    // Regression: feeding `position = scroll_offset` to
    // `ScrollbarState` left the thumb stranded mid-track at
    // max scroll because ratatui's `Scrollbar` treats
    // `position` as an index `0..content_length-1`. Once the
    // operator scrolls to the bottom, the thumb glyph (`█`)
    // must land on or below the track midpoint, *and* in a
    // row visibly past the midpoint of the inner area.
    let repo_root =
        "/home/op/src/proj-with-a-very-long-display-path-that-would-wrap-before-clipping";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_root))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(repo_root), repo_root),
        root: repo_root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    for i in 0..40 {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                "codex".to_string(),
            )
            .with_cwd(repo_root.to_string()),
        ));
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app.update(Msg::End);

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let left_pane = split[0];
    let bar_col = rightmost_inner_column(left_pane);

    // Find the rows occupied by the thumb glyph (the solid
    // `█`). Track glyphs (`║`) and arrow glyphs (`▲`/`▼`) live
    // above and below the thumb on the same column.
    let thumb_rows: Vec<u16> = (0..buffer.area.height)
        .filter(|&y| buffer[(bar_col, y)].symbol() == "█")
        .collect();
    assert!(
        !thumb_rows.is_empty(),
        "scrollbar thumb should be rendered at the rightmost left-pane column ({bar_col}); got column:\n{}\nbuffer:\n{}",
        buffer_column(&buffer, bar_col),
        buffer_to_string(&buffer),
    );
    // The thumb's bottom must extend past the vertical midpoint
    // of the inner area. The inner area for the left pane spans
    // y=1..(height-1)=23, so the midpoint is around y=11. At
    // max scroll the thumb must reach below it.
    let last_thumb_row = *thumb_rows.iter().max().unwrap();
    let inner_midpoint = left_pane.y + left_pane.height / 2;
    assert!(
        last_thumb_row > inner_midpoint,
        "thumb's bottom row ({last_thumb_row}) must extend past inner midpoint ({inner_midpoint}) at max scroll; got rows {thumb_rows:?}",
    );
}

#[test]
fn left_pane_scrollbar_column_carries_only_scrollbar_glyphs() {
    // Regression: pre-fix, the paragraph painted the entire
    // inner area and the scrollbar overpainted the rightmost
    // column. `Buffer::set_string` patches styles, so the
    // selection's `REVERSED` modifier on the underlying cell
    // bled through onto the scrollbar glyph. Reserving a
    // dedicated gutter column means scrollbar cells never
    // carry text from the paragraph.
    //
    // Walk the scrollbar column row by row and skip border /
    // empty cells (the framework draws the pane border around
    // the inner area). Every *non-blank, non-border* cell in
    // the scrollbar column must be one of the scrollbar
    // glyphs — never a borrowed paragraph character.
    let repo_root =
        "/home/op/src/proj-with-a-very-long-display-path-that-would-wrap-before-clipping";
    let mut snapshot = GraphSnapshot::empty();
    snapshot
        .nodes
        .push(GraphNode::Repo(RepoNode::new(RepoId::new(repo_root))));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(RepoId::new(repo_root), repo_root),
        root: repo_root.to_string(),
        git_dir: None,
        current_branch: None,
        worktree: None,
        head_commit: None,
    }));
    for i in 0..30 {
        snapshot.nodes.push(GraphNode::AgentSession(
            AgentSessionNode::new(
                AgentSessionId::new("codex", "/state", format!("s{i:02}")),
                "codex".to_string(),
            )
            .with_cwd(repo_root.to_string()),
        ));
    }
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });

    let area = Rect::new(0, 0, 120, 24);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let left_pane = split[0];
    let bar_col = rightmost_inner_column(left_pane);
    // Borders use box-drawing characters. Allow them through.
    let border_glyphs = ["─", "│", "┌", "┐", "└", "┘", "├", "┤", "┬", "┴", "┼"];

    let mut scrollbar_glyph_rows = 0;
    for y in 0..buffer.area.height {
        let symbol = buffer[(bar_col, y)].symbol();
        if symbol.is_empty()
            || symbol == " "
            || border_glyphs.contains(&symbol)
            || SCROLLBAR_GLYPHS.contains(&symbol)
        {
            if SCROLLBAR_GLYPHS.contains(&symbol) {
                scrollbar_glyph_rows += 1;
            }
            continue;
        }
        panic!(
            "scrollbar column {bar_col} row {y} must not contain paragraph text; got {symbol:?}\n{}",
            buffer_to_string(&buffer)
        );
    }
    assert!(
        scrollbar_glyph_rows > 0,
        "expected at least one scrollbar glyph row in column {bar_col}\n{}",
        buffer_to_string(&buffer),
    );
}

#[test]
fn right_pane_explorer_hides_scrollbar_when_related_list_fits() {
    // A workspace with two repos has only two validated rows;
    // the explorer header comfortably fits in any non-tiny
    // terminal so no scrollbar should render.
    let mut app = workspace_app_with_repos(2);
    let area = Rect::new(0, 0, 120, 30);
    let buffer = render_to_buffer(&mut app, area);
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let right_pane = split[1];
    let bar_col = rightmost_inner_column(right_pane);
    let column = buffer_column(&buffer, bar_col);
    for glyph in SCROLLBAR_GLYPHS {
        assert!(
            !column.contains(glyph),
            "right-pane column {bar_col} should not carry the `{glyph}` glyph when content fits; got {column:?}\n{}",
            buffer_to_string(&buffer)
        );
    }
}

#[test]
fn mux_view_header_counts_sessions_in_visible_muxes() {
    // The Mux view renders single agents inline in their mux
    // row, so counting agent-session rows read `0/M sessions`.
    use crate::model::{
        Confidence, GraphLink, LinkEndpoint, LinkState, MuxSessionNode, Provenance, RelationKind,
    };
    let mux_id = MuxSessionId::new("work");
    let codex = AgentSessionId::new("codex", "/state", "abc");
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(codex.clone(), "codex".to_string())
            .with_cwd("/home/op/src/proj".to_string()),
    ));
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("claude-code", "/state", "xyz"),
            "claude-code".to_string(),
        )
        .with_cwd("/home/op/src/other".to_string()),
    ));
    snapshot.nodes.push(GraphNode::MuxSession(
        MuxSessionNode::new(mux_id.clone(), "tmux".to_string(), "work".to_string())
            .with_cwd("/home/op/src/proj".to_string()),
    ));
    snapshot.candidate_links.push(GraphLink {
        id: "session-mux".to_string(),
        source: NodeId::AgentSession(codex),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_id),
        },
        relation: RelationKind::LinkedToMux,
        provenance: Provenance::Discovered,
        confidence: Confidence::Medium,
        freshness: crate::model::Freshness::Fresh,
        source_metadata: crate::model::SourceMetadata::default(),
        state: LinkState::Active,
    });
    let snapshot = resolve_snapshot(snapshot);
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot: &snapshot,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        filter: RowFilter::default(),
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Mux;
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let area = Rect::new(0, 0, 120, 12);
    let header = |app: &mut App| {
        buffer_to_string(&render_to_buffer(app, area))
            .lines()
            .next()
            .expect("header line")
            .to_string()
    };

    let unfiltered = header(&mut app);
    assert!(
        unfiltered.contains("2 sessions") && !unfiltered.contains("0/2"),
        "no filter: plain total: {unfiltered}"
    );

    app.update(Msg::SetFilter(RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["codex"])),
        ..RowFilter::default()
    }));
    let filtered = header(&mut app);
    assert!(
        filtered.contains("1/2 sessions"),
        "filtered: sessions in visible muxes: {filtered}"
    );
}

#[test]
fn reported_failure_leads_the_rows_preview_until_a_later_success() {
    use crate::tui::messages::{CommandRecord, LogEntry, LogTarget};
    let mut app = pinned_app(None);
    app.report(
        LogEntry::error("pin `ingest` launch failed: exited with status 1")
            .with_target(LogTarget::Pin("ingest".to_string()))
            .with_command(CommandRecord {
                argv: vec!["conspectus".into()],
                exit_code: Some(1),
                stdout: String::new(),
                stderr: "No conversation found with session ID: abc".into(),
            }),
    );

    let preview = preview_text_for_selection(&app, 80, 20).to_string();
    assert!(
        preview.starts_with("× pin `ingest` launch failed"),
        "{preview}"
    );
    assert!(preview.contains("exit status 1"), "{preview}");
    assert!(
        preview.contains("No conversation found with session ID: abc"),
        "{preview}"
    );
    assert!(
        preview.contains("Pin `ingest` (mux · planned)"),
        "the row's own preview follows the banner: {preview}"
    );

    app.report(
        LogEntry::info("pin `ingest` launched").with_target(LogTarget::Pin("ingest".to_string())),
    );
    let preview = preview_text_for_selection(&app, 80, 20).to_string();
    assert!(!preview.contains("launch failed"), "{preview}");
}

#[test]
fn unseen_failures_keep_a_status_bar_chip_until_the_log_is_opened() {
    use crate::tui::messages::LogEntry;
    let mut app = pinned_app(None);
    app.report(LogEntry::error("mux `w1` attach failed"));
    assert_eq!(
        app.status_message(),
        Some("mux `w1` attach failed · ! details")
    );

    // Navigation clears the status message; the chip stays.
    app.update(Msg::SetStatus(None));
    let text = buffer_to_string(&render_to_buffer(&mut app, Rect::new(0, 0, 120, 24)));
    assert!(text.contains("▲ 1 · ! messages"), "{text}");

    app.open_messages_overlay();
    assert_eq!(app.messages().unseen(), 0);
    app.close_messages_overlay();
    let text = buffer_to_string(&render_to_buffer(&mut app, Rect::new(0, 0, 120, 24)));
    assert!(!text.contains("! messages"), "{text}");
}

#[test]
fn info_report_sets_a_plain_status_message() {
    let mut app = pinned_app(None);
    app.report(crate::tui::messages::LogEntry::info("renamed: w1"));
    assert_eq!(app.status_message(), Some("renamed: w1"));
    assert_eq!(app.messages().unseen(), 0);
    assert_eq!(app.messages().len(), 1);
}

fn agentless_mux_row(pane_command: Option<&str>) -> MuxSessionRow {
    MuxSessionRow {
        mux: MuxSessionId::new("tmux:build"),
        backend: "tmux".into(),
        native_id: "build".into(),
        client_attached: Some(false),
        cwd_display: None,
        attached_count: 0,
        recency: Some("1m".into()),
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
        agent_labels: Vec::new(),
        program: pane_command.map(str::to_string),
        program_harness: None,
        single_session_preview: None,
        pin_id: None,
        primary_node: NodeId::MuxSession(MuxSessionId::new("tmux:build")),
        content_hit: None,
    }
}

#[test]
fn agentless_mux_row_labels_itself_with_the_pane_command() {
    let theme = Theme::default();
    let spans = render_mux_session_spans(&agentless_mux_row(Some("npm")), &theme, 0, 100, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();

    assert!(rendered.contains(" npm "), "pane command chip: {rendered}");
    assert!(!rendered.contains("no agent"), "no placeholder: {rendered}");
    let chip = spans
        .iter()
        .find(|span| span.content.contains("npm"))
        .expect("chip span");
    assert_eq!(chip.style.bg, Some(theme.command_badge));
}

#[test]
fn agentless_mux_row_without_a_pane_command_says_no_agent() {
    let theme = Theme::default();
    let spans = render_mux_session_spans(&agentless_mux_row(None), &theme, 0, 100, None);
    let rendered: String = spans.iter().map(|span| span.content.as_ref()).collect();

    assert!(rendered.contains("no agent"), "placeholder: {rendered}");
}

#[test]
fn agentless_mux_row_running_a_harness_uses_the_harness_badge() {
    let theme = Theme::default();
    for (program, label) in [("atelier", "codex"), ("claude", "claude")] {
        let row = MuxSessionRow {
            program_harness: Some(label.to_string()),
            ..agentless_mux_row(Some(program))
        };
        let spans = render_mux_session_spans(&row, &theme, 0, 100, None);
        let chip = spans
            .iter()
            .find(|span| span.content.trim() == label)
            .unwrap_or_else(|| panic!("{label} chip for {program}: {spans:?}"));
        assert_eq!(chip.style.fg, Some(theme.harness_color(label)), "{program}");
        assert_ne!(chip.style.bg, Some(theme.command_badge), "{program}");
    }
}

#[test]
fn row_awaiting_handoff_dims_and_spins_in_the_attach_cell() {
    use crate::model::MuxSessionId;
    let mut app = muxed_app("tmux:work", Some("before"));
    let session_row = app
        .visible_rows()
        .iter()
        .find(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| (*row).clone())
        .expect("session row");
    let render = |app: &App| {
        render_left_row(
            &session_row,
            app,
            false,
            100,
            1_700_000_000,
            None,
            GroupAlign::default(),
        )
    };

    let before = render(&app);
    let text: String = before.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains('⊸'),
        "settled row shows the attach glyph: {text}"
    );

    app.update(Msg::HandoffReturned(MuxSessionId::new("tmux:work")));
    let pending = render(&app);
    let text: String = pending.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        !text.contains('⊸'),
        "spinner replaces the attach glyph: {text}"
    );
    assert!(
        SPINNER_FRAMES.iter().any(|frame| text.contains(frame)),
        "pending row shows a spinner frame: {text}"
    );
    let dim = app.theme().placeholder;
    assert!(
        pending
            .spans
            .iter()
            .all(|s| s.style.add_modifier.contains(dim)),
        "every span of a pending row is dimmed"
    );

    app.update(Msg::HandoffSettled);
    let settled = render(&app);
    let text: String = settled.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        text.contains('⊸'),
        "settled row shows the attach glyph again: {text}"
    );
}

#[test]
fn mux_row_spinner_takes_the_attached_glyph_column() {
    let theme = Theme::default();
    let row = MuxSessionRow {
        mux: MuxSessionId::new("tmux:editor"),
        backend: "tmux".into(),
        native_id: "editor".into(),
        client_attached: Some(true),
        cwd_display: None,
        attached_count: 0,
        recency: Some("3s".into()),
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
        agent_labels: vec!["codex".into()],
        program: None,
        program_harness: None,
        single_session_preview: None,
        pin_id: None,
        primary_node: NodeId::MuxSession(MuxSessionId::new("tmux:editor")),
        content_hit: None,
    };
    let settled: String = render_mux_session_spans(&row, &theme, 0, 100, None)
        .iter()
        .map(|s| s.content.to_string())
        .collect();
    let pending: String = render_mux_session_spans(&row, &theme, 0, 100, Some("⠋"))
        .iter()
        .map(|s| s.content.to_string())
        .collect();
    assert_eq!(settled.replace('⊸', "⠋"), pending);
}

/// A transcript of `turns` alternating user and assistant messages,
/// `turn 1` through `turn N`.
fn transcript_document(turns: usize) -> crate::viewer::model::TranscriptDocument {
    use crate::viewer::model::{
        TranscriptDocument, TranscriptMeta, TranscriptTurn, TurnKind, TurnRole,
    };
    TranscriptDocument {
        meta: TranscriptMeta {
            harness: "codex".to_string(),
            session_key: "abc".to_string(),
            cwd: None,
        },
        turns: (1..=turns)
            .map(|n| TranscriptTurn {
                role: if n % 2 == 1 {
                    TurnRole::User
                } else {
                    TurnRole::Assistant
                },
                kind: TurnKind::Message,
                body: format!("turn {n}"),
                timestamp: None,
                aborted: false,
                hidden: false,
            })
            .collect(),
    }
}

fn codex_transcript() -> PreviewSource {
    PreviewSource::Transcript(AgentSessionId::new("codex", "/state", "abc"))
}

fn transcript_content(turns: usize) -> PreviewContent {
    PreviewContent::Transcript(Box::new(crate::viewer::state::ViewerState::new(
        transcript_document(turns),
    )))
}

/// An un-muxed codex session (`codex:/state:abc`), selected.
fn unmuxed_session_app(configure: impl FnOnce(&mut RunConfig)) -> App {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("codex", "/state", "abc"),
            "codex".to_string(),
        )
        .with_cwd("/home/op/src/proj".to_string())
        .with_last_message_preview("last words".to_string()),
    ));
    let snapshot = resolve_snapshot(snapshot);
    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: Some(std::path::Path::new("/home/op")),
        now: None,
        cwd: None,
        filter: RowFilter::default(),
    });
    let mut config = RunConfig::defaults();
    config.default_view = View::Sessions;
    configure(&mut config);
    let mut app = App::new(config);
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let session = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(row.kind, RowKind::AgentSession(_)))
        .map(|row| row.id.clone())
        .expect("session row");
    app.set_selection(session);
    app
}

fn preview_rows(app: &App, width: u16, height: usize) -> Vec<String> {
    preview_text_for_selection(app, width, height)
        .lines
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_session_in_tmux_previews_its_transcript_by_default() {
    let app = muxed_app_previewing(
        "editor",
        Some("pane output"),
        Vec::new(),
        crate::tui::SessionPreview::Transcript,
    );
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(codex_transcript())
    );
    let preview = preview_text_for_selection(&app, 80, 10).to_string();
    assert_eq!(preview, "loading transcript…");
}

#[test]
fn the_pane_choice_previews_a_session_in_tmux_as_its_pane() {
    let mut app = muxed_app_previewing(
        "editor",
        Some("pane output"),
        Vec::new(),
        crate::tui::SessionPreview::Pane,
    );
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(PreviewSource::Pane(MuxSessionId::new("editor")))
    );
    assert!(
        preview_text_for_selection(&app, 80, 10)
            .to_string()
            .contains("pane output")
    );

    let effects = app.update(Msg::SetSessionPreview(
        crate::tui::SessionPreview::Transcript,
    ));
    assert!(
        effects.contains(&crate::tui::Effect::Persist),
        "{effects:?}"
    );
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(codex_transcript())
    );
}

#[test]
fn a_session_outside_tmux_previews_its_transcript_whatever_the_choice() {
    let app = unmuxed_session_app(|config| {
        config.default_session_preview = crate::tui::SessionPreview::Pane;
    });
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(codex_transcript())
    );
}

#[test]
fn a_transcript_preview_keeps_its_newest_turns_in_view() {
    let mut app = unmuxed_session_app(|_| {});
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: transcript_content(30),
    });
    let rows = preview_rows(&app, 60, 6);
    assert_eq!(rows.len(), 6, "{rows:?}");
    assert!(
        rows.iter().any(|row| row.contains("turn 30")),
        "the newest turn is in view: {rows:?}"
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.contains("turn 1 ") || row.ends_with("turn 1")),
        "the oldest turn is not: {rows:?}"
    );
}

#[test]
fn scrolling_a_transcript_preview_moves_back_through_history() {
    let mut app = unmuxed_session_app(|_| {});
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: transcript_content(30),
    });
    let bottom = preview_rows(&app, 60, 6);

    // `K` scrolls up, toward older turns.
    for _ in 0..12 {
        app.update(Msg::ScrollPreviewBy(-1));
    }
    let scrolled = preview_rows(&app, 60, 6);
    assert!(
        !scrolled.iter().any(|row| row.contains("turn 30")),
        "{scrolled:?}"
    );
    assert!(
        scrolled.iter().any(|row| row.contains("turn 2")),
        "older turns come into view: {scrolled:?}"
    );

    // `J` scrolls back down, and stops at the newest turn.
    for _ in 0..40 {
        app.update(Msg::ScrollPreviewBy(1));
    }
    assert_eq!(app.preview_scroll(), 0);
    assert_eq!(preview_rows(&app, 60, 6), bottom);
}

#[test]
fn moving_the_selection_resets_the_preview_scroll() {
    // The repo group row's preview scrolls from the top.
    let mut app = seeded_app();
    app.update(Msg::ScrollPreviewBy(3));
    assert_eq!(app.preview_scroll(), 3);
    app.update(Msg::NavDown);
    assert_eq!(app.preview_scroll(), 0);
}

#[test]
fn an_unreadable_transcript_says_why() {
    let mut app = unmuxed_session_app(|_| {});
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: PreviewContent::Unavailable("transcript file not found".to_string()),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "transcript unavailable: transcript file not found"
    );

    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: PreviewContent::Failed("transcript parse failed: bad json".to_string()),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "transcript unreadable: transcript parse failed: bad json"
    );
}

#[test]
fn a_transcript_with_no_turns_says_so() {
    let mut app = unmuxed_session_app(|_| {});
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: transcript_content(0),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "no turns yet"
    );
}

#[test]
fn no_live_preview_hides_a_transcript() {
    let mut app = unmuxed_session_app(|config| config.live_preview_enabled = false);
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: transcript_content(3),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "preview disabled (--no-live-preview)"
    );
}

#[test]
fn the_right_panel_draws_a_transcript_inside_its_gutter() {
    let mut app = unmuxed_session_app(|_| {});
    app.update(Msg::SetPreview {
        source: codex_transcript(),
        content: transcript_content(30),
    });
    let buffer = render_to_buffer(&mut app, Rect::new(0, 0, 120, 30));
    let text = buffer_to_string(&buffer);
    assert!(text.contains("turn 30"), "{text}");
}

/// Walk the explorer cursor down until the Preview is about a node
/// other than the selection, and return that node.
fn walk_to_related_row(app: &mut App) -> NodeId {
    for _ in 0..12 {
        app.update(Msg::ExplorerNavDown);
        if let crate::tui::preview::PreviewSubject::Node(node) =
            crate::tui::preview::preview_subject(app)
        {
            return node;
        }
    }
    panic!("no Related row under the cursor");
}

#[test]
fn a_related_row_leads_its_neighbors_preview_with_the_evidence() {
    let mut app = muxed_app_previewing(
        "editor",
        Some("pane output"),
        Vec::new(),
        crate::tui::SessionPreview::Transcript,
    );
    let mux = MuxSessionId::new("editor");
    assert_eq!(
        walk_to_related_row(&mut app),
        NodeId::MuxSession(mux.clone())
    );
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(PreviewSource::Pane(mux))
    );
    let text = buffer_to_string(&render_to_buffer(&mut app, Rect::new(0, 0, 140, 40)));
    let preview = text.split("Preview").nth(1).expect("a Preview zone");
    assert!(preview.contains("neighbor    tmux:editor"), "{text}");
    assert!(preview.contains("edge"), "{text}");
    assert!(preview.contains("pane output"), "{text}");
    assert!(
        !preview.contains("backend"),
        "the pane replaces the neighbor's fields: {text}"
    );
}

#[test]
fn drilling_into_a_neighbor_previews_that_node() {
    let mut app = muxed_app_previewing(
        "editor",
        Some("pane output"),
        Vec::new(),
        crate::tui::SessionPreview::Transcript,
    );
    walk_to_related_row(&mut app);
    app.update(Msg::ExplorerActivate);
    let mux = NodeId::MuxSession(MuxSessionId::new("editor"));
    assert_eq!(
        crate::tui::preview::preview_subject(&app),
        crate::tui::preview::PreviewSubject::Node(mux)
    );
    assert!(
        preview_text_for_selection(&app, 80, 10)
            .to_string()
            .contains("pane output")
    );
}

#[test]
fn a_mux_hosting_this_tui_previews_no_pane() {
    let mut app = muxed_app_previewing(
        "editor",
        None,
        Vec::new(),
        crate::tui::SessionPreview::Transcript,
    );
    let mux = NodeId::MuxSession(MuxSessionId::new("editor"));
    assert!(crate::tui::preview::node_source(&app, &mux).is_some());
    app = {
        let mut config = app.config().clone();
        config.current_tmux_session = Some("editor".to_string());
        let mut hosted = App::new(config);
        hosted.update(Msg::SetData {
            snapshot: app.snapshot_handle().expect("data").clone(),
            tree: app.tree().clone(),
            loaded_at_epoch: 1_700_000_000,
            initial_selection_hint: None,
        });
        hosted
    };
    assert_eq!(crate::tui::preview::node_source(&app, &mux), None);
}

/// Walk the explorer cursor down until the Preview is about a field
/// value `wanted` accepts, and return that value.
fn walk_to_field_value(
    app: &mut App,
    wanted: impl Fn(&crate::tui::explorer::FieldValue) -> bool,
) -> crate::tui::explorer::FieldValue {
    for _ in 0..12 {
        app.update(Msg::ExplorerNavDown);
        if let crate::tui::preview::PreviewSubject::Field(value) =
            crate::tui::preview::preview_subject(app)
            && wanted(&value)
        {
            return value;
        }
    }
    panic!("no wanted field under the cursor");
}

#[test]
fn a_path_field_row_previews_that_path() {
    let mut app = unmuxed_session_app(|config| config.live_preview_enabled = false);
    let cwd = crate::tui::explorer::FieldValue::Path("/home/op/src/proj".to_string());
    assert_eq!(
        walk_to_field_value(&mut app, |value| matches!(
            value,
            crate::tui::explorer::FieldValue::Path(_)
        )),
        cwd
    );
    let source = PreviewSource::Path("/home/op/src/proj".into());
    assert_eq!(
        crate::tui::preview::preview_source(&app),
        Some(source.clone())
    );
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "loading…"
    );

    // Listings aren't conversation content, so `--no-live-preview`
    // leaves them on.
    app.update(Msg::SetPreview {
        source: source.clone(),
        content: PreviewContent::Lines(vec![Line::raw("src/"), Line::raw("Cargo.toml")]),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "src/\nCargo.toml"
    );

    app.update(Msg::SetPreview {
        source,
        content: PreviewContent::Unavailable("no longer exists".to_string()),
    });
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "no longer exists"
    );
}

#[test]
fn a_workspace_previews_a_listing_of_its_root() {
    let mut snapshot = GraphSnapshot::empty();
    let id = crate::model::WorkspaceId::new("/home/op/scratch");
    snapshot
        .nodes
        .push(GraphNode::Workspace(crate::model::WorkspaceNode {
            id: id.clone(),
            root: "/home/op/scratch".to_string(),
            provider: None,
            name: None,
        }));
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree: crate::tui::rows::RowTree::default(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    assert_eq!(
        crate::tui::preview::node_source(&app, &NodeId::Workspace(id)),
        Some(PreviewSource::Path("/home/op/scratch".into()))
    );
}

/// A repo at `/r/.git` with a checkout at `/r` on branch `main`, which
/// tracks `origin/main`.
fn git_app() -> (App, crate::model::BranchId) {
    let repo = RepoId::new("/r/.git");
    let branch = crate::model::BranchId::new(repo.clone(), "refs/heads/main");
    let mut snapshot = GraphSnapshot::empty();
    let mut repo_node = RepoNode::new(repo.clone());
    repo_node.common_dir = "/r/.git".to_string();
    snapshot.nodes.push(GraphNode::Repo(repo_node));
    snapshot.nodes.push(GraphNode::Checkout(CheckoutNode {
        id: CheckoutId::new(repo, "/r"),
        root: "/r".to_string(),
        git_dir: None,
        current_branch: Some(branch.clone()),
        worktree: None,
        head_commit: None,
    }));
    snapshot
        .nodes
        .push(GraphNode::Branch(crate::model::BranchNode {
            id: branch.clone(),
            refname: "refs/heads/main".to_string(),
            current_commit: Some("0123456789abcdef0123".to_string()),
            upstream: Some("origin/main".to_string()),
        }));
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree: crate::tui::rows::RowTree::default(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    (app, branch)
}

#[test]
fn checkouts_repos_and_branches_preview_their_git_state() {
    use crate::tui::preview::{GitQuery, node_source};
    let (app, branch) = git_app();
    let repo = RepoId::new("/r/.git");
    assert_eq!(
        node_source(&app, &NodeId::Checkout(CheckoutId::new(repo.clone(), "/r"))),
        Some(PreviewSource::Git(GitQuery::Status { root: "/r".into() }))
    );
    assert_eq!(
        node_source(&app, &NodeId::Repo(repo)),
        Some(PreviewSource::Git(GitQuery::Repo {
            dir: "/r/.git".into()
        }))
    );
    assert_eq!(
        node_source(&app, &NodeId::Branch(branch)),
        Some(PreviewSource::Git(GitQuery::Log {
            dir: "/r/.git".into(),
            rev: "refs/heads/main".to_string(),
            ahead_behind: Some(("refs/heads/main".to_string(), "origin/main".to_string())),
        }))
    );
}

#[test]
fn branch_ref_and_commit_fields_preview_git_history() {
    use crate::tui::explorer::FieldValue;
    let (app, branch) = git_app();
    let view = crate::tui::explorer::build_node_view(crate::tui::explorer::ExplorerInputs {
        snapshot: app.snapshot_handle().expect("data").snapshot(),
        target: &NodeId::Branch(branch),
        home: None,
        now: None,
    })
    .expect("branch view");
    let preview = |label: &str| {
        view.fields(true)
            .iter()
            .find(|field| field.label == label)
            .and_then(|field| field.preview.clone())
    };
    assert_eq!(
        preview("refname"),
        Some(FieldValue::Ref {
            dir: "/r/.git".to_string(),
            rev: "refs/heads/main".to_string(),
            ahead_behind: None,
        })
    );
    assert_eq!(
        preview("upstream"),
        Some(FieldValue::Ref {
            dir: "/r/.git".to_string(),
            rev: "origin/main".to_string(),
            ahead_behind: Some(("refs/heads/main".to_string(), "origin/main".to_string())),
        })
    );
    assert_eq!(
        preview("current_commit"),
        Some(FieldValue::Commit {
            dir: "/r/.git".to_string(),
            commit: "0123456789abcdef0123".to_string(),
        })
    );
}

#[test]
fn processes_and_pid_fields_preview_process_trees() {
    let mut snapshot = GraphSnapshot::empty();
    let id = crate::model::RuntimeProcessId::new("proc:1");
    snapshot.nodes.push(GraphNode::RuntimeProcess(
        crate::model::RuntimeProcessNode {
            id: id.clone(),
            observation_key: "proc:1".to_string(),
            pid: Some(4242),
            parent_pid: Some(1),
            root_pane_pid: None,
            command: None,
            cwd: None,
            harness_key: None,
            role: None,
            depth: None,
            observed_epoch: None,
        },
    ));
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree: crate::tui::rows::RowTree::default(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let node_id = NodeId::RuntimeProcess(id);
    assert_eq!(
        crate::tui::preview::node_source(&app, &node_id),
        Some(PreviewSource::Process(4242))
    );
    let view = crate::tui::explorer::build_node_view(crate::tui::explorer::ExplorerInputs {
        snapshot: app.snapshot_handle().expect("data").snapshot(),
        target: &node_id,
        home: None,
        now: None,
    })
    .expect("process view");
    let preview = |label: &str| {
        view.fields(true)
            .iter()
            .find(|field| field.label == label)
            .and_then(|field| field.preview.clone())
    };
    assert_eq!(
        preview("pid"),
        Some(crate::tui::explorer::FieldValue::Process(4242))
    );
    assert_eq!(
        preview("parent_pid"),
        Some(crate::tui::explorer::FieldValue::Process(1))
    );
}

/// An app holding only the slot `ingest` pointing at `target`, plus
/// whatever `extend` adds, with nothing selected.
fn pin_snapshot_app(target: Option<NodeId>, extend: impl FnOnce(&mut GraphSnapshot)) -> App {
    let mut snapshot = GraphSnapshot::empty();
    snapshot.pins.push(ingest_slot(target));
    extend(&mut snapshot);
    let snapshot = crate::resolve::resolve_snapshot(snapshot);
    let mut app = App::new(RunConfig::defaults());
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree: crate::tui::rows::RowTree::default(),
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn ingest_pin() -> NodeId {
    NodeId::Pin(crate::model::PinId::new("ingest".to_string()))
}

#[test]
fn a_pin_on_a_live_session_previews_its_transcript() {
    let session = AgentSessionId::new("codex", "/state", "abc");
    let app = pin_snapshot_app(Some(NodeId::AgentSession(session.clone())), |snapshot| {
        snapshot.nodes.push(GraphNode::AgentSession(
            crate::model::AgentSessionNode::new(session.clone(), "codex".to_string()),
        ));
    });
    assert_eq!(
        crate::tui::preview::node_source(&app, &ingest_pin()),
        Some(PreviewSource::Transcript(session))
    );
}

#[test]
fn a_pin_on_a_live_mux_previews_the_pane() {
    let mux = MuxSessionId::new("tmux:ingest");
    let app = pin_snapshot_app(Some(NodeId::MuxSession(mux.clone())), |snapshot| {
        snapshot
            .nodes
            .push(GraphNode::MuxSession(crate::model::MuxSessionNode::new(
                mux.clone(),
                "tmux",
                "ingest",
            )));
    });
    assert_eq!(
        crate::tui::preview::node_source(&app, &ingest_pin()),
        Some(PreviewSource::Pane(mux))
    );
}

#[test]
fn any_other_pin_previews_its_status() {
    let app = pin_snapshot_app(None, |_| {});
    assert_eq!(crate::tui::preview::node_source(&app, &ingest_pin()), None);
    let text = sourceless_node_text(&app, &ingest_pin())
        .expect("status")
        .to_string();
    assert!(text.contains("Pin `ingest` (mux · planned)"), "{text}");
}

#[test]
fn a_glossary_field_row_previews_what_its_value_means() {
    let mut app = unmuxed_session_app(|_| {});
    let harness = walk_to_field_value(&mut app, |value| {
        matches!(value, crate::tui::explorer::FieldValue::Glossary { .. })
    });
    assert_eq!(
        harness,
        crate::tui::explorer::FieldValue::Glossary {
            field: "harness",
            values: vec!["codex".to_string()],
        }
    );
    assert_eq!(crate::tui::preview::preview_source(&app), None);
    assert_eq!(
        preview_text_for_selection(&app, 80, 10).to_string(),
        "codex\nOpenAI's Codex CLI."
    );
}

#[test]
fn a_field_naming_another_node_previews_that_node() {
    use crate::tui::explorer::FieldValue;
    let (app, branch) = git_app();
    let view = crate::tui::explorer::build_node_view(crate::tui::explorer::ExplorerInputs {
        snapshot: app.snapshot_handle().expect("data").snapshot(),
        target: &NodeId::Checkout(CheckoutId::new(RepoId::new("/r/.git"), "/r")),
        home: None,
        now: None,
    })
    .expect("checkout view");
    let preview = |label: &str| {
        view.fields(true)
            .iter()
            .find(|field| field.label == label)
            .and_then(|field| field.preview.clone())
            .expect("typed field")
    };
    let current_branch = preview("current_branch");
    assert_eq!(
        current_branch,
        FieldValue::Node(NodeId::Branch(branch.clone()))
    );
    assert_eq!(
        crate::tui::preview::value_source(&app, &current_branch),
        crate::tui::preview::node_source(&app, &NodeId::Branch(branch))
    );
    assert_eq!(
        preview("repo"),
        FieldValue::Node(NodeId::Repo(RepoId::new("/r/.git")))
    );
}

/// The PRs view on a world with one PR of each state, `#1`–`#4`, all
/// on `conspectus/project`. No PR has an update time, so frames don't
/// depend on the clock.
fn prs_app() -> App {
    prs_app_in(View::Prs)
}

/// [`prs_app`], starting in `view`.
fn prs_app_in(view: View) -> App {
    let pr = |number: u64, state: &str, is_draft: bool| {
        GraphNode::ForgePr(crate::model::ForgePrNode {
            id: crate::model::ForgePrId::new(
                "github",
                "github.com",
                "conspectus",
                "project",
                number,
            ),
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "conspectus".to_string(),
            repo: "project".to_string(),
            number,
            state: Some(state.to_string()),
            title: None,
            url: Some(format!(
                "https://github.com/conspectus/project/pull/{number}"
            )),
            updated_epoch: None,
            is_draft,
            head_ref: Some(format!("feature/{number}")),
        })
    };
    let mut snapshot = GraphSnapshot::empty();
    snapshot.nodes.push(pr(1, "OPEN", false));
    snapshot.nodes.push(pr(2, "CLOSED", false));
    snapshot.nodes.push(pr(3, "MERGED", false));
    snapshot.nodes.push(pr(4, "OPEN", true));
    let snapshot = resolve_snapshot(snapshot);
    let mut config = RunConfig::defaults();
    config.default_view = view;
    let mut app = App::new(config);
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn pr_source(number: u64) -> PreviewSource {
    PreviewSource::PullRequest(crate::tui::preview::PrQuery {
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: "conspectus".to_string(),
        repo: "project".to_string(),
        number,
    })
}

#[test]
fn an_empty_prs_view_points_at_the_hard_refresh() {
    let mut config = RunConfig::defaults();
    config.default_view = View::Prs;
    let mut app = App::new(config);
    let snapshot = resolve_snapshot(GraphSnapshot::empty());
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs::from_app(
        &snapshot, &app,
    ));
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    let frame = buffer_to_string(&render_to_buffer(&mut app, Rect::new(0, 0, 200, 30)));
    assert!(frame.contains("No pull requests discovered."), "{frame}");
    assert!(
        frame.contains("Press `Ctrl-R` to ask the forges again"),
        "{frame}"
    );
    assert!(frame.contains("^R forges"), "{frame}");
}

/// Select PR `#number` and render the frame.
fn pr_frame(app: &mut App, number: u64, content: Option<PreviewContent>) -> String {
    let row = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(&row.kind, RowKind::Pr(pr) if pr.pr_number == number))
        .map(|row| row.id.clone())
        .expect("PR row");
    app.set_selection(row);
    assert_eq!(
        crate::tui::preview::preview_source(app),
        Some(pr_source(number))
    );
    if let Some(content) = content {
        app.update(Msg::SetPreview {
            source: pr_source(number),
            content,
        });
    }
    buffer_to_string(&render_to_buffer(app, Rect::new(0, 0, 130, 30)))
}

/// What `gh pr view` returns for a PR in `state`.
fn fetched_pr(state: &str, draft: bool, checks: &str, review: &str) -> PreviewContent {
    let json = format!(
        r#"{{"title":"Release notes for 0.3","body":"Adds the release summary.","state":"{state}",
            "isDraft":{draft},"reviewDecision":"{review}","statusCheckRollup":{checks},
            "latestReviews":[],"headRefName":"feature/notes","baseRefName":"main"}}"#
    );
    PreviewContent::Lines(crate::tui::preview::forge::render_pr(&json, None).expect("renders"))
}

const PASSING_CHECKS: &str = r#"[{"name":"test","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

#[test]
fn prs_view_frames_by_pr_state() {
    let mut app = prs_app();
    insta::assert_snapshot!("pr_open_before_fetch", pr_frame(&mut app, 1, None));
    insta::assert_snapshot!(
        "pr_open_fetched",
        pr_frame(
            &mut app,
            1,
            Some(fetched_pr("OPEN", false, PASSING_CHECKS, "REVIEW_REQUIRED"))
        )
    );
    insta::assert_snapshot!(
        "pr_closed",
        pr_frame(&mut app, 2, Some(fetched_pr("CLOSED", false, "[]", "")))
    );
    insta::assert_snapshot!(
        "pr_merged",
        pr_frame(
            &mut app,
            3,
            Some(fetched_pr("MERGED", false, PASSING_CHECKS, "APPROVED"))
        )
    );
    insta::assert_snapshot!(
        "pr_draft",
        pr_frame(&mut app, 4, Some(fetched_pr("OPEN", true, "[]", "")))
    );
    insta::assert_snapshot!(
        "pr_fetch_failed",
        pr_frame(
            &mut app,
            1,
            Some(PreviewContent::Unavailable(
                "gh is not authenticated".to_string()
            ))
        )
    );
}

#[test]
fn a_first_visit_to_the_prs_view_opens_its_first_group() {
    let mut app = prs_app_in(View::Sessions);
    app.update(Msg::SwitchView(View::Prs));
    let group = RowId::Remote("conspectus/project".to_string());
    assert_eq!(app.selection(), Some(&group));
    let visible: Vec<&RowId> = app.visible_rows().iter().map(|row| &row.id).collect();
    assert_eq!(visible.len(), 5, "the group and its four PRs: {visible:?}");
}

#[test]
fn mux_rows_keep_pin_and_preview_columns_aligned_across_attach_states() {
    // The attach-state column is exactly one glyph wide whatever the
    // state, so the pin marker and preview line up down the mux list.
    let theme = Theme::default();
    let render = |client_attached: Option<bool>| -> String {
        let row = MuxSessionRow {
            client_attached,
            pin_id: Some("build".into()),
            agent_labels: vec!["claude".into()],
            single_session_preview: Some("pushed to main".into()),
            ..agentless_mux_row(None)
        };
        render_mux_session_spans(&row, &theme, 0, 100, None)
            .iter()
            .map(|s| s.content.to_string())
            .collect()
    };
    let column_of = |text: &str, needle: &str| {
        let byte = text.find(needle).expect("needle in row");
        unicode_width::UnicodeWidthStr::width(&text[..byte])
    };
    let rows: Vec<String> = [Some(true), Some(false), None]
        .into_iter()
        .map(render)
        .collect();
    for row in &rows {
        assert!(
            !row.contains('◐'),
            "mux rows carry no ambiguity glyph: {row}"
        );
        assert!(
            !row.contains("📌"),
            "the pin marker is the tree's, not the row's: {row}"
        );
        assert_eq!(
            column_of(row, "pushed to main"),
            column_of(&rows[0], "pushed to main"),
            "{row}",
        );
    }
}

#[test]
fn launch_chip_stays_visible_on_a_narrow_status_bar() {
    // The chip leads the bar: the hint text is long enough to push a
    // trailing chip off the right edge of any terminal under ~190
    // columns.
    for width in [60u16, 80, 100, 120] {
        let mut app = seeded_app();
        app.update(Msg::InFlightStart {
            kind: crate::tui::app::InFlightKind::Launch,
            label: "Launching dek-serve".to_string(),
        });
        app.update(Msg::SetStatus(None));
        let buffer = render_to_buffer(&mut app, Rect::new(0, 0, width, 24));
        let text = buffer_to_string(&buffer);
        let status_line = text.lines().last().unwrap_or("");
        assert!(
            status_line.contains("Launching dek-serve"),
            "chip should lead the status bar at width {width}: {status_line}"
        );
    }
}

#[test]
fn launch_chip_shows_alongside_a_status_message() {
    let mut app = seeded_app();
    app.update(Msg::InFlightStart {
        kind: crate::tui::app::InFlightKind::Launch,
        label: "Launching dek-serve".to_string(),
    });
    app.update(Msg::SetStatus(Some("pin `dek-serve` queued".to_string())));
    let buffer = render_to_buffer(&mut app, Rect::new(0, 0, 100, 24));
    let text = buffer_to_string(&buffer);
    let status_line = text.lines().last().unwrap_or("");
    assert!(
        status_line.contains("Launching dek-serve") && status_line.contains("queued"),
        "chip and message should both render: {status_line}"
    );
}

fn pr_row_for_layout(label: &str, state: &str, age: Option<&str>) -> crate::tui::rows::PrRow {
    crate::tui::rows::PrRow {
        pr_number: 1,
        repo_display: format!("o/r{label}"),
        label: label.into(),
        state: Some(state.into()),
        is_draft: false,
        branch_name: None,
        tracked: false,
        title: None,
        updated_recency: age.map(str::to_string),
        updated_epoch: None,
        url: None,
        primary_node: NodeId::ForgePr(crate::model::ForgePrId::new(
            "github",
            "github.com",
            "o",
            "r",
            1,
        )),
    }
}

fn pr_text(spans: &[ratatui::text::Span<'_>]) -> String {
    spans.iter().map(|span| span.content.as_ref()).collect()
}

fn pr_tree(rows: Vec<crate::tui::rows::PrRow>) -> crate::tui::rows::RowTree {
    crate::tui::rows::RowTree {
        rows: rows
            .into_iter()
            .map(|pr| crate::tui::rows::Row {
                id: RowId::Pr(pr.primary_node.clone()),
                depth: 1,
                expandable: false,
                kind: RowKind::Pr(pr),
            })
            .collect(),
        view: crate::tui::rows::ViewLabel::Prs,
    }
}

#[test]
fn pr_columns_align_to_the_widest_value_in_the_tree() {
    // The number pads to the widest across the tree, so rows from
    // different groups line up; age and state have fixed cells.
    let theme = Theme::default();
    let mut long = pr_row_for_layout("#1234", "merged", Some("120d"));
    long.title = Some("long one".into());
    let mut short = pr_row_for_layout("#7", "open", Some("2h"));
    short.title = Some("short one".into());
    let align = PrAlign::of_tree(&pr_tree(vec![long.clone(), short.clone()]));

    let long_text = pr_text(&render_pr_spans(&long, &theme, align, 80, 0));
    let short_text = pr_text(&render_pr_spans(&short, &theme, align, 80, 0));

    assert_eq!(long_text, "#1234 ◌  120d  ⊕  long one");
    assert_eq!(short_text, "#7    ◌    2h  ○  short one");
    let column = |text: &str, needle: &str| text.find(needle).expect("needle");
    assert_eq!(column(&long_text, "◌"), column(&short_text, "◌"));
    assert_eq!(
        unicode_width::UnicodeWidthStr::width(&long_text[..long_text.find("long").unwrap()]),
        unicode_width::UnicodeWidthStr::width(&short_text[..short_text.find("short").unwrap()]),
        "the titles start in the same column"
    );
}

#[test]
fn pr_title_takes_the_remaining_width_and_is_cut_with_an_ellipsis() {
    let theme = Theme::default();
    let mut pr = pr_row_for_layout("#9", "open", Some("1d"));
    pr.title = Some("A rather long pull request title".into());
    let align = PrAlign::of_tree(&pr_tree(vec![pr.clone()]));

    let spans = render_pr_spans(&pr, &theme, align, 30, 0);
    let text = pr_text(&spans);

    assert!(text.ends_with('…'), "{text:?}");
    assert_eq!(unicode_width::UnicodeWidthStr::width(text.as_str()), 30);
    let title = spans.last().expect("title span");
    assert_eq!(title.style.fg, Some(theme.secondary_text));

    // No room for even a sliver of title: it is left out, not wrapped.
    let tight = pr_text(&render_pr_spans(&pr, &theme, align, 14, 0));
    assert_eq!(tight, "#9 ◌    1d  ○");
}

#[test]
fn pr_row_without_a_title_or_age_still_pads_its_columns() {
    let theme = Theme::default();
    let aged = pr_row_for_layout("#10", "open", Some("3d"));
    let bare = pr_row_for_layout("#2", "closed", None);
    let align = PrAlign::of_tree(&pr_tree(vec![aged, bare.clone()]));

    let text = pr_text(&render_pr_spans(&bare, &theme, align, 80, 0));

    assert_eq!(text, "#2  ◌     —  ⊗");
}

#[test]
fn an_open_draft_shows_the_draft_glyph_and_the_local_branch_leads_the_title() {
    let theme = Theme::default();
    let mut pr = pr_row_for_layout("#5", "open", Some("4h"));
    pr.is_draft = true;
    pr.tracked = true;
    pr.branch_name = Some("feature/login".into());
    pr.title = Some("Add login".into());
    let align = PrAlign::of_tree(&pr_tree(vec![pr.clone()]));

    let text = pr_text(&render_pr_spans(&pr, &theme, align, 80, 0));

    assert_eq!(text, "#5 ⌥    4h  ◐  feature/login  Add login");
    // A draft that was already merged keeps its real state.
    let mut merged = pr;
    merged.state = Some("merged".into());
    let merged_text = pr_text(&render_pr_spans(
        &merged,
        &theme,
        PrAlign::of_tree(&pr_tree(vec![merged.clone()])),
        80,
        0,
    ));
    assert!(
        merged_text.contains('⊕') && !merged_text.contains('◐'),
        "{merged_text:?}"
    );
}

#[test]
fn a_long_local_branch_is_capped_so_it_cannot_crowd_out_the_title() {
    let theme = Theme::default();
    let mut pr = pr_row_for_layout("#5", "open", None);
    pr.tracked = true;
    pr.branch_name = Some("a".repeat(60));
    pr.title = Some("Title".into());
    let align = PrAlign::of_tree(&pr_tree(vec![pr.clone()]));

    let text = pr_text(&render_pr_spans(&pr, &theme, align, 100, 0));

    assert!(text.contains(&format!("{}…", "a".repeat(23))), "{text:?}");
    assert!(text.ends_with("  Title"), "{text:?}");
}

#[test]
fn filter_chips_name_the_pr_state_and_branch_filters() {
    use crate::filter::{PrBranchFilter, PrStateFilter, PrStateKey, RowFilter};

    let filter = RowFilter {
        pr_state: Some(PrStateFilter::from_values([
            PrStateKey::Merged,
            PrStateKey::Open,
        ])),
        pr_branch: Some(PrBranchFilter::Untracked),
        ..RowFilter::default()
    };
    assert_eq!(
        render_filter_chips(&filter),
        "state:open,merged · branch:untracked"
    );
    // A float on its own is ordering, not a filter chip.
    let float = RowFilter {
        float_tracked_prs_top: true,
        ..RowFilter::default()
    };
    assert_eq!(render_filter_chips(&float), "");
}

#[test]
fn the_header_counts_only_muxes_that_exist() {
    use crate::model::{
        GraphNode, GraphSnapshot, MuxSessionId, MuxSessionNode, NodeId, NodePresence,
    };
    use crate::tui::app::SnapshotHandle;

    let live = MuxSessionNode::new(MuxSessionId::new("tmux:live"), "tmux", "live");
    let ghost = MuxSessionNode::new(MuxSessionId::new("tmux:ghost"), "tmux", "ghost");
    let mut snapshot = GraphSnapshot {
        nodes: vec![
            GraphNode::MuxSession(live),
            GraphNode::MuxSession(ghost.clone()),
        ],
        ..GraphSnapshot::empty()
    };
    snapshot.node_presence.insert(
        NodeId::MuxSession(ghost.id),
        NodePresence::Gone { last_seen: 1 },
    );
    let handle = SnapshotHandle::from_snapshot(&snapshot);
    let (_, mux) = header::snapshot_counts(Some(&handle));
    assert_eq!(mux, 1, "one live mux and one ghost");
}

// ----- The Pinned view (mixed-kind rows) -----------------------------

/// An app showing the Pinned view over [`fixture_world`], grouped as
/// asked, with every bucket and slot expanded.
///
/// [`fixture_world`]: crate::tui::rows::pinned::fixture_world
fn slots_app(grouping: crate::tui::PinnedGrouping) -> App {
    slots_app_over(&crate::tui::rows::pinned::fixture_world(), grouping)
}

fn slots_app_over(snapshot: &GraphSnapshot, grouping: crate::tui::PinnedGrouping) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = View::Pinned;
    let mut app = App::new(config);
    app.update(Msg::SetGrouping(crate::tui::Grouping::Pinned(grouping)));
    let tree =
        crate::tui::rows::pinned::build_pinned_tree(crate::tui::rows::pinned::PinnedBuildInputs {
            snapshot,
            home: Some(std::path::Path::new("/home/op")),
            now: Some(1_700_000_000),
            grouping,
            sort: crate::tui::Sort::Hierarchy,
        });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn pinned_frame(app: &mut App, width: u16) -> String {
    buffer_to_string(&render_to_buffer(app, Rect::new(0, 0, width, 24)))
}

#[test]
fn the_pinned_view_lists_every_kind_and_state_in_aligned_columns() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    for width in [70, 110, 160] {
        insta::assert_snapshot!(
            format!("pinned_view_ungrouped_{width}"),
            pinned_frame(&mut app, width)
        );
    }
}

#[test]
fn the_pinned_view_groups_by_project_and_by_kind() {
    let mut app = slots_app(crate::tui::PinnedGrouping::Project);
    insta::assert_snapshot!("pinned_view_by_project", pinned_frame(&mut app, 120));
    let mut app = slots_app(crate::tui::PinnedGrouping::Kind);
    insta::assert_snapshot!("pinned_view_by_kind", pinned_frame(&mut app, 120));
}

/// The Pinned view over [`all_kinds_world`], ungrouped and tall enough
/// to show every slot.
///
/// [`all_kinds_world`]: crate::tui::rows::pinned::all_kinds_world
fn all_kinds_frame(width: u16) -> String {
    let snapshot = crate::tui::rows::pinned::all_kinds_world();
    let mut config = RunConfig::defaults();
    config.default_view = View::Pinned;
    let mut app = App::new(config);
    let grouping = crate::tui::PinnedGrouping::None;
    app.update(Msg::SetGrouping(crate::tui::Grouping::Pinned(grouping)));
    let tree =
        crate::tui::rows::pinned::build_pinned_tree(crate::tui::rows::pinned::PinnedBuildInputs {
            snapshot: &snapshot,
            home: Some(std::path::Path::new("/home/op")),
            now: Some(1_700_000_000),
            grouping,
            sort: crate::tui::Sort::Hierarchy,
        });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    buffer_to_string(&render_to_buffer(&mut app, Rect::new(0, 0, width, 30)))
}

#[test]
fn the_pinned_view_shows_every_kind_in_each_state_it_can_be_in() {
    const SLOTS: [&str; 7] = [
        "mux-",
        "session-",
        "worktree-",
        "repo-",
        "pr-",
        "branch-",
        "workspace-",
    ];
    for width in [70, 110, 160] {
        // The status word starts in one column for every kind.
        let frame = all_kinds_frame(width);
        let columns: Vec<usize> = frame
            .lines()
            .filter_map(|line| {
                // The left pane is the text between the first two borders.
                let pane: Vec<char> = line.split('│').nth(1)?.chars().collect();
                if !pane.iter().any(|c| "▣●◇◆⇄⌥▦⋎".contains(*c)) {
                    return None;
                }
                let label = (0..pane.len()).find(|&i| {
                    pane[i] == ' '
                        && SLOTS.iter().any(|slot| {
                            pane[i + 1..].iter().take(slot.len()).collect::<String>() == *slot
                        })
                })?;
                let name_end = (label + 1..pane.len()).find(|&i| pane[i] == ' ')?;
                (name_end..pane.len()).find(|&i| pane[i] != ' ')
            })
            .collect();
        assert!(columns.len() >= 10, "{columns:?}\n{frame}");
        assert!(
            columns.windows(2).all(|w| w[0] == w[1]),
            "{columns:?}\n{frame}"
        );
        insta::assert_snapshot!(
            format!("pinned_view_all_kinds_{width}"),
            all_kinds_frame(width)
        );
    }
}

/// The terminal column each slot row's status word starts at.
fn status_columns(frame: &str) -> Vec<usize> {
    frame
        .lines()
        .filter(|line| {
            ["live", "stopped", "planned", "merged", "in mux"]
                .iter()
                .any(|status| line.contains(&format!(" {status} ")))
        })
        .filter_map(|line| {
            ["planned", "merged", "in mux", "live", "stopped"]
                .iter()
                .filter_map(|status| line.find(&format!("  {status}")))
                .min()
                .map(|byte| line[..byte].chars().count())
        })
        .collect()
}

#[test]
fn folding_a_group_does_not_shift_the_columns_of_the_rows_left() {
    let mut app = slots_app(crate::tui::PinnedGrouping::Project);
    let open = pinned_frame(&mut app, 120);
    let columns = status_columns(&open);
    assert!(columns.len() >= 5, "{open}");
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "status words start in one column: {columns:?}\n{open}"
    );

    // Fold the first bucket and look again.
    let first_bucket = app
        .tree()
        .rows
        .iter()
        .find(|row| matches!(row.id, RowId::Bucket(_)))
        .map(|row| row.id.clone())
        .unwrap();
    app.set_selection(first_bucket);
    app.update(Msg::ToggleExpand);
    let folded = pinned_frame(&mut app, 120);
    let after = status_columns(&folded);
    assert!(after.len() < columns.len(), "something folded: {folded}");
    assert!(
        after.iter().all(|column| *column == columns[0]),
        "columns stayed put: {after:?} vs {columns:?}"
    );
}

#[test]
fn the_pinned_view_points_at_pin_migrate_while_legacy_pins_remain() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    let frame = pinned_frame(&mut app, 130);
    assert!(frame.contains("1 legacy pin is still read"), "{frame}");
    assert!(frame.contains("conspectus pin migrate"), "{frame}");
}

#[test]
fn gone_and_planned_rows_render_dimmed_and_live_rows_do_not() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    let buffer = render_to_buffer(&mut app, Rect::new(0, 0, 130, 24));
    // Whether the first cell of `needle`'s row label carries the dim
    // modifier.
    let dimmed_row = |needle: &str| -> bool {
        (0..buffer.area.height).any(|y| {
            let cells: Vec<String> = (0..60)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect();
            let line = cells.concat();
            let Some(byte) = line.find(needle) else {
                return false;
            };
            let x = u16::try_from(line[..byte].chars().count()).unwrap();
            let style = buffer[(x, y)].style();
            style.fg == Some(Theme::default().ghost)
                && style.add_modifier.contains(Modifier::ITALIC)
        })
    };
    assert!(dimmed_row("config-agent3"));
    assert!(dimmed_row("nightly"));
    assert!(!dimmed_row("conspectus-main"));
}

#[test]
fn enter_on_a_gone_slot_rebuilds_it_and_on_a_live_mux_slot_attaches() {
    use crate::tui::Effect;
    use crate::tui::effect::ExecSpec;
    let mut app = slots_app(crate::tui::PinnedGrouping::None);

    app.set_selection(RowId::Pin {
        pin_id: "config-agent3".into(),
    });
    let effects = app.update(Msg::AttachSelected);
    assert_eq!(
        effects,
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "config-agent3".into(),
            attach: true,
        })]
    );

    app.set_selection(RowId::Pin {
        pin_id: "nightly".into(),
    });
    assert_eq!(
        app.update(Msg::AttachSelected),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "nightly".into(),
            attach: true,
        })],
        "a planned slot builds on its first rebuild"
    );

    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });
    let effects = app.update(Msg::AttachSelected);
    assert!(
        matches!(effects.as_slice(), [Effect::Exec(ExecSpec::AttachMux(target))] if target.native_id == "conspectus-main"),
        "{effects:?}"
    );

    app.set_selection(RowId::Pin {
        pin_id: "ledger-pr".into(),
    });
    let effects = app.update(Msg::AttachSelected);
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(text)] if text.contains("ledger-pr")),
        "{effects:?}"
    );
}

#[test]
fn the_detail_pane_shows_where_each_slot_field_came_from() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });
    let frame = pinned_frame(&mut app, 160);
    assert!(
        frame.contains("history") || frame.contains("live"),
        "{frame}"
    );
    assert!(frame.contains("cwd"), "{frame}");
}

// ----- The History view and the row menu -----------------------------

/// An app showing the History view over [`fixture_world`] (a gone mux
/// and a gone worktree among its records), grouped as asked.
///
/// [`fixture_world`]: crate::tui::rows::pinned::fixture_world
fn history_app_over(
    snapshot: &GraphSnapshot,
    grouping: crate::tui::HistoryGrouping,
    config: RunConfig,
) -> App {
    let mut config = config;
    config.default_view = View::History;
    let mut app = App::new(config);
    app.update(Msg::SetGrouping(crate::tui::Grouping::History(grouping)));
    let tree = crate::tui::rows::history::build_history_tree(
        crate::tui::rows::history::HistoryBuildInputs {
            snapshot,
            home: Some(std::path::Path::new("/home/op")),
            now: Some(1_700_000_000),
            grouping,
            show_pinned: true,
        },
    );
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn history_app(grouping: crate::tui::HistoryGrouping) -> App {
    history_app_over(
        &crate::tui::rows::pinned::fixture_world(),
        grouping,
        RunConfig::defaults(),
    )
}

#[test]
fn the_history_view_lists_gone_records_with_their_age() {
    let mut app = history_app(crate::tui::HistoryGrouping::None);
    insta::assert_snapshot!("history_view_flat", pinned_frame(&mut app, 110));
    let mut app = history_app(crate::tui::HistoryGrouping::Project);
    insta::assert_snapshot!("history_view_by_project", pinned_frame(&mut app, 110));
}

#[test]
fn an_empty_history_view_points_at_the_menu() {
    let mut app = history_app_over(
        &GraphSnapshot::empty(),
        crate::tui::HistoryGrouping::None,
        RunConfig::defaults(),
    );
    let frame = pinned_frame(&mut app, 100);
    assert!(frame.contains("Nothing has stopped"), "{frame}");
    assert!(frame.contains("Clean up"), "{frame}");
}

#[test]
fn enter_on_a_history_row_rebuilds_it_the_way_its_pin_says_and_attaches() {
    use crate::tui::Effect;
    use crate::tui::effect::ExecSpec;
    let mut app = history_app(crate::tui::HistoryGrouping::None);
    app.set_selection(RowId::History {
        record_id: "h_agent".into(),
    });
    assert_eq!(
        app.update(Msg::AttachSelected),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "config-agent3".into(),
            attach: true,
        })],
        "the record is pinned, so the pin's overrides apply"
    );
}

/// A frame with the padding after each line removed, so a snapshot of a
/// frame that changes often stays free of trailing whitespace.
fn trimmed(frame: String) -> String {
    frame
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_row_menu_offers_the_record_verbs_and_clean_up() {
    let mut app = history_app(crate::tui::HistoryGrouping::None);
    app.set_selection(RowId::History {
        record_id: "h_agent".into(),
    });
    assert!(app.update(Msg::OpenRowMenu).is_empty());
    let menu = app.row_menu().expect("the menu opened");
    let labels: Vec<&str> = menu.entries().iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Rebuild",
            "Show rebuild plan",
            "Edit…",
            "Rename…",
            "Retarget…",
            "Refresh snapshot",
            "Reveal in its view",
            "Unpin config-agent3",
            "Forget",
            "Clean up…"
        ]
    );
    let entry = |label: &str| {
        menu.entries()
            .iter()
            .find(|entry| entry.label == label)
            .unwrap_or_else(|| panic!("no `{label}` entry"))
    };
    assert!(entry("Forget").confirm, "forget asks twice");
    // The pinned record offers to unpin its pin, also behind a confirm.
    assert!(
        !entry("Unpin config-agent3").confirm,
        "unpin acts at once, like p"
    );
    assert_eq!(
        entry("Unpin config-agent3").msg,
        Msg::Unpin("config-agent3".into())
    );
    insta::assert_snapshot!("history_row_menu", trimmed(pinned_frame(&mut app, 100)));
}

#[test]
fn the_history_view_menu_works_with_nothing_selected() {
    let mut app = history_app_over(
        &GraphSnapshot::empty(),
        crate::tui::HistoryGrouping::None,
        RunConfig::defaults(),
    );
    app.update(Msg::OpenRowMenu);
    let menu = app.row_menu().expect("the menu opened");
    assert_eq!(menu.entries().len(), 1);
    assert_eq!(menu.entries()[0].msg, Msg::OpenCleanUp);
}

#[test]
fn rows_without_actions_say_so_instead_of_opening_an_empty_menu() {
    use crate::tui::Effect;
    let mut app = slots_app(crate::tui::PinnedGrouping::Project);
    app.set_selection(first_bucket(&app));
    let effects = app.update(Msg::OpenRowMenu);
    assert!(app.row_menu().is_none());
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(_)]),
        "{effects:?}"
    );
}

#[test]
fn row_menu_messages_become_the_matching_effects() {
    use crate::tui::Effect;
    use crate::tui::effect::{ExecSpec, StoreOp};
    let mut app = history_app(crate::tui::HistoryGrouping::None);
    assert_eq!(
        app.update(Msg::PinRecord("h_agent".into())),
        vec![Effect::WriteStore(StoreOp::PinRecord {
            record_id: "h_agent".into()
        })]
    );
    assert_eq!(
        app.update(Msg::ForgetRecord("h_agent".into())),
        vec![Effect::WriteStore(StoreOp::ForgetRecord {
            record_id: "h_agent".into()
        })]
    );
    assert_eq!(
        app.update(Msg::ReapRecords(vec!["h_a".into(), "h_b".into()])),
        vec![Effect::WriteStore(StoreOp::ReapRecords {
            ids: vec!["h_a".into(), "h_b".into()]
        })]
    );
    assert_eq!(
        app.update(Msg::ShowRebuildPlan("h_agent".into())),
        vec![Effect::Exec(ExecSpec::ShowRebuildPlan {
            reference: "h_agent".into()
        })]
    );
    assert_eq!(
        app.update(Msg::RebuildReference("h_agent".into())),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "h_agent".into(),
            attach: false,
        })]
    );
}

#[test]
fn clean_up_lists_what_has_aged_out_and_nothing_pinned() {
    let mut config = RunConfig::defaults();
    let zero = Some(std::time::Duration::from_secs(0));
    config
        .history
        .reap_after
        .insert(crate::model::HistoryKind::Mux, zero);
    config
        .history
        .reap_after
        .insert(crate::model::HistoryKind::Checkout, zero);
    // The fixture pins both gone records; unpin them to see them listed.
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    snapshot.pins.clear();
    let mut app = history_app_over(&snapshot, crate::tui::HistoryGrouping::None, config.clone());
    app.update(Msg::OpenCleanUp);
    let clean_up = app.clean_up().expect("the modal opened");
    let ids: Vec<&str> = clean_up
        .candidates()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, ["h_agent", "h_wt"], "oldest sighting first");
    assert_eq!(clean_up.selected(), [0, 1], "all preselected");
    let frame = pinned_frame(&mut app, 100);
    assert!(frame.contains("Clean up: forget these records"), "{frame}");
    assert!(frame.contains("config-agent3"), "{frame}");
    assert!(
        frame.contains("ledger-worktree") || frame.contains("conspectus-ledger"),
        "{frame}"
    );
    assert!(frame.contains("√"), "all preselected: {frame}");

    // With the slots back, their records are protected.
    let snapshot = crate::tui::rows::pinned::fixture_world();
    let mut app = history_app_over(&snapshot, crate::tui::HistoryGrouping::None, config);
    app.update(Msg::OpenCleanUp);
    assert!(app.clean_up().expect("modal").is_empty());
    let frame = pinned_frame(&mut app, 100);
    assert!(frame.contains("Nothing to clean up"), "{frame}");
}

// ----- Ghost muxes in the Mux view -----------------------------------

/// [`fixture_world`] with history projected into the graph the way a
/// discovery cycle does it, after a clean tmux probe: `config-agent3`
/// is a ghost beside the live `conspectus-main`.
///
/// [`fixture_world`]: crate::tui::rows::pinned::fixture_world
fn ghost_world() -> GraphSnapshot {
    ghost_world_with(true)
}

/// The ghost fixture, with or without its pins. Without them no pin
/// points at the stopped mux.
fn ghost_world_with(pinned: bool) -> GraphSnapshot {
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    if !pinned {
        snapshot.pins.clear();
    }
    snapshot.mux_probes.insert(
        "tmux".to_string(),
        crate::model::MuxProbe {
            status: crate::model::MuxProbeStatus::Listed,
            epoch: 1_700_000_000,
            exited: Vec::new(),
        },
    );
    let mut history = std::mem::take(&mut snapshot.history);
    crate::history::project::project_into(&mut snapshot, &mut history, 1_700_000_000);
    snapshot.history = history;
    crate::resolve::resolve_snapshot(snapshot)
}

fn mux_app_over(snapshot: &GraphSnapshot, filter: crate::filter::RowFilter) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = View::Mux;
    let mut app = App::new(config);
    let tree = crate::tui::rows::mux::build_mux_tree(crate::tui::rows::mux::MuxBuildInputs {
        snapshot,
        home: Some(std::path::Path::new("/home/op")),
        now: Some(1_700_000_000),
        filter,
        grouping: crate::tui::MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

#[test]
fn ghost_muxes_sit_below_live_ones_dimmed_with_their_age() {
    let snapshot = ghost_world();
    let mut app = mux_app_over(&snapshot, crate::filter::RowFilter::default());
    let frame = pinned_frame(&mut app, 110);
    let live = frame.find("conspectus-main").expect("live mux");
    let ghost = frame.find("config-agent3").expect("ghost mux");
    assert!(live < ghost, "{frame}");
    assert!(frame.contains('⊘'), "{frame}");
    assert!(
        frame.contains("· 1 stopped"),
        "the header counts ghosts: {frame}"
    );
}

#[test]
fn the_mux_view_lists_only_pinned_stopped_muxes_unless_asked_for_all() {
    // A pin points at the stopped mux: it stays in the Mux view.
    let pinned = ghost_world();
    let mut app = mux_app_over(&pinned, crate::filter::RowFilter::default());
    let frame = pinned_frame(&mut app, 110);
    assert!(frame.contains("config-agent3"), "{frame}");

    // Nothing pins it: it lives in History, but the header still counts it.
    let unpinned = ghost_world_with(false);
    let mut app = mux_app_over(&unpinned, crate::filter::RowFilter::default());
    let frame = pinned_frame(&mut app, 110);
    assert!(!frame.contains("config-agent3"), "{frame}");
    assert!(frame.contains("conspectus-main"), "{frame}");
    assert!(frame.contains("1 stopped"), "{frame}");

    // Controls can list every stopped mux.
    let filter = crate::filter::RowFilter {
        show_stopped_muxes: true,
        ..Default::default()
    };
    let mut app = mux_app_over(&unpinned, filter);
    let frame = pinned_frame(&mut app, 110);
    assert!(frame.contains("config-agent3"), "{frame}");
}

#[test]
fn a_narrowing_filter_cannot_match_a_ghost() {
    let snapshot = ghost_world();
    let filter = crate::filter::RowFilter {
        max_age: Some(std::time::Duration::from_secs(86_400 * 365)),
        ..Default::default()
    };
    let mut app = mux_app_over(&snapshot, filter);
    let frame = pinned_frame(&mut app, 110);
    assert!(!frame.contains("config-agent3"), "{frame}");
}

#[test]
fn actions_on_a_ghost_mux_rebuild_or_refuse_instead_of_erroring() {
    use crate::tui::Effect;
    use crate::tui::effect::ExecSpec;
    let snapshot = ghost_world();
    let mut app = mux_app_over(&snapshot, crate::filter::RowFilter::default());
    let ghost = NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:config-agent3"));
    app.set_selection(RowId::MuxSession(ghost));

    // Enter brings the ghost back the way its pin says, and attaches.
    assert_eq!(
        app.update(Msg::AttachSelected),
        vec![Effect::Exec(ExecSpec::Rebuild {
            reference: "config-agent3".into(),
            attach: true,
        })]
    );
    // The menu is the same one History offers.
    app.update(Msg::OpenRowMenu);
    let menu = app.row_menu().expect("menu");
    assert_eq!(
        menu.entries()[0].msg,
        Msg::RebuildReference("config-agent3".into())
    );
    // Attach refuses by name, never reaching tmux.
    let reason = crate::tui::actions::resolve_attach_target(&app).unwrap_err();
    assert!(
        matches!(&reason, crate::tui::actions::AttachDisabled::MuxGone(name) if name == "config-agent3"),
        "{reason:?}"
    );
}

// ---- Pin and unpin from the native views ----------------------------

/// The fixture world with one more live mux nobody pinned, resolved so
/// the slots have their nodes.
fn native_world() -> GraphSnapshot {
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    let mut loose = crate::model::MuxSessionNode::new(
        crate::model::MuxSessionId::new("tmux:scratch"),
        "tmux",
        "scratch",
    );
    loose.cwd = Some("/home/op/src/conspectus".into());
    loose.backend_session_id = Some("$9".into());
    loose.created_epoch = Some(1_699_990_000);
    snapshot
        .nodes
        .push(crate::model::GraphNode::MuxSession(loose));
    resolve_snapshot(snapshot)
}

fn native_app(view: View, filter: RowFilter) -> App {
    let snapshot = native_world();
    let mut config = RunConfig::defaults();
    config.default_view = view;
    config.initial_filter = filter.clone();
    let mut app = App::new(config);
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
        snapshot: &snapshot,
        view,
        grouping: crate::tui::Grouping::default_for(view),
        filter,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
        cwd: None,
    });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(&snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

fn first_bucket(app: &App) -> RowId {
    app.tree()
        .rows
        .iter()
        .find(|row| matches!(row.id, RowId::Bucket(_)))
        .expect("a project bucket")
        .id
        .clone()
}

fn mux_row(name: &str) -> RowId {
    RowId::MuxSession(NodeId::MuxSession(crate::model::MuxSessionId::new(
        format!("tmux:{name}"),
    )))
}

#[test]
fn native_views_mark_pinned_rows_without_moving_them() {
    let mut app = native_app(View::Mux, RowFilter::default());
    let frame = pinned_frame(&mut app, 120);
    let line_of = |name: &str| {
        frame
            .lines()
            .find(|line| line.contains(name))
            .unwrap_or_else(|| panic!("no row for {name}:\n{frame}"))
            .to_string()
    };
    assert!(line_of("conspectus-main").contains("◉ "), "{frame}");
    assert!(!line_of("scratch").contains("◉"), "{frame}");
    // The marker doesn't float the row: sort order is the view's own.
    let order: Vec<&str> = app
        .tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux.native_id.as_str()),
            _ => None,
        })
        .collect();
    let main = order.iter().position(|n| n.contains("conspectus-main"));
    let loose = order.iter().position(|n| n.contains("scratch"));
    assert!(main.is_some() && loose.is_some(), "{order:?}");
}

#[test]
fn pinned_only_keeps_pinned_rows_and_the_groups_above_them() {
    let filter = RowFilter {
        pinned_only: true,
        ..RowFilter::default()
    };
    let app = native_app(View::Mux, filter);
    let muxes: Vec<&str> = app
        .tree()
        .rows
        .iter()
        .filter_map(|row| match &row.kind {
            RowKind::MuxSession(mux) => Some(mux.native_id.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        muxes.iter().any(|n| n.contains("conspectus-main")),
        "{muxes:?}"
    );
    assert!(!muxes.iter().any(|n| n.contains("scratch")), "{muxes:?}");
}

#[test]
fn p_on_an_unpinned_row_pins_it_and_on_a_pinned_row_unpins_it_at_once() {
    use crate::tui::Effect;
    use crate::tui::effect::StoreOp;
    let mut app = native_app(View::Mux, RowFilter::default());

    app.set_selection(mux_row("scratch"));
    let effects = app.update(Msg::TogglePin);
    assert_eq!(
        effects,
        vec![Effect::WriteStore(StoreOp::PinNode {
            node: NodeId::MuxSession(crate::model::MuxSessionId::new("tmux:scratch")),
        })]
    );

    app.set_selection(mux_row("conspectus-main"));
    assert_eq!(
        app.update(Msg::TogglePin),
        vec![Effect::WriteStore(StoreOp::Unpin {
            slot: "conspectus-main".into(),
        })],
        "the inverse of p is p, with no modal in between"
    );
    assert!(app.row_menu().is_none());
}

#[test]
fn the_row_menu_of_a_native_row_offers_pin_or_unpin() {
    let mut app = native_app(View::Mux, RowFilter::default());
    app.set_selection(mux_row("scratch"));
    app.update(Msg::OpenRowMenu);
    let menu = app.row_menu().expect("menu");
    let labels: Vec<&str> = menu.entries().iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, ["Pin", "Show rebuild plan"]);
    assert!(matches!(menu.entries()[0].msg, Msg::PinNode(_)));
    app.close_row_menu();

    app.set_selection(mux_row("conspectus-main"));
    app.update(Msg::OpenRowMenu);
    let labels: Vec<String> = app
        .row_menu()
        .expect("menu")
        .entries()
        .iter()
        .map(|e| e.label.clone())
        .collect();
    // The same verbs the Pinned view offers for it: the menu depends
    // on the entity, not on the view. A running mux has nothing to
    // rebuild, so only its plan (which says so) is offered.
    assert_eq!(
        labels,
        [
            "Show rebuild plan",
            "Edit…",
            "Rename…",
            "Retarget…",
            "Refresh snapshot",
            "Unpin conspectus-main"
        ]
    );
}

#[test]
fn a_row_with_no_pinnable_entity_says_why() {
    use crate::tui::Effect;
    let mut app = slots_app(crate::tui::PinnedGrouping::Project);
    app.set_selection(first_bucket(&app));
    let effects = app.update(Msg::TogglePin);
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(text)] if text.starts_with("pin:")),
        "{effects:?}"
    );
}

#[test]
fn the_slot_menu_offers_every_gesture() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });
    assert!(app.update(Msg::OpenRowMenu).is_empty());
    let menu = app.row_menu().expect("the slot menu");
    let labels: Vec<&str> = menu.entries().iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Show rebuild plan",
            "Edit…",
            "Rename…",
            "Retarget…",
            "Refresh snapshot",
            "Reveal in its view",
            "Unpin conspectus-main"
        ],
        "a running mux has nothing to rebuild"
    );
    assert!(!menu.entries()[6].confirm, "unpin acts at once");
    insta::assert_snapshot!("slot_row_menu", trimmed(pinned_frame(&mut app, 100)));
}

#[test]
fn slot_prompts_commit_to_their_own_store_write() {
    use crate::tui::Effect;
    use crate::tui::effect::StoreOp;
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });

    app.update(Msg::OpenSlotRename("conspectus-main".into()));
    assert_eq!(
        app.rename_overlay()
            .map(crate::tui::widgets::input::TextInputState::value),
        Some("conspectus-main")
    );
    assert_eq!(
        app.update(Msg::CommitRename("renamed".into())),
        vec![Effect::WriteStore(StoreOp::RenameSlot {
            slot: "conspectus-main".into(),
            new_name: "renamed".into(),
        })]
    );

    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    assert!(app.pin_edit_form().is_some(), "edit opens the form");
    app.close_pin_edit_form();

    app.update(Msg::OpenSlotRetarget("conspectus-main".into()));
    assert!(app.picker().is_some(), "retarget offers candidates");
    app.close_picker();
    assert_eq!(
        app.update(Msg::CommitRetarget {
            pin: "conspectus-main".into(),
            target: "other".into(),
        }),
        vec![Effect::WriteStore(StoreOp::RetargetSlot {
            slot: "conspectus-main".into(),
            target: "other".into(),
        })]
    );
    app.update(Msg::OpenSlotRename("conspectus-main".into()));
    assert!(matches!(
        app.update(Msg::CommitRename("   ".into())).as_slice(),
        [Effect::Toast(_)]
    ));
}

#[test]
fn a_cancelled_slot_prompt_does_not_capture_the_next_rename() {
    use crate::tui::Effect;
    use crate::tui::effect::StoreOp;
    let mut app = native_app(View::Mux, RowFilter::default());
    app.update(Msg::OpenSlotRename("conspectus-main".into()));
    app.close_rename_overlay();

    // A later rename of the selected mux opens its own overlay, which
    // forgets the abandoned slot prompt.
    app.set_selection(mux_row("scratch"));
    app.open_rename_overlay(crate::tui::widgets::input::TextInputState::new(
        "Rename mux",
        "scratch",
    ));
    let effects = app.update(Msg::CommitRename("renamed".into()));
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::WriteStore(StoreOp::CommitMuxRename { .. })]
        ),
        "{effects:?}"
    );
}

#[test]
fn reveal_switches_to_the_native_view_and_selects_the_entity() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    // The Pinned fixture's snapshot is shared with the native views.
    app.update(Msg::RevealSlot("conspectus-main".into()));
    assert_eq!(app.active_view(), View::Mux);
    assert_eq!(app.selection(), Some(&mux_row("conspectus-main")));

    app.update(Msg::SwitchView(View::Pinned));
    let effects = app.update(Msg::RevealSlot("nightly".into()));
    assert!(
        matches!(effects.as_slice(), [crate::tui::Effect::Toast(_)]),
        "a planned slot has no entity: {effects:?}"
    );
}

#[test]
fn backspace_on_the_left_pane_returns_to_where_a_reveal_started() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.update(Msg::SwitchView(View::Pinned));
    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });
    let before = app.selection().cloned();

    app.update(Msg::RevealSelected);
    assert_eq!(app.active_view(), View::Mux);
    assert!(app.can_nav_back());

    app.update(Msg::ExplorerBack);
    assert_eq!(app.active_view(), View::Pinned);
    assert_eq!(app.selection().cloned(), before);
    assert!(!app.can_nav_back());
}

#[test]
fn backspace_on_the_right_pane_leaves_the_back_stack_alone() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.update(Msg::SwitchView(View::Pinned));
    app.set_selection(RowId::Pin {
        pin_id: "conspectus-main".into(),
    });
    app.update(Msg::RevealSelected);

    app.update(Msg::CycleFocus);
    app.update(Msg::ExplorerBack);

    assert_eq!(app.active_view(), View::Mux);
    assert!(app.can_nav_back());
}

#[test]
fn the_back_stack_keeps_only_the_newest_twenty_jumps() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    for n in 0..30 {
        app.push_nav(crate::tui::app::NavEntry {
            view: View::Sessions,
            selection: Some(RowId::Bucket(format!("b{n}"))),
        });
    }
    let mut popped = 0;
    while app.can_nav_back() {
        app.update(Msg::ExplorerBack);
        popped += 1;
    }
    assert_eq!(popped, 20);
}

#[test]
fn going_back_to_a_row_that_vanished_says_so() {
    use crate::tui::Effect;
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.push_nav(crate::tui::app::NavEntry {
        view: View::Pinned,
        selection: Some(RowId::Pin {
            pin_id: "no-longer-there".into(),
        }),
    });
    let effects = app.update(Msg::ExplorerBack);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Toast(t) if t == "that row is gone")),
        "{effects:?}"
    );
    assert_eq!(app.active_view(), View::Pinned);
}

#[test]
fn the_mux_launch_form_opens_with_defaults_and_the_worktree_toggle_off() {
    let mut app = native_app(View::Mux, RowFilter::default());
    app.update(Msg::OpenMuxLaunchForm);
    let Some(crate::tui::Modal::MuxLaunch(form)) = app.modal_stack().last() else {
        panic!("the launch form is on top");
    };
    assert!(
        !form.spec().worktree_enabled(),
        "a worktree is something the operator opts into"
    );
    assert!(
        !form.worktree_repo_value().is_empty(),
        "the repo is seeded so switching the toggle on starts from the launch directory"
    );
    assert!(form.default_hint().is_some() || form.spec().harness().value().is_empty());
}

// ----- Gone nodes in previews, reveal, and hints --------------------

#[test]
fn a_ghost_mux_has_no_pane_to_preview_but_a_live_one_does() {
    let snapshot = ghost_world();
    let app = mux_app_over(&snapshot, crate::filter::RowFilter::default());
    let mux_ids: Vec<(NodeId, bool)> = snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::MuxSession(mux) => {
                let id = NodeId::MuxSession(mux.id.clone());
                let live = snapshot.is_live(&id);
                Some((id, live))
            }
            _ => None,
        })
        .collect();
    assert!(mux_ids.iter().any(|(_, live)| *live) && mux_ids.iter().any(|(_, live)| !live));
    for (id, live) in mux_ids {
        let source = crate::tui::preview::node_source(&app, &id);
        assert_eq!(source.is_some(), live, "{id}: {source:?}");
    }
}

#[test]
fn gone_checkouts_and_repos_have_no_git_preview() {
    let snapshot = ghost_world();
    let app = mux_app_over(&snapshot, crate::filter::RowFilter::default());
    let gone: Vec<NodeId> = snapshot
        .nodes
        .iter()
        .map(GraphNode::id)
        .filter(|id| !snapshot.is_live(id))
        .filter(|id| matches!(id, NodeId::Checkout(_) | NodeId::Repo(_)))
        .collect();
    assert!(!gone.is_empty(), "the fixture has a gone worktree");
    for id in gone {
        assert_eq!(crate::tui::preview::node_source(&app, &id), None, "{id}");
    }
}

#[test]
fn a_pinned_ghost_mux_keeps_the_pin_marker_and_its_column() {
    let snapshot = ghost_world();
    let mut app = mux_app_over(&snapshot, crate::filter::RowFilter::default());
    let frame = pinned_frame(&mut app, 110);
    let ghost_line = frame
        .lines()
        .find(|line| line.contains("config-agent3"))
        .expect("ghost row");
    let live_line = frame
        .lines()
        .find(|line| line.contains("conspectus-main"))
        .expect("live row");
    assert!(ghost_line.contains('◎'), "{ghost_line}");
    let column = |line: &str, needle: &str| line.find(needle).map(|i| line[..i].chars().count());
    assert_eq!(
        column(ghost_line, "config-agent3"),
        column(live_line, "conspectus-main"),
        "ghost and live labels share a left edge:\n{frame}"
    );
}

#[test]
fn revealing_a_gone_entity_opens_the_history_view() {
    // The fixture's worktree record is gone; mark its node gone too, as
    // a discovery cycle does.
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    let node = crate::tui::rows::pin_nodes(&snapshot)
        .into_iter()
        .find(|pin| pin.id.id == "ledger-worktree")
        .and_then(|pin| pin.target_node.clone())
        .expect("the slot resolves to its worktree");
    snapshot.node_presence.insert(
        node,
        crate::model::NodePresence::Gone {
            last_seen: 1_699_000_000,
        },
    );
    let mut app = slots_app_over(&snapshot, crate::tui::PinnedGrouping::None);
    let effects = app.update(Msg::RevealSlot("ledger-worktree".into()));
    assert_eq!(app.active_view(), View::History, "{effects:?}");
    assert!(
        matches!(app.selection(), Some(RowId::History { .. })),
        "{:?}",
        app.selection()
    );
}

#[test]
fn a_failed_reveal_leaves_the_operator_where_they_were() {
    use crate::tui::Effect;
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    let before = app.selection().cloned();
    let effects = app.update(Msg::RevealSlot("no-such-slot".into()));
    assert!(
        matches!(effects.as_slice(), [Effect::Toast(_)]),
        "{effects:?}"
    );
    assert_eq!(app.active_view(), View::Pinned);
    assert_eq!(app.selection().cloned(), before);
}

#[test]
fn the_status_hint_for_a_live_repo_slot_matches_what_enter_does() {
    let snapshot = crate::tui::rows::pinned::all_kinds_world();
    let pins = crate::tui::rows::pin_nodes(&snapshot);
    let repo = pins
        .iter()
        .find(|pin| {
            pin.kind == crate::model::PinKind::Repo && pin.status == crate::model::SlotStatus::Live
        })
        .expect("a live repo slot");
    let hint = crate::tui::actions::pin_status_hint(&snapshot, &repo.id.id).expect("a hint");
    assert!(
        hint.contains("live repo") && hint.contains("nothing to rebuild"),
        "{hint}"
    );
    let mux = pins
        .iter()
        .find(|pin| {
            pin.kind == crate::model::PinKind::Mux && pin.status == crate::model::SlotStatus::Live
        })
        .expect("a live mux slot");
    assert_eq!(
        crate::tui::actions::pin_status_hint(&snapshot, &mux.id.id),
        None,
        "a live mux is attached, not described"
    );
}

#[test]
fn clean_up_shows_a_deselected_record_unticked() {
    use crate::tui::modal::Overlay;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let mut config = RunConfig::defaults();
    let zero = Some(std::time::Duration::from_secs(0));
    config
        .history
        .reap_after
        .insert(crate::model::HistoryKind::Mux, zero);
    config
        .history
        .reap_after
        .insert(crate::model::HistoryKind::Checkout, zero);
    let mut snapshot = crate::tui::rows::pinned::fixture_world();
    snapshot.pins.clear();
    let mut app = history_app_over(&snapshot, crate::tui::HistoryGrouping::None, config);
    app.update(Msg::OpenCleanUp);
    {
        let state = app.clean_up_mut().expect("the modal opened");
        let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
        state.handle((), press(KeyCode::Down));
        state.handle((), press(KeyCode::Char(' ')));
        assert_eq!(state.selected(), [0], "the second record was deselected");
    }
    let frame = pinned_frame(&mut app, 100);
    assert!(frame.contains("Clean up: forget these records"), "{frame}");
    assert!(frame.contains('√'), "one record stays ticked: {frame}");
    assert!(
        frame.lines().filter(|l| l.contains('√')).count() == 1,
        "only one record is ticked:\n{frame}"
    );
}

#[test]
fn opening_edit_builds_the_form_from_the_pin_and_saving_emits_one_store_write() {
    use crate::tui::Effect;
    use crate::tui::effect::StoreOp;
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    let form = app.pin_edit_form().expect("the edit form opens");
    assert_eq!(form.pin(), "conspectus-main");
    let labels = form.row_labels();
    assert_eq!(&labels[..2], ["pin name", "label"]);
    for field in crate::model::PinKind::Mux.overridable_fields() {
        assert!(labels.contains(field), "{field} missing from {labels:?}");
    }
    assert_eq!(form.name_text(), "conspectus-main");

    let request = crate::tui::widgets::pin_edit::PinEditRequest {
        pin: "conspectus-main".into(),
        rename: Some("renamed".into()),
        set: vec![],
        unset: vec![],
    };
    assert_eq!(
        app.update(Msg::CommitPinEdit(request.clone())),
        vec![Effect::WriteStore(StoreOp::SavePin { request })]
    );

    app.close_pin_edit_form();
    let effects = app.update(Msg::OpenSlotEdit("no-such-pin".into()));
    assert!(matches!(effects.as_slice(), [Effect::Toast(_)]));
    assert!(app.pin_edit_form().is_none());
}

#[test]
fn the_edit_form_shows_inherited_values_without_the_pins_overrides() {
    let mut app = slots_app(crate::tui::PinnedGrouping::None);
    app.update(Msg::OpenSlotEdit("conspectus-main".into()));
    let frame = pinned_frame(&mut app, 120);
    assert!(frame.contains("Edit pin"), "{frame}");
    assert!(frame.contains("pin name"), "{frame}");
    assert!(
        frame.contains("renames the tmux session too"),
        "a live mux pin:\n{frame}"
    );
}

#[test]
fn every_value_of_a_state_has_its_own_glyph() {
    use crate::tui::icons::state;
    use crate::tui::rows::{EntityState, MixedGrid, MixedRow, MixedTone, PinMarker};
    let theme = Theme::default();
    let text = |span: &ratatui::text::Span<'_>| span.content.to_string();

    // Sessions: linked to a mux or not. Ambiguity shares the unlinked
    // glyph; the enclosing group row carries the warning.
    let indicators = [
        MuxIndicator::Attached,
        MuxIndicator::Unmuxed,
        MuxIndicator::Ambiguous { candidate_count: 2 },
    ];
    for indicator in indicators {
        // Exhaustive: a new indicator must be classified here.
        match indicator {
            MuxIndicator::Attached | MuxIndicator::Unmuxed | MuxIndicator::Ambiguous { .. } => {}
        }
    }
    assert_eq!(
        text(&mux_indicator_span(indicators[0], &theme)),
        state::LINKED
    );
    assert_eq!(
        text(&mux_indicator_span(indicators[1], &theme)),
        state::UNLINKED
    );
    assert_eq!(
        text(&mux_indicator_span(indicators[2], &theme)),
        state::UNLINKED
    );

    // Mux rows: every entity state draws its own glyph.
    let mux_row = |entity: EntityState| MixedRow {
        node_kind: crate::model::NodeKind::MuxSession,
        label: "m".into(),
        status: String::new(),
        tone: MixedTone::Normal,
        age: None,
        age_epoch: None,
        state: Some(entity),
        chip: None,
        content: String::new(),
        primary_node: NodeId::MuxSession(MuxSessionId::new("m")),
        pr_state: None,
        marker: PinMarker::None,
        grid: MixedGrid::Mux,
    };
    let mut drawn = Vec::new();
    for entity in [
        EntityState::Linked,
        EntityState::Unlinked,
        EntityState::Stopped,
        EntityState::Unknown,
    ] {
        match entity {
            EntityState::Linked
            | EntityState::Unlinked
            | EntityState::Stopped
            | EntityState::Unknown => {}
        }
        let spans = render_mixed_spans(
            &mux_row(entity),
            &theme,
            MixedAlign::default(),
            4,
            80,
            0,
            None,
        );
        drawn.push(spans.last().map(text).expect("a state cell"));
    }
    assert_eq!(
        drawn,
        [
            state::LINKED,
            state::UNLINKED,
            state::STOPPED,
            state::UNKNOWN
        ]
    );

    // Pull requests: one shape per state.
    let mut pr_glyphs = Vec::new();
    for (state_name, draft) in [
        (Some("open"), false),
        (Some("open"), true),
        (Some("merged"), false),
        (Some("closed"), false),
        (None, false),
    ] {
        let mut pr = pr_row_for_layout("#1", "open", None);
        pr.state = state_name.map(str::to_string);
        pr.is_draft = draft;
        pr_glyphs.push(text(&pr_state_span(&pr, &theme)));
    }
    assert_eq!(
        pr_glyphs,
        [
            state::PR_OPEN,
            state::PR_DRAFT,
            state::PR_MERGED,
            state::PR_CLOSED,
            state::UNKNOWN
        ]
    );

    // The pin marker: filled, hollow, and an empty cell, all two cells wide.
    let markers = [PinMarker::None, PinMarker::Running, PinMarker::Stopped];
    let drawn: Vec<String> = markers
        .iter()
        .map(|marker| text(&pin_marker_span(*marker, &theme)))
        .collect();
    assert_eq!(drawn, ["  ", "◉ ", "◎ "]);
}
