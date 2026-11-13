use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::*;
use crate::age::{Age, Recency};
use crate::glyphs::marks;
use crate::selection::GUTTER_WIDTH;
use crate::text;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
    Mark,
    Name,
    Size,
    Age,
    State,
    Label,
    Note,
    Extra,
    Fill,
}

fn plain() -> Style {
    Style::default()
}

fn t(text: &str) -> Cell {
    Cell::text(text, plain())
}

fn draw_text<K: Copy + Eq>(
    schema: &ColumnSchema<K>,
    measure: &Measure<K>,
    room: usize,
    cells: &[(K, Cell)],
) -> String {
    Cell::plain(&schema.draw(measure, 0, room, cells, &Theme::default()))
}

fn measured<K: Copy + Eq>(schema: &ColumnSchema<K>, rows: &[Cells<K>]) -> Measure<K> {
    let mut measure = Measure::new();
    for cells in rows {
        measure.add(schema, 0, cells);
    }
    measure
}

// --- cells -----------------------------------------------------------------

#[test]
fn a_text_cell_cuts_by_its_rule() {
    let theme = Theme::default();
    let end = t("a long label");
    assert_eq!(Cell::plain(&end.render(8, &theme)), "a long …");
    let middle = t("/srv/data/archive/report.txt").middle();
    assert_eq!(
        Cell::plain(&middle.render(16, &theme)),
        text::truncate_middle("/srv/data/archive/report.txt", 16)
    );
    let clip = t("0199a3f2-77aa").clip();
    assert_eq!(Cell::plain(&clip.render(8, &theme)), "0199a3f2");
    assert_eq!(
        Cell::plain(&t("fits").render(8, &theme)),
        "fits",
        "unpadded"
    );
}

#[test]
fn a_blank_text_is_an_empty_cell() {
    assert!(t("").is_empty());
    assert!(Cell::spans(vec![Span::raw("")]).is_empty());
    assert!(Cell::chips(Vec::new(), plain()).is_empty());
    assert_eq!(Cell::empty().width(), 0);
    assert!(
        !Cell::age(None, None).is_empty(),
        "an unknown age shows its mark"
    );
}

#[test]
fn a_tail_stays_whole_while_the_text_before_it_is_cut() {
    let theme = Theme::default();
    let cell = t("~/src/a/long/path").middle().tail(vec![Span::styled(
        "  (here)",
        Style::default().fg(Color::Cyan),
    )]);
    assert_eq!(cell.width(), 17 + 8);
    let cut = cell.render(16, &theme);
    let shown = Cell::plain(&cut);
    assert!(shown.ends_with("  (here)"), "{shown}");
    assert_eq!(text::width(&shown), 16);
    assert_eq!(
        cut.last().map(|span| span.style.fg),
        Some(Some(Color::Cyan))
    );
}

#[test]
fn a_chip_list_shows_whole_chips_and_counts_the_rest() {
    let theme = Theme::default();
    let chip =
        |label: &str| Span::styled(format!(" {label:<8} "), Style::default().bg(Color::Blue));
    let rest = Style::default().fg(Color::DarkGray);
    let cell = Cell::chips(vec![chip("one"), chip("two"), chip("three")], rest);
    assert_eq!(cell.width(), 32);
    let two = cell.render(24, &theme);
    assert_eq!(Cell::plain(&two), " one        two       +1");
    assert_eq!(two.last().map(|span| span.style), Some(rest));
    let one = cell.render(13, &theme);
    assert_eq!(Cell::plain(&one), " one       +2");
    let cut = cell.render(6, &theme);
    assert!(
        text::spans_width(&cut) <= 6,
        "a chip too wide for the column is cut"
    );
    assert!(Cell::plain(&cut).starts_with(" one"));
}

#[test]
fn an_age_cell_is_the_frameworks_age_cell() {
    let theme = Theme::default();
    let age = Age::from_secs(90);
    let cell = Cell::age(Some(age.label()), Some(age.recency()));
    assert_eq!(
        cell.render(4, &theme),
        vec![crate::age::cell(
            Some("1m"),
            Some(Recency::Fresh),
            4,
            &theme
        )]
    );
    assert_eq!(
        Cell::plain(&Cell::age(None, None).render(4, &theme)),
        "   —"
    );
}

#[test]
fn a_snippet_keeps_its_match_in_view_and_paints_it() {
    let theme = Theme::default();
    let body = "a long run of words before the needle and after it";
    let at = body.find("needle").unwrap();
    let matched = Style::default().add_modifier(Modifier::UNDERLINED);
    let cell = Cell::snippet(body, at..at + 6, plain(), matched);
    let cut = cell.render(20, &theme);
    assert!(text::spans_width(&cut) <= 20);
    let painted = cut
        .iter()
        .find(|span| span.style == matched)
        .expect("the match");
    assert_eq!(painted.content, "needle");
}

// --- measure ---------------------------------------------------------------

#[test]
fn the_measure_keeps_each_keys_widest_cell() {
    let schema = ColumnSchema::new([Column::cap(Col::Name, 30), Column::fixed(Col::Size, 4)]);
    let mut measure = Measure::new();
    measure.add(&schema, 0, &[(Col::Name, t("alpha"))]);
    measure.add(&schema, 4, &[(Col::Name, t("be")), (Col::Size, t("9"))]);
    assert_eq!(measure.widest(Col::Name), 5);
    assert_eq!(
        measure.widest_with_lead(Col::Name),
        6,
        "a lead is added: 4 + 2"
    );
    assert!(measure.any(Col::Size));
    assert!(!measure.any(Col::Note));
    assert_eq!(measure.widest(Col::Note), 0);
}

#[test]
fn a_ragged_column_is_widened_only_by_cells_something_follows() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::cap(Col::Note, usize::MAX).ragged(),
        Column::cap(Col::Size, 9),
    ]);
    let measure = measured(
        &schema,
        &[
            vec![
                (Col::Name, t("a")),
                (Col::Note, t("short")),
                (Col::Size, t("(1)")),
            ],
            vec![(Col::Name, t("b")), (Col::Note, t("a much longer note"))],
        ],
    );
    assert_eq!(measure.widest(Col::Note), 5);
    assert!(measure.any(Col::Note));
}

