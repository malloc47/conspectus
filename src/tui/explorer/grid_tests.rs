//! The explorer's rows as grid rows: fields cut where they say with the
//! inspect key's hint, a long label kept apart from its value, the edge
//! tones in their theme roles, the Other header's counts, an unresolved
//! stub as one inert row, and the intents Enter and `o` read.

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;
use tuikit::property_grid::{Intent, Tone};
use tuikit::text;
use tuikit::theme::TokenValue;

use super::*;
use crate::model::{Confidence, Provenance, RelationKind};
use crate::tui::explorer::{
    Direction, LinkStateLabel, RelationshipCounts, UnresolvedEvidence, UnresolvedRow,
};
use crate::tui::keymap::{Action, INSPECT_KEY, translate};

fn text_of(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// `row` drawn alone at `width`, as the pane draws it, with no cursor.
fn drawn(row: impl Into<Row>, width: usize, theme: &Theme) -> String {
    let lines = grid(theme).row(row).lines(width, None, false, theme).lines;
    text_of(&lines[0])
}

fn field(label: &'static str, value: &str) -> CoreField {
    CoreField {
        label,
        value: value.to_string(),
        placeholder: false,
        annotation: None,
        cut_at: None,
        kind_chip: None,
        preview: None,
    }
}

#[test]
fn a_long_field_is_cut_where_it_says_and_its_hint_names_the_key_that_opens_it() {
    let theme = Theme::default();
    let whole = "a message much longer than the forty-eight cells the pane shows of it";
    let long = CoreField {
        cut_at: Some(crate::tui::explorer::LONG_VALUE_CELLS),
        ..field("last_message", whole)
    };
    let line = drawn(field_row(&long, &theme), 120, &theme);
    assert!(
        line.ends_with(&format!("…  (truncated · {INSPECT_KEY})")),
        "{line}"
    );
    assert_eq!(
        translate(Event::Key(KeyEvent::new(
            KeyCode::Char(INSPECT_KEY),
            KeyModifiers::NONE
        ))),
        Some(Action::OpenValueModal),
        "the key the hint names opens the value"
    );
    let row = field_row(&long, &theme);
    assert_eq!(
        row.inspect(),
        Some(Intent::Inspect {
            label: "last_message".into(),
            text: whole.into()
        })
    );
    assert_eq!(
        row.activate(),
        Some(Intent::Copy {
            label: "last_message".into(),
            text: whole.into()
        }),
        "Enter copies the whole value"
    );
    assert_eq!(field_row(&field("id", "abc"), &theme).inspect(), None);
}

#[test]
fn a_label_wider_than_the_column_keeps_a_space_before_its_value() {
    let theme = Theme::default();
    let line = drawn(
        field_row(&field("current_commit", "e52b2ef"), &theme),
        80,
        &theme,
    );
    assert_eq!(line, "  current_commit e52b2ef");
    let line = drawn(
        field_row(&field("last_active_epoch", "17"), &theme),
        80,
        &theme,
    );
    assert_eq!(line, "  last_active_epoch 17");
}

#[test]
fn a_placeholder_field_recedes_and_has_nothing_to_copy() {
    let theme = Theme::default();
    let stand_in = CoreField {
        placeholder: true,
        ..field("remotes", marks::NO_VALUE)
    };
    assert_eq!(field_row(&stand_in, &theme).activate(), None);
    let lines = grid(&theme)
        .row(field_row(&stand_in, &theme))
        .lines(80, None, false, &theme)
        .lines;
    let value = lines[0]
        .spans
        .iter()
        .find(|span| span.content == marks::NO_VALUE)
        .expect("the stand-in is drawn");
    assert!(value.style.add_modifier.contains(theme.muted));
}

#[test]
fn the_edge_tones_take_their_theme_roles() {
    let mut theme = Theme::default();
    assert!(theme.set("edge_alt_of", TokenValue::Color(Color::Gray)));
    assert!(theme.set("edge_conflict", TokenValue::Color(Color::LightRed)));
    let tones = tones(&theme);
    assert_eq!(tones.style(Tone::Quiet).fg, Some(Color::Gray));
    let alert = tones.style(Tone::Alert);
    assert_eq!(alert.fg, Some(Color::LightRed));
    assert!(alert.add_modifier.contains(Modifier::BOLD));
    assert_eq!(tones.style(Tone::Primary), Tone::Primary.style(&theme));
    assert_eq!(tones.style(Tone::Inert), Tone::Inert.style(&theme));
}

#[test]
fn the_other_header_counts_its_rows_and_its_marked_ones() {
    let counts = RelationshipCounts {
        validated: 2,
        other: 3,
        ambiguous: 1,
        unresolved: 2,
    };
    let header = other_header(false, counts);
    assert_eq!(
        header.summary(),
        format!("(3 · 1 {} · 2 {})", marks::WARNING, marks::NO_VALUE)
    );
    assert!(!header.is_open());
    assert_eq!(header.activate(), Some(Intent::Toggle));
    let quiet = RelationshipCounts {
        other: 4,
        ..RelationshipCounts::default()
    };
    assert_eq!(other_header(true, quiet).summary(), "(4)");
}

fn group() -> RelationshipGroup {
    RelationshipGroup {
        direction: Direction::Downstream,
        relation: RelationKind::ParentSession,
        neighbor_kind: "agent_session".into(),
        links: Vec::new(),
        unresolved: Vec::new(),
        ambiguous: false,
        unresolved_count: 1,
    }
}

fn stub() -> UnresolvedRow {
    UnresolvedRow {
        link_id: "u1".into(),
        node_type: "agent_session".into(),
        evidence: UnresolvedEvidence {
            harness_key: Some("claude-code".into()),
            native_id: Some("9a1f".into()),
            state_scope: None,
            path: Some("/fixture/a/rather/long/path/to/the/missing/state".into()),
        },
        provenance: Provenance::Discovered,
        confidence: Confidence::Low,
        state: LinkStateLabel::Active,
    }
}

#[test]
fn an_unresolved_stub_is_one_inert_row_with_its_edge_meta_under_e() {
    let theme = Theme::default();
    for width in [40, 57, 77, 120] {
        let line = drawn(unresolved_row(&group(), &stub(), false), width, &theme);
        assert!(text::width(&line) <= width, "{width}: one row: {line}");
        assert!(
            line.starts_with(&format!(
                "      {}",
                directional_verb(&RelationKind::ParentSession, Direction::Downstream)
            )),
            "under the Other header: {line}"
        );
        assert!(
            line.contains(&format!("{} 9a1f", marks::NO_VALUE)),
            "{line}"
        );
        assert!(
            !line.contains("discovered"),
            "{width}: no edge meta: {line}"
        );
    }
    let wide = drawn(unresolved_row(&group(), &stub(), false), 200, &theme);
    assert!(
        wide.ends_with("9a1f · claude-code · /fixture/a/rather/long/path/to/the/missing/state"),
        "{wide}"
    );
    let meta = drawn(unresolved_row(&group(), &stub(), true), 200, &theme);
    assert!(
        meta.ends_with("missing/state  · discovered · low · active"),
        "{meta}"
    );

    let row = unresolved_row(&group(), &stub(), false);
    assert_eq!(row.activate(), None, "an unresolved stub leads nowhere");
    let bare = UnresolvedRow {
        evidence: UnresolvedEvidence::default(),
        ..stub()
    };
    assert_eq!(unresolved_label(&bare), "agent_session");
}

#[test]
fn every_explorer_row_converts_and_a_vanished_link_stays_a_row() {
    let theme = Theme::default();
    let view = NodeView {
        focused: crate::model::NodeId::Repo(crate::model::RepoId::new("/r")),
        kind: NodeKind::Repo,
        title_line: String::new(),
        short_label: String::new(),
        short_id: String::new(),
        full_id: crate::model::NodeId::Repo(crate::model::RepoId::new("/r")),
        core_fields: vec![field("id", "abc")],
        all_fields: vec![field("id", "abc")],
        relationships: crate::tui::explorer::RelationshipExplorer {
            groups: vec![RelationshipGroup {
                unresolved: vec![stub()],
                ..group()
            }],
        },
    };
    let rows = view.flat_rows(true, false);
    let converted: Vec<Row> = rows
        .iter()
        .map(|row| grid_row(&view, row, &theme, false))
        .collect();
    assert_eq!(converted.len(), rows.len());
    assert!(matches!(converted[0], Row::Property(_)));
    assert!(matches!(converted[1], Row::Disclosure(_)));
    assert!(matches!(converted[2], Row::Link(_)));
    assert_eq!(converted[0].activate().map(|_| ()), Some(()));

    let gone = ExplorerRow::ValidatedLink {
        group_index: 9,
        link_index: 0,
    };
    assert_eq!(grid_row(&view, &gone, &theme, false).activate(), None);
}
