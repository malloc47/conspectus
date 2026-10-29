use super::*;

/// A result row's plan, drawn after a 2-cell cursor gutter: where it was
/// found (7), a badge (10), a name that is a fifth of the row (12 to 32),
/// a count whose width steps with the row, an age (4, at the end), and an
/// excerpt that keeps 20 cells. The badge drops first, then the place,
/// then the age; the name narrows last, to 8.
fn result_plan(badge: usize) -> ColumnPlan {
    const COUNT: &[(usize, usize)] = &[(100, 22), (78, 20), (0, 14)];
    ColumnPlan::new([
        Column::fixed(7).gap(1).drops(1),
        Column::fixed(badge).gap(1).drops(0),
        Column::share(1, 5).within(12, 32).gap(2).shrinks(3, 8),
        Column::steps(COUNT).gap(2),
        Column::fixed(4).gap(1).drops(2).end(),
        Column::fill(20),
    ])
    .lead(2)
}

/// The same grid written out by hand, as a host computed it before it had
/// a plan: `(place, badge, name, count, age, excerpt)`.
fn by_hand(width: usize, badge: usize) -> [usize; 6] {
    let count = match width {
        100.. => 22,
        78.. => 20,
        _ => 14,
    };
    let mut cells = [7, badge, (width / 5).clamp(12, 32), count, 4];
    let excerpt = |cells: &[usize; 5]| {
        let shown = [cells[0], cells[1], cells[4]];
        let gaps = shown.iter().filter(|cell| **cell > 0).count();
        width.saturating_sub(2 + shown.iter().sum::<usize>() + gaps + cells[2] + cells[3] + 2 + 2)
    };
    for column in [1, 0, 4] {
        if excerpt(&cells) < 20 {
            cells[column] = 0;
        }
    }
    while excerpt(&cells) < 20 && cells[2] > 8 {
        cells[2] -= 1;
    }
    [
        cells[0],
        cells[1],
        cells[2],
        cells[3],
        cells[4],
        excerpt(&cells),
    ]
}

fn widths(layout: &ColumnLayout) -> [usize; 6] {
    std::array::from_fn(|column| layout.width(column))
}

#[test]
fn a_plan_reproduces_a_hand_written_grid_at_every_width() {
    for badge in [5, 10, 14] {
        for width in 0..=240 {
            assert_eq!(
                widths(&result_plan(badge).layout(width)),
                by_hand(width, badge),
                "badge {badge}, width {width}"
            );
        }
    }
}

#[test]
fn columns_give_way_in_their_order_not_the_plans() {
    let plan = result_plan(10);
    let wide = plan.layout(200);
    assert!((0..5).all(|column| wide.is_shown(column)), "{wide:?}");
    assert_eq!(wide.width(2), 32, "the share stops at its maximum");
    // The badge (order 0) goes before the place (order 1), which sits
    // before it in the row.
    let mid = plan.layout(83);
    assert_eq!((mid.width(0), mid.width(1), mid.width(4)), (7, 0, 4));
    let narrow = plan.layout(60);
    assert_eq!(
        (narrow.width(0), narrow.width(1), narrow.width(4)),
        (0, 0, 4)
    );
    let tiny = plan.layout(40);
    assert_eq!((tiny.width(0), tiny.width(1), tiny.width(4)), (0, 0, 0));
    assert!(tiny.width(2) < 12, "the name narrows once nothing can drop");
}

#[test]
fn a_dropped_column_takes_its_gap_with_it() {
    let plan = ColumnPlan::new([
        Column::fixed(3).gap(4).drops(0),
        Column::fixed(2).gap(1),
        Column::fill(10),
    ]);
    let roomy = plan.layout(30);
    assert_eq!((roomy.gap(0), roomy.fill()), (4, 30 - 3 - 4 - 2 - 1));
    let short = plan.layout(14);
    assert!(!short.is_shown(0));
    assert_eq!((short.width(0), short.gap(0)), (0, 0));
    assert_eq!(short.fill(), 14 - 2 - 1, "the gap left with the column");
    assert_eq!(short.fit(0, "abc"), None);
}