#[test]
fn cells_under_keys_the_schema_lacks_are_measured_too() {
    let schema = ColumnSchema::new([Column::fixed(Col::Mark, 2)]);
    let mut measure = Measure::new();
    measure.add(
        &schema,
        6,
        &[(Col::Mark, t("*")), (Col::Label, t("drawn by the host"))],
    );
    assert_eq!(measure.widest(Col::Label), 17);
    assert_eq!(measure.widest_with_lead(Col::Label), 23);
}

// --- layout ------------------------------------------------------------------

#[test]
fn a_capped_column_with_nothing_to_show_takes_no_cells_and_no_gap() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Mark, 2).gap(0),
        Column::cap(Col::Name, 30),
        Column::fixed(Col::Size, 3),
    ]);
    let rows = [vec![(Col::Name, t("alpha")), (Col::Size, t("1"))]];
    let measure = measured(&schema, &rows);
    assert!(!schema.layout(&measure, 0, 40).is_shown(Col::Mark));
    assert_eq!(draw_text(&schema, &measure, 40, &rows[0]), "alpha  1  ");
    let marked = [
        vec![
            (Col::Mark, t("* ")),
            (Col::Name, t("alpha")),
            (Col::Size, t("1")),
        ],
        vec![(Col::Name, t("beta")), (Col::Size, t("2"))],
    ];
    let measure = measured(&schema, &marked);
    assert_eq!(schema.layout(&measure, 0, 40).width(Col::Mark), Some(2));
    assert_eq!(
        draw_text(&schema, &measure, 40, &marked[1]),
        "  beta   2  ",
        "reserved on every row"
    );
}

#[test]
fn a_fixed_column_is_reserved_even_when_every_cell_is_blank() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 2).gap(0),
        Column::cap(Col::Name, 9),
    ]);
    let rows = [vec![(Col::Name, t("alpha"))]];
    let measure = measured(&schema, &rows);
    assert_eq!(draw_text(&schema, &measure, 20, &rows[0]), "  alpha");
}

#[test]
fn a_stepped_column_steps_with_the_room_left_for_it() {
    const STEPS: &[(usize, usize)] = &[(60, 24), (30, 16), (0, 8)];
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 6).gap(0),
        Column::steps(Col::Name, STEPS),
    ]);
    let measure = Measure::new();
    assert_eq!(schema.layout(&measure, 0, 66).width(Col::Name), Some(24));
    assert_eq!(
        schema.layout(&measure, 0, 65).width(Col::Name),
        Some(16),
        "59 left after the mark"
    );
    assert_eq!(schema.layout(&measure, 0, 20).width(Col::Name), Some(8));
}

#[test]
fn a_column_below_its_room_is_left_out() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::fixed(Col::Size, 4).drop_below(50),
    ]);
    let rows = [vec![(Col::Name, t("alpha")), (Col::Size, t("9"))]];
    let measure = measured(&schema, &rows);
    assert!(schema.layout(&measure, 0, 50).is_shown(Col::Size));
    assert!(
        !schema.layout(&measure, 0, 49).is_shown(Col::Size),
        "there would be room, but not 50"
    );
}

#[test]
fn columns_give_way_lowest_priority_first_and_the_later_of_a_tie_first() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Name, 10).drops(2),
        Column::fixed(Col::Size, 10).drops(1),
        Column::fixed(Col::Age, 10).drops(1),
        Column::fixed(Col::State, 10),
    ]);
    let measure = Measure::new();
    let shown = |room: usize| -> Vec<Col> {
        let layout = schema.layout(&measure, 0, room);
        [Col::Name, Col::Size, Col::Age, Col::State]
            .into_iter()
            .filter(|key| layout.is_shown(*key))
            .collect()
    };
    assert_eq!(shown(46), [Col::Name, Col::Size, Col::Age, Col::State]);
    assert_eq!(
        shown(45),
        [Col::Name, Col::Size, Col::State],
        "the later of priority 1 first"
    );
    assert_eq!(shown(33), [Col::Name, Col::State]);
    assert_eq!(shown(21), [Col::State], "priority 2 last");
    assert_eq!(
        shown(5),
        [Col::State],
        "a column that never gives way stays"
    );
}

#[test]
fn a_shrinking_column_narrows_no_more_than_needed_and_takes_cells_back() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Name, 20).shrinks(0, 8),
        Column::fixed(Col::Size, 10).drops(1),
    ]);
    let measure = Measure::new();
    let at = |room: usize| {
        let layout = schema.layout(&measure, 0, room);
        (layout.width(Col::Name), layout.width(Col::Size))
    };
    assert_eq!(at(32), (Some(20), Some(10)));
    assert_eq!(
        at(28),
        (Some(16), Some(10)),
        "four cells short, four cells narrower"
    );
    assert_eq!(at(20), (Some(8), Some(10)));
    // At its floor the name still does not fit beside the size: the size
    // leaves, and the name takes its cells back.
    assert_eq!(at(19), (Some(19), None));
}

#[test]
fn a_shrinking_column_that_also_drops_narrows_first_and_leaves_later() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::State, 4),
        Column::fixed(Col::Name, 20).shrinks(0, 8).drops(1),
    ]);
    let measure = Measure::new();
    assert_eq!(schema.layout(&measure, 0, 20).width(Col::Name), Some(14));
    assert_eq!(schema.layout(&measure, 0, 13).width(Col::Name), None);
}

#[test]
fn the_fill_takes_what_is_left_and_leaves_below_its_minimum() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Name, 10),
        Column::fill(Col::Fill).drops(0),
    ]);
    let rows = [vec![
        (Col::Name, t("n")),
        (Col::Fill, t("a long note that will not fit")),
    ]];
    let measure = measured(&schema, &rows);
    assert_eq!(schema.layout(&measure, 0, 40).width(Col::Fill), Some(28));
    assert_eq!(
        schema
            .layout(&measure, 0, 10 + GAP + MIN_FILL)
            .width(Col::Fill),
        Some(MIN_FILL)
    );
    assert_eq!(
        schema
            .layout(&measure, 0, 10 + GAP + MIN_FILL - 1)
            .width(Col::Fill),
        None
    );
    let drawn = draw_text(&schema, &measure, 30, &rows[0]);
    assert_eq!(text::width(&drawn), 30);
    assert!(drawn.ends_with(marks::ELLIPSIS), "{drawn}");
}

