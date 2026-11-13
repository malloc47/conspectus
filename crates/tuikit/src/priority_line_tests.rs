use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::*;

/// `alpha · bravo · charlie · delta`: 5 + 5 + 7 + 5 cells and three
/// separators, 31 in all. `bravo` stays longest, then `delta`, then
/// `alpha`, then `charlie`.
fn sample() -> PriorityLine {
    let theme = Theme::default();
    PriorityLine::new(&theme)
        .segment(Segment::new("alpha").rank(2))
        .segment(Segment::new("bravo").rank(4))
        .segment(Segment::new("charlie").rank(1))
        .segment(Segment::new("delta").rank(3))
}

fn text(line: &PriorityLine, room: usize) -> String {
    line.line(room).to_string()
}

#[test]
fn every_segment_shows_while_they_fit() {
    let line = sample();
    for room in [31, 32, 80] {
        assert_eq!(
            text(&line, room),
            "alpha · bravo · charlie · delta",
            "room {room}"
        );
    }
}

#[test]
fn segments_give_way_lowest_rank_first_and_the_line_says_so() {
    let line = sample();
    // Without `charlie`: 5 + 3 + 5 + 3 + 5 and the two-cell mark.
    assert_eq!(text(&line, 30), "alpha · bravo · delta …");
    assert_eq!(text(&line, 23), "alpha · bravo · delta …");
    // Without `alpha` too.
    assert_eq!(text(&line, 22), "bravo · delta …");
    assert_eq!(text(&line, 15), "bravo · delta …");
    assert_eq!(text(&line, 14), "bravo …");
    assert_eq!(text(&line, 7), "bravo …");
}

#[test]
fn of_two_with_one_rank_the_later_gives_way_first() {
    let theme = Theme::default();
    let line = PriorityLine::new(&theme)
        .segment(Segment::new("one").rank(1))
        .segment(Segment::new("two").rank(1))
        .segment(Segment::new("three").rank(1));
    assert_eq!(text(&line, 16), "one · two …");
    assert_eq!(text(&line, 6), "one …");
}

#[test]
fn the_room_is_filled_back_with_what_still_fits() {
    let theme = Theme::default();
    // A wide hint that matters more than a narrow one after it.
    let line = PriorityLine::new(&theme)
        .segment(Segment::new("q quit").rank(9))
        .segment(Segment::new("Enter open the selected item").rank(5))
        .segment(Segment::new("Tab focus").rank(2));
    // The wide hint does not fit beside `q quit`; strictly `Tab focus`
    // would have gone before it, but it fits, so it comes back.
    assert_eq!(text(&line, 24), "q quit · Tab focus …");
    assert_eq!(text(&line, 40), "q quit · Enter open the selected item …");
}

#[test]
fn segments_of_one_rank_come_back_in_order() {
    let theme = Theme::default();
    let line = PriorityLine::new(&theme)
        .segment(Segment::new("first").rank(5))
        .segment(Segment::new("a").rank(1))
        .segment(Segment::new("wide one").rank(1))
        .segment(Segment::new("b").rank(1))
        .segment(Segment::new("the widest of all").rank(3));
    // `b` would fit after `a`, but `wide one` before it does not, so the
    // ones of that rank kept are its first ones.
    assert_eq!(text(&line, 16), "first · a …");
}

#[test]
fn a_segment_with_no_rank_never_gives_way_and_is_cut_last() {
    let theme = Theme::default();
    let line = PriorityLine::new(&theme)
        .segment(Segment::new("updated 3m ago").rank(1))
        .segment(Segment::new("12 items"));
    assert_eq!(text(&line, 25), "updated 3m ago · 12 items");
    assert_eq!(text(&line, 24), "12 items …");
    // The mark goes before the segment is cut.
    assert_eq!(text(&line, 9), "12 items");
    assert_eq!(text(&line, 8), "12 items");
    assert_eq!(text(&line, 5), "12 i…");
    assert_eq!(text(&line, 0), "");
}

#[test]
fn a_segment_may_have_a_gap_of_its_own() {
    let theme = Theme::default();
    let line = PriorityLine::new(&theme)
        .segment(Segment::new("total 4").rank(3))
        .segment(Segment::new("[a] 1").rank(1).gap("  ·  "))
        .segment(Segment::new("[b] 3").rank(1).gap("  "));
    assert_eq!(text(&line, 80), "total 4  ·  [a] 1  [b] 3");
    // `[b] 3` and its gap go together.
    assert_eq!(text(&line, 23), "total 4  ·  [a] 1 …");
    // The first item has nothing before it, gap or not.
    assert_eq!(text(&line, 7), "[a] 1 …");
    let lead = PriorityLine::new(&theme).segment(Segment::new("only").gap("  ·  "));
    assert_eq!(text(&lead, 80), "only");
}

