//! Property tests over generated key sequences: a cursor stepped, paged,
//! sent home and to the end, and a window resized under it, in lists of
//! one-row items and of wrapped items several rows tall, and a pager's
//! window scrolled on its own.
//!
//! After every operation the cursor is inside the window, the window is
//! inside the content, and a cursor that moved within the window did not
//! scroll it. The generator is a fixed-seed xorshift, so a failure names
//! its seed and step and reproduces on every run.

use super::*;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).unwrap_or(0)
    }

    fn delta(&mut self, reach: usize) -> i32 {
        let magnitude = i32::try_from(self.below(reach) + 1).unwrap_or(1);
        if self.below(2) == 0 {
            magnitude
        } else {
            -magnitude
        }
    }
}

/// What a key or a terminal event does to a list and its window.
#[derive(Debug, Clone, Copy)]
enum Op {
    Step(i32),
    PageDown,
    PageUp,
    Home,
    End,
    /// The terminal or the content changed: new viewport and content.
    Resize(usize, usize),
}

impl Gen {
    fn op(&mut self) -> Op {
        match self.below(10) {
            0..=3 => Op::Step(self.delta(3)),
            4 => Op::PageDown,
            5 => Op::PageUp,
            6 => Op::Home,
            7 => Op::End,
            _ => Op::Resize(self.below(16), self.below(70)),
        }
    }
}

const SEEDS: [u64; 6] = [
    0x5eed_0000_0000_0001,
    0x5eed_0000_0000_0002,
    0x0123_4567_89ab_cdef,
    0xfeed_face_dead_beef,
    0x0000_0000_0000_0001,
    0x7777_3333_1111_0000,
];

const STEPS: usize = 400;

/// The rows item `index` takes when item `i` is `heights[i]` rows tall.
fn span(heights: &[usize], index: usize) -> std::ops::Range<usize> {
    let start: usize = heights[..index].iter().sum();
    start..start + heights[index]
}

fn assert_window_inside_content(window: &ScrollWindow, context: &str) {
    assert!(
        window.offset() <= window.last_offset(),
        "{context}: offset {} past the last offset {}",
        window.offset(),
        window.last_offset()
    );
    assert!(
        window.visible().end <= window.content(),
        "{context}: window {:?} runs past {} rows",
        window.visible(),
        window.content()
    );
    if !window.overflows() {
        assert_eq!(window.offset(), 0, "{context}: content that fits scrolls");
    }
}

/// Drive a list whose items take `heights` rows (regenerated on resize
/// when `tall` is set) and check the invariants after every operation.
fn drive(seed: u64, tall: bool) {
    let mut generator = Gen(seed);
    let heights_for = |generator: &mut Gen, items: usize| -> Vec<usize> {
        (0..items)
            .map(|_| if tall { 1 + generator.below(4) } else { 1 })
            .collect()
    };
    let mut heights = heights_for(&mut generator, 30);
    let mut cursor = 0;
    let mut window = ScrollWindow::new(8, heights.iter().sum());
    window.follow(span(&heights, cursor));
    for step in 0..STEPS {
        let op = generator.op();
        let before = window;
        let items = heights.len();
        match op {
            Op::Step(delta) => cursor = clamp_step(cursor, items, delta),
            Op::PageDown | Op::PageUp => {
                let page = i32::try_from(window.page()).unwrap_or(i32::MAX);
                let delta = if matches!(op, Op::PageDown) {
                    page
                } else {
                    -page
                };
                cursor = clamp_step(cursor, items, delta);
            }
            Op::Home => cursor = 0,
            Op::End => cursor = items.saturating_sub(1),
            Op::Resize(viewport, items) => {
                heights = heights_for(&mut generator, items);
                cursor = cursor.min(items.saturating_sub(1));
                window.resize(viewport, heights.iter().sum());
            }
        }
        let context = format!("seed {seed:#x} step {step} {op:?}");
        if heights.is_empty() {
            window.follow(0..0);
            assert_window_inside_content(&window, &context);
            continue;
        }
        let rows = span(&heights, cursor);
        let moved_inside = matches!(
            op,
            Op::Step(_) | Op::PageDown | Op::PageUp | Op::Home | Op::End
        ) && before.visible().start <= rows.start
            && rows.end <= before.visible().end;
        window.follow(rows.clone());
        assert_window_inside_content(&window, &context);
        if window.viewport() > 0 {
            assert!(
                window.visible().contains(&rows.start),
                "{context}: cursor rows {rows:?} start outside the window {:?}",
                window.visible()
            );
            if rows.len() <= window.viewport() {
                assert!(
                    window.visible().start <= rows.start && rows.end <= window.visible().end,
                    "{context}: cursor rows {rows:?} not all inside the window {:?}",
                    window.visible()
                );
            }
        }
        if moved_inside {
            assert_eq!(
                window.offset(),
                before.offset(),
                "{context}: a cursor that stayed inside the window scrolled it"
            );
        }
    }
}

