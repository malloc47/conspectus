//! Laying a row out: which columns show, how wide each is, and the row's
//! spans.

use ratatui::style::Style;
use ratatui::text::Span;

use super::MIN_FILL;
use super::cell::Cell;
use super::measure::{Measure, lookup};
use super::schema::{Align, Column, ColumnSchema, Width};
use crate::text;
use crate::theme::Theme;

/// A schema laid out for one room: the columns that line up across rows,
/// each shown at a width or left out.
///
/// Every row drawn from one schema and one measure in the same room shows
/// the same lined-up columns at the same widths, whatever its own cells
/// hold; [`ColumnSchema::layout`] is that decision on its own, for a host
/// or a test that wants to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout<K> {
    room: usize,
    columns: Vec<(K, Option<usize>)>,
}

impl<K: Copy + Eq> Layout<K> {
    /// The cells the row is laid out in.
    pub const fn room(&self) -> usize {
        self.room
    }

    /// The width of the lined-up column under `key`, or `None` when it
    /// gave way, has nothing to show, or does not line up (a hugging
    /// column and the ones after it).
    pub fn width(&self, key: K) -> Option<usize> {
        self.columns
            .iter()
            .find(|(have, _)| *have == key)
            .and_then(|(_, width)| *width)
    }

    /// Whether the lined-up column under `key` shows.
    pub fn is_shown(&self, key: K) -> bool {
        self.width(key).is_some()
    }
}

/// One column as the layout handles it: where it is in the schema, the
/// width it wanted, the width it has (`None` once it gave way), and the
/// cells its own text takes, which is all it needs when it ends the row
/// (the pad after a row's last cell runs past its edge unseen).
#[derive(Clone, Copy, Debug)]
struct Item {
    index: usize,
    wanted: usize,
    width: Option<usize>,
    own: usize,
}

/// Where the last shown item is.
fn last_shown(items: &[Item]) -> Option<usize> {
    items.iter().rposition(|item| item.width.is_some())
}

/// The cells item `position` counts for: its width, or only its own text
/// when it ends the row.
fn counted(items: &[Item], position: usize, width: usize) -> usize {
    if last_shown(items) == Some(position) {
        width.min(items[position].own)
    } else {
        width
    }
}

/// The cells the shown `items` take, with the gaps between them and
/// `lead_gap` before the first one when any shows.
fn total<K>(columns: &[Column<K>], items: &[Item], lead_gap: usize) -> usize {
    let mut total = 0;
    let mut gap = lead_gap;
    let mut any = false;
    for (position, item) in items.iter().enumerate() {
        if let Some(width) = item.width {
            total += gap + counted(items, position, width);
            gap = columns[item.index].gap;
            any = true;
        }
    }
    if any { total } else { 0 }
}

/// Makes `items` fit in `room` by the one policy: the shown columns give
/// way in ascending priority (a later column before an earlier one of the
/// same priority, a narrowing before a leaving), each narrowing no more
/// than the row needs and leaving whole, until the row fits. A column
/// that narrowed takes back what a later leaving freed. The fill, if it
/// still shows, takes what is left.
fn give_way<K>(columns: &[Column<K>], items: &mut [Item], room: usize, lead_gap: usize) {
    // (priority, position, leaves, floor)
    let mut turns: Vec<(u8, usize, bool, usize)> = Vec::new();
    for (position, item) in items.iter().enumerate() {
        if item.width.is_none() {
            continue;
        }
        let column = &columns[item.index];
        if let Some((priority, floor)) = column.shrink {
            turns.push((priority, position, false, floor));
        }
        if let Some(priority) = column.drop {
            turns.push((priority, position, true, 0));
        }
        // A fill with no rule of its own gives way last of all, narrowing
        // below its minimum until it is gone rather than push the row
        // past its edge.
        if matches!(column.width, Width::Fill) && column.shrink.is_none() && column.drop.is_none() {
            turns.push((u8::MAX, position, false, 0));
        }
    }
    turns.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    let mut narrowed: Vec<usize> = Vec::new();
    for (_, position, leaves, floor) in turns {
        let used = total(columns, items, lead_gap);
        if used <= room {
            break;
        }
        let Some(width) = items[position].width else {
            continue;
        };
        if leaves {
            items[position].width = None;
            continue;
        }
        let over = used - room;
        let width = counted(items, position, width);
        // A cell whose whole text is narrower than the floor may narrow to
        // its text: the floor keeps a cut from leaving only the mark.
        let floor = floor.min(items[position].own).min(width);
        let narrower = width.saturating_sub(over).max(floor);
        items[position].width = (narrower > 0).then_some(narrower);
        narrowed.push(position);
    }
    let mut slack = room.saturating_sub(total(columns, items, lead_gap));
    for &position in narrowed.iter().rev() {
        let item = &mut items[position];
        // A column that narrowed to nothing left the row with its gap;
        // only one still shown takes cells back.
        let Some(width) = item.width else {
            continue;
        };
        let back = slack.min(item.wanted - width);
        item.width = Some(width + back);
        slack -= back;
    }
    if let Some(item) = items
        .iter_mut()
        .find(|item| item.width.is_some() && matches!(columns[item.index].width, Width::Fill))
    {
        item.width = item.width.map(|width| width + slack);
    }
}

