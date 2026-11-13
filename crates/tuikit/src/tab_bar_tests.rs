use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use super::*;

/// Six names, 33 cells, and 48 with the separators.
const NAMES: [&str; 6] = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];

fn bar(active: usize, room: usize) -> String {
    let theme = Theme::default();
    TabBar::new(&NAMES, active, &theme).line(room).to_string()
}

#[test]
fn every_tab_shows_while_they_fit() {
    for room in [48, 49, 80] {
        assert_eq!(
            bar(0, room),
            "alpha · bravo · charlie · delta · echo · foxtrot",
            "room {room}"
        );
    }
    // An exact fit needs no free cell after it: nothing gave way.
    assert_eq!(text::width(&bar(3, 48)), 48);
}

#[test]
fn inactive_tabs_give_way_from_the_end_of_the_bar() {
    // `foxtrot` goes first: `… · echo …` with the free cell is 41 cells.
    assert_eq!(bar(0, 47), "alpha · bravo · charlie · delta · echo …");
    assert_eq!(bar(0, 41), "alpha · bravo · charlie · delta · echo …");
    assert_eq!(bar(0, 40), "alpha · bravo · charlie · delta …");
    assert_eq!(bar(0, 30), "alpha · bravo · charlie …");
    assert_eq!(bar(0, 20), "alpha · bravo …");
    assert_eq!(bar(0, 12), "alpha …");
}

#[test]
fn the_active_tab_never_gives_way() {
    // Active last: the tabs before it still go from the end.
    assert_eq!(bar(5, 47), "alpha · bravo · charlie · delta · foxtrot …");
    assert_eq!(bar(5, 30), "alpha · bravo · foxtrot …");
    assert_eq!(bar(5, 12), "foxtrot …");
    // Active in the middle: the tabs after it go first, then the ones
    // before it, from the end.
    assert_eq!(bar(2, 47), "alpha · bravo · charlie · delta · echo …");
    assert_eq!(bar(2, 30), "alpha · bravo · charlie …");
    assert_eq!(bar(2, 22), "alpha · charlie …");
    assert_eq!(bar(2, 12), "charlie …");
}

#[test]
fn a_shortened_bar_keeps_a_cell_free_after_its_mark() {
    for room in 0..=60 {
        for active in 0..NAMES.len() {
            let line = bar(active, room);
            if line.ends_with(marks::MORE) {
                assert!(
                    text::width(&line) < room,
                    "active {active} at {room}: {line:?} leaves no cell after its mark"
                );
            }
        }
    }
}

#[test]
fn the_active_tab_shows_alone_when_the_mark_does_not_fit() {
    // `delta …` and its free cell take 8 cells; `delta` alone fits 5 to 7.
    assert_eq!(bar(3, 8), "delta …");
    for room in 5..8 {
        assert_eq!(bar(3, room), "delta", "room {room}");
    }
}

#[test]
fn the_active_tab_alone_too_wide_is_cut_to_the_room() {
    assert_eq!(bar(2, 6), "charl…");
    assert_eq!(bar(2, 1), "…");
    assert_eq!(bar(2, 0), "");
}

#[test]
fn the_bar_never_runs_past_its_room_or_cuts_a_name_beside_another() {
    for room in 0..=60 {
        for (active, name) in NAMES.iter().enumerate() {
            let line = bar(active, room);
            assert!(
                text::width(&line) <= room,
                "active {active} at {room}: {line:?}"
            );
            let words: Vec<&str> = line
                .split(marks::SPACED_SEPARATOR)
                .map(|word| word.trim_end_matches(marks::MORE))
                .collect();
            if words.len() > 1 {
                assert!(
                    words.iter().all(|word| NAMES.contains(word)),
                    "active {active} at {room}: {line:?}"
                );
            }
            assert!(
                line.is_empty() || line.contains(name) || words.len() == 1,
                "active {active} at {room}: {line:?} lost the active tab"
            );
        }
    }
}

#[test]
fn the_active_tab_takes_the_accent_and_the_rest_recede() {
    let theme = Theme::default();
    let line = TabBar::new(&NAMES, 1, &theme).line(20);
    assert_eq!(line.to_string(), "alpha · bravo …");
    let inert = Style::default().fg(theme.inert);
    let active = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let looks: Vec<(&str, Style)> = line
        .spans
        .iter()
        .map(|span| (span.content.as_ref(), span.style))
        .collect();
    assert_eq!(
        looks,
        [
            ("alpha", inert),
            (marks::SPACED_SEPARATOR, inert),
            ("bravo", active),
            (marks::MORE, inert),
        ]
    );
}

#[test]
fn a_host_may_restyle_the_tabs() {
    let theme = Theme::default();
    let chosen = Style::default().add_modifier(Modifier::UNDERLINED);
    let quiet = Style::default().add_modifier(Modifier::ITALIC);
    let line = TabBar::new(&NAMES[..2], 0, &theme)
        .active_style(chosen)
        .style(quiet)
        .line(80);
    let looks: Vec<Style> = line.spans.iter().map(|span| span.style).collect();
    assert_eq!(looks, [chosen, quiet, quiet]);
}

#[test]
fn names_are_measured_in_cells() {
    let theme = Theme::default();
    // Two characters, four cells each.
    let names = ["東京", "大阪", "abc"];
    let bar = TabBar::new(&names, 2, &theme);
    assert_eq!(bar.line(17).to_string(), "東京 · 大阪 · abc");
    // At 16, `大阪` goes: `東京 · abc …` and the free cell take 13.
    assert_eq!(bar.line(16).to_string(), "東京 · abc …");
    assert_eq!(bar.line(13).to_string(), "東京 · abc …");
    assert_eq!(bar.line(12).to_string(), "abc …");
}

#[test]
fn the_fit_reports_what_shows() {
    let theme = Theme::default();
    let bar = TabBar::new(&NAMES, 2, &theme);
    let fit = bar.fit(30);
    assert_eq!(fit.kept, [0, 1, 2]);
    assert!(fit.end_mark);
    assert_eq!(bar.fit(80).kept, [0, 1, 2, 3, 4, 5]);
    assert_eq!(bar.fit(6).cut, Some(6));
}

#[test]
fn an_active_index_past_the_end_is_the_last_tab() {
    let theme = Theme::default();
    let bar = TabBar::new(&NAMES, 99, &theme);
    assert_eq!(bar.active(), 5);
    assert_eq!(bar.line(12).to_string(), "foxtrot …");
}

#[test]
fn no_tabs_draw_an_empty_line() {
    let theme = Theme::default();
    assert_eq!(TabBar::new(&[], 0, &theme).line(80), Line::default());
}

#[test]
fn the_mark_is_a_space_and_the_ellipsis() {
    assert_eq!(marks::MORE, format!(" {}", marks::ELLIPSIS));
}
