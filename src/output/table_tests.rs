// Extracted from table.rs H-HYG-011 rolling wave via #[path = "table_tests.rs"] mod tests;
use super::*;
// H-HYG-010: render substrate items formerly re-exported from super
use crate::model::{
    AgentSessionId, AgentSessionNode, BranchId, Confidence, ForgePrId, ForgePrNode, ForkNode,
    Freshness, GraphLink, GraphNode, LinkEndpoint, LinkState, Metadata, MuxSessionId,
    MuxSessionNode, NodeId, Provenance, RelationKind, RepoId, RepoNode, SourceMetadata,
    WorkspaceId, WorkspaceNode,
};
use crate::output::render::{
    ColumnsError, MIN_COLUMN_BUDGET, Projection, RenderOptions, SHORT_ID_FLOOR, columns_for,
    default_columns, display_width, fit_to_width, format_relative_age, parse_columns_spec,
    render_columns_listing, resolve_explicit_columns, strip_branch_prefix, truncate_to_width,
    unique_prefix_len,
};
use crate::resolve::resolve_snapshot;

fn agent_session(harness: &str, key: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, "global", key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: None,
        last_active_epoch: None,
        session_kind: None,
    })
}

fn agent_session_with_preview(
    harness: &str,
    key: &str,
    cwd: Option<&str>,
    preview: &str,
) -> GraphNode {
    GraphNode::AgentSession(AgentSessionNode {
        id: AgentSessionId::new(harness, "global", key),
        harness_key: harness.to_string(),
        cwd: cwd.map(str::to_string),
        title: None,
        last_message_preview: Some(preview.to_string()),
        last_active_epoch: None,
        session_kind: None,
    })
}

fn mux_session(backend: &str, name: &str, cwd: Option<&str>) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("{backend}:{name}")),
        backend: backend.to_string(),
        native_id: name.to_string(),
        cwd: cwd.map(str::to_string),
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch: None,
        created_epoch: None,
        last_attached_epoch: None,
    })
}

fn linked_to_mux_link(
    id: &str,
    session_id: AgentSessionId,
    mux_id: MuxSessionId,
    provenance: Provenance,
    confidence: Confidence,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::AgentSession(session_id),
        target: LinkEndpoint::Node {
            id: NodeId::MuxSession(mux_id),
        },
        relation: RelationKind::LinkedToMux,
        provenance,
        confidence,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn associated_with_workspace(session_id: AgentSessionId, root: &str) -> GraphLink {
    let workspace = NodeId::Workspace(WorkspaceId::new(root));
    GraphLink {
        id: format!("workspace-{root}"),
        source: NodeId::AgentSession(session_id),
        target: LinkEndpoint::Node { id: workspace },
        relation: RelationKind::AssociatedWith,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn indicator_renders_provenance_confidence_and_ambiguity_marker() {
    assert_eq!(
        indicator(Provenance::LocalDeclared, Confidence::High, false),
        "LD/H"
    );
    assert_eq!(
        indicator(Provenance::StrongDiscovered, Confidence::Medium, true),
        "SD/M*"
    );
    assert_eq!(indicator(Provenance::Cached, Confidence::Low, false), "$/L");
    assert_eq!(
        indicator(Provenance::Convention, Confidence::High, false),
        "C/H"
    );
}

#[test]
fn empty_snapshot_renders_header_only_for_each_projection() {
    let snapshot = GraphSnapshot::empty();
    for projection in [Projection::Agent, Projection::Mux, Projection::Union] {
        let rendered = render(&snapshot, projection);
        // Header line + dashes line, no data rows.
        assert_eq!(
            rendered.lines().count(),
            2,
            "projection {projection:?} should render only a header",
        );
    }
}

#[test]
fn agent_projection_emits_one_row_per_agent_session() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            agent_session("codex", "beta", None),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);

    let body_rows: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body_rows.len(), 2);
    assert!(body_rows.iter().any(|row| row.contains("alpha")));
    assert!(body_rows.iter().any(|row| row.contains("beta")));
}

#[test]
fn agent_projection_shows_single_mux_without_ambiguity_marker() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let mux_id = MuxSessionId::new("tmux:editor");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            mux_session("tmux", "editor", Some("/work/a")),
        ],
        candidate_links: vec![linked_to_mux_link(
            "link-1",
            session_id,
            mux_id,
            Provenance::StrongDiscovered,
            Confidence::High,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);

    assert!(rendered.contains("tmux:editor"));
    assert!(rendered.contains("SD/H"));
    assert!(!rendered.contains("SD/H*"));
}

#[test]
fn agent_projection_marks_ambiguous_mux_selection() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            mux_session("tmux", "one", Some("/work/a")),
            mux_session("tmux", "two", Some("/work/a")),
        ],
        candidate_links: vec![
            linked_to_mux_link(
                "link-1",
                session_id.clone(),
                MuxSessionId::new("tmux:one"),
                Provenance::Discovered,
                Confidence::Medium,
            ),
            linked_to_mux_link(
                "link-2",
                session_id,
                MuxSessionId::new("tmux:two"),
                Provenance::Discovered,
                Confidence::Medium,
            ),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);

    assert!(
        rendered.contains("D/M*"),
        "expected ambiguity marker in:\n{rendered}",
    );
}

#[test]
fn mux_projection_lists_attached_agents() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            mux_session("tmux", "editor", Some("/work/a")),
        ],
        candidate_links: vec![linked_to_mux_link(
            "link-1",
            session_id,
            MuxSessionId::new("tmux:editor"),
            Provenance::StrongDiscovered,
            Confidence::High,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Mux);

    assert!(rendered.contains("tmux:editor"));
    assert!(rendered.contains("codex:alpha"));
    assert!(rendered.contains("SD/H"));
}

#[test]
fn mux_projection_shows_zero_agents_for_orphan_mux() {
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Mux);

    let body: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body.len(), 1);
    assert!(body[0].contains("tmux:lonely"));
    assert!(body[0].contains("—"));
}

#[test]
fn union_projection_preserves_both_node_types() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            mux_session("tmux", "lonely", Some("/work")),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Union);

    let body: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body.len(), 2);
    // The leftmost column is now the short ID; the kind cell follows
    // a column gap.
    assert!(body.iter().any(|row| row.contains("  agent ")));
    assert!(body.iter().any(|row| row.contains("  mux ")));
}

#[test]
fn union_projection_renders_mux_relationship_for_session() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            mux_session("tmux", "editor", Some("/work/a")),
        ],
        candidate_links: vec![linked_to_mux_link(
            "link-1",
            session_id,
            MuxSessionId::new("tmux:editor"),
            Provenance::StrongDiscovered,
            Confidence::High,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Union);

    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let agent_row = body
        .iter()
        .find(|row| row.contains("  agent "))
        .expect("agent row");
    assert!(agent_row.contains("mux=tmux:editor"));
    assert!(agent_row.contains("SD/H"));
}

fn parent_session_link(id: &str, child: AgentSessionId, parent: AgentSessionId) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::AgentSession(child),
        target: LinkEndpoint::Node {
            id: NodeId::AgentSession(parent),
        },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

fn unresolved_parent_session_link(
    id: &str,
    child: AgentSessionId,
    parent_native_id: &str,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::AgentSession(child),
        target: LinkEndpoint::Unresolved {
            evidence: crate::model::UnresolvedEndpoint {
                node_type: "agent_session".to_string(),
                harness_key: Some("claude-code".to_string()),
                native_id: Some(parent_native_id.to_string()),
                state_scope: None,
                path: None,
                metadata: Default::default(),
            },
        },
        relation: RelationKind::ParentSession,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn agent_projection_lineage_column_shows_resolved_parent_short_id() {
    let parent_id = AgentSessionId::new("claude-code", "global", "parent");
    let child_id = AgentSessionId::new("claude-code", "global", "child");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("claude-code", "parent", Some("/work")),
            agent_session("claude-code", "child", Some("/work")),
        ],
        candidate_links: vec![parent_session_link("lineage-1", child_id, parent_id)],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);

    assert!(rendered.contains("LINEAGE"));
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let child_row = body
        .iter()
        .find(|row| row.contains("claude-code:child"))
        .expect("child row");
    assert!(
        child_row.contains("parent"),
        "expected parent short id in child row:\n{child_row}",
    );
    let parent_row = body
        .iter()
        .find(|row| row.contains("claude-code:parent"))
        .expect("parent row");
    // Parent has no parent of its own, so it shows the empty lineage cell.
    assert!(
        parent_row.trim_end().ends_with("—"),
        "expected empty lineage in parent row:\n{parent_row}",
    );
}