#[test]
fn a_fill_with_no_rule_narrows_last_rather_than_push_the_row_over() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Name, 10).drops(0),
        Column::fill(Col::Fill),
    ]);
    let rows = [vec![(Col::Name, t("n")), (Col::Fill, t("note"))]];
    let measure = measured(&schema, &rows);
    assert_eq!(
        schema.layout(&measure, 0, 15).width(Col::Fill),
        Some(15),
        "the name leaves first"
    );
    assert_eq!(schema.layout(&measure, 0, 5).width(Col::Fill), Some(5));
}

#[test]
fn a_share_takes_its_part_of_the_whole_row_its_lead_included() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 7).gap(1).drops(0),
        Column::share(Col::Name, 1, 5).within(12, 32),
        Column::fill(Col::Fill),
    ]);
    let rows = [vec![(Col::Name, t("name")), (Col::Fill, t("note"))]];
    let measure = measured(&schema, &rows);
    let name = |lead: usize, room: usize| schema.layout(&measure, lead, room).width(Col::Name);
    assert_eq!(name(0, 100), Some(20), "a fifth of the row");
    assert_eq!(name(0, 40), Some(12), "no fewer than its minimum");
    assert_eq!(name(0, 200), Some(32), "no more than its maximum");
    assert_eq!(
        name(2, 98),
        Some(20),
        "a gutter counts: the same as a row of the whole width"
    );
    assert_eq!(
        name(0, 24),
        Some(12),
        "the mark gave way, and the share stays what the row gives it"
    );
    // A share that comes to nothing takes no cells and no gap.
    let bare = ColumnSchema::new([Column::share(Col::Name, 1, 10), Column::fixed(Col::Size, 3)]);
    let layout = bare.layout(&Measure::new(), 0, 9);
    assert_eq!(
        (layout.width(Col::Name), layout.width(Col::Size)),
        (None, Some(3))
    );
    // After a hugging column the share is still of the whole row.
    let flowing = ColumnSchema::new([Column::hug(Col::Label, 10), Column::share(Col::Name, 1, 4)]);
    let cells = vec![(Col::Label, t("label")), (Col::Name, t("abcdefghijklmnop"))];
    let measure = measured(&flowing, std::slice::from_ref(&cells));
    assert_eq!(
        draw_text(&flowing, &measure, 40, &cells),
        "label  abcdefghi…"
    );
}

/// A mark, a label that takes 45% of what the mark and the word leave
/// (16 to 48 cells, narrowing last), a note that is not worth fewer than
/// 10 cells and hands its cells to the label when it leaves, and a word a
/// third of the whole row (at most 16) kept to the right edge.
fn rest_schema() -> ColumnSchema<Col> {
    ColumnSchema::new([
        Column::fixed(Col::Mark, 2).gap(0),
        Column::share_of_rest(Col::Label, 45, 100)
            .within(16, 48)
            .shrinks(1, 0),
        Column::fill(Col::Note)
            .at_least(10)
            .drops(0)
            .gives_to(Col::Label),
        Column::share(Col::Extra, 1, 3).within(0, 16).right(),
    ])
}

#[test]
fn a_share_of_the_rest_takes_its_part_of_what_the_other_columns_leave() {
    let schema = rest_schema();
    let rows = [vec![(Col::Note, t("a note")), (Col::Extra, t("word"))]];
    let measure = measured(&schema, &rows);
    let at = |lead: usize, room: usize| {
        let layout = schema.layout(&measure, lead, room);
        [Col::Label, Col::Note, Col::Extra].map(|key| layout.width(key))
    };
    // 83 cells after a 2-cell gutter: the word is a third of 85, at most
    // 16; the mark (2), the word, and the gap before it (2) leave 63, and
    // the label takes 45% of them (28). The gap between the label and the
    // note is theirs to split.
    assert_eq!(at(2, 83), [Some(28), Some(33), Some(16)]);
    // Wide rows cap the label at 48.
    assert_eq!(at(2, 158)[0], Some(48));
    // Narrow rows keep the label at 16 while the note keeps its 10.
    assert_eq!(at(2, 50), [Some(16), Some(12), Some(16)]);
    // A column that leaves for the room is not one the share counts.
    let bare = ColumnSchema::new([
        Column::fixed(Col::Mark, 4).drop_below(30),
        Column::share_of_rest(Col::Label, 1, 2),
        Column::fill(Col::Note),
    ]);
    let measure = measured(&bare, &rows);
    assert_eq!(bare.layout(&measure, 0, 40).width(Col::Label), Some(17));
    assert_eq!(bare.layout(&measure, 0, 20).width(Col::Label), Some(10));
}

#[test]
fn a_fill_that_does_not_show_gives_its_cells_to_the_column_it_names() {
    let schema = rest_schema();
    let rows = [vec![(Col::Note, t("a note")), (Col::Extra, t("word"))]];
    let measure = measured(&schema, &rows);
    let at = |room: usize| {
        let layout = schema.layout(&measure, 2, room);
        [Col::Label, Col::Note, Col::Extra].map(|key| layout.width(key))
    };
    // At 48 cells the note gets its 10; at 47 it would get 9: it leaves,
    // and the label takes the row up to the word, past its share.
    assert_eq!(at(48), [Some(16), Some(10), Some(16)]);
    assert_eq!(at(47), [Some(27), None, Some(16)]);
    assert_eq!(at(44), [Some(25), None, Some(15)]);
    // Narrower still, the label narrows last, below its least.
    assert_eq!(at(20), [Some(9), None, Some(7)]);
    // A list with no cell for the note lays every row out without it, and
    // the label takes the cells instead of leaving them blank.
    let blank = measured(&schema, &[vec![(Col::Extra, t("word"))]]);
    let layout = schema.layout(&blank, 2, 83);
    assert_eq!(
        (layout.width(Col::Label), layout.width(Col::Note)),
        (Some(63), None)
    );
    let drawn = Cell::plain(&schema.draw(
        &blank,
        2,
        83,
        &[(Col::Label, t("label")), (Col::Extra, t("word"))],
        &Theme::default(),
    ));
    assert_eq!(
        text::width(&drawn),
        83,
        "the word keeps to the edge: {drawn:?}"
    );
    assert!(drawn.ends_with("  word"), "{drawn:?}");
    // Without one to give them to, a fill that leaves leaves blank cells.
    let plain = ColumnSchema::new([
        Column::cap(Col::Label, 20),
        Column::fill(Col::Note).at_least(10).drops(0),
    ]);
    let measure = measured(
        &plain,
        &[vec![(Col::Label, t("label")), (Col::Note, t("a note"))]],
    );
    let layout = plain.layout(&measure, 0, 14);
    assert_eq!(
        (layout.width(Col::Label), layout.width(Col::Note)),
        (Some(5), None)
    );
}

