use std::cmp::Reverse;

use super::*;
use crate::text::width;

/// ` › `-like separators and a one-cell mark, as a path of steps uses.
const SEP: usize = 3;
const MARK: usize = 1;

fn gap(pieces: &[Piece], room: usize) -> Elided {
    elide(pieces, SEP, MARK, room, MarkAt::Gap)
}

#[test]
fn every_piece_stays_when_the_line_fits() {
    let pieces = [Piece::ranked(4, 1), Piece::ranked(4, 2), Piece::unranked(4)];
    // 4 + 3 + 4 + 3 + 4: an exact fit keeps everything.
    for room in [18, 19, 80] {
        let line = gap(&pieces, room);
        assert_eq!(line.kept, [0, 1, 2], "room {room}");
        assert!(line.marks.is_empty() && !line.end_mark && line.cut.is_none());
        assert_eq!(line.width(&pieces, SEP, MARK), 18);
    }
    assert_eq!(elide(&[], SEP, MARK, 0, MarkAt::Gap), Elided::default());
}

#[test]
fn pieces_give_way_lowest_rank_first() {
    // Ranks out of order on purpose: index 2 goes first, then 0, then 1.
    let pieces = [
        Piece::ranked(2, 5),
        Piece::ranked(2, 9),
        Piece::ranked(2, 1),
        Piece::unranked(2),
    ];
    let kept_at = |room| elide(&pieces, 1, 1, room, MarkAt::End).kept;
    // 2 + 1 + 2 + 1 + 2 + 1 + 2 = 11 with every piece.
    assert_eq!(kept_at(11), [0, 1, 2, 3]);
    // Without index 2: 2 + 1 + 2 + 1 + 2 and the end mark.
    assert_eq!(kept_at(10), [0, 1, 3]);
    assert_eq!(kept_at(9), [0, 1, 3]);
    assert_eq!(kept_at(8), [1, 3]);
    assert_eq!(kept_at(6), [1, 3]);
    assert_eq!(kept_at(5), [3]);
}

#[test]
fn of_two_pieces_with_one_rank_the_later_gives_way_first() {
    let pieces = [
        Piece::ranked(3, 1),
        Piece::ranked(3, 1),
        Piece::ranked(3, 1),
    ];
    let line = elide(&pieces, 1, 1, 8, MarkAt::End);
    assert_eq!(line.kept, [0, 1]);
    let line = elide(&pieces, 1, 1, 4, MarkAt::End);
    assert_eq!(line.kept, [0]);
}

#[test]
fn an_unranked_piece_never_gives_way_to_a_ranked_one() {
    let pieces = [Piece::unranked(6), Piece::ranked(1, 0), Piece::unranked(6)];
    let line = gap(&pieces, 14);
    // `aaaaaa › … › cccccc` is 19 cells, so the marks go and the last
    // unranked piece is cut; the ranked one does not come back.
    assert_eq!(line.kept, [0, 2]);
    assert!(line.marks.is_empty());
    assert_eq!(line.cut, Some(5));
    assert_eq!(line.width(&pieces, SEP, MARK), 14);
}

#[test]
fn each_run_that_gave_way_leaves_one_mark_in_its_gap() {
    // a b c d eeeee f: b and c give way, then e, then a.
    let pieces = [
        Piece::ranked(1, 4),
        Piece::ranked(1, 1),
        Piece::ranked(1, 2),
        Piece::unranked(1),
        Piece::ranked(5, 3),
        Piece::unranked(1),
    ];
    // a › … › d › eeeee › f (21 cells): one mark for the run of two.
    let line = gap(&pieces, 21);
    assert_eq!(line.kept, [0, 3, 4, 5]);
    assert_eq!(line.marks, [1]);
    assert_eq!(
        line.items(),
        [
            Item::Piece(0),
            Item::Mark,
            Item::Piece(3),
            Item::Piece(4),
            Item::Piece(5)
        ]
    );
    // a › … › d › … › f (17 cells): two runs, two marks.
    for room in 17..21 {
        let line = gap(&pieces, room);
        assert_eq!(line.kept, [0, 3, 5], "room {room}");
        assert_eq!(line.marks, [1, 2], "room {room}");
    }
    // … › d › … › f (13 cells): a run at the start has its mark first.
    let line = gap(&pieces, 13);
    assert_eq!(line.kept, [3, 5]);
    assert_eq!(line.marks, [0, 1]);
    assert_eq!(
        line.items(),
        [Item::Mark, Item::Piece(3), Item::Mark, Item::Piece(5)]
    );
}