#[test]
fn agent_projection_lineage_column_marks_chain_with_arrow() {
    let grand_id = AgentSessionId::new("claude-code", "global", "grand");
    let mid_id = AgentSessionId::new("claude-code", "global", "mid");
    let leaf_id = AgentSessionId::new("claude-code", "global", "leaf");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("claude-code", "grand", Some("/work")),
            agent_session("claude-code", "mid", Some("/work")),
            agent_session("claude-code", "leaf", Some("/work")),
        ],
        candidate_links: vec![
            parent_session_link("link-mid", mid_id.clone(), grand_id),
            parent_session_link("link-leaf", leaf_id, mid_id),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);
    let body: Vec<&str> = rendered.lines().skip(2).collect();

    let leaf_row = body
        .iter()
        .find(|row| row.contains("claude-code:leaf"))
        .expect("leaf row");
    assert!(
        leaf_row.contains("mid←"),
        "leaf's lineage cell should mark longer ancestry with ←:\n{leaf_row}",
    );

    let mid_row = body
        .iter()
        .find(|row| row.contains("claude-code:mid"))
        .expect("mid row");
    // mid's parent (grand) has no parent itself, so no chain marker.
    assert!(
        mid_row.contains("grand") && !mid_row.contains("grand←"),
        "mid's lineage cell should be `grand` without chain marker:\n{mid_row}",
    );
}

#[test]
fn agent_projection_lineage_column_marks_unresolved_parent_with_question_prefix() {
    let child_id = AgentSessionId::new("claude-code", "global", "child");
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("claude-code", "child", Some("/work"))],
        candidate_links: vec![unresolved_parent_session_link(
            "lineage-unresolved",
            child_id,
            "missing-parent",
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let child_row = body
        .iter()
        .find(|row| row.contains("claude-code:child"))
        .expect("child row");
    assert!(
        child_row.contains("?missing-parent") || child_row.contains("?…"),
        "unresolved parent should appear with `?` prefix:\n{child_row}",
    );
}

#[test]
fn short_session_id_abbreviates_long_uuids() {
    assert_eq!(short_session_id("alpha"), "alpha");
    assert_eq!(short_session_id("session-1234"), "session-1234");
    let uuid = "019e2454-8f7e-7543-aac5-b0d8ff75be49";
    let abbr = short_session_id(uuid);
    assert!(
        abbr.starts_with('…') && abbr.len() < uuid.len(),
        "expected abbreviation, got {abbr}",
    );
    assert!(abbr.ends_with(&uuid[uuid.len() - 8..]));
}

#[test]
fn agent_projection_shows_preferred_pr_when_branch_has_one() {
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    let branch_id = BranchId::new(
        RepoId::new("/workspace/repo/.git"),
        "refs/heads/feature".to_string(),
    );
    let mut pr_link = GraphLink::new(
        "pr-link",
        NodeId::ForgePr(pr_id.clone()),
        LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        },
        RelationKind::BranchHasForgePr,
        Provenance::StrongDiscovered,
    );
    pr_link.confidence = Confidence::High;
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            GraphNode::ForgePr(ForgePrNode {
                id: pr_id,
                provider: "github".to_string(),
                host: "github.com".to_string(),
                owner: "octo".to_string(),
                repo: "repo".to_string(),
                number: 7,
                state: Some("open".to_string()),
                url: None,
                updated_epoch: None,
                is_draft: false,
            }),
        ],
        candidate_links: vec![pr_link],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);

    assert!(
        rendered.contains("octo/repo#7"),
        "expected PR label in:\n{rendered}",
    );
}

#[test]
fn render_columns_listing_marks_default_columns() {
    let listing = render_columns_listing(Projection::Agent, false);
    // ID is in the default set.
    let id_line = listing
        .lines()
        .find(|line| line.starts_with("id "))
        .expect("id line");
    assert!(id_line.ends_with("(default)"), "got: {id_line}");

    // `checkout` was added as opt-in by H-TBL-010, so its line
    // should not carry the (default) marker.
    let checkout_line = listing
        .lines()
        .find(|line| line.starts_with("checkout "))
        .expect("checkout line");
    assert!(
        !checkout_line.contains("(default)"),
        "checkout should not be marked default: {checkout_line}",
    );
    assert!(
        checkout_line.contains("matches the session's cwd"),
        "checkout line should include the description: {checkout_line}",
    );
}

#[test]
fn render_columns_listing_includes_every_registered_key() {
    for projection in [
        Projection::Agent,
        Projection::Mux,
        Projection::Union,
        Projection::Pr,
        Projection::Fork,
    ] {
        let listing = render_columns_listing(projection, false);
        for spec in columns_for(projection) {
            assert!(
                listing
                    .lines()
                    .any(|line| line.starts_with(&format!("{} ", spec.key))),
                "listing for {projection:?} missing column {key}:\n{listing}",
                key = spec.key,
            );
        }
    }
}

#[test]
fn default_columns_for_each_projection_matches_registry() {
    for projection in [Projection::Agent, Projection::Mux, Projection::Union] {
        let registry: Vec<&str> = columns_for(projection)
            .iter()
            .filter(|c| c.default)
            .map(|c| c.key)
            .collect();
        assert_eq!(default_columns(projection), registry);
    }
}

#[test]
fn parse_columns_empty_spec_returns_default_set() {
    // An empty string yields an empty-token error; the harness-side
    // default (no flag at all) is to keep defaults — tested in
    // CLI integration. Here we just check that ".." returns default.
    let result = parse_columns_spec(Projection::Agent, "default").expect("default token");
    assert_eq!(result, default_columns(Projection::Agent));
}

#[test]
fn parse_columns_explicit_list_resets_running_set() {
    let result = parse_columns_spec(Projection::Agent, "id,agent,cwd").expect("explicit list");
    assert_eq!(result, vec!["id", "agent", "cwd"]);
}

#[test]
fn parse_columns_plus_appends_to_defaults() {
    // Default already contains "lineage", so adding it is a no-op
    // but should still succeed.
    let result = parse_columns_spec(Projection::Agent, "+lineage").expect("plus lineage");
    assert_eq!(result, default_columns(Projection::Agent));
}

#[test]
fn parse_columns_minus_removes_from_defaults() {
    let result = parse_columns_spec(Projection::Agent, "-cwd,-mux-conf").expect("minus tokens");
    assert_eq!(
        result,
        vec!["id", "agent", "mux", "pr", "pr-conf", "lineage"]
    );
}

#[test]
fn parse_columns_all_resets_to_every_registered_column() {
    let result = parse_columns_spec(Projection::Mux, "all").expect("all");
    // `all` reflects every registered column in registry order;
    // the mux registry is the smallest and the easiest to lock in.
    assert_eq!(
        result,
        vec![
            "id",
            "mux",
            "cwd",
            "agents",
            "preview",
            "attached-count",
            "activity",
            "created",
        ],
    );
}

#[test]
fn parse_columns_mixed_explicit_list_then_plus() {
    // Bare token resets, then `+` appends after.
    let result = parse_columns_spec(Projection::Agent, "id,agent,+cwd").expect("mixed");
    assert_eq!(result, vec!["id", "agent", "cwd"]);
}

#[test]
fn parse_columns_unknown_name_errors_with_available_listed() {
    let err = parse_columns_spec(Projection::Agent, "+nope").unwrap_err();
    match err {
        ColumnsError::UnknownColumn {
            name,
            row_type,
            available,
        } => {
            assert_eq!(name, "nope");
            assert_eq!(row_type, "sessions");
            assert!(available.contains(&"agent"));
        }
        other @ ColumnsError::EmptyToken => panic!("expected UnknownColumn, got {other:?}"),
    }
}

#[test]
fn parse_columns_rejects_legacy_worktree_column() {
    let err = parse_columns_spec(Projection::Agent, "id,worktree").unwrap_err();
    match err {
        ColumnsError::UnknownColumn {
            name,
            row_type,
            available,
        } => {
            assert_eq!(name, "worktree");
            assert_eq!(row_type, "sessions");
            assert!(available.contains(&"checkout"));
            assert!(!available.contains(&"worktree"));
        }
        other @ ColumnsError::EmptyToken => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn parse_columns_empty_token_errors() {
    let err = parse_columns_spec(Projection::Agent, "id,,agent").unwrap_err();
    assert!(matches!(err, ColumnsError::EmptyToken));
}

#[test]
fn resolve_explicit_columns_validates_each_name() {
    let names: Vec<String> = vec!["id".into(), "agent".into(), "lineage".into()];
    let result = resolve_explicit_columns(Projection::Agent, &names).expect("resolve explicit");
    assert_eq!(result, vec!["id", "agent", "lineage"]);

    let bad: Vec<String> = vec!["id".into(), "nope".into()];
    let err = resolve_explicit_columns(Projection::Agent, &bad).unwrap_err();
    assert!(matches!(err, ColumnsError::UnknownColumn { .. }));
}

#[test]
fn render_with_explicit_columns_emits_only_those_cells() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
        ..GraphSnapshot::empty()
    };
    let options = RenderOptions::wide().with_columns(vec!["id", "agent", "cwd"]);
    let rendered = render_with(&snapshot, Projection::Agent, &options);
    let header_tokens: Vec<&str> = rendered
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    // Three columns, no MUX/PR/LINEAGE.
    assert_eq!(header_tokens, vec!["ID", "AGENT", "CWD"]);
    assert!(!rendered.contains("MUX"));
    assert!(!rendered.contains("PR"));
    assert!(!rendered.contains("LINEAGE"));

    let body_tokens: Vec<&str> = rendered
        .lines()
        .nth(2)
        .unwrap()
        .split_whitespace()
        .collect();
    // [<short>, codex:alpha, /work/a]
    assert_eq!(body_tokens.len(), 3);
    assert_eq!(body_tokens[1], "codex:alpha");
    assert_eq!(body_tokens[2], "/work/a");
}