#[test]
fn a_share_of_the_rest_after_a_hugging_column_splits_what_that_row_leaves() {
    let schema = ColumnSchema::new([
        Column::hug(Col::Name, 20),
        Column::share_of_rest(Col::Label, 1, 2),
        Column::fill(Col::Note),
    ]);
    let cells = vec![
        (Col::Name, t("name")),
        (Col::Label, t("abcdefghijklmnopqrstuvwxyz")),
        (Col::Note, t("the note runs on and on and on")),
    ];
    let measure = measured(&schema, std::slice::from_ref(&cells));
    // 40 cells less the name (4) and its gap (2) leave 34: half is 17.
    assert_eq!(
        draw_text(&schema, &measure, 40, &cells),
        "name  abcdefghijklmnop…  the note runs …"
    );
}

#[test]
fn a_row_stepped_column_steps_with_the_whole_row_whatever_comes_before_it() {
    const STEPS: &[(usize, usize)] = &[(100, 22), (78, 20), (0, 14)];
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 30).drops(0),
        Column::row_steps(Col::Name, STEPS),
        Column::fill(Col::Fill).at_least(50),
    ]);
    let rows = [vec![(Col::Fill, t("note"))]];
    let measure = measured(&schema, &rows);
    let at = |lead: usize, room: usize| {
        let layout = schema.layout(&measure, lead, room);
        (layout.width(Col::Mark), layout.width(Col::Name))
    };
    assert_eq!(
        at(0, 100),
        (None, Some(22)),
        "the mark left; the step stays"
    );
    assert_eq!(at(0, 99), (None, Some(20)));
    assert_eq!(at(4, 96), (None, Some(22)), "the lead counts");
    assert_eq!(at(0, 60), (None, Some(14)));
}

// --- a room beside a bar -----------------------------------------------------

#[test]
fn a_number_of_cells_is_a_room_of_them() {
    let room = Room::from(30);
    assert_eq!(room, Room::new(30));
    assert_eq!((room.cells(), room.whole()), (30, 30));
    let beside = Room::new(30).beside(1);
    assert_eq!((beside.cells(), beside.whole()), (29, 30));
    assert_eq!(Room::new(0).beside(1).cells(), 0);
}

#[test]
fn beside_a_bar_the_width_rules_count_the_whole_room_and_the_fill_gives_the_cell() {
    const STEPS: &[(usize, usize)] = &[(60, 24), (30, 16), (0, 8)];
    const ROW: &[(usize, usize)] = &[(78, 20), (0, 14)];
    let schema = ColumnSchema::new([
        Column::steps(Col::Name, STEPS),
        Column::row_steps(Col::Label, ROW),
        Column::share(Col::Note, 1, 13).within(2, 10),
        Column::fixed(Col::Size, 3).drop_below(78),
        Column::fill(Col::Fill),
    ]);
    let rows = [vec![
        (Col::Name, t("name")),
        (Col::Label, t("label")),
        (Col::Note, t("note")),
        (Col::Size, t("9")),
        (
            Col::Fill,
            t("a long note that runs on past the end of every row"),
        ),
    ]];
    let measure = measured(&schema, &rows);
    let widths = |room: Room| {
        let layout = schema.layout(&measure, 0, room);
        [Col::Name, Col::Label, Col::Note, Col::Size, Col::Fill].map(|key| layout.width(key))
    };
    let whole = widths(Room::new(78));
    assert_eq!(&whole[..4], [Some(24), Some(20), Some(6), Some(3)]);
    // One cell narrower, every rule steps down: the name's room left, the
    // label's row, the note's share, and the size's room below.
    assert_eq!(
        &widths(Room::new(77))[..4],
        [Some(24), Some(14), Some(5), None]
    );
    // Beside a bar's column, every rule sees the whole room and the fill
    // gives the bar its cell.
    let beside = widths(Room::new(78).beside(1));
    assert_eq!(beside[..4], whole[..4]);
    assert_eq!(beside[4], whole[4].map(|fill| fill - 1));
    let drawn =
        |room: Room| Cell::plain(&schema.draw(&measure, 0, room, &rows[0], &Theme::default()));
    let (wide, narrow) = (drawn(Room::new(78)), drawn(Room::new(78).beside(1)));
    assert_eq!(text::width(&wide), 78);
    assert_eq!(text::width(&narrow), 77);
    assert_eq!(wide[..40], narrow[..40], "{wide:?} vs {narrow:?}");
}

#[test]
fn beside_a_bar_a_fill_at_its_minimum_gives_way_by_the_policy() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 20),
        Column::fixed(Col::Size, 4).right().drops(1),
        Column::fill(Col::Fill).drops(0),
    ]);
    let rows = [vec![
        (Col::Name, t("alpha")),
        (Col::Size, t("12k")),
        (Col::Fill, t("a note too long for the row")),
    ]];
    let measure = measured(&schema, &rows);
    // Five, two, four, two, and the fill's seven: 20 cells.
    let shown = |room: Room| {
        let layout = schema.layout(&measure, 0, room);
        (layout.is_shown(Col::Size), layout.is_shown(Col::Fill))
    };
    assert_eq!(shown(Room::new(20)), (true, true));
    assert_eq!(
        shown(Room::new(20).beside(1)),
        (true, false),
        "the fill cannot give a cell below its minimum, so it leaves first"
    );
    // A row with no fill gives the bar its cell by the policy too.
    let bare = ColumnSchema::new([
        Column::cap(Col::Name, 20).shrinks(0, 3),
        Column::fixed(Col::Size, 4).right(),
    ]);
    let rows = [vec![(Col::Name, t("alpha")), (Col::Size, t("12k"))]];
    let measure = measured(&bare, &rows);
    let row = |room: Room| Cell::plain(&bare.draw(&measure, 0, room, &rows[0], &Theme::default()));
    assert_eq!(row(Room::new(11)), "alpha   12k");
    assert_eq!(row(Room::new(11).beside(1)), "alp…   12k");
}

