use super::*;
use crate::model::{
    AgentSessionId, BranchId, CheckoutId, ForgePrId, ForkId, MuxSessionId, NodeId, RepoId,
    RuntimeProcessId, WorkspaceId,
};

fn repo_id() -> RepoId {
    RepoId::new("/tmp/.git".to_string())
}

#[test]
fn snake_case_matches_the_kind_vocabulary() {
    // These strings appear in unresolved endpoints' `node_type`,
    // `node show` output, and the CLI, so they must not drift.
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
        (NodeKind::RuntimeProcess, "◈"),
        (NodeKind::Branch, "⌥"),
        (NodeKind::Fork, "⋎"),
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
        theme.node_workspace(),
    );
    assert_eq!(
        node_kind_style(NodeKind::Repo, &theme).color,
        theme.node_repo(),
    );
    assert_eq!(
        node_kind_style(NodeKind::AgentSession, &theme).color,
        theme.node_agent_session(),
    );
    // ForgePr owns no color: without a PR state it takes the open hue,
    // and callers that know the state pick from the `pr_*` roles.
    assert_eq!(
        node_kind_style(NodeKind::ForgePr, &theme).color,
        theme.pr_open(),
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

#[test]
fn state_glyphs_are_one_cell_wide_and_all_different() {
    let mut seen = std::collections::BTreeSet::new();
    for glyph in state::ALL {
        assert_eq!(
            UnicodeWidthStr::width(glyph),
            1,
            "`{glyph}` must be one cell wide"
        );
        assert!(seen.insert(glyph), "`{glyph}` is used for two states");
    }
}

#[test]
fn state_glyphs_do_not_reuse_a_node_kind_glyph_except_the_pin_marker() {
    for kind in NodeKind::ALL {
        let glyph = kind.default_glyph();
        if glyph == state::PIN_RUNNING {
            assert_eq!(kind, NodeKind::Pin);
            continue;
        }
        assert!(
            !state::ALL.contains(&glyph),
            "{kind:?}'s glyph `{glyph}` is also a state glyph"
        );
    }
}

fn vetted(glyph: &str) -> bool {
    VETTED.iter().any(|(g, _)| *g == glyph)
}

#[test]
fn all_glyphs_are_vetted() {
    for glyph in all_glyphs() {
        assert!(
            vetted(glyph),
            "`{glyph}` is drawn by the left panel but is not in icons::VETTED: measure it with \
             scripts/glyph-widths.py and record the result"
        );
    }
}

#[test]
fn vetted_glyphs_have_no_width_or_emoji_risk() {
    let mut seen = std::collections::HashSet::new();
    for (glyph, note) in VETTED {
        assert!(seen.insert(*glyph), "`{glyph}` is listed twice in VETTED");
        assert!(
            !note.is_empty(),
            "`{glyph}` needs a note saying how it was vetted"
        );
        if let Some(why) = glyph_risk(glyph) {
            panic!("`{glyph}` is in VETTED but {why}");
        }
    }
}

#[test]
fn risky_glyphs_are_rejected() {
    for glyph in [
        "◀",
        "⚠",
        "⚙",
        "✔",
        "😀",
        "東",
        "ab",
        "\u{200D}",
        "▶\u{FE0F}",
    ] {
        assert!(glyph_risk(glyph).is_some(), "`{glyph}` should be rejected");
    }
    for glyph in ["▼", "▶", "◉", "⊸", "?"] {
        assert_eq!(glyph_risk(glyph), None, "`{glyph}` should be allowed");
    }
}

/// Every non-ASCII character the TUI sources draw, on a line that isn't a
/// comment, must have been vetted. Test modules and snapshots are skipped.
#[test]
fn every_glyph_the_sources_draw_is_vetted() {
    fn walk(dir: &std::path::Path, found: &mut Vec<(String, char)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if name == "tests" || name == "snapshots" || name.ends_with("_tests") {
                    continue;
                }
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") && !name.ends_with("_tests.rs") {
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    for ch in line.chars().filter(|c| !c.is_ascii()) {
                        found.push((path.display().to_string(), ch));
                    }
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui"),
        &mut found,
    );
    assert!(
        !found.is_empty(),
        "the scan found no glyphs, so it is broken"
    );
    for (file, ch) in found {
        assert!(
            vetted(&ch.to_string()),
            "`{ch}` (U+{:04X}) is drawn in {file} but is not in icons::VETTED: run \
             scripts/glyph-widths.py --source and record it",
            ch as u32
        );
    }
}