#[test]
fn render_color_disabled_is_byte_identical_to_plain_text() {
    // Regression: turning the `color` knob on/off must not perturb
    // the bytes when color is off. This guards every `insta`
    // snapshot from silently growing ANSI escapes.
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
        ..GraphSnapshot::empty()
    };
    let plain = render(&snapshot, Projection::Agent);
    let off = render_with(&snapshot, Projection::Agent, &RenderOptions::wide());
    let off_with_color_false = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_color(false),
    );
    assert_eq!(plain, off);
    assert_eq!(plain, off_with_color_false);
    assert!(!plain.contains('\u{1b}'));
}

#[test]
fn render_color_enabled_wraps_header_and_styled_cells_in_ansi() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_color(true),
    );
    // Header row gets the bold envelope.
    assert!(
        colored.contains("\u{1b}[1m"),
        "expected bold escape for header:\n{colored:?}",
    );
    // The ID column is colored blue.
    assert!(
        colored.contains("\u{1b}[34m"),
        "expected blue escape for ID column:\n{colored:?}",
    );
    // The `—` placeholder uses 256-color 244 (faded gray); the
    // standard sequence is `\x1b[38;5;244m`.
    assert!(
        colored.contains("\u{1b}[38;5;244m"),
        "expected 256-color 244 for `—` placeholder:\n{colored:?}",
    );
    // Every opened escape closes with the reset sequence.
    assert!(
        colored.contains("\u{1b}[0m"),
        "expected reset escape in:\n{colored:?}",
    );
}

#[test]
fn render_color_enabled_colors_agent_harness_prefix_only() {
    // The agent cell should colorize the `claude-code` portion of
    // `claude-code:alpha` but leave the `:alpha` suffix
    // uncolored. The bright-yellow escape sequence appears, the
    // session-key portion does not get a fresh escape introduced
    // (the prefix's escape is followed by reset, then plain
    // text).
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("claude-code", "alpha", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_color(true),
    );
    // Bright yellow = `\x1b[93m`.
    let bright_yellow = "\u{1b}[93m";
    assert!(
        colored.contains(bright_yellow),
        "expected bright yellow for claude-code harness:\n{colored:?}",
    );
    // The styled span is just the harness name; the body looks
    // like `<esc>[93m claude-code <reset>:alpha` (no surrounding
    // escapes around `:alpha`).
    assert!(
        colored.contains(&format!("{bright_yellow}claude-code\u{1b}[0m:alpha")),
        "expected claude-code prefix wrapped, suffix plain:\n{colored:?}",
    );
}

#[test]
fn render_color_enabled_uses_distinct_color_per_harness() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("claude-code", "a", Some("/w/a")),
            agent_session("codex", "b", Some("/w/b")),
            agent_session("opencode", "c", Some("/w/c")),
            agent_session("aider", "d", Some("/w/d")),
        ],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_color(true),
    );
    // Each harness has its own bright-color escape.
    assert!(
        colored.contains("\u{1b}[93m"),
        "claude-code = bright yellow"
    );
    assert!(colored.contains("\u{1b}[94m"), "codex = bright blue");
    assert!(colored.contains("\u{1b}[92m"), "opencode = bright green");
    assert!(colored.contains("\u{1b}[91m"), "aider = bright red");
}

#[test]
fn render_color_enabled_union_label_colors_agent_harness_prefix() {
    // The union projection's LABEL column should carry the same
    // harness-prefix colouring as the sessions projection's
    // AGENT cell. Agent rows show coloured `harness:`; mux
    // rows render `backend:native_id` as plain text because
    // the mux backend is not in the harness palette.
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work")),
            mux_session("tmux", "editor", Some("/work")),
        ],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Union,
        &RenderOptions::wide().with_color(true),
    );
    // Codex prefix gets bright blue; the `:alpha` tail stays
    // plain (no fresh escape after the reset).
    assert!(
        colored.contains("\u{1b}[94mcodex\u{1b}[0m:alpha"),
        "union LABEL should colour the codex prefix:\n{colored:?}",
    );
    // Mux label has no harness palette slot — `tmux` falls
    // through to plain text, no bright-color escape adjacent
    // to it.
    let mux_line = colored
        .lines()
        .find(|line| line.contains("tmux:editor"))
        .expect("mux row");
    assert!(
        !mux_line.contains("\u{1b}[9"),
        "mux row LABEL should not have a bright-color escape:\n{mux_line:?}",
    );
}

#[test]
fn render_color_enabled_unknown_harness_stays_uncolored() {
    // A novel harness key should fall through to no styling so
    // the cell still reads cleanly until a palette slot is
    // chosen for it.
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("future-tool", "x", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_color(true),
    );
    // No bright-color escape should appear adjacent to
    // `future-tool` — the cell renders verbatim.
    let needle = "future-tool:x";
    assert!(
        colored.contains(needle),
        "expected verbatim agent cell for unknown harness:\n{colored:?}",
    );
}

#[test]
fn render_color_enabled_colors_each_harness_in_mux_agents_cell() {
    // The mux projection's AGENTS column joins multiple agents
    // into one cell. Each agent's harness prefix should be
    // colored separately, the `:session-key` tail stays plain,
    // and the trailing `[indicator]` suffix stays uncolored.
    let editor = MuxSessionId::new("tmux:editor");
    let snapshot = GraphSnapshot {
        nodes: vec![
            mux_session("tmux", "editor", Some("/work")),
            agent_session("codex", "alpha", Some("/work")),
            agent_session("claude-code", "beta", Some("/work")),
        ],
        candidate_links: vec![
            linked_to_mux_link(
                "link-c",
                AgentSessionId::new("codex", "global", "alpha"),
                editor.clone(),
                Provenance::StrongDiscovered,
                Confidence::High,
            ),
            linked_to_mux_link(
                "link-cc",
                AgentSessionId::new("claude-code", "global", "beta"),
                editor,
                Provenance::StrongDiscovered,
                Confidence::High,
            ),
        ],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_color(true),
    );
    // Bright blue = codex; bright yellow = claude-code.
    assert!(
        colored.contains("\u{1b}[94mcodex\u{1b}[0m:alpha "),
        "expected codex prefix coloured inside agents cell:\n{colored:?}",
    );
    assert!(
        colored.contains("\u{1b}[93mclaude-code\u{1b}[0m:beta "),
        "expected claude-code prefix coloured inside agents cell:\n{colored:?}",
    );
    // The comma separator between entries should be plain (no
    // escape immediately after `]`).
    assert!(
        colored.contains("], \u{1b}[94mcodex") || colored.contains("], \u{1b}[93mclaude-code"),
        "expected plain `, ` separator between coloured entries:\n{colored:?}",
    );
}

#[test]
fn render_color_enabled_dash_in_mux_agents_cell_uses_placeholder_style() {
    // A mux with no attached agents renders the cell as `—`,
    // which should pick up the faded-gray placeholder style
    // (256-color 244) rather than dropping out as plain text.
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_color(true),
    );
    assert!(
        colored.contains("\u{1b}[38;5;244m—\u{1b}[0m"),
        "expected faded `—` for empty agents cell:\n{colored:?}",
    );
}

#[test]
fn render_color_enabled_colors_pr_state_cell() {
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::ForgePr(ForgePrNode {
            id: pr_id,
            provider: "github".to_string(),
            host: "github.com".to_string(),
            owner: "octo".to_string(),
            repo: "repo".to_string(),
            number: 7,
            state: Some("open".to_string()),
            url: None,
            updated_epoch: None,
            is_draft: false,
        })],
        ..GraphSnapshot::empty()
    };
    let colored = render_with(
        &snapshot,
        Projection::Pr,
        &RenderOptions::wide().with_color(true),
    );
    // Green is `\x1b[32m` in ANSI 16-color.
    assert!(
        colored.contains("\u{1b}[32m"),
        "expected green for PR state `open`:\n{colored:?}",
    );
}

#[test]
fn render_with_card_layout_honors_column_selection() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
        ..GraphSnapshot::empty()
    };
    let options = RenderOptions::card().with_columns(vec!["id", "agent"]);
    let rendered = render_with(&snapshot, Projection::Agent, &options);
    let lines: Vec<&str> = rendered.lines().filter(|l| !l.is_empty()).collect();
    // Card emits one `KEY: value` line per selected column.
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("ID:"));
    assert!(lines[1].starts_with("AGENT:"));
}

#[test]
fn format_relative_age_picks_largest_unit_under_threshold() {
    assert_eq!(format_relative_age(100, 100), "0s");
    assert_eq!(format_relative_age(100, 159), "59s");
    assert_eq!(format_relative_age(100, 160), "1m");
    assert_eq!(format_relative_age(100, 100 + 3600), "1h");
    assert_eq!(format_relative_age(100, 100 + 86_400), "1d");
    assert_eq!(format_relative_age(100, 100 + 7 * 86_400), "1w");
}

#[test]
fn format_relative_age_future_value_renders_as_now() {
    assert_eq!(format_relative_age(2_000_000_000, 1_000_000_000), "now");
}

#[test]
fn strip_branch_prefix_drops_refs_heads_only() {
    assert_eq!(strip_branch_prefix("refs/heads/feature"), "feature");
    assert_eq!(strip_branch_prefix("main"), "main");
    assert_eq!(strip_branch_prefix("refs/tags/v1"), "refs/tags/v1");
}

