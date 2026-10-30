//! The checks of the conformance suite, one function each. Each returns
//! its failures as messages; the suite names the case and the check.

use std::collections::BTreeSet;

use ratatui::buffer::{Buffer, Cell};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Position, Rect};
use ratatui::style::Modifier;
use ratatui::symbols::line;

use super::{Case, HEIGHTS, KeyRole, Look, Rig, Selection, WIDTHS};
use crate::glyphs::marks;
use crate::keys::{Footer, Segment};
use crate::testkit::buffer::{self, frame_rect, inner_rows, is_side, restyled_rows};
use crate::testkit::keys::{ctrl, press};
use crate::text;
use crate::theme::Theme;

/// The sizes the `Drawn` check compares at.
const DRAWN_SIZES: [(u16, u16); 2] = [(100, 30), (60, 20)];
/// The size the `Title` check reads the frame at.
const TITLE_SIZE: (u16, u16) = (120, 40);
/// The widths the `Selection` check moves the selection at, all at
/// [`SELECTION_HEIGHT`] rows.
const SELECTION_WIDTHS: [u16; 2] = [80, 120];
const SELECTION_HEIGHT: u16 = 40;
/// The screen every word of a modal is whole on.
const WIDE: (u16, u16) = (240, 40);
/// The screen a footer is read on: wide and tall enough that no footer
/// wraps inside a segment or scrolls away.
const FOOTER_SIZE: (u16, u16) = (200, 50);
/// The screen a footer's fit is read on: narrow and short enough that a
/// footer of several rows has fewer rows than it wants.
const FOOTER_SHORT: (u16, u16) = (40, 10);
/// The two-cell cursor gutter at a list's left edge, which the cursor's
/// marker moves through.
const GUTTER: u16 = 2;
/// The status text the `Close` check shows before closing, which closing
/// must clear.
const HINT: &str = "a hint the modal showed";

fn area((width, height): (u16, u16)) -> Rect {
    Rect::new(0, 0, width, height)
}

fn size_label((width, height): (u16, u16)) -> String {
    format!("{width}x{height}")
}

/// The screen with the case's modal open, and the same screen with it
/// closed. The host's status rows, which it redraws for whatever is on
/// top, are the same in both: they are the host's, not the modal's.
fn with_and_without<R: Rig>(rig: &R, case: &Case<R::State>, size: (u16, u16)) -> (Buffer, Buffer) {
    let mut state = case.open();
    let with = rig.render(&mut state, area(size));
    rig.close_top(&mut state);
    let mut without = rig.render(&mut state, area(size));
    for rect in rig.status_rows(area(size)) {
        let rect = rect.intersection(with.area);
        for y in rect.top()..rect.bottom() {
            for x in rect.left()..rect.right() {
                without[(x, y)] = with[(x, y)].clone();
            }
        }
    }
    (with, without)
}

/// Whether `shown` is `under` with only some of the `selection`
/// modifiers added or taken away: the same symbol, colors, and every
/// other modifier.
fn only_selection_changed(shown: &Cell, under: &Cell, selection: Modifier) -> bool {
    let changed = shown.modifier ^ under.modifier;
    if changed.is_empty() || !selection.contains(changed) {
        return false;
    }
    let mut restyled = under.clone();
    restyled.modifier = shown.modifier;
    &restyled == shown
}

/// Every size the frame checks draw at.
fn frame_sizes() -> impl Iterator<Item = (u16, u16)> {
    WIDTHS
        .into_iter()
        .flat_map(|width| HEIGHTS.into_iter().map(move |height| (width, height)))
}

pub(super) fn drawn<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    DRAWN_SIZES
        .into_iter()
        .filter_map(|size| {
            let (with, without) = with_and_without(rig, case, size);
            (with == without).then(|| format!("draws nothing at {}", size_label(size)))
        })
        .collect()
}

