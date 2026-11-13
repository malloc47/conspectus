//! The grid's three rows as drawn (columns, cuts, hints, tones, the
//! disclosure mark), the shared cursor look in both focus states, the
//! lines that are no row, and the intents each row reports.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;
use crate::glyphs::marks;
use crate::selection::GUTTER_WIDTH;
use crate::text;
use crate::theme::Theme;

fn text_of(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// The one row `row` draws as, at `width`, with no cursor.
fn drawn(row: impl Into<Row>, width: usize) -> Line<'static> {
    let theme = Theme::default();
    let mut lines = PropertyGrid::new()
        .inspect_key("o")
        .row(row)
        .lines(width, None, true, &theme)
        .lines;
    assert_eq!(lines.len(), 1);
    lines.remove(0)
}

/// The span that holds `needle`.
fn span_with<'a>(line: &'a Line<'static>, needle: &str) -> &'a Span<'static> {
    line.spans
        .iter()
        .find(|span| span.content.contains(needle))
        .unwrap_or_else(|| panic!("no span holds {needle:?} in {:?}", text_of(line)))
}

#[test]
fn a_property_row_is_the_gutter_a_bold_label_column_and_the_value() {
    let line = drawn(PropertyRow::new("name", "build cache"), 80);
    assert_eq!(text_of(&line), "  name          build cache");
    let label = span_with(&line, "name");
    assert_eq!(label.content, text::pad("name", LABEL_WIDTH));
    assert!(label.style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(span_with(&line, "build").style, Style::default());
}

#[test]
fn a_label_as_wide_as_its_column_keeps_a_blank_cell_before_the_value() {
    for label in ["fourteen_cells", "a_label_wider_than_the_column"] {
        let line = drawn(PropertyRow::new(label, "value"), 80);
        assert_eq!(text_of(&line), format!("  {label} value"));
    }
    let short = drawn(PropertyRow::new("thirteen_cell", "value"), 80);
    assert_eq!(text_of(&short), "  thirteen_cell value");
}

#[test]
fn the_label_column_is_the_grids_to_set() {
    let theme = Theme::default();
    let lines = PropertyGrid::new()
        .label_width(6)
        .row(PropertyRow::new("id", "7"))
        .lines(40, None, true, &theme)
        .lines;
    assert_eq!(text_of(&lines[0]), "  id    7");
}

#[test]
fn a_glyph_sits_between_the_label_and_the_value() {
    let glyph = Span::styled("#", Style::default().fg(Color::Magenta));
    let line = drawn(PropertyRow::new("root", "/srv").glyph(glyph.clone()), 80);
    assert_eq!(text_of(&line), "  root          # /srv");
    assert_eq!(*span_with(&line, "#"), glyph, "the glyph keeps its style");
}

#[test]
fn a_value_cut_at_its_end_or_middle_says_so_and_names_the_inspect_key() {
    let end = drawn(
        PropertyRow::new("note", "a long note that will not fit").cut(Cut::End, 12),
        80,
    );
    assert_eq!(
        text_of(&end),
        "  note          a long note…  (truncated · o)"
    );
    let hint = span_with(&end, "truncated");
    assert!(hint.style.add_modifier.contains(Theme::default().muted));

    let middle = drawn(
        PropertyRow::new("path", "/srv/archive/2026/report.txt").cut(Cut::Middle, 15),
        80,
    );
    assert!(text_of(&middle).contains("/srv/ar…ort.txt  (truncated · o)"));

    let fits = drawn(PropertyRow::new("note", "short").cut(Cut::End, 12), 80);
    assert_eq!(
        text_of(&fits),
        "  note          short",
        "a value that fits is not cut"
    );
}

#[test]
fn a_cut_counts_cells_not_characters() {
    let row = PropertyRow::new("city", "東京大阪名古屋").cut(Cut::End, 6);
    assert_eq!(row.shown(), "東京…");
    assert!(text::width(&row.shown()) <= 6);
    assert!(row.is_cut());
}

#[test]
fn without_an_inspect_key_the_hint_names_no_key_and_a_hint_of_its_own_replaces_it() {
    let theme = Theme::default();
    let cut = PropertyRow::new("note", "a long note that will not fit").cut(Cut::End, 12);
    let lines = PropertyGrid::new()
        .row(cut.clone())
        .row(cut.hint("see below"))
        .lines(80, None, true, &theme)
        .lines;
    assert!(text_of(&lines[0]).ends_with("a long note…  (truncated)"));
    assert!(text_of(&lines[1]).ends_with("a long note…  see below"));
}

#[test]
fn a_placeholder_recedes_and_an_annotation_needs_attention() {
    let theme = Theme::default();
    let line = drawn(
        PropertyRow::new("owner", marks::NO_VALUE)
            .placeholder()
            .annotation("stale"),
        80,
    );
    assert_eq!(
        text_of(&line),
        format!("  owner         {} stale", marks::NO_VALUE)
    );
    let value = span_with(&line, marks::NO_VALUE);
    assert!(value.style.add_modifier.contains(theme.muted));
    assert_eq!(span_with(&line, "stale").style.fg, Some(theme.warning));
}

#[test]
fn a_value_given_as_spans_keeps_its_styles_and_is_cut_at_its_end() {
    let spans = vec![
        Span::styled("left ", Style::default().fg(Color::Green)),
        Span::styled("right side", Style::default().fg(Color::Red)),
    ];
    let row = PropertyRow::spans("pair", spans.clone());
    assert_eq!(row.text(), "left right side");
    let line = drawn(row.clone(), 80);
    assert_eq!(span_with(&line, "left").style.fg, Some(Color::Green));
    assert_eq!(span_with(&line, "right").style.fg, Some(Color::Red));

    let cut = row.cut(Cut::Middle, 8);
    assert_eq!(cut.shown(), "left ri…");
    assert_eq!(
        cut.inspect(),
        Some(Intent::Inspect {
            label: "pair".into(),
            text: "left right side".into()
        })
    );
}

#[test]
fn a_property_copies_its_whole_value_and_inspects_only_when_cut() {
    let cut = PropertyRow::new("note", "a long note that will not fit").cut(Cut::End, 12);
    let whole = Intent::Copy {
        label: "note".into(),
        text: "a long note that will not fit".into(),
    };
    assert_eq!(cut.activate(), Some(whole));
    assert!(matches!(cut.inspect(), Some(Intent::Inspect { ref text, .. }) if text.len() == 29));

    let short = PropertyRow::new("note", "short");
    assert!(short.activate().is_some());
    assert_eq!(
        short.inspect(),
        None,
        "a value drawn whole has nothing more to show"
    );
    assert_eq!(PropertyRow::new("note", "").activate(), None);
    assert_eq!(
        PropertyRow::new("note", marks::NO_VALUE)
            .placeholder()
            .activate(),
        None,
        "a stand-in is not a value to copy"
    );
}

fn link_text(row: LinkRow, width: usize) -> String {
    text_of(&drawn(row, width))
}

#[test]
fn a_link_row_is_the_verb_column_the_glyph_and_the_label() {
    let row = LinkRow::new("stored in", "/srv/cache").glyph(Span::raw("#"));
    assert_eq!(
        link_text(row.clone(), 80),
        format!("  {}# /srv/cache", text::pad("stored in", VERB_WIDTH + 1))
    );
    let indented = link_text(row.indent(4), 80);
    assert!(indented.starts_with("      stored in"), "{indented}");
}

#[test]
fn a_link_label_is_cut_in_the_middle_so_the_row_never_wraps() {
    let path = "/srv/archive/a-rather-long-directory-name/report";
    for width in [40, 48, 56, 70] {
        let line = link_text(LinkRow::new("stored in", path).glyph(Span::raw("#")), width);
        assert!(text::width(&line) <= width, "{width}: {line}");
        assert!(line.ends_with("report"), "the end stays: {line}");
    }
    let wide = link_text(LinkRow::new("stored in", path), 200);
    assert!(wide.ends_with(path), "a label that fits is whole");
}

#[test]
fn the_label_leaves_its_meta_room_down_to_its_minimum_and_then_the_meta_runs_on() {
    let label = "/srv/archive/a-much-longer-cache-path";
    let row = LinkRow::new("stored in", label)
        .glyph(Span::raw("#"))
        .meta("high · active");
    let wide = link_text(row.clone(), 100);
    assert!(
        wide.ends_with(&format!("{label}  · high · active")),
        "{wide}"
    );

    // Gutter, verb column and its space, glyph and its space: 27 cells.
    let fixed = GUTTER_WIDTH + VERB_WIDTH + 1 + 2;
    let meta = text::width("  · high · active");
    let width = fixed + meta + MIN_LINK_LABEL;
    let just = link_text(row.clone(), width);
    assert_eq!(text::width(&just), width, "the row fits: {just}");
    assert!(just.contains(&text::truncate_middle(label, MIN_LINK_LABEL)));

    let short = link_text(row, width - 1);
    assert!(
        short.contains(&text::truncate_middle(label, MIN_LINK_LABEL + meta - 1)),
        "the label takes the room: {short}"
    );
    assert!(
        short.ends_with("· high · active"),
        "the meta is kept, past the width, for the host to wrap: {short}"
    );
}

#[test]
fn each_tone_draws_the_label_and_meta_in_its_look() {
    let theme = Theme::default();
    let tones = Tones::new(&theme);
    for tone in Tone::ALL {
        let line = drawn(
            LinkRow::new("stored in", "target")
                .glyph(Span::raw("#"))
                .meta("detail")
                .tone(tone),
            80,
        );
        assert_eq!(
            span_with(&line, "target").style,
            tones.style(tone),
            "{tone:?}"
        );
        assert_eq!(
            span_with(&line, "detail").style,
            tones.style(tone),
            "{tone:?}"
        );
        let verb = span_with(&line, "stored in").style;
        if tone == Tone::Inert {
            assert_eq!(verb, tones.style(tone), "an inert row recedes whole");
        } else {
            assert_eq!(verb, Style::default(), "{tone:?}: the verb stays plain");
        }
    }
    assert_eq!(Tone::Primary.style(&theme).fg, Some(theme.link));
    assert_eq!(Tone::Quiet.style(&theme).fg, Some(theme.inert));
    assert_eq!(Tone::Alert.style(&theme).fg, Some(theme.warning));
    assert!(Tone::Inert.style(&theme).add_modifier.contains(theme.muted));
}

#[test]
fn an_alert_row_leads_with_the_warning_mark_in_its_indent() {
    let theme = Theme::default();
    let row = LinkRow::new("stored in", "target")
        .glyph(Span::raw("#"))
        .tone(Tone::Alert);
    let quiet = link_text(row.clone().indent(4).tone(Tone::Quiet), 80);
    let alert = drawn(row.clone().indent(4), 80);
    let text = text_of(&alert);
    assert_eq!(text, format!("  {} {}", marks::WARNING, &quiet[4..]));
    let column = |line: &str| line.find("stored").map(|at| text::width(&line[..at]));
    assert_eq!(column(&text), column(&quiet), "the verb column holds");
    let mark = span_with(&alert, marks::WARNING);
    assert_eq!(mark.style, Tone::Alert.style(&theme));
    let unindented = link_text(row, 80);
    assert!(
        unindented.starts_with(&format!("  {} stored", marks::WARNING)),
        "{unindented}"
    );
}

#[test]
fn a_link_with_no_glyph_draws_the_no_value_mark_in_its_place() {
    let line = link_text(
        LinkRow::new("rooted at", "/srv/missing").tone(Tone::Inert),
        80,
    );
    assert!(
        line.contains(&format!("{} /srv/missing", marks::NO_VALUE)),
        "{line}"
    );
}

#[test]
fn the_host_can_draw_a_tone_in_a_role_of_its_own() {
    let theme = Theme::default();
    let look = Style::default().fg(Color::Magenta);
    let lines = PropertyGrid::new()
        .tones(Tones::new(&theme).with(Tone::Quiet, look))
        .row(LinkRow::new("replaces", "old").tone(Tone::Quiet))
        .lines(80, None, true, &theme)
        .lines;
    assert_eq!(span_with(&lines[0], "old").style, look);
}

#[test]
fn a_link_follows_unless_it_is_inert() {
    for tone in [Tone::Primary, Tone::Quiet, Tone::Alert] {
        let row = LinkRow::new("to", "there").tone(tone);
        assert_eq!(row.activate(), Some(Intent::Follow), "{tone:?}");
        assert_eq!(Row::from(row).inspect(), None);
    }
    assert_eq!(
        LinkRow::new("to", "nowhere").tone(Tone::Inert).activate(),
        None
    );
}

#[test]
fn a_disclosure_row_is_the_tree_mark_a_label_and_its_summary() {
    let theme = Theme::default();
    let closed = DisclosureRow::new("Other", false)
        .count(4)
        .tally(2, marks::WARNING)
        .tally(1, marks::NO_VALUE);
    let line = drawn(closed.clone(), 80);
    assert_eq!(
        text_of(&line),
        format!(
            "  {} Other  (4 · 2 {} · 1 {})",
            marks::COLLAPSED,
            marks::WARNING,
            marks::NO_VALUE
        )
    );
    let mark = span_with(&line, marks::COLLAPSED);
    let tree_mark = crate::tree::disclosure(true, false, &theme).render(1, &theme);
    assert_eq!(
        (mark.content.as_ref(), mark.style),
        (tree_mark[0].content.as_ref(), tree_mark[0].style),
        "the mark is a tree's folding mark, in its look"
    );
    let label = span_with(&line, "Other");
    assert_eq!(label.style.fg, Some(theme.inert));
    assert!(label.style.add_modifier.contains(Modifier::BOLD));

    let open = drawn(DisclosureRow::new("Other", true), 80);
    assert_eq!(text_of(&open), format!("  {} Other", marks::EXPANDED));
    assert!(DisclosureRow::new("Other", true).is_open());
    assert_eq!(closed.activate(), Some(Intent::Toggle));
    assert_eq!(Row::from(closed).inspect(), None);
}

#[test]
fn the_cursor_row_takes_the_gutter_mark_and_the_selection_pair_whole() {
    let theme = Theme::default();
    let grid = PropertyGrid::new()
        .row(PropertyRow::new("name", "build cache").glyph(Span::raw("#")))
        .row(LinkRow::new("stored in", "/srv/cache").glyph(Span::raw("#")))
        .row(DisclosureRow::new("Other", false).count(1));
    let plain = grid.lines(80, None, true, &theme).lines;
    for cursor in 0..grid.len() {
        for focused in [true, false] {
            let drawn = grid.lines(80, Some(cursor), focused, &theme);
            let pair = if focused {
                theme.selection_active
            } else {
                theme.selection_inactive
            };
            for (index, line) in drawn.lines.iter().enumerate() {
                let marked = line.spans[0].content.starts_with(marks::POINTER);
                assert_eq!(marked, index == cursor, "{cursor} {focused} {index}");
                if index != cursor {
                    assert_eq!(
                        *line, plain[index],
                        "{cursor} {focused}: row {index} is untouched"
                    );
                    continue;
                }
                assert_eq!(
                    text_of(line)[marks::POINTER.len()..],
                    text_of(&plain[index])[1..]
                );
                for span in &line.spans {
                    assert!(
                        span.style.add_modifier.contains(pair),
                        "{cursor} {focused}: every span of the cursor row is highlighted: {:?}",
                        span.content
                    );
                }
            }
        }
    }
}

#[test]
fn lines_that_are_no_row_are_drawn_as_given_and_skipped_by_the_cursor() {
    let theme = Theme::default();
    let mut grid = PropertyGrid::new().row(PropertyRow::new("name", "a"));
    grid.push_line(Line::raw("---- rule ----"));
    grid.push(LinkRow::new("to", "b"));
    assert_eq!(grid.len(), 2);
    assert!(!grid.is_empty());
    let drawn = grid.lines(40, Some(1), true, &theme);
    assert_eq!(drawn.lines.len(), 3);
    assert_eq!(text_of(&drawn.lines[1]), "---- rule ----");
    assert_eq!(
        drawn.cursor_line,
        Some(2),
        "the cursor's row is after the rule"
    );
    assert_eq!(grid.activate(1), Some(Intent::Follow));
    assert!(matches!(grid.get(1), Some(Row::Link(_))));
    assert_eq!(grid.activate(2), None, "past the last row there is nothing");

    let none = grid.lines(40, None, true, &theme);
    assert_eq!(none.cursor_line, None);
    assert!(PropertyGrid::default().is_empty());
}

#[test]
fn with_no_color_and_no_highlight_the_marks_still_tell_rows_apart() {
    let mut theme = Theme::default();
    theme.selection_active = Modifier::empty();
    theme.selection_inactive = Modifier::empty();
    let grid = PropertyGrid::new()
        .row(LinkRow::new("to", "a").indent(4).tone(Tone::Alert))
        .row(DisclosureRow::new("Other", true).count(1));
    let drawn = grid.lines(60, Some(0), true, &theme);
    let text: Vec<String> = drawn.lines.iter().map(text_of).collect();
    assert!(text[0].starts_with(&format!("{} {}", marks::POINTER, marks::WARNING)));
    assert!(text[1].contains(marks::EXPANDED));
}