#[test]
fn a_run_at_the_end_has_its_mark_last() {
    let pieces = [Piece::unranked(2), Piece::ranked(2, 2), Piece::ranked(2, 1)];
    // `aa › bb › …` is 11 cells.
    let line = gap(&pieces, 11);
    assert_eq!(line.kept, [0, 1]);
    assert_eq!(line.marks, [2]);
    assert_eq!(line.items(), [Item::Piece(0), Item::Piece(1), Item::Mark]);
    assert_eq!(line.width(&pieces, SEP, MARK), 11);
}

#[test]
fn the_end_mark_is_one_whatever_gave_way_and_takes_no_separator() {
    let pieces = [
        Piece::ranked(3, 1),
        Piece::unranked(3),
        Piece::ranked(3, 2),
        Piece::unranked(3),
    ];
    // Without index 0: 3 + 3 + 3 + 3 + 3 and a two-cell mark.
    let line = elide(&pieces, 3, 2, 17, MarkAt::End);
    assert_eq!(line.kept, [1, 2, 3]);
    assert!(line.end_mark && line.marks.is_empty());
    assert_eq!(line.width(&pieces, 3, 2), 17);
    // Two pieces gone from two places, still one mark.
    let line = elide(&pieces, 3, 2, 11, MarkAt::End);
    assert_eq!(line.kept, [1, 3]);
    assert!(line.end_mark && line.marks.is_empty());
    assert_eq!(line.items(), [Item::Piece(1), Item::Piece(3)]);
    assert_eq!(line.width(&pieces, 3, 2), 11);
}

#[test]
fn a_mark_is_paid_for_before_the_line_counts_as_fitting() {
    // Dropping the middle piece saves 4 cells but its mark costs 4 (a
    // separator and the mark), so the first piece has to go as well.
    let pieces = [Piece::ranked(5, 2), Piece::ranked(1, 1), Piece::unranked(5)];
    assert_eq!(gap(&pieces, 17).kept, [0, 1, 2]);
    let line = gap(&pieces, 16);
    assert_eq!(line.kept, [2]);
    assert_eq!(line.marks, [0]);
    assert_eq!(line.width(&pieces, SEP, MARK), 9);
}

#[test]
fn the_last_resort_drops_the_marks_before_it_cuts() {
    let pieces = [Piece::ranked(4, 1), Piece::unranked(6)];
    // `… › bbbbbb` is 10 cells; `bbbbbb` alone fits 6 to 9.
    for room in 6..10 {
        let line = gap(&pieces, room);
        assert_eq!(line.kept, [1], "room {room}");
        assert!(line.marks.is_empty() && line.cut.is_none(), "room {room}");
    }
    // Below that the piece is cut to the room.
    for room in 0..6 {
        let line = gap(&pieces, room);
        assert_eq!(line.cut, Some(room));
        assert_eq!(line.width(&pieces, SEP, MARK), room);
    }
    // At the end, too.
    let line = elide(&pieces, SEP, 2, 3, MarkAt::End);
    assert!(!line.end_mark);
    assert_eq!(line.cut, Some(3));
}

#[test]
fn the_last_resort_cuts_the_last_unranked_piece_to_the_room_the_others_leave() {
    let pieces = [Piece::unranked(4), Piece::ranked(2, 0), Piece::unranked(8)];
    let line = gap(&pieces, 10);
    assert_eq!(line.kept, [0, 2]);
    assert_eq!(line.cut, Some(10 - 4 - SEP));
    // With no room after the others, the cut leaves nothing of it, and
    // the line is as wide as the others.
    let line = gap(&pieces, 5);
    assert_eq!(line.cut, Some(0));
    assert_eq!(line.width(&pieces, SEP, MARK), 4 + SEP);
}