fn mux_session_with_epochs(
    backend: &str,
    name: &str,
    activity_epoch: Option<i64>,
    created_epoch: Option<i64>,
) -> GraphNode {
    GraphNode::MuxSession(MuxSessionNode {
        id: MuxSessionId::new(format!("{backend}:{name}")),
        backend: backend.to_string(),
        native_id: name.to_string(),
        cwd: None,
        active_pane_command: None,
        active_pane_pid: None,
        active_pane_current_path: None,
        active_pane_start_command: None,
        client_attached: None,
        activity_epoch,
        created_epoch,
        last_attached_epoch: None,
    })
}

#[test]
fn mux_projection_attached_count_column() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let mux_id = MuxSessionId::new("tmux:editor");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work")),
            mux_session("tmux", "editor", Some("/work")),
            mux_session("tmux", "lonely", Some("/work")),
        ],
        candidate_links: vec![linked_to_mux_link(
            "link-1",
            session_id,
            mux_id,
            Provenance::StrongDiscovered,
            Confidence::High,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_columns(vec!["id", "mux", "attached-count"]),
    );
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body.len(), 2);
    let editor_row = body
        .iter()
        .find(|line| line.contains("tmux:editor"))
        .expect("editor row");
    let lonely_row = body
        .iter()
        .find(|line| line.contains("tmux:lonely"))
        .expect("lonely row");
    // Editor has one attached agent; lonely has none.
    assert!(
        editor_row.split_whitespace().any(|t| t == "1"),
        "editor row should show count 1:\n{editor_row}",
    );
    assert!(
        lonely_row.trim_end().ends_with('—'),
        "lonely row should show — for zero attached:\n{lonely_row}",
    );
}

#[test]
fn mux_projection_activity_and_created_columns_format_relative_age() {
    // Use the formatter directly to lock in the recency string;
    // the actual rendered row passes the same value through.
    let activity = 100;
    let created = 0;
    let now = activity + 7200; // 2h after activity, ~2h after created.
    assert_eq!(format_relative_age(activity, now), "2h");
    assert_eq!(format_relative_age(created, now), "2h");

    // Render with explicit columns; verify the cells contain a unit
    // suffix (the exact recency depends on SystemTime::now()).
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session_with_epochs(
            "tmux",
            "editor",
            Some(activity),
            Some(created),
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_columns(vec!["id", "mux", "activity", "created"]),
    );
    let body = rendered.lines().nth(2).expect("body row");
    // Recency for epoch 100 is many years; assert the cell ends in
    // a recognized unit suffix.
    let cells: Vec<&str> = body.split_whitespace().collect();
    // [<short>, tmux:editor, <activity-cell>, <created-cell>]
    assert!(cells.len() >= 4, "row should have 4 cells: {body:?}");
    for cell in &cells[2..4] {
        let last = cell.chars().last().expect("non-empty cell");
        assert!(
            matches!(last, 's' | 'm' | 'h' | 'd' | 'w'),
            "expected recency suffix, got {cell:?}",
        );
    }
}

#[test]
fn mux_projection_activity_and_created_dashes_when_epoch_is_none() {
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session_with_epochs("tmux", "editor", None, None)],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_columns(vec!["id", "mux", "activity", "created"]),
    );
    let body = rendered.lines().nth(2).expect("body row");
    let cells: Vec<&str> = body.split_whitespace().collect();
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[2], "—");
    assert_eq!(cells[3], "—");
}

#[test]
fn sessions_projection_optional_branch_repo_worktree_columns() {
    use crate::model::{CheckoutId, CheckoutNode};

    let repo_id = RepoId::new("/workspace/repo/.git");
    let branch_id = BranchId::new(repo_id.clone(), "refs/heads/feature".to_string());
    let worktree_id = CheckoutId::new(repo_id, "/workspace/repo");

    let worktree_to_branch = GraphLink {
        id: "wt-branch".to_string(),
        source: NodeId::Checkout(worktree_id.clone()),
        target: LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        },
        relation: RelationKind::CheckedOutBranch,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    };

    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/workspace/repo/crates/core")),
            GraphNode::Checkout(CheckoutNode {
                id: worktree_id,
                root: "/workspace/repo".to_string(),
                git_dir: None,
                current_branch: None,
            }),
        ],
        candidate_links: vec![worktree_to_branch],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "checkout", "branch", "repo"]),
    );
    let body = rendered.lines().nth(2).expect("body row");
    assert!(body.contains("/workspace/repo"), "got:\n{body}");
    assert!(body.contains("feature"), "got:\n{body}");
    assert!(body.contains("/workspace/repo/.git"), "got:\n{body}");
}

#[test]
fn sessions_projection_workspace_column_renders_session_context() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![agent_session(
            "codex",
            "alpha",
            Some("/workspace/repo/crates/core"),
        )],
        candidate_links: vec![associated_with_workspace(session_id, "/workspace")],
        ..GraphSnapshot::empty()
    });

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "workspace"]),
    );

    assert!(
        rendered.contains("/workspace"),
        "workspace column should expose resolved session context:\n{rendered}",
    );
}

fn workspace_node(root: &str, provider: Option<&str>) -> GraphNode {
    GraphNode::Workspace(WorkspaceNode {
        id: WorkspaceId::new(root),
        root: root.to_string(),
        provider: provider.map(str::to_string),
        name: None,
    })
}

fn repo_node(common_dir: &str) -> GraphNode {
    GraphNode::Repo(RepoNode::new(RepoId::new(common_dir)))
}

fn workspace_contains_repo_link(
    link_id: &str,
    workspace_root: &str,
    repo_common_dir: &str,
    logical_path: &str,
) -> GraphLink {
    let mut fields = Metadata::new();
    fields.insert(
        "logical_path".to_string(),
        serde_json::Value::String(logical_path.to_string()),
    );
    GraphLink {
        id: link_id.to_string(),
        source: NodeId::Workspace(WorkspaceId::new(workspace_root)),
        target: LinkEndpoint::Node {
            id: NodeId::Repo(RepoId::new(repo_common_dir)),
        },
        relation: RelationKind::WorkspaceContainsRepo,
        provenance: Provenance::Discovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata {
            adapter: "test".to_string(),
            evidence: None,
            fields,
            freshness_epoch: None,
        },
        state: LinkState::Active,
    }
}

#[test]
fn sessions_projection_workspace_joins_member_repo_names_when_multi_repo() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent_session(
                "codex",
                "alpha",
                Some("/work/atelier/conspectus/crates/core"),
            ),
            workspace_node("/work/atelier", Some("atelier")),
            repo_node("/work/atelier/atelier/.git"),
            repo_node("/work/atelier/conspectus/.git"),
        ],
        candidate_links: vec![
            associated_with_workspace(session_id, "/work/atelier"),
            workspace_contains_repo_link(
                "ws-member-atelier",
                "/work/atelier",
                "/work/atelier/atelier/.git",
                "/work/atelier/atelier",
            ),
            workspace_contains_repo_link(
                "ws-member-conspectus",
                "/work/atelier",
                "/work/atelier/conspectus/.git",
                "/work/atelier/conspectus",
            ),
        ],
        ..GraphSnapshot::empty()
    });

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "workspace"]),
    );

    assert!(
        rendered.contains("atelier+conspectus"),
        "multi-repo workspace should render joined member names:\n{rendered}",
    );
    assert!(
        !rendered.contains("/work/atelier "),
        "multi-repo workspace should not also render the root path:\n{rendered}",
    );
}

#[test]
fn sessions_projection_workspace_falls_back_to_root_when_single_repo() {
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/solo/repo-a/src")),
            workspace_node("/work/solo", None),
            repo_node("/work/solo/repo-a/.git"),
        ],
        candidate_links: vec![
            associated_with_workspace(session_id, "/work/solo"),
            workspace_contains_repo_link(
                "ws-member-solo",
                "/work/solo",
                "/work/solo/repo-a/.git",
                "/work/solo/repo-a",
            ),
        ],
        ..GraphSnapshot::empty()
    });

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "workspace"]),
    );

    assert!(
        rendered.contains("/work/solo"),
        "single-repo workspace should keep the root path:\n{rendered}",
    );
    assert!(
        !rendered.contains("repo-a+"),
        "single-repo workspace should not render a joined name:\n{rendered}",
    );
}

#[test]
fn sessions_projection_workspace_dedups_duplicate_member_names() {
    // Two member links resolving to distinct repo identities but
    // the same logical path leaf (e.g. an agent-deck workspace
    // with symlinks to two checkouts both named `repo`). The
    // joined display should collapse the duplicate so the threshold
    // check sees only one distinct name and falls back to the root.
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let snapshot = resolve_snapshot(GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/deck/abc/src")),
            workspace_node("/work/deck/abc", Some("agent-deck")),
            repo_node("/repos/first/.git"),
            repo_node("/repos/second/.git"),
        ],
        candidate_links: vec![
            associated_with_workspace(session_id, "/work/deck/abc"),
            workspace_contains_repo_link(
                "deck-link-a",
                "/work/deck/abc",
                "/repos/first/.git",
                "/work/deck/abc/repo",
            ),
            workspace_contains_repo_link(
                "deck-link-b",
                "/work/deck/abc",
                "/repos/second/.git",
                "/work/deck/abc/repo",
            ),
        ],
        ..GraphSnapshot::empty()
    });

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "workspace"]),
    );

    assert!(
        rendered.contains("/work/deck/abc"),
        "duplicate member names should collapse and fall back to the workspace root:\n{rendered}",
    );
}

