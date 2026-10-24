//! The reference document `docs/tui-columns.md` describes the left panel
//! as it is drawn. These tests fail when it stops doing so: a glyph, a
//! state word, a view, or a row kind that the code knows and the
//! document doesn't (or the reverse). The example frames are checked by
//! `tests/tui_columns_doc.rs`, which needs the built binary.

use std::collections::BTreeSet;

use super::text::{artifact_in, says_slot};
use super::*;
use crate::tui::rows::{RowKind, words};

const DOC: &str = "docs/tui-columns.md";

fn doc() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(DOC))
        .unwrap_or_else(|err| panic!("{DOC}: {err}"))
}

/// The lines under the `## {title}` heading, up to the next `## `.
fn section<'a>(doc: &'a str, title: &str) -> Vec<&'a str> {
    let heading = format!("## {title}");
    let mut lines = doc.lines().skip_while(|line| line.trim_end() != heading);
    assert!(lines.next().is_some(), "{DOC} has no `{heading}` section");
    lines.take_while(|line| !line.starts_with("## ")).collect()
}

/// The backticked tokens in the first cell of each table row.
fn first_cells(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let Some(row) = line.strip_prefix('|') else {
            continue;
        };
        let cell = row.split('|').next().unwrap_or("");
        out.extend(
            cell.split('`')
                .enumerate()
                .filter(|(index, token)| index % 2 == 1 && !token.is_empty())
                .map(|(_, token)| token.to_string()),
        );
    }
    out
}

fn has_heading(doc: &str, name: &str) -> bool {
    doc.lines()
        .any(|line| line.starts_with('#') && line.trim_start_matches('#').trim() == name)
}

#[test]
fn every_glyph_the_left_panel_can_draw_is_in_the_glyph_table_and_nothing_else_is() {
    let doc = doc();
    let documented: BTreeSet<String> = first_cells(&section(&doc, "Glyphs")).into_iter().collect();
    let known: BTreeSet<String> = crate::tui::icons::all_glyphs()
        .into_iter()
        .map(str::to_string)
        .collect();
    let missing: Vec<_> = known.difference(&documented).collect();
    let unknown: Vec<_> = documented.difference(&known).collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "the glyph table in {DOC} must list exactly the glyphs in icons.rs.\n\
         drawn but not documented: {missing:?}\ndocumented but not drawn: {unknown:?}"
    );
}

/// Every status word a row builder or helper can produce.
fn vocabulary() -> BTreeSet<String> {
    use crate::model::{NodePresence, PinKind, SlotStatus};
    // Exhaustive: a new status makes this fail to compile until it is
    // listed here and in the document.
    fn slot_statuses(status: SlotStatus) -> SlotStatus {
        match status {
            SlotStatus::Live
            | SlotStatus::Gone
            | SlotStatus::Unknown
            | SlotStatus::Missing
            | SlotStatus::Planned => status,
        }
    }
    let mut produced = BTreeSet::new();
    for status in [
        SlotStatus::Live,
        SlotStatus::Gone,
        SlotStatus::Unknown,
        SlotStatus::Missing,
        SlotStatus::Planned,
    ] {
        for kind in PinKind::ALL {
            produced.insert(slot_statuses(status).label_for(kind).to_string());
        }
    }
    for kind in crate::model::NodeKind::ALL {
        for presence in [
            NodePresence::Gone { last_seen: 0 },
            NodePresence::Unknown { last_seen: 0 },
        ] {
            produced.insert(presence.label_for(kind).to_string());
        }
    }
    let listed: BTreeSet<String> = words::ALL.iter().map(|w| (*w).to_string()).collect();
    assert!(
        produced.is_subset(&listed),
        "a status word is produced that rows::words doesn't list: {:?}",
        produced.difference(&listed).collect::<Vec<_>>()
    );
    listed
}

#[test]
fn every_status_word_is_in_the_vocabulary_table_and_nothing_else_is() {
    let doc = doc();
    let documented: BTreeSet<String> = first_cells(&section(&doc, "State words"))
        .into_iter()
        .collect();
    let produced = vocabulary();
    let missing: Vec<_> = produced.difference(&documented).collect();
    let unknown: Vec<_> = documented.difference(&produced).collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "the state-word table in {DOC} must list exactly the words rows::words lists.\n\
         used but not documented: {missing:?}\ndocumented but never used: {unknown:?}"
    );
}

/// The heading each view has in the document. Exhaustive over `View`:
/// adding a view fails to compile until it is named here.
fn view_heading(view: View) -> &'static str {
    match view {
        View::Sessions => "Sessions view",
        View::Mux => "Mux view",
        View::Union => "Union view",
        View::Prs => "PRs view",
        View::Forks => "Forks view",
        View::Pinned => "Pinned view",
        View::History => "History view",
        View::Search => "Search view",
    }
}

/// The headings a row kind has in the document. Exhaustive over
/// `RowKind`: adding a kind fails to compile until it is named here.
fn row_kind_headings(kind: &RowKind) -> &'static [&'static str] {
    match kind {
        RowKind::Group(_) => &["Group header"],
        RowKind::AgentSession(_) => &["Session row"],
        RowKind::MuxSession(_) => &["Mux row"],
        RowKind::Pr(_) => &["PR row"],
        RowKind::Fork(_) => &["Fork row"],
        RowKind::Mixed(_) => &["Pin row", "History row"],
    }
}

#[test]
fn every_view_and_row_kind_has_a_heading() {
    let doc = doc();
    let mut wanted: BTreeSet<&str> = ALL_VIEWS.iter().map(|view| view_heading(*view)).collect();
    // The kinds the fixture world actually produces, in every view...
    for view in ALL_VIEWS {
        let app = app_in(view);
        for row in &app.tree().rows {
            wanted.extend(row_kind_headings(&row.kind).iter().copied());
        }
    }
    // ...and the full list, for kinds the fixture doesn't reach.
    wanted.extend([
        "Group header",
        "Session row",
        "Mux row",
        "PR row",
        "Fork row",
        "Pin row",
        "History row",
        "Stopped mux row",
        "Nested row",
    ]);
    let missing: Vec<_> = wanted
        .iter()
        .filter(|name| !has_heading(&doc, name))
        .collect();
    assert!(missing.is_empty(), "{DOC} needs a heading for: {missing:?}");
}

#[test]
fn the_reference_speaks_the_operators_language() {
    let doc = doc();
    for (number, line) in doc.lines().enumerate() {
        // The frames show the screen exactly; the rest is prose.
        if let Some(found) = artifact_in(line) {
            panic!("{DOC}:{}: names a developer artifact: {found}", number + 1);
        }
        assert!(
            !says_slot(line),
            "{DOC}:{}: say pin, not slot: {line}",
            number + 1
        );
        let says_gone = line
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word.eq_ignore_ascii_case("gone"));
        assert!(
            !says_gone,
            "{DOC}:{}: say stopped or removed, not gone: {line}",
            number + 1
        );
    }
}

#[test]
fn the_reference_is_linked_from_the_places_people_start() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in ["docs/index.md", "docs/design.md", "README.md"] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        assert!(text.contains("tui-columns.md"), "{file} should link {DOC}");
    }
}