#[test]
fn a_fill_wants_its_own_minimum() {
    let schema =
        |fill: Column<Col>| ColumnSchema::new([Column::fixed(Col::Name, 10).drops(0), fill]);
    let rows = [vec![(Col::Name, t("n")), (Col::Fill, t("a note"))]];
    let wide = schema(Column::fill(Col::Fill).at_least(20));
    let measure = measured(&wide, &rows);
    let layout = wide.layout(&measure, 0, 32);
    assert_eq!(
        (layout.width(Col::Name), layout.width(Col::Fill)),
        (Some(10), Some(20))
    );
    let layout = wide.layout(&measure, 0, 31);
    assert_eq!(
        (layout.width(Col::Name), layout.width(Col::Fill)),
        (None, Some(31)),
        "the name gives way for the fill's twenty cells"
    );
    let plain = schema(Column::fill(Col::Fill));
    let measure = measured(&plain, &rows);
    assert_eq!(
        plain
            .layout(&measure, 0, 10 + GAP + MIN_FILL)
            .width(Col::Name),
        Some(10),
        "a fill not told wants seven"
    );
}

// --- drawing -----------------------------------------------------------------

#[test]
fn a_row_ends_at_its_last_cell_and_pads_the_blank_ones_before_it() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::cap(Col::Note, 30),
        Column::cap(Col::Size, 9).right(),
    ]);
    let rows = [
        vec![
            (Col::Name, t("alpha")),
            (Col::Note, t("first")),
            (Col::Size, t("(12)")),
        ],
        vec![(Col::Name, t("be")), (Col::Size, t("(3)"))],
        vec![(Col::Name, t("gamma")), (Col::Note, t("x"))],
    ];
    let measure = measured(&schema, &rows);
    assert_eq!(
        draw_text(&schema, &measure, 60, &rows[0]),
        "alpha  first  (12)"
    );
    assert_eq!(
        draw_text(&schema, &measure, 60, &rows[1]),
        "be             (3)",
        "a blank note is padded; the count keeps to the right"
    );
    assert_eq!(
        draw_text(&schema, &measure, 60, &rows[2]),
        "gamma  x    ",
        "no gap after the last cell"
    );
}

#[test]
fn a_text_cell_is_padded_in_its_own_style_and_a_gap_in_none() {
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let schema = ColumnSchema::new([Column::cap(Col::Name, 30), Column::fixed(Col::Size, 3)]);
    let rows = [
        vec![(Col::Name, Cell::text("al", bold)), (Col::Size, t("1"))],
        vec![(Col::Name, Cell::text("alpha", bold)), (Col::Size, t("2"))],
    ];
    let measure = measured(&schema, &rows);
    let spans = schema.draw(&measure, 0, 30, &rows[0], &Theme::default());
    assert_eq!(spans[0], Span::styled("al", bold));
    assert_eq!(spans[1], Span::styled("   ", bold));
    assert_eq!(spans[2], Span::raw("  "));
}

#[test]
fn a_ragged_cell_that_ends_its_row_runs_on_into_the_room() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::cap(Col::Note, usize::MAX).ragged(),
        Column::cap(Col::Size, 9),
    ]);
    let rows = [
        vec![
            (Col::Name, t("a")),
            (Col::Note, t("short")),
            (Col::Size, t("(1)")),
        ],
        vec![(Col::Name, t("b")), (Col::Note, t("a much longer note"))],
    ];
    let measure = measured(&schema, &rows);
    assert_eq!(draw_text(&schema, &measure, 40, &rows[0]), "a  short  (1)");
    assert_eq!(
        draw_text(&schema, &measure, 40, &rows[1]),
        "b  a much longer note"
    );
    assert_eq!(draw_text(&schema, &measure, 12, &rows[1]), "b  a much l…");
}

#[test]
fn a_ragged_column_that_gave_way_still_ends_a_row_with_nothing_after_it() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::cap(Col::Note, usize::MAX)
            .ragged()
            .shrinks(0, 8)
            .drops(0),
        Column::cap(Col::Size, 9),
    ]);
    let rows = [
        vec![
            (Col::Name, t("first")),
            (Col::Note, t("a note long enough to give way")),
            (Col::Size, t("(1)")),
        ],
        vec![
            (Col::Name, t("second")),
            (Col::Note, t("another long note")),
        ],
    ];
    let measure = measured(&schema, &rows);
    // Too narrow for the note beside the count: it leaves the row that
    // has a count, and still ends the row that has none.
    assert_eq!(draw_text(&schema, &measure, 14, &rows[0]), "first   (1)");
    assert_eq!(
        draw_text(&schema, &measure, 20, &rows[1]),
        "second  another lon…"
    );
    // Unless what is left would not hold its floor.
    assert_eq!(draw_text(&schema, &measure, 14, &rows[1]), "second");
}

#[test]
fn a_column_that_absorbs_the_lead_ends_in_one_place_at_every_depth() {
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 30),
        Column::cap(Col::Note, 30).absorbs_lead(),
        Column::cap(Col::Size, 9),
    ]);
    let top = vec![
        (Col::Name, t("top")),
        (Col::Note, t("path")),
        (Col::Size, t("(1)")),
    ];
    let nested = vec![
        (Col::Name, t("deep")),
        (Col::Note, t("path")),
        (Col::Size, t("(2)")),
    ];
    let mut measure = Measure::new();
    measure.add(&schema, 0, &top);
    measure.add(&schema, 2, &nested);
    let indent = |lead: usize, cells: &Cells<Col>| {
        format!(
            "{}{}",
            " ".repeat(lead),
            Cell::plain(&schema.draw(&measure, lead, 40, cells, &Theme::default()))
        )
    };
    let (top, nested) = (indent(0, &top), indent(2, &nested));
    assert_eq!(top.find("(1)"), nested.find("(2)"), "\n{top}\n{nested}");
    // The name before it moves right with the indent; the column after
    // it does not.
    assert_eq!(top.find("top"), Some(0));
    assert_eq!(nested.find("deep"), Some(2));
}