#[test]
fn sessions_projection_fork_column_renders_owning_fork_label() {
    // Match the AgentSessionId state_scope to the `agent_session`
    // helper (which uses "global") so the fork's ChildSession
    // candidate resolves to the discovered session node.
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let fork_id = crate::model::ForkId::new("atelier:alpha");

    let snapshot = GraphSnapshot {
        nodes: vec![
            fork_node(Some("alpha"), "atelier", "alpha"),
            agent_session("codex", "alpha", Some("/work")),
        ],
        candidate_links: vec![fork_lineage_link(
            "child",
            fork_id,
            RelationKind::ChildSession,
            session_id,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "fork"]),
    );
    assert!(
        rendered.contains("atelier:alpha"),
        "fork column should label the owning fork:\n{rendered}",
    );
}

#[test]
fn sessions_projection_declared_column_reflects_link_state() {
    // The `agent_session` helper uses state_scope "global"; the
    // declared link's source AgentSessionId must match for the
    // extractor to find it.
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let mux_id = MuxSessionId::new("tmux:editor");

    let mut declared_link = linked_to_mux_link(
        "declared-link",
        session_id,
        mux_id,
        Provenance::LocalDeclared,
        Confidence::High,
    );
    declared_link.state = LinkState::Ignored {
        reason: Some("stale".to_string()),
    };

    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work"))],
        candidate_links: vec![declared_link],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "declared"]),
    );
    assert!(
        rendered.contains("ignored"),
        "declared column should reflect the link's state:\n{rendered}",
    );
}

fn forge_pr(owner: &str, repo: &str, number: u64, state: &str, draft: bool) -> GraphNode {
    GraphNode::ForgePr(ForgePrNode {
        id: ForgePrId::new("github", "github.com", owner, repo, number),
        provider: "github".to_string(),
        host: "github.com".to_string(),
        owner: owner.to_string(),
        repo: repo.to_string(),
        number,
        state: Some(state.to_string()),
        url: None,
        updated_epoch: None,
        is_draft: draft,
    })
}

fn branch_has_pr_link(
    id: &str,
    pr: ForgePrId,
    branch: BranchId,
    provenance: Provenance,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::ForgePr(pr),
        target: LinkEndpoint::Node {
            id: NodeId::Branch(branch),
        },
        relation: RelationKind::BranchHasForgePr,
        provenance,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn prs_projection_default_renders_id_pr_state_branch_attached() {
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    let branch_id = BranchId::new(
        RepoId::new("/workspace/repo/.git"),
        "refs/heads/feature".to_string(),
    );
    let snapshot = GraphSnapshot {
        nodes: vec![forge_pr("octo", "repo", 7, "open", false)],
        candidate_links: vec![branch_has_pr_link(
            "pr-link",
            pr_id,
            branch_id,
            Provenance::StrongDiscovered,
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Pr);
    let header_tokens: Vec<&str> = rendered
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(
        header_tokens,
        vec!["ID", "PR", "STATE", "BRANCH", "ATTACHED"]
    );

    let body_line = rendered.lines().nth(2).expect("body row");
    assert!(body_line.contains("octo/repo#7"), "got:\n{body_line}");
    assert!(body_line.contains("open"), "got:\n{body_line}");
    assert!(body_line.contains("feature"), "got:\n{body_line}");
}

#[test]
fn prs_projection_attached_shows_agent_with_matching_cwd() {
    // PR -> branch (BranchHasForgePr) -> checkout (CheckedOutBranch
    // reversed) -> agent_session with matching cwd. Renders the
    // agent label in the ATTACHED column.
    use crate::model::{CheckoutId, CheckoutNode};

    let repo_id = RepoId::new("/workspace/repo/.git");
    let pr_id = ForgePrId::new("github", "github.com", "octo", "repo", 7);
    let branch_id = BranchId::new(repo_id.clone(), "refs/heads/feature".to_string());
    let worktree_id = CheckoutId::new(repo_id, "/workspace/repo");

    let worktree_node = GraphNode::Checkout(CheckoutNode {
        id: worktree_id.clone(),
        root: "/workspace/repo".to_string(),
        git_dir: None,
        current_branch: None,
    });
    let agent_node = agent_session("codex", "alpha", Some("/workspace/repo/crates/core"));
    let pr_node = forge_pr("octo", "repo", 7, "open", false);

    let mut pr_to_branch = branch_has_pr_link(
        "pr-link",
        pr_id,
        branch_id.clone(),
        Provenance::StrongDiscovered,
    );
    pr_to_branch.confidence = Confidence::High;

    let worktree_to_branch = GraphLink {
        id: "wt-branch".to_string(),
        source: NodeId::Checkout(worktree_id),
        target: LinkEndpoint::Node {
            id: NodeId::Branch(branch_id),
        },
        relation: RelationKind::CheckedOutBranch,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    };

    let snapshot = GraphSnapshot {
        nodes: vec![pr_node, worktree_node, agent_node],
        candidate_links: vec![pr_to_branch, worktree_to_branch],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Pr);
    assert!(
        rendered.contains("codex:alpha"),
        "attached column should list codex:alpha:\n{rendered}",
    );
}

fn fork_node(name: Option<&str>, provider: &str, key: &str) -> GraphNode {
    GraphNode::Fork(ForkNode {
        id: crate::model::ForkId::new(format!("{provider}:{key}")),
        provider: provider.to_string(),
        provider_source_key: key.to_string(),
        name: name.map(str::to_string),
        scope: None,
        capabilities: Vec::new(),
    })
}

fn fork_lineage_link(
    id: &str,
    fork_id: crate::model::ForkId,
    relation: RelationKind,
    target_session: crate::model::AgentSessionId,
) -> GraphLink {
    GraphLink {
        id: id.to_string(),
        source: NodeId::Fork(fork_id),
        target: LinkEndpoint::Node {
            id: NodeId::AgentSession(target_session),
        },
        relation,
        provenance: Provenance::StrongDiscovered,
        confidence: Confidence::High,
        freshness: Freshness::Fresh,
        source_metadata: SourceMetadata::default(),
        state: LinkState::Active,
    }
}

#[test]
fn forks_projection_default_renders_id_fork_provider_parent_children() {
    let fork_node = fork_node(Some("alpha"), "atelier", "alpha");
    let snapshot = GraphSnapshot {
        nodes: vec![fork_node],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Fork);
    let header_tokens: Vec<&str> = rendered
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(
        header_tokens,
        vec!["ID", "FORK", "PROVIDER", "PARENT", "CHILDREN"],
    );
    let body = rendered.lines().nth(2).expect("body row");
    assert!(body.contains("atelier:alpha"), "got:\n{body}");
    assert!(body.contains("atelier"), "got:\n{body}");
}

#[test]
fn forks_projection_fork_label_falls_back_to_source_key_without_name() {
    let snapshot = GraphSnapshot {
        nodes: vec![fork_node(None, "atelier", "raw-source")],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Fork);
    assert!(
        rendered.contains("raw-source"),
        "fork label should fall back to provider_source_key:\n{rendered}",
    );
}

#[test]
fn forks_projection_parent_and_children_count() {
    let fork_id = crate::model::ForkId::new("atelier:alpha");
    let parent_session_id = crate::model::AgentSessionId::new("codex", "/state", "parent");
    let child_one_id = crate::model::AgentSessionId::new("codex", "/state", "child-one");
    let child_two_id = crate::model::AgentSessionId::new("codex", "/state", "child-two");

    let snapshot = GraphSnapshot {
        nodes: vec![
            fork_node(Some("alpha"), "atelier", "alpha"),
            agent_session("codex", "parent", Some("/work")),
            agent_session("codex", "child-one", Some("/work")),
            agent_session("codex", "child-two", Some("/work")),
        ],
        candidate_links: vec![
            fork_lineage_link(
                "parent",
                fork_id.clone(),
                RelationKind::ParentSession,
                parent_session_id,
            ),
            fork_lineage_link(
                "child-1",
                fork_id.clone(),
                RelationKind::ChildSession,
                child_one_id,
            ),
            fork_lineage_link("child-2", fork_id, RelationKind::ChildSession, child_two_id),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Fork);
    let body = rendered
        .lines()
        .nth(2)
        .expect("at least one body row")
        .to_string();
    // Parent cell shows the parent session's short id.
    assert!(body.contains("parent"), "got:\n{body}");
    // Children cell counts the two child_session candidates; it's the
    // last cell on the line, so check the trailing token.
    let last_token = body
        .split_whitespace()
        .next_back()
        .expect("at least one cell");
    assert_eq!(last_token, "2", "expected children count of 2:\n{body}");
}

#[test]
fn forks_projection_capabilities_optional_column() {
    let snapshot = GraphSnapshot {
        nodes: vec![GraphNode::Fork(ForkNode {
            id: crate::model::ForkId::new("atelier:alpha"),
            provider: "atelier".to_string(),
            provider_source_key: "alpha".to_string(),
            name: Some("alpha".to_string()),
            scope: None,
            capabilities: vec!["native_lineage".to_string(), "compaction".to_string()],
        })],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Fork,
        &RenderOptions::wide().with_columns(vec!["id", "fork", "capabilities"]),
    );
    assert!(
        rendered.contains("native_lineage, compaction"),
        "capabilities cell should join the list:\n{rendered}",
    );
}

#[test]
fn prs_projection_empty_snapshot_renders_header_only() {
    let snapshot = GraphSnapshot::empty();
    let rendered = render(&snapshot, Projection::Pr);
    let lines: Vec<&str> = rendered.lines().collect();
    // Header + dash separator only, no body rows.
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("ID"));
}

#[test]
fn truncate_to_width_appends_ellipsis_only_when_string_overflows() {
    assert_eq!(truncate_to_width("alpha", 5), "alpha");
    assert_eq!(truncate_to_width("alpha", 10), "alpha");
    assert_eq!(truncate_to_width("alphabetagamma", 8), "alphabe…");
    assert_eq!(truncate_to_width("", 5), "");
}

#[test]
fn truncate_to_width_degenerate_budgets_emit_ellipsis_or_empty() {
    assert_eq!(truncate_to_width("abc", 0), "");
    assert_eq!(truncate_to_width("abc", 1), "…");
    assert_eq!(truncate_to_width("a", 1), "a");
}

#[test]
fn truncate_to_width_respects_wide_unicode_columns() {
    // CJK characters are two columns wide; budget=4 fits exactly one
    // CJK char plus the ellipsis (1 column reserved → 3-column limit,
    // first wide char fits, second would overflow).
    let cjk = "東京駅前";
    assert_eq!(truncate_to_width(cjk, 4), "東…");
}

#[test]
fn render_with_unbounded_width_matches_render() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            agent_session("codex", "beta", None),
        ],
        ..GraphSnapshot::empty()
    };
    assert_eq!(
        render(&snapshot, Projection::Agent),
        render_with(&snapshot, Projection::Agent, &RenderOptions::wide()),
    );
}