#[test]
fn a_line_of_ranked_pieces_alone_narrows_to_its_mark_and_then_to_nothing() {
    let pieces = [Piece::ranked(3, 1), Piece::ranked(3, 2)];
    let line = gap(&pieces, 2);
    assert!(line.kept.is_empty());
    assert_eq!(line.marks, [0]);
    assert_eq!(line.items(), [Item::Mark]);
    assert_eq!(line.width(&pieces, SEP, MARK), 1);
    assert_eq!(gap(&pieces, 0), Elided::default());
    let line = elide(&pieces, SEP, 2, 2, MarkAt::End);
    assert!(line.kept.is_empty() && line.end_mark);
    assert_eq!(elide(&pieces, SEP, 2, 1, MarkAt::End), Elided::default());
}

#[test]
fn a_piece_with_a_gap_of_its_own_takes_it_in_place_of_the_separator() {
    // aaaa · bb  ·  cc  dd: a wider gap before a group, a narrow one in it.
    let pieces = [
        Piece::unranked(4),
        Piece::ranked(2, 4),
        Piece::ranked(2, 2).gap(5),
        Piece::ranked(2, 1).gap(2),
    ];
    let line = elide(&pieces, 3, 2, 80, MarkAt::End);
    assert_eq!(line.width(&pieces, 3, 2), 4 + 3 + 2 + 5 + 2 + 2 + 2);
    // Without `dd`, its gap goes with it; the end mark takes none.
    let line = elide(&pieces, 3, 2, 19, MarkAt::End);
    assert_eq!(line.kept, [0, 1, 2]);
    assert_eq!(line.width(&pieces, 3, 2), 4 + 3 + 2 + 5 + 2 + 2);
    // The first item has nothing before it, whichever it is.
    let first = [Piece::ranked(2, 1).gap(5), Piece::unranked(3).gap(4)];
    let line = elide(&first, 3, 1, 9, MarkAt::Gap);
    assert_eq!(line.kept, [0, 1]);
    assert_eq!(line.width(&first, 3, 1), 2 + 4 + 3);
    let line = elide(&first, 3, 1, 8, MarkAt::Gap);
    assert_eq!(line.items(), [Item::Mark, Item::Piece(1)]);
    assert_eq!(line.width(&first, 3, 1), 1 + 4 + 3);
    let line = elide(&first, 3, 1, 3, MarkAt::Gap);
    assert_eq!(line.items(), [Item::Piece(1)]);
    assert_eq!(line.width(&first, 3, 1), 3);
    // A mark in a gap takes the separator; the piece after it its own gap.
    let marked = [
        Piece::unranked(2),
        Piece::ranked(2, 1).gap(5),
        Piece::unranked(3).gap(4),
    ];
    for room in 11..16 {
        let line = elide(&marked, 1, 1, room, MarkAt::Gap);
        assert_eq!(line.items(), [Item::Piece(0), Item::Mark, Item::Piece(2)]);
        assert_eq!(line.width(&marked, 1, 1), 2 + 1 + 1 + 4 + 3, "room {room}");
    }
    assert_eq!(elide(&marked, 1, 1, 16, MarkAt::Gap).kept, [0, 1, 2]);
}

#[test]
fn a_cut_leaves_the_last_piece_what_the_others_and_its_gap_leave() {
    let pieces = [
        Piece::unranked(4),
        Piece::ranked(3, 1),
        Piece::unranked(9).gap(1),
    ];
    let line = elide(&pieces, 3, 2, 10, MarkAt::End);
    assert_eq!(line.kept, [0, 2]);
    assert_eq!(line.cut, Some(10 - 4 - 1));
    assert_eq!(line.width(&pieces, 3, 2), 10);
}

#[test]
fn filling_brings_back_a_narrow_piece_after_a_wider_one_went() {
    // a · bbbbbbbb · c · dd: b matters most, then c, then d.
    let pieces = [
        Piece::unranked(1),
        Piece::ranked(8, 3),
        Piece::ranked(1, 2),
        Piece::ranked(2, 1),
    ];
    // Strictly at 12 cells: d, c, and b go (a …); filling brings c and d
    // back (a · c · dd …, 12 cells), and b stays out.
    let strict = elide(&pieces, 3, 2, 12, MarkAt::End);
    assert_eq!(strict.kept, [0]);
    let filled = elide_filled(&pieces, 3, 2, 12, MarkAt::End);
    assert_eq!(filled.kept, [0, 2, 3]);
    assert!(filled.end_mark);
    assert_eq!(filled.width(&pieces, 3, 2), 12);
    // A line that fits needs no filling, and with room enough nothing is
    // left out.
    let all = elide_filled(&pieces, 3, 2, 80, MarkAt::End);
    assert_eq!(all, elide(&pieces, 3, 2, 80, MarkAt::End));
    assert_eq!(all.kept, [0, 1, 2, 3]);
}