#[test]
fn the_separator_and_the_mark_recede_unless_the_host_says_otherwise() {
    let theme = Theme::default();
    let muted = Style::default().add_modifier(theme.muted);
    let line = sample().line(23);
    let looks: Vec<(&str, Style)> = line
        .spans
        .iter()
        .map(|span| (span.content.as_ref(), span.style))
        .collect();
    let plain = Style::default();
    assert_eq!(
        looks,
        [
            ("alpha", plain),
            (marks::SPACED_SEPARATOR, muted),
            ("bravo", plain),
            (marks::SPACED_SEPARATOR, muted),
            ("delta", plain),
            (marks::MORE, muted),
        ]
    );
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let line = PriorityLine::new(&theme)
        .separator(Span::styled(" | ", bold))
        .mark(Span::styled(" +", bold))
        .segment(Segment::new("one").rank(2))
        .segment(Segment::new("two").rank(1));
    assert_eq!(text(&line, 80), "one | two");
    assert_eq!(text(&line, 8), "one +");
}

#[test]
fn a_segment_keeps_its_own_spans_and_styles() {
    let theme = Theme::default();
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let segment = Segment::new(vec![Span::styled("3", bold), Span::raw(" left")]);
    let line = PriorityLine::new(&theme).segment(segment).line(80);
    assert_eq!(line.spans, [Span::styled("3", bold), Span::raw(" left")]);
    // A cut keeps the style of the span it falls in.
    let segment = Segment::new(vec![Span::styled("3", bold), Span::raw(" left")]);
    let line = PriorityLine::new(&theme).segment(segment).line(3);
    assert_eq!(line.to_string(), "3 …");
}

#[test]
fn the_line_never_runs_past_its_room() {
    let line = sample();
    for room in 0..=40 {
        assert!(line.line(room).width() <= room, "room {room}");
    }
}

#[test]
fn no_segments_draw_an_empty_line() {
    let theme = Theme::default();
    let line = PriorityLine::new(&theme);
    assert!(line.is_empty());
    assert_eq!(line.line(80), Line::default());
}

#[test]
fn the_fit_reports_what_shows() {
    let line = sample();
    let fit = line.fit(22);
    assert_eq!(fit.kept, [1, 3]);
    assert!(fit.end_mark);
    assert_eq!(line.segments().len(), 4);
    assert_eq!(line.segments()[1].rank_of(), Some(4));
}

/// A fixed-seed xorshift, so a failure names its case and reproduces.
struct Gen(u64);

impl Gen {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound.max(1) as u64).unwrap_or(0)
    }
}

/// Packing by priority, as a status line does by hand: in order of
/// priority, keep each segment that fits beside the ones kept; when any is
/// left out, pack again into the room less the mark.
fn packed_by_hand(
    widths: &[usize],
    ranks: &[u32],
    separator: usize,
    mark: usize,
    room: usize,
) -> Vec<usize> {
    let pack = |room: usize| {
        let mut order: Vec<usize> = (0..widths.len()).collect();
        order.sort_by_key(|&index| std::cmp::Reverse(ranks[index]));
        let mut kept: Vec<usize> = Vec::new();
        for index in order {
            let mut trial = kept.clone();
            trial.push(index);
            let cells: usize = trial.iter().map(|&i| widths[i]).sum::<usize>()
                + separator * trial.len().saturating_sub(1);
            if cells <= room {
                kept = trial;
            }
        }
        kept.sort_unstable();
        kept
    };
    let kept = pack(room);
    if kept.len() == widths.len() {
        kept
    } else {
        pack(room.saturating_sub(mark))
    }
}

#[test]
fn filling_packs_what_fits_by_priority() {
    // With every rank different, the line keeps what packing by priority
    // keeps.
    let theme = Theme::default();
    let mut generator = Gen(0x7a1e_5eed_0003);
    for case in 0..5_000 {
        let count = generator.below(8);
        let widths: Vec<usize> = (0..count).map(|_| 1 + generator.below(20)).collect();
        let mut ranks: Vec<u32> = (0..u32::try_from(count).unwrap_or(0)).collect();
        for index in (1..ranks.len()).rev() {
            ranks.swap(index, generator.below(index + 1));
        }
        let room = generator.below(90);
        let mut line = PriorityLine::new(&theme);
        for (width, rank) in widths.iter().zip(&ranks) {
            line.push(Segment::new("x".repeat(*width)).rank(*rank));
        }
        let fit = line.fit(room);
        let expected = packed_by_hand(&widths, &ranks, 3, 2, room);
        assert_eq!(
            fit.kept, expected,
            "case {case}: widths {widths:?} ranks {ranks:?} room {room}"
        );
        assert!(line.line(room).width() <= room, "case {case}");
    }
}
