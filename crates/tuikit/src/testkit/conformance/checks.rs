//! The checks of the conformance suite, one function each. Each returns
//! its failures as messages; the suite names the case and the check.

use std::collections::BTreeSet;

use ratatui::buffer::{Buffer, Cell};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::symbols::line;

use super::{Case, HEIGHTS, KeyRole, Look, Rig, Selection, WIDTHS};
use crate::glyphs::marks;
use crate::testkit::buffer::{self, frame_rect, inner_rows, is_side, restyled_rows};
use crate::testkit::keys::{ctrl, press};

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
/// closed.
fn with_and_without<R: Rig>(rig: &R, case: &Case<R::State>, size: (u16, u16)) -> (Buffer, Buffer) {
    let mut state = case.open();
    let with = rig.render(&mut state, area(size));
    rig.close_top(&mut state);
    let without = rig.render(&mut state, area(size));
    (with, without)
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
    let mut out = Vec::new();
    for size in frame_sizes() {
        let (with, without) = with_and_without(rig, case, size);
        let at = size_label(size);
        let Some(rect) = frame_rect(&with, &without) else {
            out.push(format!("draws no frame at {at}"));
            continue;
        };
        let escaped = (0..with.area.height)
            .flat_map(|y| (0..with.area.width).map(move |x| (x, y)))
            .find(|&(x, y)| !rect.contains((x, y).into()) && with[(x, y)] != without[(x, y)]);
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
    let rows = match (case.look, frame_rect(&with, &without)) {
        (Look::Framed, Some(rect)) => inner_rows(&with, rect),
        _ => buffer::rows(&with),
    };
    footer
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
