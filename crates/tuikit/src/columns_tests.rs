use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::*;
use crate::age::{Age, Recency};
use crate::glyphs::marks;
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
        let mut column = match (key, rng.below(5)) {
            (Col::Fill, _) => Column::fill(key),
            (_, 0) => Column::fixed(key, 1 + rng.below(8)),
            (_, 1) => Column::steps(key, STEPS),
            (_, 2) if index > 3 => Column::hug(key, 2 + rng.below(12)),
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
        for room in 0..60 {
            for cells in &rows {
                let spans = schema.draw(&measure, 0, room, cells, &theme);
                assert!(
                    text::spans_width(&spans) <= room,
                    "seed {seed}, room {room}: {:?}\n{schema:?}",
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