pub(super) fn contained<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    if case.look == Look::FullScreen {
        return Vec::new();
    }
    let theme = rig.theme();
    let selection = theme.selection_active | theme.selection_inactive;
    let mut out = Vec::new();
    for size in frame_sizes() {
        let (with, without) = with_and_without(rig, case, size);
        let at = size_label(size);
        let Some(rect) = frame_rect(&with, &without) else {
            out.push(format!("draws no frame at {at}"));
            continue;
        };
        // Under a step, the entry it is a step of shows its cursor row
        // in the inactive look; nothing else may change.
        let changed = |x: u16, y: u16| {
            let (shown, under) = (&with[(x, y)], &without[(x, y)]);
            shown != under && !(case.step && only_selection_changed(shown, under, selection))
        };
        let escaped = (0..with.area.height)
            .flat_map(|y| (0..with.area.width).map(move |x| (x, y)))
            .find(|&(x, y)| !rect.contains((x, y).into()) && changed(x, y));
        if let Some((x, y)) = escaped {
            out.push(format!(
                "at {at}, cell ({x},{y}) outside the frame {} changed to {:?}",
                describe(rect),
                with[(x, y)].symbol()
            ));
        }
        let right = rect.right() - 1;
        let into_side = (rect.y + 1..rect.bottom().saturating_sub(1))
            .flat_map(|y| [(rect.x, y), (right, y)])
            .find(|&(x, y)| !is_side(with[(x, y)].symbol()));
        if let Some((x, y)) = into_side {
            out.push(format!(
                "at {at}, text runs into the frame at ({x},{y}): {:?}\n{}",
                with[(x, y)].symbol(),
                buffer::text(&with)
            ));
        }
    }
    out
}

/// Whether `symbol` is a cell of a border's top or bottom rule.
fn is_rule(symbol: &str) -> bool {
    [line::NORMAL, line::ROUNDED, line::DOUBLE, line::THICK]
        .iter()
        .any(|set| set.horizontal == symbol)
}

pub(super) fn title<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    if case.look == Look::FullScreen {
        return Vec::new();
    }
    let theme = rig.theme();
    let (with, without) = with_and_without(rig, case, TITLE_SIZE);
    let Some(rect) = frame_rect(&with, &without) else {
        return vec![format!("draws no frame at {}", size_label(TITLE_SIZE))];
    };
    let mut out = Vec::new();
    let (right, bottom) = (rect.right() - 1, rect.bottom() - 1);
    for (x, y) in [
        (rect.x, rect.y),
        (right, rect.y),
        (rect.x, bottom),
        (right, bottom),
    ] {
        if with[(x, y)].fg != theme.accent {
            out.push(format!(
                "the frame's corner at ({x},{y}) is {:?}, not the accent",
                with[(x, y)].fg
            ));
        }
    }
    let cells: Vec<&Cell> = (rect.x + 1..right)
        .map(|x| &with[(x, rect.y)])
        .take_while(|cell| !is_rule(cell.symbol()))
        .collect();
    if cells.is_empty() {
        return out;
    }
    let title: String = cells.iter().map(|cell| cell.symbol()).collect();
    if title.len() <= 2 || !title.starts_with(' ') || title.starts_with("  ") {
        out.push(format!("the title needs one space before it: {title:?}"));
    }
    if !title.ends_with(' ') || title.ends_with("  ") {
        out.push(format!("the title needs one space after it: {title:?}"));
    }
    let chip = |cell: &Cell| !theme.badge.is_empty() && cell.modifier.contains(theme.badge);
    for cell in cells.iter().filter(|cell| !chip(cell)) {
        if cell.fg != theme.accent || cell.modifier.contains(Modifier::BOLD) {
            out.push(format!(
                "the title is plain text in the accent, regular weight: {title:?}"
            ));
            break;
        }
    }
    out
}

pub(super) fn selection<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    if case.look == Look::FullScreen || matches!(case.selection, Selection::Unchecked(_)) {
        return Vec::new();
    }
    let keys = [
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Down,
        KeyCode::Up,
    ];
    let mut out = Vec::new();
    let mut compared = false;
    for width in SELECTION_WIDTHS {
        let size = (width, SELECTION_HEIGHT);
        let at = size_label(size);
        let (first, without) = with_and_without(rig, case, size);
        let mut state = case.open();
        let depth = rig.depth(&state);
        let mut frames = vec![first];
        for code in keys {
            rig.press(&mut state, press(code));
            if rig.depth(&state) != depth {
                out.push(format!("at {at}, {code:?} closed the modal"));
                break;
            }
            frames.push(rig.render(&mut state, area(size)));
        }
        let rect = frame_rect(&frames[0], &without);
        for frame in &frames[1..] {
            if frame_rect(frame, &without) != rect {
                out.push(format!(
                    "at {at}, the frame moved or resized with the selection"
                ));
                break;
            }
        }
        // Rows that restyle as the selection moves hold the choices; up to
        // the last of them no text moves outside the cursor gutter. Rows
        // below carry explanations that follow the selection.
        let repainted: Vec<u16> = frames
            .windows(2)
            .flat_map(|pair| restyled_rows(&pair[0], &pair[1]))
            .collect();
        let Some(&last) = repainted.iter().max() else {
            continue;
        };
        compared = true;
        let (x0, x1) = rect.map_or((0, frames[0].area.width), |r| (r.x, r.right() - 1));
        'frames: for pair in frames.windows(2) {
            for y in 0..=last {
                for x in (x0 + 1 + GUTTER)..x1 {
                    if pair[0][(x, y)].symbol() != pair[1][(x, y)].symbol() {
                        out.push(format!(
                            "at {at}, text at ({x},{y}) changed when only the selection moved\nbefore:\n{}after:\n{}",
                            buffer::text(&pair[0]),
                            buffer::text(&pair[1])
                        ));
                        break 'frames;
                    }
                }
            }
        }
    }
    if case.selection == Selection::Moves && !compared {
        out.push("the arrow keys moved no selection, so nothing was compared".to_string());
    }
    out
}

