//! Property tests over generated trees and generated sequences of what a
//! user and a host do to them: rebuilds (rows added, rows dropped, a new
//! tree), folds, steps, pages, the ends, jumps, and frames drawn at new
//! sizes.
//!
//! After every operation, while any row shows, something is selected and
//! the selected row shows; after every frame, the window is inside the
//! showing rows and shows the cursor's row. A rebuild that keeps the
//! selected row keeps the cursor on it, and one that drops it keeps the
//! cursor's position. The generator is a fixed-seed xorshift, so a failure
//! names its seed and step and reproduces on every run.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;
use crate::scroll::ScrollWindow;
use crate::theme::Theme;

type Tree = Vec<Row<u32, ()>>;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        usize::try_from(self.next() % bound as u64).unwrap_or(0)
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    /// An identity from a pool a little larger than the tree, so a tree
    /// sometimes lists an identity twice.
    fn id(&mut self, pool: usize) -> u32 {
        u32::try_from(self.below(pool)).unwrap_or(0)
    }

    /// A tree of up to 40 rows: each row at most one level deeper than the
    /// one above it, expandable when the next row is deeper (and now and
    /// then when it is not, as a row whose children load later is).
    fn tree(&mut self) -> Tree {
        let len = self.below(41);
        let pool = len + len / 8 + 1;
        let mut depths: Vec<u16> = Vec::with_capacity(len);
        for index in 0..len {
            let deepest = depths.last().map_or(0, |&depth| depth + 1);
            let depth = if index == 0 {
                0
            } else {
                u16::try_from(self.below(usize::from(deepest) + 1)).unwrap_or(0)
            };
            depths.push(depth);
        }
        (0..len)
            .map(|index| {
                let has_children = depths
                    .get(index + 1)
                    .is_some_and(|&next| next > depths[index]);
                Row {
                    id: self.id(pool),
                    depth: depths[index],
                    expandable: has_children || self.chance(8),
                    payload: (),
                }
            })
            .collect()
    }

    fn delta(&mut self) -> i32 {
        let magnitude = i32::try_from(self.below(4) + 1).unwrap_or(1);
        if self.chance(2) {
            magnitude
        } else {
            -magnitude
        }
    }
}

/// What a user or a host does to a tree.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// Rows the selected one is not among join the tree.
    Grow,
    /// Some rows leave the tree, perhaps the selected one.
    Shrink,
    /// The host builds a different tree.
    Replace,
    Expand,
    Collapse,
    Toggle,
    Step(i32),
    PageDown,
    PageUp,
    Home,
    End,
    /// Jump to the row at this index, opening its ancestors first.
    Jump(usize),
    /// Draw a frame of this many columns and rows and keep its window.
    Draw(u16, u16),
}

impl Gen {
    fn op(&mut self, rows: usize) -> Op {
        match self.below(14) {
            0 => Op::Grow,
            1 => Op::Shrink,
            2 => Op::Replace,
            3 => Op::Expand,
            4 => Op::Collapse,
            5 => Op::Toggle,
            6 | 7 => Op::Step(self.delta()),
            8 => Op::PageDown,
            9 => Op::PageUp,
            10 => Op::Home,
            11 => Op::End,
            12 => Op::Jump(self.below(rows.max(1))),
            _ => Op::Draw(
                u16::try_from(self.below(30) + 2).unwrap_or(2),
                u16::try_from(self.below(16) + 2).unwrap_or(2),
            ),
        }
    }

    /// `tree` with a few rows added at random places, none with
    /// `keep_out`'s identity.
    fn grow(&mut self, tree: &Tree, keep_out: Option<u32>) -> Tree {
        let mut grown = tree.clone();
        for _ in 0..=self.below(4) {
            let at = self.below(grown.len() + 1);
            let depth = if at == 0 {
                0
            } else {
                grown[at - 1].depth + u16::from(self.chance(2))
            };
            let mut id = 1000 + self.id(1000);
            while Some(id) == keep_out {
                id += 1;
            }
            grown.insert(
                at,
                Row {
                    id,
                    depth,
                    expandable: self.chance(3),
                    payload: (),
                },
            );
        }
        grown
    }