#[test]
fn a_capped_column_that_absorbs_the_lead_ends_in_one_place_past_its_cap() {
    // The cap bounds each cell: a long top-level name is cut to 8 cells,
    // and the deeper row's column still ends where the top row's does.
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 8).absorbs_lead(),
        Column::cap(Col::Size, 9),
    ]);
    let top = vec![(Col::Name, t("a-long-top-name")), (Col::Size, t("(1)"))];
    let nested = vec![(Col::Name, t("deep")), (Col::Size, t("(2)"))];
    let deepest = vec![(Col::Name, t("deeper-name")), (Col::Size, t("(3)"))];
    let mut measure = Measure::new();
    measure.add(&schema, 0, &top);
    measure.add(&schema, 2, &nested);
    measure.add(&schema, 4, &deepest);
    assert_eq!(measure.widest(Col::Name), 8, "each cell counts at most 8");
    assert_eq!(measure.widest_with_lead(Col::Name), 12);
    let indent = |lead: usize, cells: &Cells<Col>| {
        format!(
            "{}{}",
            " ".repeat(lead),
            Cell::plain(&schema.draw(&measure, lead, 40 - lead, cells, &Theme::default()))
        )
    };
    let rows = [indent(0, &top), indent(2, &nested), indent(4, &deepest)];
    assert_eq!(rows[0], format!("a-long-{}      (1)", marks::ELLIPSIS));
    assert_eq!(rows[1].find("(2)"), Some(14), "{rows:#?}");
    assert_eq!(rows[2], format!("    deeper-{}  (3)", marks::ELLIPSIS));
}

#[test]
fn rows_that_absorb_the_lead_give_way_alike_at_every_depth() {
    // A label that absorbs the lead and narrows to a floor, a word that
    // stays, and an age that leaves: at every room the deeper row loses
    // what the top row loses and keeps its word in the top row's column,
    // even once its own label is below the floor.
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 28).absorbs_lead().shrinks(2, 8),
        Column::cap(Col::State, 9),
        Column::fixed(Col::Age, 4).drops(3),
    ]);
    let top = vec![
        (Col::Name, t("project-main")),
        (Col::State, t("live")),
        (Col::Age, t("5m")),
    ];
    let nested = vec![
        (Col::Name, t("nested-entry-a")),
        (Col::State, t("in use")),
        (Col::Age, t("1h")),
    ];
    let mut measure = Measure::new();
    measure.add(&schema, 0, &top);
    measure.add(&schema, 4, &nested);
    let theme = Theme::default();
    let column = |line: &str, word: &str| line.find(word).map(|at| line[..at].chars().count());
    // From the narrowest room the deeper row's word still fits in whole.
    for width in 16..40 {
        let row = |lead: usize, cells: &Cells<Col>| {
            let room = width - lead;
            format!(
                "{}{}",
                " ".repeat(lead),
                Cell::plain(&schema.draw(&measure, lead, room, cells, &theme))
            )
        };
        let (top, nested) = (row(0, &top), row(4, &nested));
        assert_eq!(
            column(&top, "live"),
            column(&nested, "in use"),
            "at {width}:\n{top}\n{nested}"
        );
        assert!(column(&top, "live").is_some(), "{top}");
        assert_eq!(
            top.contains("5m"),
            nested.contains("1h"),
            "at {width}:\n{top}\n{nested}"
        );
        assert_eq!(
            schema.layout(&measure, 0, width).is_shown(Col::Age),
            schema.layout(&measure, 4, width - 4).is_shown(Col::Age)
        );
    }
}

#[test]
fn a_lead_every_row_shares_is_not_absorbed() {
    // A list that draws a two-cell gutter before every row lays each row
    // out as it would with no gutter in a room two cells narrower: the
    // column that absorbs the lead gives back only a row's own indent, so
    // the top row's label keeps its floor.
    let schema = ColumnSchema::new([
        Column::cap(Col::Name, 28).absorbs_lead().shrinks(2, 8),
        Column::cap(Col::State, 9),
        Column::fixed(Col::Age, 4).drops(3),
    ]);
    let top = vec![
        (Col::Name, t("project-main")),
        (Col::State, t("live")),
        (Col::Age, t("5m")),
    ];
    let nested = vec![
        (Col::Name, t("nested-entry-a")),
        (Col::State, t("in use")),
        (Col::Age, t("1h")),
    ];
    let (mut bare, mut guttered) = (Measure::new(), Measure::new());
    for (lead, cells) in [(0, &top), (4, &nested)] {
        bare.add(&schema, lead, cells);
        guttered.add(&schema, GUTTER_WIDTH + lead, cells);
    }
    assert_eq!(bare.least_lead(), 0);
    assert_eq!(guttered.least_lead(), GUTTER_WIDTH);
    let theme = Theme::default();
    for room in 12..40 {
        for (lead, cells) in [(0, &top), (4, &nested)] {
            let without = Cell::plain(&schema.draw(&bare, lead, room, cells, &theme));
            let with =
                Cell::plain(&schema.draw(&guttered, GUTTER_WIDTH + lead, room, cells, &theme));
            assert_eq!(with, without, "at {room} after a lead of {lead}");
        }
    }
    let narrow = Cell::plain(&schema.draw(&guttered, GUTTER_WIDTH, 14, &top, &theme));
    assert_eq!(narrow, format!("project{}  live", marks::ELLIPSIS));
}

#[test]
fn draw_split_parts_a_row_where_a_column_starts() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 2).gap(0),
        Column::cap(Col::Name, 12),
        Column::fixed(Col::Size, 4).right().drops(1),
        Column::fill(Col::Note).drops(0),
    ]);
    let rows = [
        vec![
            (Col::Mark, t("*")),
            (Col::Name, t("alpha")),
            (Col::Size, t("12k")),
            (Col::Note, t("a note")),
        ],
        vec![(Col::Mark, t("*")), (Col::Name, t("beta"))],
    ];
    let measure = measured(&schema, &rows);
    let theme = Theme::default();
    let split = |row: usize, room: usize, at: Col| {
        let (head, rest) = schema.draw_split(&measure, 0, room, &rows[row], &theme, at);
        (Cell::plain(&head), Cell::plain(&rest))
    };
    assert_eq!(
        split(0, 40, Col::Name),
        ("* ".into(), "alpha   12k  a note".into())
    );
    // The gap before the column stays with the part before it.
    assert_eq!(
        split(0, 40, Col::Size),
        ("* alpha  ".into(), " 12k  a note".into())
    );
    // A column that gave way: the part starts at the next that shows.
    assert_eq!(
        split(0, 15, Col::Note),
        ("* alpha   12k".into(), String::new())
    );
    assert_eq!(split(0, 12, Col::Size), ("* alpha".into(), String::new()));
    // A blank cell starts nothing; a row with nothing after it has an
    // empty second part.
    assert_eq!(split(1, 40, Col::Size), ("* beta ".into(), String::new()));
    // Joined, the parts are the row.
    for room in [0, 5, 9, 15, 40] {
        let (mut head, rest) = schema.draw_split(&measure, 0, room, &rows[0], &theme, Col::Name);
        head.extend(rest);
        assert_eq!(head, schema.draw(&measure, 0, room, &rows[0], &theme));
    }
}