#[test]
fn narrow_width_truncates_cwd_with_ellipsis() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session(
            "codex",
            "alpha",
            Some("/very/long/workspace/path/that/will/not/fit/in/eighty/columns"),
        )],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::columnar_width(80),
    );
    let body_rows: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body_rows.len(), 1);
    assert!(
        body_rows[0].contains('…'),
        "expected ellipsis after truncation in:\n{rendered}",
    );
    for line in rendered.lines() {
        assert!(
            display_width(line) <= 80,
            "line width {} exceeds 80 columns: {line:?}",
            display_width(line),
        );
    }
}

#[test]
fn wide_width_passes_through_untruncated() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session(
            "codex",
            "alpha",
            Some("/some/workspace/path"),
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::columnar_width(500),
    );
    assert!(rendered.contains("/some/workspace/path"));
    assert!(!rendered.contains('…'));
}

#[test]
fn narrow_width_keeps_short_columns_at_natural_width() {
    // The MUX/CONF and PR/CONF indicator columns are 4-6 chars wide and
    // should never get truncated to "…" — the floor protects them.
    let session_id = AgentSessionId::new("codex", "global", "alpha");
    let mux_id = MuxSessionId::new("tmux:editor");
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/aaaaaaaaaaaaaaaaaaa")),
            mux_session("tmux", "editor", Some("/work/aaaaaaaaaaaaaaaaaaa")),
        ],
        candidate_links: vec![linked_to_mux_link(
            "link-1",
            session_id,
            mux_id,
            Provenance::StrongDiscovered,
            Confidence::High,
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::columnar_width(60),
    );
    assert!(
        rendered.contains("SD/H"),
        "indicator should survive truncation:\n{rendered}",
    );
}

#[test]
fn node_short_id_is_deterministic_for_a_given_node_id() {
    // Lock in the FNV-1a-over-Display contract: this string must not
    // change without a deliberate decision, because users paste short
    // ids into `node show` between runs (H-TBL-005).
    let id = NodeId::AgentSession(AgentSessionId::new("codex", "global", "alpha"));
    let short = node_short_id(&id);
    assert_eq!(short.len(), 16);
    assert!(short.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(short, node_short_id(&id), "must be deterministic");
}

#[test]
fn node_short_id_distinguishes_different_node_ids() {
    let alpha = NodeId::AgentSession(AgentSessionId::new("codex", "global", "alpha"));
    let beta = NodeId::AgentSession(AgentSessionId::new("codex", "global", "beta"));
    assert_ne!(node_short_id(&alpha), node_short_id(&beta));
}

#[test]
fn unique_prefix_len_returns_floor_for_one_row() {
    let ids = vec!["0123456789abcdef".to_string()];
    assert_eq!(unique_prefix_len(&ids), SHORT_ID_FLOOR);
}

#[test]
fn unique_prefix_len_returns_floor_when_prefixes_already_unique() {
    let ids = vec![
        "aaaaaa1111".to_string(),
        "bbbbbb1111".to_string(),
        "cccccc1111".to_string(),
    ];
    assert_eq!(unique_prefix_len(&ids), SHORT_ID_FLOOR);
}

#[test]
fn unique_prefix_len_grows_past_floor_to_break_collisions() {
    // First six chars collide; the seventh resolves them.
    let ids = vec!["abcdefXone".to_string(), "abcdefYone".to_string()];
    assert_eq!(unique_prefix_len(&ids), 7);
}

#[test]
fn agent_projection_id_column_uses_session_key() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            agent_session("codex", "beta", Some("/work/b")),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render(&snapshot, Projection::Agent);
    let header = rendered.lines().next().expect("header");
    assert!(
        header.starts_with("ID"),
        "agent projection header should start with ID:\n{header}",
    );

    let body: Vec<&str> = rendered.lines().skip(2).collect();
    assert_eq!(body.len(), 2);
    let leading: Vec<String> = body
        .iter()
        .map(|row| row.chars().take_while(|c| !c.is_whitespace()).collect())
        .collect();
    assert_eq!(leading, vec!["alpha", "beta"]);
}

#[test]
fn agent_projection_external_ids_are_stable_across_renders() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work/a"))],
        ..GraphSnapshot::empty()
    };
    let first = render(&snapshot, Projection::Agent);
    let second = render(&snapshot, Projection::Agent);
    assert_eq!(first, second);
}

#[test]
fn mux_projection_prepends_id_column() {
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session("tmux", "editor", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Mux);
    let header = rendered.lines().next().expect("header");
    assert!(header.starts_with("ID"), "header was: {header:?}");
}

#[test]
fn union_projection_header_renames_existing_id_to_label() {
    let snapshot = GraphSnapshot::empty();
    let rendered = render(&snapshot, Projection::Union);
    let header = rendered.lines().next().expect("header");
    assert!(
        header.starts_with("ID  KIND  LABEL"),
        "union header should be ID KIND LABEL …; got: {header:?}",
    );
}

#[test]
fn card_layout_emits_one_block_per_body_row() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session("codex", "alpha", Some("/work/a")),
            agent_session("codex", "beta", Some("/work/b")),
        ],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());

    // Two body rows produce two card blocks separated by a blank line.
    // Each block has one line per header column (8 in the agent
    // projection: ID, AGENT, CWD, MUX, MUX/CONF, PR, PR/CONF, LINEAGE).
    let blocks: Vec<&str> = rendered.split("\n\n").collect();
    assert_eq!(blocks.len(), 2);
    for block in &blocks {
        let non_empty_lines = block.lines().filter(|line| !line.is_empty()).count();
        assert_eq!(
            non_empty_lines, 8,
            "card block should have 8 lines: {block:?}"
        );
    }
}

#[test]
fn card_layout_uses_keys_with_aligned_colons() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "alpha", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());

    // Keys (ID, AGENT, CWD, MUX, MUX/CONF, PR, PR/CONF, LINEAGE) have
    // the longest as MUX/CONF and PR/CONF at 8 chars. Every line's
    // value should start at the same column.
    let lines: Vec<&str> = rendered.lines().filter(|l| !l.is_empty()).collect();
    let mut value_starts: Vec<usize> = Vec::new();
    for line in &lines {
        let colon = line.find(':').expect("each card line has a colon");
        // After the colon, padding goes up to the longest key width,
        // then a single space, then the value.
        let value_col = line[colon + 1..]
            .chars()
            .position(|c| !c.is_whitespace())
            .map_or(line.len(), |i| colon + 1 + i);
        value_starts.push(value_col);
    }
    let first = value_starts[0];
    for start in &value_starts {
        assert_eq!(
            *start, first,
            "value columns should align across keys: starts={value_starts:?}",
        );
    }
}

#[test]
fn card_layout_empty_snapshot_renders_empty_string() {
    let snapshot = GraphSnapshot::empty();
    let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card());
    assert!(rendered.is_empty(), "got: {rendered:?}");
}