    /// `tree` without a few of its rows.
    fn shrink(&mut self, tree: &Tree) -> Tree {
        tree.iter().filter(|_| !self.chance(4)).cloned().collect()
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

const STEPS: usize = 500;

/// The identities of the rows that show.
fn shown_ids(state: &TreeState<u32>, tree: &Tree) -> Vec<u32> {
    state.visible(tree).iter().map(|row| row.id).collect()
}

/// While any row shows, something is selected, the selected row shows,
/// and the cursor's position names a showing row with that identity.
fn assert_selection_valid(state: &TreeState<u32>, tree: &Tree, context: &str) {
    let shown = shown_ids(state, tree);
    if shown.is_empty() {
        assert_eq!(state.selected(), None, "{context}: no row shows");
        return;
    }
    let selected = state
        .selected()
        .unwrap_or_else(|| panic!("{context}: rows show and nothing is selected"));
    let cursor = state
        .cursor(tree)
        .unwrap_or_else(|| panic!("{context}: the selected row {selected} does not show"));
    assert_eq!(
        shown.get(cursor),
        Some(selected),
        "{context}: the cursor names its row"
    );
}

fn assert_window_shows_cursor(
    state: &TreeState<u32>,
    tree: &Tree,
    window: ScrollWindow,
    context: &str,
) {
    let shown = state.visible(tree).len();
    assert_eq!(
        window.content(),
        shown,
        "{context}: the window counts the showing rows"
    );
    assert!(
        window.offset() <= window.last_offset(),
        "{context}: offset {} past the last {}",
        window.offset(),
        window.last_offset()
    );
    if window.viewport() > 0
        && let Some(cursor) = state.cursor(tree)
    {
        assert!(
            window.visible().contains(&cursor),
            "{context}: cursor {cursor} outside {:?}",
            window.visible()
        );
    }
}

/// Draw the tree as a host does and keep the window the frame drew.
fn draw(
    state: &mut TreeState<u32>,
    tree: &Tree,
    width: u16,
    height: u16,
    theme: &Theme,
) -> Option<ScrollWindow> {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let line = |row: &Row<u32, ()>, cx: &RowCtx<'_>| {
        assert!(cx.width <= usize::from(width.saturating_sub(2)));
        Line::from(row.id.to_string())
    };
    let window = TreeView::new(tree, state, line, theme).render(area, &mut buf)?;
    state.set_viewport(window);
    Some(window)
}

#[test]
fn the_cursor_always_shows_and_the_window_always_shows_the_cursor() {
    let theme = Theme::default();
    for seed in SEEDS {
        let mut generator = Gen(seed);
        let mut tree = generator.tree();
        let mut state = TreeState::new();
        state.rebuild(&[], &tree, None);
        for step in 0..STEPS {
            let op = generator.op(tree.len());
            let context = format!("seed {seed:#x}, step {step}, {op:?}");
            match op {
                Op::Grow | Op::Shrink | Op::Replace => {
                    let new = match op {
                        Op::Grow => generator.grow(&tree, state.selected().copied()),
                        Op::Shrink => generator.shrink(&tree),
                        _ => generator.tree(),
                    };
                    let before = state.begin_rebuild(&tree);
                    let kept = before.selected().copied();
                    let was_at = before.position();
                    state.finish_rebuild(before, &new, None);
                    tree = new;
                    let shown = shown_ids(&state, &tree);
                    match kept {
                        Some(id) if shown.contains(&id) => {
                            assert_eq!(state.selected(), Some(&id), "{context}: kept by identity");
                        }
                        Some(_) if !shown.is_empty() => {
                            let at = was_at.map_or(0, |at| at.min(shown.len() - 1));
                            assert_eq!(state.cursor(&tree), Some(at), "{context}: kept its place");
                        }
                        _ => {}
                    }
                }
                Op::Expand => {
                    state.expand(&tree);
                }
                Op::Collapse => {
                    let selected = state.selected().copied();
                    state.collapse(&tree);
                    assert_eq!(
                        state.selected().copied(),
                        selected,
                        "{context}: collapse keeps the cursor"
                    );
                }
                Op::Toggle => {
                    state.toggle(&tree);
                }
                Op::Step(delta) => {
                    state.step(&tree, delta);
                }
                Op::PageDown => {
                    state.page_down(&tree);
                }
                Op::PageUp => {
                    state.page_up(&tree);
                }
                Op::Home => {
                    if state.home(&tree).is_some() {
                        assert_eq!(state.cursor(&tree), Some(0), "{context}");
                    }
                }
                Op::End => {
                    let last = state.visible(&tree).len().checked_sub(1);
                    assert_eq!(state.end(&tree), last, "{context}");
                }
                Op::Jump(index) => {
                    if let Some(row) = tree.get(index) {
                        state.reveal(&tree, index);
                        assert!(state.select(&tree, row.id), "{context}");
                    }
                }
                Op::Draw(width, height) => {
                    if let Some(window) = draw(&mut state, &tree, width, height, &theme) {
                        assert_window_shows_cursor(&state, &tree, window, &context);
                    }
                }
            }
            assert_selection_valid(&state, &tree, &context);
        }
    }
}

#[test]
fn a_page_moves_the_cursor_by_the_rows_the_frame_showed() {
    let theme = Theme::default();
    for seed in SEEDS {
        let mut generator = Gen(seed);
        for round in 0..40 {
            let tree: Tree = (0..generator.below(60) + 1)
                .map(|id| Row {
                    id: u32::try_from(id).unwrap_or(0),
                    depth: 0,
                    expandable: false,
                    payload: (),
                })
                .collect();
            let mut state = TreeState::new();
            state.home(&tree);
            let height = u16::try_from(generator.below(20) + 3).unwrap_or(3);
            let Some(window) = draw(&mut state, &tree, 20, height, &theme) else {
                continue;
            };
            let context = format!(
                "seed {seed:#x}, round {round}, {} rows in {height}",
                tree.len()
            );
            let before = state.cursor(&tree).unwrap_or(0);
            let after = state.page_down(&tree).unwrap_or(0);
            assert_eq!(
                after,
                (before + window.page()).min(tree.len() - 1),
                "{context}"
            );
            draw(&mut state, &tree, 20, height, &theme);
            assert_window_shows_cursor(&state, &tree, state.window(), &context);
        }
    }
}
