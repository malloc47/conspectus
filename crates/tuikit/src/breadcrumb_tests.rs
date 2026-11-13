use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;

fn plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn glyph(text: &'static str, color: Color) -> Span<'static> {
    Span::styled(text, Style::default().fg(color))
}

/// A hop led by a one-cell ASCII glyph in a color of its own.
fn hop(tag: &str, suffix: &str) -> Hop {
    Hop::new(tag).glyph(glyph("*", Color::Cyan)).suffix(suffix)
}

fn chain(hops: &[Hop], room: usize) -> String {
    let theme = Theme::default();
    plain(&Breadcrumb::new(hops, &theme).line(room).expect("hops"))
}

#[test]
fn a_chain_joins_its_hops_with_the_crumb_mark() {
    let hops = [
        Hop::new("abc").glyph(glyph("*", Color::Cyan)),
        Hop::new("editor").glyph(glyph("#", Color::Blue)),
        Hop::new("helper").glyph(glyph("@", Color::Gray)),
    ];
    assert_eq!(chain(&hops, 80), "* abc › # editor › @ helper");
    assert_eq!(
        marks::SPACED_CRUMB,
        format!(" {} ", marks::CRUMB),
        "the separator is the crumb mark between spaces"
    );
}

#[test]
fn no_hops_draw_no_line() {
    let theme = Theme::default();
    assert!(Breadcrumb::new(&[], &theme).line(80).is_none());
    assert!(Breadcrumb::new(&[], &theme).line(0).is_none());
}

#[test]
fn the_middle_gives_way_while_the_first_and_last_hops_stay() {
    // 10, 13, 15, and 9 cells: 56 with the separators.
    let hops = [
        hop("abcdefgh", "0001"),
        hop("editor-east", "0002"),
        hop("lookup-helper", "0003"),
        hop("xyzlast", "0004"),
    ];
    assert_eq!(
        chain(&hops, 56),
        "* abcdefgh › * editor-east › * lookup-helper › * xyzlast"
    );
    // The oldest middle hop goes first, so the newer one stays while it
    // fits beside the mark.
    assert_eq!(
        chain(&hops, 44),
        "* abcdefgh › … › * lookup-helper › * xyzlast"
    );
    // One mark stands for the whole run that gave way.
    assert_eq!(chain(&hops, 40), "* abcdefgh › … › * xyzlast");
    assert_eq!(chain(&hops, 26), "* abcdefgh › … › * xyzlast");
    assert_eq!(chain(&hops, 25), "… › * xyzlast");
}

#[test]
fn the_last_hop_is_cut_when_even_it_does_not_fit() {
    let hops = [hop("abc", ""), hop("editor", ""), hop("helper", "")];
    // `… › * helper` is 12 cells and `* helper` 8: below that the last
    // hop is cut at its end, with the mark inside the room.
    assert_eq!(chain(&hops, 12), "… › * helper");
    assert_eq!(chain(&hops, 11), "* helper");
    assert_eq!(chain(&hops, 8), "* helper");
    assert_eq!(chain(&hops, 5), "* he…");
    assert_eq!(chain(&hops, 1), "…");
    assert_eq!(chain(&hops, 0), "");
}

#[test]
fn hops_with_one_tag_show_their_suffixes() {
    let hops = [
        hop("abc", "1234"),
        hop("editor", "9999"),
        hop("abc", "5678"),
    ];
    // Only the two that read alike carry the suffix.
    assert_eq!(chain(&hops, 80), "* abc·1234 › * editor › * abc·5678");
}

#[test]
fn an_empty_suffix_shows_nothing_even_when_tags_repeat() {
    let hops = [hop("abc", ""), hop("abc", "5678")];
    assert_eq!(chain(&hops, 80), "* abc › * abc·5678");
}

#[test]
fn a_chain_narrows_one_hop_at_a_time() {
    let hops = [
        hop("a", ""),
        hop("b", ""),
        hop("c", ""),
        hop("d", ""),
        hop("e", ""),
    ];
    let forms = [
        "* a › * b › * c › * d › * e",
        "* a › … › * c › * d › * e",
        "* a › … › * d › * e",
        "* a › … › * e",
        "… › * e",
        "* e",
        "*…",
        "…",
        "",
    ];
    // Every room from the whole chain down to nothing draws one of the
    // forms, in this order, each as wide as fits.
    let full = text::width(forms[0]);
    let mut seen = Vec::new();
    for room in (0..=full).rev() {
        let line = chain(&hops, room);
        assert!(text::width(&line) <= room, "room {room}: {line:?}");
        if seen.last() != Some(&line) {
            seen.push(line);
        }
    }
    assert_eq!(seen, forms);
}

#[test]
fn the_line_never_takes_more_cells_than_its_room() {
    let wide = Span::styled("東", Style::default().fg(Color::Red));
    let chains = [
        vec![hop("東京", "01"), hop("大阪", "02"), hop("東京", "03")],
        vec![
            Hop::new("first").glyph(wide.clone()),
            Hop::new("second"),
            Hop::new("third").glyph(wide),
        ],
        vec![hop("only", "")],
    ];
    let theme = Theme::default();
    for hops in &chains {
        let crumbs = Breadcrumb::new(hops, &theme);
        let full = text::spans_width(&crumbs.line(usize::MAX).expect("hops").spans);
        for room in 0..=full + 2 {
            let line = crumbs.line(room).expect("hops");
            assert!(
                text::spans_width(&line.spans) <= room,
                "room {room}: {:?}",
                plain(&line)
            );
        }
        assert_eq!(
            text::spans_width(&crumbs.line(full).expect("hops").spans),
            full
        );
    }
    // Cells, not characters: the chain is 10 characters but 12 cells.
    let hops = [hop("東京", ""), hop("x", "")];
    assert_eq!(chain(&hops, 12), "* 東京 › * x");
    assert_eq!(chain(&hops, 11), "… › * x");
}

#[test]
fn the_glyph_keeps_its_style_and_the_rest_take_the_chain_style() {
    let theme = Theme::default();
    let inert = Style::default().fg(theme.inert);
    let hops = [hop("abc", ""), hop("def", ""), hop("ghi", "")];
    let line = Breadcrumb::new(&hops, &theme).line(80).expect("hops");
    for span in &line.spans {
        let expected = if span.content == "*" {
            Style::default().fg(Color::Cyan)
        } else {
            inert
        };
        assert_eq!(span.style, expected, "{:?}", span.content);
    }

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let line = Breadcrumb::new(&hops, &theme)
        .style(bold)
        .line(17)
        .expect("hops");
    assert_eq!(plain(&line), "* abc › … › * ghi");
    let mark = line
        .spans
        .iter()
        .find(|span| span.content == marks::ELLIPSIS)
        .expect("a mark");
    assert_eq!(mark.style, bold);
    let separator = line
        .spans
        .iter()
        .find(|span| span.content == marks::SPACED_CRUMB)
        .expect("a separator");
    assert_eq!(separator.style, bold);

    // A cut that falls in the glyph draws the mark in the chain's style,
    // not the glyph's.
    let line = Breadcrumb::new(&hops, &theme).line(1).expect("hops");
    assert_eq!(plain(&line), marks::ELLIPSIS);
    assert_eq!(line.spans[0].style, inert);
}

#[test]
fn a_lone_hop_is_never_marked() {
    let hops = [hop("alone", "")];
    assert_eq!(chain(&hops, 80), "* alone");
    assert_eq!(chain(&hops, 7), "* alone");
    assert_eq!(chain(&hops, 6), "* alo…");
}

#[test]
fn a_hop_without_a_glyph_draws_its_tag_alone() {
    let hops = [Hop::new("plain"), hop("led", "")];
    assert_eq!(chain(&hops, 80), "plain › * led");
    assert_eq!(Hop::new("plain").tag(), "plain");
}

#[test]
fn ranks_keep_the_last_hop_then_the_first_then_the_newest_middle_hops() {
    assert_eq!(rank(0, 1), None);
    assert_eq!(rank(1, 2), None);
    assert_eq!(rank(0, 2), Some(2));
    let ranks: Vec<Option<u32>> = (0..5).map(|index| rank(index, 5)).collect();
    assert_eq!(ranks, [Some(5), Some(1), Some(2), Some(3), None]);
}
