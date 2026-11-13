use ratatui::layout::Rect;

use super::*;

#[test]
fn the_threshold_is_the_narrowest_width_that_keeps_the_panes_side_by_side() {
    let split = SplitPane::new(100);
    assert_eq!(split.threshold(), 100);
    assert!(!split.stacks(Rect::new(0, 0, 100, 10)));
    assert!(split.stacks(Rect::new(0, 0, 99, 10)));
    let [left, right] = split.areas(Rect::new(0, 0, 100, 10));
    assert_eq!(left, Rect::new(0, 0, 50, 10));
    assert_eq!(right, Rect::new(50, 0, 50, 10));
    let [top, bottom] = split.areas(Rect::new(0, 0, 99, 10));
    assert_eq!(top, Rect::new(0, 0, 99, 5));
    assert_eq!(bottom, Rect::new(0, 5, 99, 5));
}

#[test]
fn the_halves_cover_the_area_and_the_first_takes_an_odd_cell() {
    let split = SplitPane::new(100);
    for width in 100..=161 {
        let area = Rect::new(3, 1, width, 20);
        let [left, right] = split.areas(area);
        assert_eq!(left.x, 3, "{width}");
        assert_eq!(left.width, width.div_ceil(2), "{width}");
        assert_eq!(right.x, left.right(), "{width}");
        assert_eq!(right.right(), area.right(), "{width}");
        assert_eq!((left.y, left.height), (1, 20), "{width}");
        assert_eq!((right.y, right.height), (1, 20), "{width}");
    }
    for height in 3..=41 {
        let area = Rect::new(0, 1, 60, height);
        let [top, bottom] = split.areas(area);
        assert_eq!(top.y, 1, "{height}");
        assert_eq!(top.height, height.div_ceil(2), "{height}");
        assert_eq!(bottom.y, top.bottom(), "{height}");
        assert_eq!(bottom.bottom(), area.bottom(), "{height}");
        assert_eq!((top.width, bottom.width), (60, 60), "{height}");
    }
}

#[test]
fn a_threshold_of_zero_never_stacks_and_an_empty_area_splits_into_empty_panes() {
    let split = SplitPane::new(0);
    assert!(!split.stacks(Rect::new(0, 0, 1, 1)));
    let [first, second] = SplitPane::new(10).areas(Rect::default());
    assert!(first.is_empty() && second.is_empty());
}