#[test]
fn a_hugging_cell_takes_its_own_width_and_the_cells_after_it_flow() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::State, 1),
        Column::hug(Col::Label, 12).shrinks(2, 4).drops(3),
        Column::hug(Col::Note, 20).drops(1),
        Column::fill(Col::Fill).drops(0),
    ]);
    let rows = [
        vec![
            (Col::State, t("*")),
            (Col::Label, t("named")),
            (Col::Note, t("note")),
            (Col::Fill, t("the rest of it")),
        ],
        vec![(Col::State, t("*")), (Col::Fill, t("the rest of it"))],
        vec![
            (Col::State, t("*")),
            (Col::Label, t("a label too long to fit")),
        ],
    ];
    let measure = measured(&schema, &rows);
    assert_eq!(
        draw_text(&schema, &measure, 40, &rows[0]),
        "*  named  note  the rest of it"
    );
    assert_eq!(
        draw_text(&schema, &measure, 40, &rows[1]),
        "*  the rest of it",
        "a blank hugging cell takes no gap"
    );
    assert_eq!(
        draw_text(&schema, &measure, 40, &rows[2]),
        "*  a label too…",
        "cut at its own cap"
    );
    // Row by row: the fill leaves first, then the note, then the label narrows.
    assert_eq!(draw_text(&schema, &measure, 20, &rows[0]), "*  named  note");
    assert_eq!(draw_text(&schema, &measure, 12, &rows[0]), "*  named");
    assert_eq!(draw_text(&schema, &measure, 8, &rows[0]), "*  named");
    assert_eq!(draw_text(&schema, &measure, 7, &rows[0]), "*  nam…");
    assert_eq!(
        draw_text(&schema, &measure, 6, &rows[0]),
        "*",
        "below its floor the label leaves"
    );
    assert_eq!(
        draw_text(&schema, &measure, 20, &rows[1]),
        "*  the rest of it",
        "another row keeps its fill"
    );
}

#[test]
fn a_rows_last_cell_needs_only_its_own_text() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::State, 1),
        Column::hug(Col::Label, 20),
        Column::fixed(Col::Note, 16).shrinks(1, 4).drops(1),
        Column::fill(Col::Fill).drops(0),
    ]);
    let rows = [
        vec![
            (Col::State, t("*")),
            (Col::Label, t("label")),
            (Col::Note, t("short")),
        ],
        vec![
            (Col::State, t("*")),
            (Col::Label, t("label")),
            (Col::Note, t("a longer project name")),
        ],
        vec![
            (Col::State, t("*")),
            (Col::Label, t("label")),
            (Col::Fill, t("ok")),
        ],
    ];
    let measure = measured(&schema, &rows);
    // Sixteen cells do not fit after the label, but the five of its text
    // do: its pad is cut at the edge, with no mark for cut spaces.
    assert_eq!(
        draw_text(&schema, &measure, 17, &rows[0]),
        "*  label  short  "
    );
    // A longer one narrows to what is left, with the mark.
    assert_eq!(
        draw_text(&schema, &measure, 17, &rows[1]),
        "*  label  a long…"
    );
    // A fill whose whole text fits shows though it gets fewer than
    // MIN_FILL cells.
    assert_eq!(draw_text(&schema, &measure, 12, &rows[2]), "*  label  ok");
}

#[test]
fn a_row_too_wide_for_what_never_gives_way_is_cut_at_its_edge() {
    let schema = ColumnSchema::new([Column::fixed(Col::Name, 12), Column::fixed(Col::Size, 6)]);
    let rows = [vec![(Col::Name, t("alpha-beta")), (Col::Size, t("123456"))]];
    let measure = measured(&schema, &rows);
    let drawn = draw_text(&schema, &measure, 15, &rows[0]);
    assert_eq!(drawn, format!("alpha-beta    {}", marks::ELLIPSIS));
}

#[test]
fn draw_then_pads_every_column_and_says_what_is_left() {
    let schema = ColumnSchema::new([
        Column::fixed(Col::Mark, 2).gap(0),
        Column::cap(Col::Extra, 2).gap(0),
    ]);
    let rows = [
        vec![(Col::Mark, t("v")), (Col::Extra, t("* "))],
        vec![(Col::Mark, t("v"))],
    ];
    let measure = measured(&schema, &rows);
    let (spans, left) = schema.draw_then(&measure, 0, 30, &rows[1], &Theme::default());
    assert_eq!(
        Cell::plain(&spans),
        "v   ",
        "the blank mark is padded: the host's cells follow"
    );
    assert_eq!(left, 26);
}

// --- properties -----------------------------------------------------------------

/// A small deterministic generator for the property checks.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }

    fn word(&mut self) -> String {
        const WORDS: &[&str] = &[
            "a",
            "bb",
            "ccc",
            "dddd",
            "eeeee",
            "東京",
            "naïve",
            "longer-word",
            "",
        ];
        let mut out = String::new();
        for _ in 0..self.below(4) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(WORDS[self.below(WORDS.len())]);
        }
        out
    }
}