#[test]
fn filling_brings_back_the_higher_rank_first() {
    // Both fit back alone; only one fits: the higher rank wins.
    let pieces = [
        Piece::unranked(1),
        Piece::ranked(9, 5),
        Piece::ranked(3, 1),
        Piece::ranked(3, 2),
    ];
    let filled = elide_filled(&pieces, 1, 1, 7, MarkAt::End);
    assert_eq!(filled.kept, [0, 3]);
}

#[test]
fn pieces_of_one_rank_come_back_in_order() {
    // Three of one rank: the second is too wide, so the third, which
    // would fit, stays out behind it.
    let pieces = [
        Piece::unranked(1),
        Piece::ranked(1, 1),
        Piece::ranked(6, 1),
        Piece::ranked(1, 1),
        Piece::ranked(9, 2),
    ];
    let filled = elide_filled(&pieces, 1, 1, 6, MarkAt::End);
    assert_eq!(filled.kept, [0, 1]);
    assert!(filled.end_mark);
}

#[test]
fn filling_keeps_one_mark_for_each_run_in_the_gaps() {
    // a b c d e: c (wide, rank 3) goes after b and d; b and d come back.
    let pieces = [
        Piece::unranked(1),
        Piece::ranked(1, 1),
        Piece::ranked(7, 3),
        Piece::ranked(1, 2),
        Piece::unranked(1),
    ];
    let filled = elide_filled(&pieces, 1, 1, 9, MarkAt::Gap);
    assert_eq!(filled.kept, [0, 1, 3, 4]);
    assert_eq!(
        filled.items(),
        [
            Item::Piece(0),
            Item::Piece(1),
            Item::Mark,
            Item::Piece(3),
            Item::Piece(4)
        ]
    );
    assert_eq!(filled.width(&pieces, 1, 1), 9);
}

#[test]
fn nothing_comes_back_after_the_last_resort() {
    let pieces = [Piece::ranked(1, 1), Piece::unranked(8)];
    let line = elide_filled(&pieces, 1, 1, 5, MarkAt::End);
    assert_eq!(line.kept, [1]);
    assert_eq!(line.cut, Some(5));
    assert!(!line.end_mark);
}

#[test]
fn widths_are_the_cells_the_caller_measured() {
    // Two characters, four cells: a piece measured in characters would
    // claim to fit a room of two.
    let wide = width("東京");
    assert_eq!(wide, 4);
    let pieces = [Piece::ranked(wide, 1), Piece::unranked(width("ab"))];
    assert_eq!(gap(&pieces, 4 + SEP + 2).kept, [0, 1]);
    assert_eq!(gap(&pieces, 4 + SEP + 1).kept, [1]);
}

/// A fixed-seed xorshift, so a failure names its case and reproduces.
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

    /// Up to eight pieces, a few cells each; about one in four unranked,
    /// ranks from a small pool so ties are common, and about one in three
    /// with a gap of its own.
    fn pieces(&mut self) -> Vec<Piece> {
        (0..self.below(9))
            .map(|_| {
                let width = self.below(9);
                let piece = if self.below(4) == 0 {
                    Piece::unranked(width)
                } else {
                    Piece::ranked(width, u32::try_from(self.below(5)).unwrap_or(0))
                };
                if self.below(3) == 0 {
                    piece.gap(self.below(6))
                } else {
                    piece
                }
            })
            .collect()
    }
}

/// The order pieces give way in: lowest rank first, the later of a tie.
fn give_way_key(pieces: &[Piece], index: usize) -> (Option<u32>, Reverse<usize>) {
    (pieces[index].rank, Reverse(index))
}

