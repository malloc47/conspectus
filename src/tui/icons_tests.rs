// Extracted from icons.rs H-HYG-011 rolling wave via #[path = "icons_tests.rs"] mod tests;
use super::*;
use crate::model::{
    AgentSessionId, BranchId, CheckoutId, ForgePrId, ForkId, MuxSessionId, NodeId, RepoId,
    RuntimeProcessId, WorkspaceId,
};

fn repo_id() -> RepoId {
    RepoId::new("/tmp/.git".to_string())
}

#[test]
fn snake_case_matches_existing_kind_label_vocabulary() {
    // These strings are returned by the duplicated `kind_label`
    // helpers in `src/tui/detail.rs:358` and
    // `src/tui/explorer.rs:831`. Subsequent stories swap those
    // callers onto `NodeKind::snake_case`; the test pins the
    // vocabulary so the swap is a no-op on the wire.
    assert_eq!(NodeKind::Workspace.snake_case(), "workspace");
    assert_eq!(NodeKind::Repo.snake_case(), "repo");
    assert_eq!(NodeKind::Checkout.snake_case(), "checkout");
    assert_eq!(NodeKind::AgentSession.snake_case(), "agent_session");
    assert_eq!(NodeKind::MuxSession.snake_case(), "mux_session");
    assert_eq!(NodeKind::RuntimeProcess.snake_case(), "runtime_process");
    assert_eq!(NodeKind::Branch.snake_case(), "branch");
    assert_eq!(NodeKind::Fork.snake_case(), "fork");
    assert_eq!(NodeKind::ForgePr.snake_case(), "forge_pr");
}

#[test]
fn from_snake_case_round_trips_every_variant() {
    for kind in NodeKind::ALL {
        let tag = kind.snake_case();
        assert_eq!(NodeKind::from_snake_case(tag), Some(kind));
    }
    assert_eq!(NodeKind::from_snake_case("not_a_kind"), None);
    assert_eq!(NodeKind::from_snake_case(""), None);
}

#[test]
fn default_glyphs_match_adr_0073_slate() {
    // Catalog: pin every glyph against the ADR so a slate change
    // requires an ADR amendment.
    let expected: [(NodeKind, &str); 9] = [
        (NodeKind::Workspace, "▦"),
        (NodeKind::Repo, "◆"),
        (NodeKind::Checkout, "◇"),
        (NodeKind::AgentSession, "●"),
        (NodeKind::MuxSession, "▣"),
        (NodeKind::RuntimeProcess, "⚙"),
        (NodeKind::Branch, "⎇"),
        (NodeKind::Fork, "⑂"),
        (NodeKind::ForgePr, "⇄"),
    ];
    for (kind, glyph) in expected {
        assert_eq!(kind.default_glyph(), glyph, "slate drift for {kind:?}");
    }
}

#[test]
fn default_glyphs_are_all_one_cell_wide() {
    // The row-layout law in ADR 0073 §3 only holds if every
    // glyph is 1 cell. A future variant whose glyph is 2-cell
    // would silently break column budgets; catch it at build
    // time via this assertion.
    for kind in NodeKind::ALL {
        let width = UnicodeWidthStr::width(kind.default_glyph());
        assert_eq!(
            width, 1,
            "default glyph for {kind:?} is {width} cells wide, expected 1"
        );
    }
}

#[test]
fn theme_keys_are_unique_and_match_known_keys_pattern() {
    let mut seen = std::collections::HashSet::new();
    for kind in NodeKind::ALL {
        let key = kind.theme_key();
        assert!(
            key.starts_with("node_"),
            "theme key `{key}` must start with `node_`"
        );
        assert!(seen.insert(key), "duplicate theme key `{key}`");
    }
}

#[test]
fn node_kind_style_uses_theme_color_for_each_kind() {
    let theme = Theme::default();
    // Spot-check the color resolution against the slate. The
    // exhaustive default-color coverage lives in
    // `tui::theme::tests` so the assertion here is the *wiring*
    // — `node_kind_style` reads from the right field per kind.
    assert_eq!(
        node_kind_style(NodeKind::Workspace, &theme).color,
        theme.node_workspace,
    );
    assert_eq!(
        node_kind_style(NodeKind::Repo, &theme).color,
        theme.node_repo,
    );
    assert_eq!(
        node_kind_style(NodeKind::AgentSession, &theme).color,
        theme.node_agent_session,
    );
    // ForgePr is the documented sentinel — callers pick from
    // theme.pr_* themselves.
    assert_eq!(
        node_kind_style(NodeKind::ForgePr, &theme).color,
        Color::Reset,
    );
}