fn tokens_of(rows: &[String]) -> BTreeSet<String> {
    rows.iter()
        .flat_map(|row| row.split_whitespace().map(str::to_string))
        .collect()
}

pub(super) fn words<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    if case.look == Look::FullScreen {
        return Vec::new();
    }
    let (wide, wide_bare) = with_and_without(rig, case, WIDE);
    let Some(wide_rect) = frame_rect(&wide, &wide_bare) else {
        return vec![format!("draws no frame at {}", size_label(WIDE))];
    };
    let whole = tokens_of(&inner_rows(&wide, wide_rect));
    let mut out = Vec::new();
    for size in frame_sizes() {
        let (with, without) = with_and_without(rig, case, size);
        let Some(rect) = frame_rect(&with, &without) else {
            continue;
        };
        let inner = usize::from(rect.width.saturating_sub(2));
        for (index, row) in inner_rows(&with, rect).iter().enumerate() {
            let Some(last) = row.split_whitespace().last() else {
                continue;
            };
            let touches_edge = row.chars().count() <= row.trim_end().chars().count() + 1;
            if !touches_edge || last.ends_with(marks::ELLIPSIS) || whole.contains(last) {
                continue;
            }
            // A word longer than the modal has to break somewhere; that
            // is wrapping, not cutting.
            let cut = whole.iter().any(|full| {
                full.len() > last.len() && full.len() <= inner && full.starts_with(last)
            });
            if cut {
                out.push(format!(
                    "at {}, row {index} ends in {last:?}, the start of a longer word, at the frame's edge\n{}",
                    size_label(size),
                    buffer::text(&with)
                ));
            }
        }
    }
    out
}

pub(super) fn keys<S>(case: &Case<S>, tables_seen: &mut Vec<&'static str>) -> Vec<String> {
    let mut out = Vec::new();
    for level in &case.levels {
        let roles = |event: KeyEvent| -> Vec<KeyRole> {
            level
                .tables
                .iter()
                .filter_map(|table| (table.role)(event))
                .collect()
        };
        out.extend(level.convention.breaks(&roles));
        let bound = roles(ctrl('c'));
        if !bound
            .first()
            .is_some_and(|role| matches!(role, KeyRole::Abort | KeyRole::Leave))
        {
            out.push(format!(
                "{}: Ctrl-C is bound to {bound:?}, not first to dismissing",
                level.convention.modal
            ));
        }
        for table in &level.tables {
            if tables_seen.contains(&table.name) {
                continue;
            }
            tables_seen.push(table.name);
            out.extend(
                table
                    .problems
                    .iter()
                    .map(|problem| format!("{} {problem}", table.name)),
            );
        }
    }
    out
}

pub(super) fn footer<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    let Some(footer) = &case.footer else {
        return Vec::new();
    };
    let (with, without) = with_and_without(rig, case, FOOTER_SIZE);
    let frame = frame_rect(&with, &without);
    let rows = match (case.look, frame) {
        // A frame's footer is in its body or, for a body with no row to
        // spare, on its bottom border.
        (Look::Framed, Some(rect)) => {
            let mut rows = inner_rows(&with, rect);
            let bottom = rect.bottom() - 1;
            rows.push(
                (rect.x..rect.right())
                    .map(|x| with[(x, bottom)].symbol())
                    .collect(),
            );
            rows
        }
        _ => buffer::rows(&with),
    };
    let mut out: Vec<String> = footer
        .segments()
        .iter()
        .filter(|segment| !rows.iter().any(|row| row.contains(&segment.text)))
        .map(|segment| {
            format!(
                "draws no `{}`, which its table's footer has, so its footer is not the generated one\n{}",
                segment.text,
                buffer::text(&with)
            )
        })
        .collect();
    if let (Look::Framed, Some(rect), true) = (case.look, frame, out.is_empty()) {
        let leaves = |segment: &Segment| leaves(case, segment);
        out.extend(footer_look(rig.theme(), footer, &with, rect, &leaves));
        out.extend(footer_fit(rig, case, footer, &leaves));
    }
    out
}