#[test]
fn card_layout_truncates_values_when_width_is_set() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session(
            "codex",
            "alpha",
            Some("/very/long/workspace/path/that/will/not/fit"),
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(&snapshot, Projection::Agent, &RenderOptions::card_width(30));
    assert!(
        rendered.contains('…'),
        "long cwd should have been truncated in:\n{rendered}",
    );
    for line in rendered.lines() {
        assert!(display_width(line) <= 30, "line exceeded 30 cols: {line:?}",);
    }
}

#[test]
fn agent_label_uses_session_key_not_title() {
    // Regression for H-TBL-015: the AGENT cell used to fall back
    // to `harness:title` when the adapter populated `title`. That
    // surfaced opencode's long chat topics in the leading cell.
    // The label now always renders `harness:session_key` (with
    // the UUID truncator) and title belongs to the opt-in
    // `title` column.
    let node = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "global", "ses_abc123"),
            "opencode".to_string(),
        )
        .with_cwd("/work".to_string())
        .with_title("a very long conversation topic".to_string()),
    );
    let snapshot = GraphSnapshot {
        nodes: vec![node],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Agent);
    let body = rendered.lines().nth(2).expect("body row");
    assert!(
        body.contains("opencode:ses_abc123"),
        "AGENT cell should show session_key:\n{body}",
    );
    assert!(
        !body.contains("a very long conversation topic"),
        "AGENT cell must not fall back to title:\n{body}",
    );
}

#[test]
fn agent_label_truncates_uuid_session_keys_to_short_form() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session(
            "claude-code",
            "0b34e59c-14d0-4d04-be79-4dc1d4c120c2",
            Some("/work"),
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Agent);
    let body = rendered.lines().nth(2).expect("body row");
    // UUID collapses to the last-8-chars form.
    assert!(
        body.contains("claude-code:…d4c120c2"),
        "AGENT cell should truncate UUID session_key:\n{body}",
    );
}

#[test]
fn agent_label_preserves_short_session_keys_verbatim() {
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session("codex", "session-alpha", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Agent);
    let body = rendered.lines().nth(2).expect("body row");
    assert!(
        body.contains("codex:session-alpha"),
        "AGENT cell should render short session_key verbatim:\n{body}",
    );
}

#[test]
fn sessions_title_column_renders_set_value_or_dash() {
    let with_title = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "global", "ses_a"),
            "opencode".to_string(),
        )
        .with_cwd("/work".to_string())
        .with_title("clipboard sync over SSH".to_string()),
    );
    let without = agent_session("codex", "no-title", Some("/work"));
    let snapshot = GraphSnapshot {
        nodes: vec![with_title, without],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "title"]),
    );
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let titled = body
        .iter()
        .find(|line| line.contains("opencode:ses_a"))
        .expect("titled row");
    assert!(
        titled.contains("clipboard sync over SSH"),
        "title row missing content:\n{titled}",
    );
    let untitled = body
        .iter()
        .find(|line| line.contains("codex:no-title"))
        .expect("untitled row");
    assert!(
        untitled.trim_end().ends_with('—'),
        "untitled row should show — in TITLE column:\n{untitled}",
    );
}

#[test]
fn sessions_title_column_prefers_alias_over_harness_title() {
    let session = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "global", "ses_a"),
            "opencode".to_string(),
        )
        .with_cwd("/work".to_string())
        .with_title("harness title that should be hidden".to_string()),
    );
    let session_id = session.id();
    let mut snapshot = GraphSnapshot {
        nodes: vec![session],
        ..GraphSnapshot::empty()
    };
    snapshot
        .aliases
        .insert(session_id, "ingest-refactor".to_string());

    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "title"]),
    );
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let row = body
        .iter()
        .find(|line| line.contains("opencode:ses_a"))
        .expect("session row");
    assert!(
        row.contains("ingest-refactor"),
        "alias should replace harness title in title column:\n{row}",
    );
    assert!(
        !row.contains("harness title that should be hidden"),
        "harness title should be hidden when alias is set:\n{row}",
    );
}

#[test]
fn union_title_column_renders_only_for_agent_rows() {
    let titled_agent = GraphNode::AgentSession(
        AgentSessionNode::new(
            AgentSessionId::new("opencode", "global", "ses_a"),
            "opencode".to_string(),
        )
        .with_cwd("/work".to_string())
        .with_title("agent title".to_string()),
    );
    let mux = mux_session("tmux", "editor", Some("/work"));
    let snapshot = GraphSnapshot {
        nodes: vec![titled_agent, mux],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Union,
        &RenderOptions::wide().with_columns(vec!["id", "kind", "label", "title"]),
    );
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let agent = body
        .iter()
        .find(|line| line.contains(" agent "))
        .expect("agent row");
    assert!(
        agent.contains("agent title"),
        "agent row should carry title:\n{agent}",
    );
    let mux_row = body
        .iter()
        .find(|line| line.contains(" mux "))
        .expect("mux row");
    assert!(
        mux_row.trim_end().ends_with('—'),
        "mux row should render — for title:\n{mux_row}",
    );
}

#[test]
fn sessions_preview_column_renders_set_value_or_dash() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session_with_preview("codex", "alpha", Some("/work"), "wired up the column"),
            agent_session("codex", "beta", Some("/work")),
        ],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Agent,
        &RenderOptions::wide().with_columns(vec!["id", "agent", "preview"]),
    );
    let header: Vec<&str> = rendered
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(header, vec!["ID", "AGENT", "PREVIEW"]);

    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let alpha = body
        .iter()
        .find(|line| line.contains("codex:alpha"))
        .expect("alpha row");
    assert!(
        alpha.contains("wired up the column"),
        "alpha row preview missing:\n{alpha}",
    );
    let beta = body
        .iter()
        .find(|line| line.contains("codex:beta"))
        .expect("beta row");
    assert!(
        beta.trim_end().ends_with('—'),
        "beta row should show — for missing preview:\n{beta}",
    );
}

#[test]
fn mux_preview_column_shows_first_attached_agent_preview() {
    let session_alpha = AgentSessionId::new("codex", "global", "alpha");
    let session_beta = AgentSessionId::new("codex", "global", "beta");
    let editor = MuxSessionId::new("tmux:editor");

    let snapshot = GraphSnapshot {
        nodes: vec![
            mux_session("tmux", "editor", Some("/work")),
            agent_session_with_preview("codex", "alpha", Some("/work"), "alpha preview"),
            agent_session_with_preview("codex", "beta", Some("/work"), "beta preview"),
        ],
        candidate_links: vec![
            linked_to_mux_link(
                "link-a",
                session_alpha,
                editor.clone(),
                Provenance::StrongDiscovered,
                Confidence::High,
            ),
            linked_to_mux_link(
                "link-b",
                session_beta,
                editor,
                Provenance::StrongDiscovered,
                Confidence::High,
            ),
        ],
        ..GraphSnapshot::empty()
    };

    let rendered = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_columns(vec!["id", "mux", "preview"]),
    );
    let body = rendered.lines().nth(2).expect("body row");
    // `alpha preview` should win — it's the first attached agent
    // in `attached_to_mux` (BTreeMap-ordered by source NodeId).
    assert!(
        body.contains("alpha preview"),
        "mux row should show first attached agent's preview:\n{body}",
    );
    assert!(
        !body.contains("beta preview"),
        "mux row should not include the second attached agent's preview:\n{body}",
    );
}

#[test]
fn mux_preview_column_renders_dash_when_no_agents_attached_have_preview() {
    // A mux with no attached agents at all.
    let snapshot = GraphSnapshot {
        nodes: vec![mux_session("tmux", "lonely", Some("/work"))],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Mux,
        &RenderOptions::wide().with_columns(vec!["id", "mux", "preview"]),
    );
    let body = rendered.lines().nth(2).expect("body row");
    assert!(
        body.trim_end().ends_with('—'),
        "lonely mux row should render — for preview:\n{body}",
    );
}

#[test]
fn union_preview_column_only_populates_for_agent_rows() {
    let snapshot = GraphSnapshot {
        nodes: vec![
            agent_session_with_preview("codex", "alpha", Some("/work"), "agent preview"),
            mux_session("tmux", "editor", Some("/work")),
        ],
        ..GraphSnapshot::empty()
    };
    let rendered = render_with(
        &snapshot,
        Projection::Union,
        &RenderOptions::wide().with_columns(vec!["id", "kind", "label", "preview"]),
    );
    let body: Vec<&str> = rendered.lines().skip(2).collect();
    let agent_row = body
        .iter()
        .find(|line| line.contains("agent "))
        .expect("agent row");
    assert!(
        agent_row.contains("agent preview"),
        "union agent row should carry the preview:\n{agent_row}",
    );
    let mux_row = body
        .iter()
        .find(|line| line.contains("mux "))
        .expect("mux row");
    assert!(
        mux_row.trim_end().ends_with('—'),
        "union mux row should render — for preview:\n{mux_row}",
    );
}

#[test]
fn preview_column_is_opt_in_not_default() {
    // Default snapshots stay byte-stable when the preview field
    // is populated — the column has to be requested explicitly.
    let snapshot = GraphSnapshot {
        nodes: vec![agent_session_with_preview(
            "codex",
            "alpha",
            Some("/work"),
            "should not appear in default render",
        )],
        ..GraphSnapshot::empty()
    };
    let rendered = render(&snapshot, Projection::Agent);
    assert!(
        !rendered.contains("should not appear"),
        "default sessions render must not include preview content:\n{rendered}",
    );
    assert!(
        !rendered.contains("PREVIEW"),
        "default sessions render must not include the PREVIEW header:\n{rendered}",
    );
}