/// The cells a stepped column takes with `left` cells left from it on.
fn step(steps: &[(usize, usize)], left: usize) -> Option<usize> {
    steps
        .iter()
        .find(|(from, _)| left >= *from)
        .map(|(_, cells)| *cells)
        .filter(|cells| *cells > 0)
}

impl<K: Copy + Eq> ColumnSchema<K> {
    /// The columns that line up, laid out for `room` after a `lead` from
    /// the widths in `measure`: each at its width (a capped column at its
    /// widest cell, less the lead when it absorbs it, a stepped one at its
    /// step, a fill at [`MIN_FILL`] until it takes what is left), then given
    /// way by the policy until they fit.
    fn aligned(&self, measure: &Measure<K>, lead: usize, room: usize) -> Vec<Item> {
        let mut items = Vec::with_capacity(self.split);
        let mut used = 0;
        let mut gap: Option<usize> = None;
        for (index, column) in self.columns[..self.split].iter().enumerate() {
            let left = room.saturating_sub(used + gap.unwrap_or(0));
            let wanted = if column.drop_below > room {
                None
            } else {
                match column.width {
                    Width::Fixed(cells) => Some(cells),
                    Width::Cap(max) => measure.any(column.key).then(|| {
                        let widest = if column.absorbs_lead {
                            measure.widest_with_lead(column.key).saturating_sub(lead)
                        } else {
                            measure.widest(column.key)
                        };
                        widest.min(max)
                    }),
                    Width::Steps(steps) => step(steps, left),
                    Width::Fill => measure.any(column.key).then_some(MIN_FILL),
                    Width::Hug(_) => None,
                }
            };
            if let Some(width) = wanted {
                used += gap.unwrap_or(0) + width;
                gap = Some(column.gap);
            }
            items.push(Item {
                index,
                wanted: wanted.unwrap_or(0),
                width: wanted,
                own: wanted.unwrap_or(0),
            });
        }
        give_way(&self.columns, &mut items, room, 0);
        items
    }

    /// The flowing columns of one row (from the first hugging column on),
    /// laid out for the `room` the lined-up ones leave after `lead_gap`:
    /// each at its own cell's width, then given way by the policy, row by
    /// row. A blank cell takes no cells and no gap.
    fn flowing(
        &self,
        cells: &[(K, Cell)],
        whole: usize,
        room: usize,
        lead_gap: usize,
    ) -> Vec<Item> {
        let mut items = Vec::new();
        let mut used = 0;
        let mut gap = lead_gap;
        for (index, column) in self.columns.iter().enumerate().skip(self.split) {
            if column.drop_below > whole {
                continue;
            }
            let Some(cell) = lookup(cells, column.key).filter(|cell| !cell.is_empty()) else {
                continue;
            };
            let left = room.saturating_sub(used + gap);
            let wanted = match column.width {
                Width::Fixed(cells) => Some(cells),
                Width::Cap(max) | Width::Hug(max) => Some(cell.width().min(max)),
                Width::Steps(steps) => step(steps, left),
                Width::Fill => Some(MIN_FILL),
            };
            let Some(width) = wanted.filter(|width| *width > 0) else {
                continue;
            };
            used += gap + width;
            gap = column.gap;
            items.push(Item {
                index,
                wanted: width,
                width: Some(width),
                own: cell.width().min(width).max(1),
            });
        }
        give_way(&self.columns, &mut items, room, lead_gap);
        items
    }

    /// The lined-up columns of a row with a `lead` of its own (a tree
    /// row's indent; 0 for a flat list) laid out for the `room` after it
    /// from the widths in `measure`: which show, and how wide.
    pub fn layout(&self, measure: &Measure<K>, lead: usize, room: usize) -> Layout<K> {
        let items = self.aligned(measure, lead, room);
        Layout {
            room,
            columns: items
                .iter()
                .map(|item| (self.columns[item.index].key, item.width))
                .collect(),
        }
    }