/// Whether `segment` names a key that leaves the case's modal at its
/// first level (its role there is to leave or to dismiss).
fn leaves<S>(case: &Case<S>, segment: &Segment) -> bool {
    case.levels.first().is_some_and(|level| {
        segment.keys.iter().any(|key| {
            level.tables.iter().any(|table| {
                matches!(
                    (table.role)(key.example()),
                    Some(KeyRole::Leave | KeyRole::Abort)
                )
            })
        })
    })
}

/// The rows of `rect`'s frame a footer can be drawn on, inside its sides:
/// the inner rows and the bottom border, from the bottom up.
fn footer_rows(rect: Rect) -> impl Iterator<Item = Rect> {
    let bottom = rect.bottom() - 1;
    (rect.y + 1..=bottom)
        .rev()
        .map(move |y| Rect::new(rect.x + 1, y, rect.width.saturating_sub(2), 1))
}

/// Where `segment` is drawn lowest in `rect`'s frame.
fn lowest(buf: &Buffer, rect: Rect, segment: &Segment) -> Option<Position> {
    footer_rows(rect).find_map(|row| buffer::find_in(buf, row, &segment.text))
}

/// The one look of a modal footer, read on a frame drawn at
/// [`FOOTER_SIZE`]: every segment in the theme's `muted`, regular weight,
/// with no color of its own (the color of the blank row above it, or of
/// the border it sits on); on the frame's last rows, under a blank row,
/// the keys that leave last.
fn footer_look(
    theme: &Theme,
    footer: &Footer,
    buf: &Buffer,
    rect: Rect,
    leaves: &dyn Fn(&Segment) -> bool,
) -> Vec<String> {
    let bottom = rect.bottom() - 1;
    let on_border = |at: &Position| at.y == bottom;
    let placed: Vec<(&Segment, Position)> = footer
        .segments()
        .iter()
        .filter_map(|segment| lowest(buf, rect, segment).map(|at| (segment, at)))
        .collect();
    let (Some(&(_, first)), Some(&(_, last))) = (placed.first(), placed.last()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let screen = || buffer::text(buf);
    if !on_border(&last) {
        if last.y + 1 != bottom {
            out.push(format!(
                "its footer's last row is not the frame's last row, so the footer is not on the frame's last rows\n{}",
                screen()
            ));
        }
        let above = first.y.saturating_sub(1);
        let blank = above > rect.y
            && (rect.x + 1..rect.right() - 1).all(|x| buf[(x, above)].symbol() == " ");
        if !blank {
            out.push(format!(
                "its footer has no blank row above it\n{}",
                screen()
            ));
        }
    }
    // The color a footer cell takes: the border's on the border, the
    // blank body's in the body.
    let border = buf[(rect.x, bottom)].fg;
    let body = buf[(rect.x + 1, first.y.saturating_sub(1).max(rect.y + 1))].fg;
    for (segment, at) in &placed {
        let color = if on_border(at) { border } else { body };
        let cells = u16::try_from(text::width(&segment.text)).unwrap_or(u16::MAX);
        let off = (at.x..at.x.saturating_add(cells))
            .map(|x| &buf[(x, at.y)])
            .filter(|cell| !cell.symbol().trim().is_empty())
            .any(|cell| {
                !cell.modifier.contains(theme.muted)
                    || cell.modifier.contains(Modifier::BOLD)
                    || (segment.available && cell.fg != color)
            });
        if off {
            out.push(format!(
                "its footer's `{}` is not in the footer's look: the theme's muted, regular weight, no color of its own",
                segment.text
            ));
        }
    }
    // The last segment on the footer's last row is a key that leaves,
    // when the footer names one: a footer lists them last.
    if placed.iter().any(|(segment, _)| leaves(segment)) {
        let rightmost = placed
            .iter()
            .filter(|(_, at)| at.y == last.y)
            .max_by_key(|(_, at)| at.x);
        if let Some((segment, _)) = rightmost
            && !leaves(segment)
        {
            out.push(format!(
                "its footer ends with `{}`, not with the keys that leave, which a footer lists last",
                segment.text
            ));
        }
    }
    out
}

/// The one fit of a modal footer, read on a frame drawn at
/// [`FOOTER_SHORT`]: the frame's last row (the last inner row, or the
/// bottom border) still names a key that leaves, so a footer too tall
/// for its frame keeps its last rows.
fn footer_fit<R: Rig>(
    rig: &R,
    case: &Case<R::State>,
    footer: &Footer,
    leaves: &dyn Fn(&Segment) -> bool,
) -> Vec<String> {
    let leaving: Vec<&Segment> = footer
        .segments()
        .iter()
        .filter(|segment| leaves(segment))
        .collect();
    if leaving.is_empty() {
        return Vec::new();
    }
    let (with, without) = with_and_without(rig, case, FOOTER_SHORT);
    let Some(rect) = frame_rect(&with, &without) else {
        return Vec::new();
    };
    let last: Vec<Rect> = footer_rows(rect).take(2).collect();
    let shown = leaving.iter().any(|segment| {
        last.iter()
            .any(|row| buffer::find_in(&with, *row, &segment.text).is_some())
    });
    if shown {
        return Vec::new();
    }
    vec![format!(
        "at {}, the frame's last row names none of the keys that leave ({}): a footer too tall for its frame keeps its last rows\n{}",
        size_label(FOOTER_SHORT),
        leaving
            .iter()
            .map(|segment| format!("`{}`", segment.text))
            .collect::<Vec<_>>()
            .join(", "),
        buffer::text(&with)
    )]
}

pub(super) fn hints<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    let state = case.open();
    let named = rig.hints(&state);
    let takes = |key: &crate::keys::KeyMatcher| match &case.footer {
        Some(footer) => footer
            .segments()
            .iter()
            .any(|segment| segment.keys.contains(key)),
        None => case.levels.first().is_some_and(|level| {
            level
                .tables
                .iter()
                .any(|table| (table.role)(key.example()).is_some())
        }),
    };
    let whose = if case.footer.is_some() {
        "its footer names"
    } else {
        "its tables bind"
    };
    named
        .iter()
        .filter(|key| !takes(key))
        .map(|key| {
            format!(
                "the status line names `{}`, which is not a key {whose}",
                crate::keys::key_label(key)
            )
        })
        .collect()
}