#[test]
fn a_column_shrinks_only_as_far_as_the_fill_needs_and_never_past_its_floor() {
    let plan = ColumnPlan::new([Column::fixed(20).gap(1).shrinks(0, 6), Column::fill(10)]);
    assert_eq!(plan.layout(31).width(0), 20, "room enough");
    assert_eq!(plan.layout(27).width(0), 16, "one cell at a time");
    let floor = plan.layout(12);
    assert_eq!((floor.width(0), floor.fill()), (6, 5), "the floor holds");
}

#[test]
fn steps_and_shares_follow_the_row_width_the_lead_included() {
    const STEPS: &[(usize, usize)] = &[(50, 9), (20, 5)];
    let plan = ColumnPlan::new([
        Column::steps(STEPS).gap(1),
        Column::share(1, 4).within(3, 10),
        Column::fill(0),
    ])
    .lead(2);
    let at = |width| {
        let layout = plan.layout(width);
        (layout.width(0), layout.width(1), layout.fill())
    };
    assert_eq!(at(60), (9, 10, 60 - 2 - 10 - 10));
    assert_eq!(at(49), (5, 10, 49 - 2 - 6 - 10));
    assert_eq!(at(19), (0, 4, 19 - 2 - 4), "below every step: no cells");
    assert_eq!(at(8), (0, 3, 3), "the share's minimum");
}

#[test]
fn a_share_of_nothing_and_bounds_out_of_order_do_not_panic() {
    let plan = ColumnPlan::new([Column::share(1, 0).within(4, 2), Column::fill(0)]);
    let layout = plan.layout(20);
    assert_eq!(layout.width(0), 2, "the maximum wins");
}

#[test]
fn a_plan_without_a_fill_gives_way_only_when_it_overflows() {
    let plan = ColumnPlan::new([
        Column::fixed(10).gap(1).drops(0),
        Column::fixed(10).shrinks(1, 4),
    ]);
    assert_eq!(
        plan.layout(30).fill(),
        0,
        "no fill, nothing left over is told"
    );
    let fits = plan.layout(21);
    assert_eq!((fits.width(0), fits.width(1)), (10, 10));
    let over = plan.layout(15);
    assert_eq!((over.width(0), over.width(1)), (0, 10));
    let tight = plan.layout(6);
    assert_eq!(tight.width(1), 6);
}

#[test]
fn only_the_first_fill_takes_the_rest() {
    let plan = ColumnPlan::new([Column::fill(0).gap(1), Column::fill(5)]);
    let layout = plan.layout(20);
    assert_eq!((layout.width(0), layout.fill()), (19, 19));
    assert!(!layout.is_shown(1), "a second fill gets nothing");
}

#[test]
fn the_fill_is_shown_even_with_no_cells_left() {
    let plan = ColumnPlan::new([Column::fixed(10).gap(2), Column::fill(4).gap(1)]);
    let layout = plan.layout(9);
    assert!(layout.is_shown(1));
    assert_eq!((layout.fill(), layout.gap(1)), (0, 1));
    assert_eq!(layout.fit(1, "abc").as_deref(), Some(" "));
}

#[test]
fn fit_cuts_pads_and_aligns_then_adds_the_gap() {
    let plan = ColumnPlan::new([
        Column::fixed(6).gap(2),
        Column::fixed(4).gap(1).end(),
        Column::fill(0),
    ]);
    let layout = plan.layout(40);
    assert_eq!(layout.fit(0, "ab").as_deref(), Some("ab      "));
    assert_eq!(layout.fit(0, "abcdefgh").as_deref(), Some("abcde…  "));
    assert_eq!(layout.fit(1, "3d").as_deref(), Some("  3d "));
    assert_eq!(layout.fit(1, "12345").as_deref(), Some("123… "));
    assert_eq!(
        layout.fit(0, "東京都").as_deref(),
        Some("東京都  "),
        "measured in cells"
    );
    assert_eq!(layout.fit(9, "x"), None, "no such column");
    assert_eq!((layout.width(9), layout.gap(9)), (0, 0));
}

#[test]
fn an_empty_plan_lays_out_to_nothing() {
    let plan = ColumnPlan::default();
    assert!(plan.is_empty());
    assert_eq!(plan.len(), 0);
    let layout = plan.layout(30);
    assert_eq!((layout.fill(), layout.width(0)), (0, 0));
    assert_eq!(result_plan(10).len(), 6);
}