    /// One row's spans: its `cells` laid out in the `room` cells after
    /// its `lead` (a tree row's indent, which the host draws; 0 for a flat
    /// list) by the schema and the widths in `measure`, cut and padded,
    /// never wider than `room`.
    ///
    /// The lined-up columns take the widths every row in `room` takes; a
    /// blank cell among them is padded while a later cell on the row shows,
    /// so the columns after it stay in line, and the row ends at its last
    /// cell that is not blank. The cells after the first hugging column
    /// flow, and give way row by row. A row still too wide once nothing
    /// more gives way is cut at its edge with `…`.
    pub fn draw(
        &self,
        measure: &Measure<K>,
        lead: usize,
        room: usize,
        cells: &[(K, Cell)],
        theme: &Theme,
    ) -> Vec<Span<'static>> {
        let (spans, used) = self.draw_row(measure, (lead, room), cells, theme, false);
        if used > room {
            text::fit_spans(spans, room)
        } else {
            spans
        }
    }

    /// [`draw`](Self::draw) for a row that the host goes on to draw
    /// itself: every column that shows is drawn (a blank one padded) and
    /// the gap after the last, and the cells left in `room` after them are
    /// returned beside the spans. The row is not cut: what follows is the
    /// host's to fit. A schema drawn this way has no fill, which would
    /// leave the host nothing.
    pub fn draw_then(
        &self,
        measure: &Measure<K>,
        lead: usize,
        room: usize,
        cells: &[(K, Cell)],
        theme: &Theme,
    ) -> (Vec<Span<'static>>, usize) {
        let (spans, used) = self.draw_row(measure, (lead, room), cells, theme, true);
        (spans, room.saturating_sub(used))
    }

    /// The row's spans and the cells they take, after its lead and in its
    /// room (`place`).
    fn draw_row(
        &self,
        measure: &Measure<K>,
        place: (usize, usize),
        cells: &[(K, Cell)],
        theme: &Theme,
        continued: bool,
    ) -> (Vec<Span<'static>>, usize) {
        let (lead, room) = place;
        let aligned = self.aligned(measure, lead, room);
        let lined_up = total(&self.columns, &aligned, 0);
        let lead_gap = aligned
            .iter()
            .rev()
            .find(|item| item.width.is_some())
            .map_or(0, |item| self.columns[item.index].gap);
        let flow = self.flowing(
            cells,
            room,
            room.saturating_sub(lined_up),
            if lined_up > 0 { lead_gap } else { 0 },
        );
        let drawn: Vec<(usize, usize)> = aligned
            .iter()
            .chain(flow.iter())
            .filter_map(|item| item.width.map(|width| (item.index, width)))
            .collect();
        let shows = |index: usize| {
            lookup(cells, self.columns[index].key).is_some_and(|cell| !cell.is_empty())
        };
        let last = if continued {
            drawn.len()
        } else {
            drawn
                .iter()
                .rposition(|(index, _)| shows(*index))
                .map_or(0, |position| position + 1)
        };
        let empty = Cell::empty();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut used = 0;
        let mut gap: Option<usize> = None;
        for (position, &(index, width)) in drawn[..last].iter().enumerate() {
            if let Some(gap) = gap.filter(|gap| *gap > 0) {
                spans.push(Span::raw(" ".repeat(gap)));
                used += gap;
            }
            let column = &self.columns[index];
            let cell = lookup(cells, column.key).unwrap_or(&empty);
            let ends = !continued && position + 1 == last;
            let (cut_to, padded) = match column.width {
                Width::Hug(_) => (width, false),
                Width::Fill => (width, !ends),
                _ if column.ragged && ends => (room.saturating_sub(used), false),
                _ => (width, true),
            };
            let mut body = cell.render(cut_to, theme);
            if padded {
                pad(&mut body, width, column.align, cell.pad_style());
            }
            used += text::spans_width(&body);
            spans.extend(body);
            gap = Some(column.gap);
        }
        if continued && let Some(gap) = gap.filter(|gap| *gap > 0) {
            spans.push(Span::raw(" ".repeat(gap)));
            used += gap;
        }
        (spans, used)
    }
}

/// Pads `spans` to `width` cells at the edge `align` keeps away from, in
/// `style`.
fn pad(spans: &mut Vec<Span<'static>>, width: usize, align: Align, style: Style) {
    let short = width.saturating_sub(text::spans_width(spans));
    if short == 0 {
        return;
    }
    let fill = Span::styled(" ".repeat(short), style);
    match align {
        Align::Left => spans.push(fill),
        Align::Right => spans.insert(0, fill),
    }
}