/// What pressing `key` does to the case's modal at its first level: the
/// failures, given whether it should close the modal.
fn closing<R: Rig>(rig: &R, case: &Case<R::State>, key: KeyEvent, closes: bool) -> Vec<String> {
    let mut state = case.open();
    rig.note(&mut state, HINT);
    let depth = rig.depth(&state);
    rig.press(&mut state, key);
    let name = crate::keys::key_label(&crate::keys::KeyMatcher::Exact {
        modifiers: key.modifiers,
        code: key.code,
    });
    let after = rig.depth(&state);
    match (closes, after < depth) {
        (true, false) => vec![format!("{name} did not close it")],
        (false, true) => vec![format!("{name} closed it where a level types")],
        (true, true) => {
            let said = rig.feedback(&state);
            if said.is_empty() {
                Vec::new()
            } else {
                vec![format!("closing with {name} said {said:?}")]
            }
        }
        (false, false) => Vec::new(),
    }
}

pub(super) fn close<R: Rig>(rig: &R, case: &Case<R::State>) -> Vec<String> {
    let mut out = closing(rig, case, ctrl('c'), true);
    out.extend(closing(rig, case, press(KeyCode::Esc), true));
    out.extend(closing(rig, case, press(KeyCode::Char('q')), !case.typed));
    out
}

fn describe(rect: Rect) -> String {
    format!(
        "({},{})-({},{})",
        rect.x,
        rect.y,
        rect.right() - 1,
        rect.bottom() - 1
    )
}

/// Where two drawings of one frame (`rect` in both) differ on the frame
/// itself: its top and bottom rows (title and footer rule included) and
/// its sides, compared cell by cell, symbol and style. Empty when two
/// surfaces share their chrome, as two pages of one control should.
pub fn same_chrome(a: &Buffer, b: &Buffer, rect: Rect) -> Vec<String> {
    let (right, bottom) = (
        rect.right().saturating_sub(1),
        rect.bottom().saturating_sub(1),
    );
    let rows = [rect.y, bottom]
        .into_iter()
        .flat_map(|y| (rect.x..=right).map(move |x| (x, y)));
    let sides = (rect.y..=bottom).flat_map(|y| [(rect.x, y), (right, y)]);
    let mut out: Vec<String> = Vec::new();
    for (x, y) in rows.chain(sides) {
        let (left, other) = (&a[(x, y)], &b[(x, y)]);
        if left != other {
            let finding = format!(
                "({x},{y}): {:?} {:?} against {:?} {:?}",
                left.symbol(),
                left.style(),
                other.symbol(),
                other.style()
            );
            if !out.contains(&finding) {
                out.push(finding);
            }
        }
    }
    out
}