/// The line's width counted from its items, independently of
/// [`Elided::width`].
fn drawn_width(line: &Elided, pieces: &[Piece], separator: usize, mark: usize) -> usize {
    let items = line.items();
    let last = line.kept.last().copied();
    let cells: usize = items
        .iter()
        .map(|item| match *item {
            Item::Piece(index) if Some(index) == last => line.cut.unwrap_or(pieces[index].width),
            Item::Piece(index) => pieces[index].width,
            Item::Mark => mark,
        })
        .sum();
    let gaps: usize = items
        .iter()
        .skip(1)
        .map(|item| match *item {
            Item::Piece(index) => pieces[index].gap.unwrap_or(separator),
            Item::Mark => separator,
        })
        .sum();
    cells + gaps + if line.end_mark { mark } else { 0 }
}

/// The kept pieces of each rank are its first ones.
fn kept_of_each_rank_lead(line: &Elided, pieces: &[Piece]) -> bool {
    (0..pieces.len()).all(|index| {
        !line.kept.contains(&index)
            || pieces[index].rank.is_none()
            || (0..index)
                .filter(|&earlier| pieces[earlier].rank == pieces[index].rank)
                .all(|earlier| line.kept.contains(&earlier))
    })
}

#[test]
fn generated_filled_lines_keep_the_rule_and_use_the_room() {
    let mut generator = Gen(0x0f11_ed11_5eed_0002);
    for case in 0..20_000 {
        let pieces = generator.pieces();
        let separator = generator.below(4);
        let mark = generator.below(3);
        let at = if generator.below(2) == 0 {
            MarkAt::Gap
        } else {
            MarkAt::End
        };
        let room = generator.below(60);
        let strict = elide(&pieces, separator, mark, room, at);
        let line = elide_filled(&pieces, separator, mark, room, at);
        let why = format!("case {case}: {pieces:?} sep {separator} mark {mark} {at:?} room {room}");

        // Filling only adds to what the rule kept, in order, and the
        // reported width is the drawn width.
        assert!(
            strict.kept.iter().all(|index| line.kept.contains(index)),
            "{why}"
        );
        assert!(line.kept.windows(2).all(|w| w[0] < w[1]), "{why}");
        assert_eq!(
            line.width(&pieces, separator, mark),
            drawn_width(&line, &pieces, separator, mark),
            "{why}"
        );
        assert!(kept_of_each_rank_lead(&strict, &pieces), "{why}");
        assert!(kept_of_each_rank_lead(&line, &pieces), "{why}");

        let marked = !strict.marks.is_empty() || strict.end_mark;
        if !marked || strict.kept.len() == pieces.len() {
            // A line that fit, or the last resort: nothing comes back.
            assert_eq!(line, strict, "{why}");
            continue;
        }
        assert!(line.width(&pieces, separator, mark) <= room, "{why}");
        assert!(line.cut.is_none(), "{why}");
        // At the end, the line uses the room: the next piece of each rank
        // that is out would not fit beside what is kept.
        if at == MarkAt::End {
            for index in 0..pieces.len() {
                let next_of_rank = !line.kept.contains(&index)
                    && (0..index)
                        .filter(|&earlier| pieces[earlier].rank == pieces[index].rank)
                        .all(|earlier| line.kept.contains(&earlier));
                if next_of_rank {
                    let mut shown = vec![false; pieces.len()];
                    for &kept in line.kept.iter().chain([&index]) {
                        shown[kept] = true;
                    }
                    assert!(
                        layout(&shown, at).width(&pieces, separator, mark) > room,
                        "{why}: {index} would fit back"
                    );
                }
            }
        }
    }
}