#[test]
fn the_cursor_stays_inside_the_window_over_one_row_items() {
    for seed in SEEDS {
        drive(seed, false);
    }
}

#[test]
fn the_cursor_stays_inside_the_window_over_wrapped_items() {
    for seed in SEEDS {
        drive(seed, true);
    }
}

#[test]
fn a_pager_window_stays_inside_its_content() {
    for seed in SEEDS {
        let mut generator = Gen(seed);
        let mut window = ScrollWindow::new(10, 50);
        for step in 0..STEPS {
            let before = window;
            let context = format!("seed {seed:#x} step {step}");
            match generator.below(7) {
                0 => {
                    let delta = generator.delta(20);
                    window.scroll_by(delta);
                }
                1 => {
                    window.page_down();
                    let expected = (before.offset() + before.page()).min(before.last_offset());
                    assert_eq!(window.offset(), expected, "{context}: page down");
                }
                2 => {
                    window.page_up();
                    let expected = before.offset().saturating_sub(before.page());
                    assert_eq!(window.offset(), expected, "{context}: page up");
                }
                3 => {
                    window.to_top();
                    assert_eq!(window.offset(), 0, "{context}: top");
                }
                4 => {
                    window.to_bottom();
                    assert_eq!(window.visible().end, window.content(), "{context}: bottom");
                }
                _ => window.resize(generator.below(16), generator.below(70)),
            }
            assert_window_inside_content(&window, &context);
        }
    }
}

#[test]
fn wrapping_steps_stay_in_the_list_and_come_back() {
    let mut generator = Gen(0x5eed_0000_0000_00aa);
    for _ in 0..2000 {
        let len = generator.below(12);
        let index = generator.below(40);
        let delta = generator.delta(30);
        let moved = wrap_step(index, len, delta);
        if len == 0 {
            assert_eq!(moved, 0);
            continue;
        }
        assert!(moved < len, "wrap_step({index}, {len}, {delta}) = {moved}");
        assert_eq!(wrap_step(moved, len, -delta), index % len);
    }
}

#[test]
fn clamping_steps_stay_in_the_list() {
    let mut generator = Gen(0x5eed_0000_0000_00bb);
    for _ in 0..2000 {
        let len = generator.below(12);
        let index = generator.below(12);
        let delta = generator.delta(30);
        let moved = clamp_step(index, len, delta);
        assert!(
            moved < len.max(1),
            "clamp_step({index}, {len}, {delta}) = {moved}"
        );
        if delta > 0 && index < len {
            assert!(moved >= index);
        }
    }
}

#[test]
fn the_steps_stop_or_wrap_at_the_ends() {
    assert_eq!(wrap_step(0, 3, 1), 1);
    assert_eq!(wrap_step(2, 3, 1), 0);
    assert_eq!(wrap_step(0, 3, -1), 2);
    assert_eq!(wrap_step(1, 3, -4), 0);
    assert_eq!(wrap_step(1, 3, 7), 2);
    assert_eq!(wrap_step(5, 0, 1), 0);
    assert_eq!(clamp_step(0, 3, -1), 0);
    assert_eq!(clamp_step(1, 3, 1), 2);
    assert_eq!(clamp_step(2, 3, 10), 2);
    assert_eq!(clamp_step(2, 3, -10), 0);
    assert_eq!(clamp_step(0, 0, 1), 0);
}