#[test]
fn fit_to_width_settles_at_floors_when_target_is_impossible() {
    // Seven columns at floor 4 + 6 gaps × 2 = 40 minimum. Asking for 10
    // forces every column down to its floor and stops; the resulting
    // line may exceed the target but doesn't degenerate.
    let header = vec![
        "AGENT".to_string(),
        "CWD".to_string(),
        "MUX".to_string(),
        "MUX/CONF".to_string(),
        "PR".to_string(),
        "PR/CONF".to_string(),
        "LINEAGE".to_string(),
    ];
    let naturals = vec![20, 60, 20, 5, 30, 5, 12];
    let budgets = fit_to_width(&naturals, &header, 10);
    for (idx, &budget) in budgets.iter().enumerate() {
        let header_width = display_width(&header[idx]);
        let expected_floor = naturals[idx].min(header_width.max(MIN_COLUMN_BUDGET));
        assert_eq!(
            budget, expected_floor,
            "column {idx} should be at floor; got {budget} expected {expected_floor}",
        );
    }
}

// ---- ADR 0031 / F8-010: filter parity ----

fn three_session_snapshot() -> GraphSnapshot {
    let mut snapshot = GraphSnapshot::empty();
    // claude session, recent
    let mut claude = match agent_session("claude-code", "c1", Some("/home/op/src/proj")) {
        GraphNode::AgentSession(s) => s,
        _ => unreachable!(),
    };
    claude.last_active_epoch = Some(1_000_000);
    snapshot.nodes.push(GraphNode::AgentSession(claude));

    // codex session, 8 days old
    let mut codex = match agent_session("codex", "x1", Some("/home/op/src/proj")) {
        GraphNode::AgentSession(s) => s,
        _ => unreachable!(),
    };
    codex.last_active_epoch = Some(1_000_000 - 8 * 24 * 60 * 60);
    snapshot.nodes.push(GraphNode::AgentSession(codex));

    // opencode session, recent, attached to mux
    let mut opencode = match agent_session("opencode", "o1", Some("/home/op/src/proj")) {
        GraphNode::AgentSession(s) => s,
        _ => unreachable!(),
    };
    opencode.last_active_epoch = Some(1_000_000 - 600);
    snapshot.nodes.push(GraphNode::AgentSession(opencode));
    snapshot.nodes.push(mux_session("tmux", "editor", None));
    snapshot.candidate_links.push(linked_to_mux_link(
        "lnk-1",
        AgentSessionId::new("opencode", "global", "o1"),
        MuxSessionId::new("tmux:editor"),
        Provenance::Discovered,
        Confidence::High,
    ));

    resolve_snapshot(snapshot)
}

fn body_row_count(rendered: &str) -> usize {
    // Subtract 2 for the header row plus the dashed-separator
    // row beneath it. Both render even when the body is empty.
    rendered
        .lines()
        .filter(|line| !line.is_empty())
        .count()
        .saturating_sub(2)
}

#[test]
fn filter_harness_narrows_agent_table() {
    let snapshot = three_session_snapshot();
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Agent, &options);
    assert_eq!(body_row_count(&table), 1, "table:\n{table}");
    assert!(table.contains("claude"));
    assert!(!table.contains("codex"));
    assert!(!table.contains("opencode"));
}

#[test]
fn filter_max_age_drops_stale_agent_rows() {
    let snapshot = three_session_snapshot();
    let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            max_age: Some(week),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Agent, &options);
    // codex (8d old) drops; claude + opencode remain.
    assert_eq!(body_row_count(&table), 2, "table:\n{table}");
    assert!(table.contains("claude"));
    assert!(table.contains("opencode"));
    assert!(!table.contains("codex"));
}

#[test]
fn filter_mux_state_unmuxed_drops_attached_rows() {
    let snapshot = three_session_snapshot();
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            mux_state: Some(crate::filter::MuxStateFilter::from_values([
                crate::filter::MuxStateKey::Unmuxed,
            ])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Agent, &options);
    // opencode has a mux link; it should drop.
    assert_eq!(body_row_count(&table), 2, "table:\n{table}");
    assert!(table.contains("claude"));
    assert!(table.contains("codex"));
    assert!(!table.contains("opencode"));
}

#[test]
fn filter_empty_table_keeps_header_row() {
    let snapshot = three_session_snapshot();
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["aider"])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Agent, &options);
    assert_eq!(body_row_count(&table), 0, "table:\n{table}");
}

#[test]
fn filter_parity_with_tui_sessions_row_tree() {
    use crate::tui::SessionsGrouping;
    use crate::tui::rows::sessions::{SessionsBuildInputs, build_sessions_tree};
    let snapshot = three_session_snapshot();
    let filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values([
            "claude-code",
            "codex",
        ])),
        max_age: Some(std::time::Duration::from_secs(7 * 24 * 60 * 60)),
        mux_state: Some(crate::filter::MuxStateFilter::from_values([
            crate::filter::MuxStateKey::Unmuxed,
        ])),
        ..crate::filter::RowFilter::default()
    };

    let table_options = RenderOptions::wide()
        .with_filter(filter.clone())
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Agent, &table_options);

    let tree = build_sessions_tree(SessionsBuildInputs {
        snapshot: &snapshot,
        grouping: SessionsGrouping::Graph,
        home: None,
        now: Some(1_000_000),
        cwd: None,
        filter,
    });
    let tree_sessions = tree
        .rows
        .iter()
        .filter(|row| matches!(row.kind, crate::tui::rows::RowKind::AgentSession(_)))
        .count();

    assert_eq!(
        body_row_count(&table),
        tree_sessions,
        "table rows ({}) should equal TUI sessions ({}) for the same filter\ntable:\n{table}",
        body_row_count(&table),
        tree_sessions
    );
}

#[test]
fn filter_harness_narrows_mux_table_via_attached_agents() {
    // The fixture has one mux (`editor`) with `opencode` attached.
    // Filtering by `opencode` keeps the row; filtering by
    // `claude-code` drops it because the mux has at least one
    // attached agent but none survives the filter.
    let snapshot = three_session_snapshot();
    let opts_keep = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["opencode"])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table_keep = render_with(&snapshot, Projection::Mux, &opts_keep);
    assert_eq!(body_row_count(&table_keep), 1, "table:\n{table_keep}");
    assert!(table_keep.contains("opencode"));

    let opts_drop = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table_drop = render_with(&snapshot, Projection::Mux, &opts_drop);
    assert_eq!(body_row_count(&table_drop), 0, "table:\n{table_drop}");
}

#[test]
fn filter_mux_state_unmuxed_drops_attached_muxes() {
    // The editor mux has one attached agent. `mux_state=unmuxed`
    // alone is the only filter shape where the mux row is kept
    // for *empty* muxes; here the mux is non-empty so it drops.
    let snapshot = three_session_snapshot();
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            mux_state: Some(crate::filter::MuxStateFilter::from_values([
                crate::filter::MuxStateKey::Unmuxed,
            ])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Mux, &options);
    assert_eq!(body_row_count(&table), 0, "table:\n{table}");
}

#[test]
fn filter_parity_with_tui_mux_row_tree() {
    use crate::tui::MuxGrouping;
    use crate::tui::rows::mux::{MuxBuildInputs, build_mux_tree};

    let snapshot = three_session_snapshot();
    let filter = crate::filter::RowFilter {
        harness: Some(crate::filter::HarnessFilter::from_values(["opencode"])),
        max_age: Some(std::time::Duration::from_secs(24 * 60 * 60)),
        ..crate::filter::RowFilter::default()
    };
    let table_options = RenderOptions::wide()
        .with_filter(filter.clone())
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Mux, &table_options);

    let tree = build_mux_tree(MuxBuildInputs {
        snapshot: &snapshot,
        home: None,
        now: Some(1_000_000),
        filter,
        grouping: MuxGrouping::Session,
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
    });
    let tree_mux_rows = tree
        .rows
        .iter()
        .filter(|row| matches!(row.kind, crate::tui::rows::RowKind::MuxSession(_)))
        .count();

    assert_eq!(
        body_row_count(&table),
        tree_mux_rows,
        "table mux rows ({}) should equal TUI mux rows ({}) for the same filter\ntable:\n{table}",
        body_row_count(&table),
        tree_mux_rows,
    );
}

#[test]
fn filter_union_drops_mux_rows_when_narrowing_active() {
    // The union projection mirrors the TUI union view: any
    // narrowing predicate hides every mux row since none of the
    // v1 dimensions describe a bare mux.
    let snapshot = three_session_snapshot();
    let options = RenderOptions::wide()
        .with_filter(crate::filter::RowFilter {
            harness: Some(crate::filter::HarnessFilter::from_values(["claude-code"])),
            ..crate::filter::RowFilter::default()
        })
        .with_now_epoch(Some(1_000_000));
    let table = render_with(&snapshot, Projection::Union, &options);
    // One agent matches; the editor mux row drops.
    assert_eq!(body_row_count(&table), 1, "table:\n{table}");
    assert!(table.contains("claude"));
    assert!(!table.contains("editor"));
}