#[test]
fn generated_lines_keep_the_rule() {
    let mut generator = Gen(0x5eed_e11d_e000_0001);
    for case in 0..20_000 {
        let pieces = generator.pieces();
        let separator = generator.below(4);
        let mark = generator.below(3);
        let at = if generator.below(2) == 0 {
            MarkAt::Gap
        } else {
            MarkAt::End
        };
        let room = generator.below(60);
        let line = elide(&pieces, separator, mark, room, at);
        let why = format!("case {case}: {pieces:?} sep {separator} mark {mark} {at:?} room {room}");

        // The kept pieces are in order, and none that never gives way is
        // missing.
        assert!(line.kept.windows(2).all(|w| w[0] < w[1]), "{why}");
        assert!(line.kept.iter().all(|&index| index < pieces.len()), "{why}");
        let unranked: Vec<usize> = (0..pieces.len())
            .filter(|&index| pieces[index].rank.is_none())
            .collect();
        assert!(
            unranked.iter().all(|index| line.kept.contains(index)),
            "{why}"
        );

        // What gave way went in order: every dropped piece comes before
        // every kept ranked piece in the give-way order.
        let dropped: Vec<usize> = (0..pieces.len())
            .filter(|index| !line.kept.contains(index))
            .collect();
        for &gone in &dropped {
            for &kept in line.kept.iter().filter(|&&k| pieces[k].rank.is_some()) {
                assert!(
                    give_way_key(&pieces, gone) < give_way_key(&pieces, kept),
                    "{why}: {gone} went before {kept}"
                );
            }
        }

        // The width the line reports is the width its items draw.
        assert_eq!(
            line.width(&pieces, separator, mark),
            drawn_width(&line, &pieces, separator, mark),
            "{why}"
        );

        let marked = !line.marks.is_empty() || line.end_mark;
        let last_resort = !dropped.is_empty() && !marked;
        if dropped.is_empty() {
            // Nothing gave way, so nothing is marked; a cut means the
            // unranked pieces alone were too wide.
            assert!(!marked, "{why}");
        }
        if last_resort || line.cut.is_some() {
            // Only unranked pieces are left, and they did not fit with
            // their marks.
            assert_eq!(line.kept, unranked, "{why}");
            assert!(line.marks.is_empty() && !line.end_mark, "{why}");
            let mut shown = vec![false; pieces.len()];
            for &index in &unranked {
                shown[index] = true;
            }
            if !dropped.is_empty() {
                assert!(
                    layout(&shown, at).width(&pieces, separator, mark) > room,
                    "{why}: the marks went though they fit"
                );
            }
            let uncut = Elided {
                kept: unranked.clone(),
                ..Elided::default()
            }
            .width(&pieces, separator, mark);
            assert_eq!(line.cut.is_some(), uncut > room, "{why}");
            if let (Some(cut), Some(&last)) = (line.cut, line.kept.last()) {
                // The cut loses cells, unless the others leave none and
                // the last piece had none to lose.
                assert!(cut < pieces[last].width || cut == 0, "{why}");
                let others = uncut - pieces[last].width;
                assert_eq!(cut, room.saturating_sub(others), "{why}");
            }
        } else {
            // A line that kept its marks fits, and it gave way no further
            // than it had to: with its last dropped piece back, it would
            // not fit.
            assert!(line.width(&pieces, separator, mark) <= room, "{why}");
            if let Some(&last_gone) = dropped
                .iter()
                .max_by_key(|&&index| give_way_key(&pieces, index))
            {
                let mut shown = vec![false; pieces.len()];
                for &index in line.kept.iter().chain([&last_gone]) {
                    shown[index] = true;
                }
                assert!(
                    layout(&shown, at).width(&pieces, separator, mark) > room,
                    "{why}: {last_gone} gave way though the line fit with it"
                );
            }
        }

        // With at most one piece that never gives way, the line fits.
        if unranked.len() <= 1 {
            assert!(line.width(&pieces, separator, mark) <= room, "{why}");
        }

        // The marks: at the gap, one per run that gave way, each in a
        // gap that lost something; at the end, one for all of them.
        match at {
            MarkAt::Gap => {
                assert!(!line.end_mark, "{why}");
                if !last_resort && line.cut.is_none() {
                    let mut runs = Vec::new();
                    let mut position = 0;
                    let mut in_run = false;
                    for index in 0..pieces.len() {
                        if line.kept.contains(&index) {
                            position += 1;
                            in_run = false;
                        } else if !in_run {
                            runs.push(position);
                            in_run = true;
                        }
                    }
                    assert_eq!(line.marks, runs, "{why}");
                }
            }
            MarkAt::End => {
                assert!(line.marks.is_empty(), "{why}");
                if !last_resort && line.cut.is_none() {
                    assert_eq!(line.end_mark, !dropped.is_empty(), "{why}");
                }
            }
        }
    }
}