#[test]
fn node_kind_style_returns_default_glyph_without_overrides() {
    let theme = Theme::default();
    for kind in NodeKind::ALL {
        let style = node_kind_style(kind, &theme);
        assert_eq!(style.glyph, kind.default_glyph());
        assert_eq!(style.width, 1);
    }
}

#[test]
fn icon_overrides_are_honored_when_present() {
    let mut theme = Theme::default();
    theme.icons.insert(NodeKind::Repo, "R".to_string());
    let style = node_kind_style(NodeKind::Repo, &theme);
    assert_eq!(style.glyph, "R");
    assert_eq!(style.width, 1);
    // Kinds without overrides still resolve to the default.
    assert_eq!(
        node_kind_style(NodeKind::Workspace, &theme).glyph,
        NodeKind::Workspace.default_glyph(),
    );
}

#[test]
fn parse_icon_override_accepts_one_cell_glyph() {
    let (kind, glyph) = parse_icon_override("node_fork", "Y").unwrap();
    assert_eq!(kind, NodeKind::Fork);
    assert_eq!(glyph, "Y");
}

#[test]
fn parse_icon_override_rejects_unknown_key() {
    let err = parse_icon_override("workspace_glyph", "▦").unwrap_err();
    assert!(
        err.contains("unknown `[tui.theme.icons]` key"),
        "got: {err}"
    );
}

#[test]
fn parse_icon_override_rejects_empty_glyph() {
    let err = parse_icon_override("node_repo", "").unwrap_err();
    assert!(err.contains("must not be empty"), "got: {err}");
}

#[test]
fn parse_icon_override_rejects_wide_glyph() {
    // CJK ideographs are unambiguously East Asian Wide (2
    // cells); the validator should refuse so the row layout law
    // holds. Same goes for any emoji-variation-selector-16
    // sequence, but the bare CJK case is the most stable proxy.
    let err = parse_icon_override("node_workspace", "中").unwrap_err();
    assert!(err.contains("display width"), "got: {err}");
}

#[test]
fn parse_icon_override_rejects_multi_codepoint_glyph() {
    // Two ASCII characters = width 2 → reject.
    let err = parse_icon_override("node_workspace", "WS").unwrap_err();
    assert!(err.contains("expected 1"), "got: {err}");
}

#[test]
fn from_node_id_covers_every_variant() {
    // Compiler exhaustiveness already guarantees this stays in
    // sync with `NodeId`; the explicit cases here turn a future
    // miss into a behavioral failure and document the mapping
    // for readers. `From<&GraphNode>` is the same shape and is
    // tested implicitly by the renderer's exhaustive matches.
    let pairs: [(NodeId, NodeKind); 9] = [
        (
            NodeId::Workspace(WorkspaceId::new("/tmp")),
            NodeKind::Workspace,
        ),
        (NodeId::Repo(repo_id()), NodeKind::Repo),
        (
            NodeId::Checkout(CheckoutId::new(repo_id(), "/tmp/wt")),
            NodeKind::Checkout,
        ),
        (
            NodeId::AgentSession(AgentSessionId::new("claude", "/state", "abc")),
            NodeKind::AgentSession,
        ),
        (
            NodeId::MuxSession(MuxSessionId::new("x")),
            NodeKind::MuxSession,
        ),
        (
            NodeId::RuntimeProcess(RuntimeProcessId::new("host:1")),
            NodeKind::RuntimeProcess,
        ),
        (
            NodeId::Branch(BranchId::new(repo_id(), "main")),
            NodeKind::Branch,
        ),
        (NodeId::Fork(ForkId::new("alpha")), NodeKind::Fork),
        (
            NodeId::ForgePr(ForgePrId::new("github", "github.com", "owner", "repo", 1)),
            NodeKind::ForgePr,
        ),
    ];
    for (id, expected) in pairs {
        assert_eq!(NodeKind::from(&id), expected, "mismatch for {id}");
    }
}