fn random_schema(rng: &mut Rng) -> ColumnSchema<Col> {
    const STEPS: &[(usize, usize)] = &[(50, 12), (20, 8), (0, 4)];
    let keys = [
        Col::Mark,
        Col::Name,
        Col::Size,
        Col::Age,
        Col::State,
        Col::Label,
        Col::Note,
        Col::Fill,
    ];
    let mut columns = Vec::new();
    for (index, key) in keys.into_iter().enumerate() {
        if rng.below(4) == 0 {
            continue;
        }
        let mut column = match (key, rng.below(7)) {
            (Col::Fill, 0) => Column::fill(key).at_least(rng.below(24)),
            (Col::Fill, 1) => Column::fill(key)
                .at_least(rng.below(16))
                .gives_to(Col::Name),
            (Col::Fill, _) => Column::fill(key),
            (_, 0) => Column::fixed(key, 1 + rng.below(8)),
            (_, 1) => Column::steps(key, STEPS),
            (_, 2) if index > 3 => Column::hug(key, 2 + rng.below(12)),
            (_, 3) => Column::share_of_rest(key, 1, 1 + rng.below(4))
                .within(rng.below(6), 4 + rng.below(30)),
            (_, 5) => {
                Column::share(key, 1, 1 + rng.below(6)).within(rng.below(6), 4 + rng.below(20))
            }
            (_, 6) => Column::row_steps(key, STEPS),
            _ => Column::cap(key, 2 + rng.below(16)),
        };
        column = column.gap(rng.below(3));
        let priority = u8::try_from(rng.below(5)).unwrap_or(0);
        match rng.below(4) {
            0 => column = column.drops(priority),
            1 => column = column.shrinks(priority, rng.below(5)),
            2 => column = column.shrinks(priority, rng.below(5)).drops(priority + 1),
            _ => {}
        }
        if rng.below(6) == 0 {
            column = column.right();
        }
        if rng.below(8) == 0 {
            column = column.ragged();
        }
        columns.push(column);
    }
    ColumnSchema::new(columns)
}

fn random_cells(rng: &mut Rng, schema: &ColumnSchema<Col>) -> Cells<Col> {
    schema
        .columns()
        .iter()
        .map(|column| (column.key(), t(&rng.word())))
        .collect()
}

#[test]
fn no_row_is_ever_wider_than_its_room() {
    let theme = Theme::default();
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let schema = random_schema(&mut rng);
        let rows: Vec<Cells<Col>> = (0..6).map(|_| random_cells(&mut rng, &schema)).collect();
        let measure = measured(&schema, &rows);
        let keys: Vec<Col> = schema.columns().iter().map(Column::key).collect();
        for room in 0..60 {
            for cells in &rows {
                let spans = schema.draw(&measure, 0, room, cells, &theme);
                assert!(
                    text::spans_width(&spans) <= room,
                    "seed {seed}, room {room}: {:?}\n{schema:?}",
                    Cell::plain(&spans)
                );
                // Parted anywhere, the row is the same row.
                if let Some(&at) = keys.get(rng.below(keys.len().max(1))) {
                    let (mut head, rest) = schema.draw_split(&measure, 0, room, cells, &theme, at);
                    head.extend(rest);
                    assert_eq!(head, spans, "seed {seed}, room {room}, parted at {at:?}");
                }
                // Beside a bar's column, the row fits the cells it leaves.
                let beside = Room::new(room + 1).beside(1);
                let spans = schema.draw(&measure, 0, beside, cells, &theme);
                assert!(
                    text::spans_width(&spans) <= room,
                    "seed {seed}, room {room} beside a bar: {:?}\n{schema:?}",
                    Cell::plain(&spans)
                );
            }
        }
    }
}

#[test]
fn lined_up_columns_start_in_one_place_on_every_row_of_a_room() {
    let theme = Theme::default();
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03));
        let schema = random_schema(&mut rng);
        let rows: Vec<Cells<Col>> = (0..6).map(|_| random_cells(&mut rng, &schema)).collect();
        let measure = measured(&schema, &rows);
        for room in 10..60 {
            let layout = schema.layout(&measure, 0, room);
            // Where each lined-up column starts, from the widths and gaps
            // of the ones before it.
            let mut start = 0;
            let mut gap = 0;
            let mut starts = Vec::new();
            let mut fits = true;
            for column in &schema.columns()[..schema.split] {
                if let Some(width) = layout.width(column.key()) {
                    start += gap;
                    starts.push((column.key(), start, width));
                    start += width;
                    gap = column.gap;
                }
            }
            if start > room {
                fits = false;
            }
            for cells in &rows {
                let drawn = Cell::plain(&schema.draw(&measure, 0, room, cells, &theme));
                let drawn: Vec<char> = drawn.chars().collect();
                if !fits || drawn.iter().any(|c| !c.is_ascii()) {
                    continue;
                }
                for (key, at, width) in &starts {
                    let Some(cell) = cells
                        .iter()
                        .find(|(have, _)| have == key)
                        .map(|(_, cell)| cell)
                    else {
                        continue;
                    };
                    let shown = Cell::plain(&cell.render(*width, &theme));
                    if shown.is_empty() || !shown.is_ascii() || at + width > drawn.len() {
                        continue;
                    }
                    let column = schema.column(*key).unwrap();
                    let slot: String = drawn[*at..at + width].iter().collect();
                    // The cell sits at one edge of its slot (the start, or
                    // the end when padded to the right), wherever the row.
                    let found = slot.starts_with(&shown) || slot.ends_with(&shown) || column.ragged;
                    assert!(
                        found,
                        "seed {seed}, room {room}, {key:?} at {at}: {slot:?} vs {shown:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn beside_a_bar_the_lined_up_columns_keep_their_widths_while_the_fill_has_a_cell_to_give() {
    let mut checked = 0;
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0xA24B_AED4_963E_E407));
        let schema = random_schema(&mut rng);
        let rows: Vec<Cells<Col>> = (0..6).map(|_| random_cells(&mut rng, &schema)).collect();
        let measure = measured(&schema, &rows);
        let Some(fill) = schema.column(Col::Fill).filter(|column| {
            schema.columns()[..schema.split]
                .iter()
                .any(|lined| lined.key() == column.key())
        }) else {
            continue;
        };
        for room in 2..60 {
            let whole = schema.layout(&measure, 0, room);
            let beside = schema.layout(&measure, 0, Room::new(room).beside(1));
            let Some(width) = whole
                .width(Col::Fill)
                .filter(|width| *width > fill.fill_min)
            else {
                continue;
            };
            checked += 1;
            for column in &schema.columns()[..schema.split] {
                let key = column.key();
                let expected = if key == Col::Fill {
                    Some(width - 1)
                } else {
                    whole.width(key)
                };
                assert_eq!(
                    beside.width(key),
                    expected,
                    "seed {seed}, room {room}, {key:?}\n{schema:?}"
                );
            }
        }
    }
    assert!(
        checked > 100,
        "only {checked} rooms had a fill to give a cell"
    );
}
