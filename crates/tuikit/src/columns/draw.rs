//! Laying a row out: which columns show, how wide each is, and the row's
//! spans.

use ratatui::style::Style;
use ratatui::text::Span;

use super::cell::Cell;
use super::measure::{Measure, lookup};
use super::schema::{Align, Column, ColumnSchema, Width};
use crate::text;
use crate::theme::Theme;

/// The cells a row is laid out in after its lead: the cells its columns'
/// width rules count, and the cells its cells fit in.
///
/// The two are one number for most rows: a `usize` room converts to a
/// `Room` of that many cells. They part for a list that gives the end of
/// its rows to something only some of the time, such as a scrollbar's
/// column carved from a list only while its rows overflow:
/// [`Room::new(width).beside(1)`](Self::beside) lays such a row out by
/// its width rules over the whole width (a column's steps, its share of
/// the row, the rooms a column leaves below), so no column moves when the
/// bar comes or goes, and then fits the row's cells into the width less
/// the bar's column by the one narrow-width policy: the fill gives its
/// last cell first, and where it is at its minimum the policy's next
/// column gives way.
///
/// ```
/// use ratatui::style::Style;
/// use tuikit::columns::{Cell, Column, ColumnSchema, Measure, Room};
/// use tuikit::theme::Theme;
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// enum Col {
///     Name,
///     Note,
/// }
///
/// // A name that takes a quarter of the row and a note in the rest.
/// let schema = ColumnSchema::new([
///     Column::share(Col::Name, 1, 4).within(4, 20),
///     Column::fill(Col::Note),
/// ]);
/// let cells = vec![
///     (Col::Name, Cell::text("alphabetical", Style::default())),
///     (Col::Note, Cell::text("a note that runs on and on past the end", Style::default())),
/// ];
/// let mut measure = Measure::new();
/// measure.add(&schema, 0, &cells);
/// let theme = Theme::default();
/// let row = |room: Room| Cell::plain(&schema.draw(&measure, 0, room, &cells, &theme));
///
/// // Laid out in 40 cells, the name takes ten; in 39 it takes nine.
/// assert_eq!(row(Room::new(40)), "alphabeti…  a note that runs on and on …");
/// assert_eq!(row(Room::new(39)), "alphabet…  a note that runs on and on …");
/// // Beside a bar's column the name keeps the ten of 40 cells, and the
/// // note gives the bar its cell.
/// let beside = Room::new(40).beside(1);
/// assert_eq!(beside.cells(), 39);
/// assert_eq!(row(beside), "alphabeti…  a note that runs on and on…");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room {
    whole: usize,
    beside: usize,
}

impl Room {
    /// A room of `cells`: the width rules count them and the cells fit in
    /// them.
    pub const fn new(cells: usize) -> Self {
        Self {
            whole: cells,
            beside: 0,
        }
    }

    /// The room with its last `cells` given to something drawn beside the
    /// row (a scrollbar's column): the width rules still count them, the
    /// row's cells no longer fit in them.
    #[must_use]
    pub const fn beside(self, cells: usize) -> Self {
        Self {
            whole: self.whole,
            beside: self.beside + cells,
        }
    }

    /// The cells the row's cells fit in.
    pub const fn cells(&self) -> usize {
        self.whole.saturating_sub(self.beside)
    }

    /// The cells the width rules count, those beside the row included.
    pub const fn whole(&self) -> usize {
        self.whole
    }

    /// The room less `cells` at its start, on both counts.
    const fn after(self, cells: usize) -> Self {
        Self {
            whole: self.whole.saturating_sub(cells),
            beside: self.beside,
        }
    }

    /// The room with `cells` more at its start, on both counts.
    const fn widened(self, cells: usize) -> Self {
        Self {
            whole: self.whole + cells,
            beside: self.beside,
        }
    }
}

impl From<usize> for Room {
    fn from(cells: usize) -> Self {
        Self::new(cells)
    }
}

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
/// still shows, takes what is left; a fill that does not show hands it to
/// the column it [gives its cells to](Column::gives_to).
fn give_way<K: Copy + Eq>(columns: &[Column<K>], items: &mut [Item], room: usize, lead_gap: usize) {
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
    let is_fill = |item: &Item| matches!(columns[item.index].width, Width::Fill);
    let taker = items
        .iter()
        .position(|item| item.width.is_some() && is_fill(item))
        .or_else(|| {
            let key = items
                .iter()
                .filter(|item| item.width.is_none() && is_fill(item))
                .find_map(|item| columns[item.index].gives_to)?;
            items
                .iter()
                .position(|item| item.width.is_some() && columns[item.index].key == key)
        });
    if let Some(position) = taker {
        let item = &mut items[position];
        item.width = item.width.map(|width| width + slack);
    }
}

/// Gives each share of what is left (`pending`, positions in `items`
/// whose width is still to come) its cells: `per` of every `of` of the
/// `room` the other shown columns leave, less the gaps beside them and
/// the `lead_gap` before the first, kept between its bounds. The shares
/// and the fills are not the other columns: they split what is left, the
/// gaps between them included.
fn share_the_rest<K>(
    columns: &[Column<K>],
    items: &mut [Item],
    pending: &[usize],
    room: usize,
    lead_gap: usize,
) {
    if pending.is_empty() {
        return;
    }
    let flexible = |position: usize, items: &[Item]| {
        pending.contains(&position) || matches!(columns[items[position].index].width, Width::Fill)
    };
    let shown: Vec<usize> = (0..items.len())
        .filter(|position| items[*position].width.is_some() || pending.contains(position))
        .collect();
    let mut taken = if shown.is_empty() { 0 } else { lead_gap };
    for (n, &position) in shown.iter().enumerate() {
        if !flexible(position, items) {
            taken += items[position].width.unwrap_or(0);
        }
        let apart = shown
            .get(n + 1)
            .is_some_and(|&next| !(flexible(position, items) && flexible(next, items)));
        if apart {
            taken += columns[items[position].index].gap;
        }
    }
    let rest = room.saturating_sub(taken);
    for &position in pending {
        let Width::ShareOfRest { per, of, min, max } = columns[items[position].index].width else {
            continue;
        };
        let share = rest.saturating_mul(per).checked_div(of).unwrap_or(0);
        let width = Some(share.max(min).min(max)).filter(|cells| *cells > 0);
        let item = &mut items[position];
        item.wanted = width.unwrap_or(0);
        item.width = width;
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

/// The cells a column whose width follows the whole row's takes in a row
/// `row` cells wide, its lead included; `None` for the other rules, and
/// for a share or step that comes to nothing.
fn of_row(width: Width, row: usize) -> Option<usize> {
    match width {
        Width::RowSteps(steps) => step(steps, row),
        Width::Share { per, of, min, max } => {
            let share = row.saturating_mul(per).checked_div(of).unwrap_or(0);
            Some(share.max(min).min(max)).filter(|cells| *cells > 0)
        }
        _ => None,
    }
}

impl<K: Copy + Eq> ColumnSchema<K> {
    /// The columns that line up, laid out for `room` after a `lead`. When
    /// one of them absorbs the lead, they are laid out as the list's
    /// shallowest row would be (after the lead every row shares, the
    /// measure's least) in the whole width (`lead + room`), and the row's
    /// own lead is taken off the column that absorbs it: rows at every
    /// depth then give way alike, at the same widths, and the columns
    /// after it stay in one place, though a deeper row's cell may narrow
    /// below its floor.
    fn aligned(&self, measure: &Measure<K>, lead: usize, room: Room) -> Vec<Item> {
        let absorbs =
            |column: &Column<K>| column.absorbs_lead && matches!(column.width, Width::Cap(_));
        let shared = measure.least_lead().min(lead);
        if lead == shared || !self.columns[..self.split].iter().any(absorbs) {
            return self.laid_out(measure, lead, room);
        }
        let own = lead - shared;
        let mut items = self.laid_out(measure, shared, room.widened(own));
        for item in items
            .iter_mut()
            .filter(|item| absorbs(&self.columns[item.index]))
        {
            item.width = item.width.map(|width| width.saturating_sub(own));
            item.wanted = item.wanted.saturating_sub(own);
            item.own = item.own.saturating_sub(own);
        }
        items
    }

    /// The columns that line up, laid out for `room` after a `lead` from
    /// the widths in `measure`: each at its width (a capped column at its
    /// widest cell, less the lead when it absorbs it, a stepped one at its
    /// step, a share of the whole row at its share, a fill at its minimum
    /// until it takes what is left), then given way by the policy until
    /// they fit. The width rules count the room's whole width; the columns
    /// fit in its cells.
    fn laid_out(&self, measure: &Measure<K>, lead: usize, room: Room) -> Vec<Item> {
        let row = lead + room.whole();
        let mut items = Vec::with_capacity(self.split);
        let mut pending = Vec::new();
        let mut used = 0;
        let mut gap: Option<usize> = None;
        for (index, column) in self.columns[..self.split].iter().enumerate() {
            let left = room.whole().saturating_sub(used + gap.unwrap_or(0));
            let wanted = if column.drop_below > room.whole() {
                None
            } else if matches!(column.width, Width::ShareOfRest { .. }) {
                pending.push(items.len());
                None
            } else {
                match column.width {
                    Width::Fixed(cells) => Some(cells),
                    // The measure counts each cell at most its cap, so a
                    // column that absorbs the lead is as wide as it takes
                    // to end where the deepest capped cell does.
                    Width::Cap(max) => measure.any(column.key).then(|| {
                        if column.absorbs_lead {
                            measure.widest_with_lead(column.key).saturating_sub(lead)
                        } else {
                            measure.widest(column.key).min(max)
                        }
                    }),
                    Width::Steps(steps) => step(steps, left),
                    Width::RowSteps(_) | Width::Share { .. } => of_row(column.width, row),
                    Width::Fill => measure.any(column.key).then_some(column.fill_min),
                    Width::Hug(_) | Width::ShareOfRest { .. } => None,
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
        share_the_rest(&self.columns, &mut items, &pending, room.whole(), 0);
        for &position in &pending {
            items[position].own = items[position].wanted;
        }
        give_way(&self.columns, &mut items, room.cells(), 0);
        items
    }

    /// The flowing columns of one row (from the first hugging column on),
    /// laid out for the `room` the lined-up ones leave after `lead_gap`,
    /// in a row laid out in `whole` cells after its lead and `row` cells
    /// wide with it: each at its own cell's width, then given way by the
    /// policy, row by row. A blank cell takes no cells and no gap.
    fn flowing(
        &self,
        cells: &[(K, Cell)],
        (whole, row): (usize, usize),
        room: Room,
        lead_gap: usize,
    ) -> Vec<Item> {
        let mut items = Vec::new();
        let mut pending = Vec::new();
        let mut used = 0;
        let mut gap = lead_gap;
        for (index, column) in self.columns.iter().enumerate().skip(self.split) {
            if column.drop_below > whole {
                continue;
            }
            let Some(cell) = lookup(cells, column.key).filter(|cell| !cell.is_empty()) else {
                continue;
            };
            if matches!(column.width, Width::ShareOfRest { .. }) {
                pending.push(items.len());
                items.push(Item {
                    index,
                    wanted: 0,
                    width: None,
                    own: cell.width(),
                });
                continue;
            }
            let left = room.whole().saturating_sub(used + gap);
            let wanted = match column.width {
                Width::Fixed(cells) => Some(cells),
                Width::Cap(max) | Width::Hug(max) => Some(cell.width().min(max)),
                Width::Steps(steps) => step(steps, left),
                Width::RowSteps(_) | Width::Share { .. } => of_row(column.width, row),
                Width::Fill => Some(column.fill_min),
                Width::ShareOfRest { .. } => None,
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
        share_the_rest(&self.columns, &mut items, &pending, room.whole(), lead_gap);
        for &position in &pending {
            let item = &mut items[position];
            item.own = item.own.min(item.wanted).max(1);
        }
        give_way(&self.columns, &mut items, room.cells(), lead_gap);
        items
    }

    /// The lined-up columns of a row with a `lead` of its own (a tree
    /// row's indent, a list's cursor gutter; 0 when the list draws nothing
    /// before the schema) laid out for the `room` after it from the widths
    /// in `measure`: which show, and how wide. The room is a number of
    /// cells, or a [`Room`] whose width rules count cells beside the row.
    pub fn layout(&self, measure: &Measure<K>, lead: usize, room: impl Into<Room>) -> Layout<K> {
        let room = room.into();
        let items = self.aligned(measure, lead, room);
        Layout {
            room: room.cells(),
            columns: items
                .iter()
                .map(|item| (self.columns[item.index].key, item.width))
                .collect(),
        }
    }

    /// One row's spans: its `cells` laid out in the `room` cells after
    /// its `lead` (a tree row's indent or a list's cursor gutter, which the
    /// list draws; 0 when it draws nothing before the schema) by the schema
    /// and the widths in `measure`, cut and padded, never wider than
    /// `room`. A share and a row-stepped column count the lead in the
    /// row's width.
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
        room: impl Into<Room>,
        cells: &[(K, Cell)],
        theme: &Theme,
    ) -> Vec<Span<'static>> {
        let room = room.into();
        let (spans, used, _) =
            self.draw_row(measure, (lead, room), cells, theme, Ends::AtLastCell(None));
        if used > room.cells() {
            text::fit_spans(spans, room.cells())
        } else {
            spans
        }
    }

    /// [`draw`](Self::draw) parted where the column under `at` starts:
    /// the spans of the columns before it (their gaps included), and the
    /// spans from it to the row's end, for a host that gives part of a row
    /// a look of its own, such as a dimmed body after marks that keep
    /// their colors. When `at`'s cell does not show on the row (it is
    /// blank or gave way), the second part starts at the next column that
    /// shows, and it is empty when none does. The two parts joined are
    /// what `draw` returns.
    pub fn draw_split(
        &self,
        measure: &Measure<K>,
        lead: usize,
        room: impl Into<Room>,
        cells: &[(K, Cell)],
        theme: &Theme,
        at: K,
    ) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
        let room = room.into();
        let from = self.index_of(at).unwrap_or(self.columns.len());
        let (spans, used, split) = self.draw_row(
            measure,
            (lead, room),
            cells,
            theme,
            Ends::AtLastCell(Some(from)),
        );
        let mut spans = if used > room.cells() {
            text::fit_spans(spans, room.cells())
        } else {
            spans
        };
        let rest = spans.split_off(split.min(spans.len()));
        (spans, rest)
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
        room: impl Into<Room>,
        cells: &[(K, Cell)],
        theme: &Theme,
    ) -> (Vec<Span<'static>>, usize) {
        let room = room.into();
        let (spans, used, _) = self.draw_row(measure, (lead, room), cells, theme, Ends::Continued);
        (spans, room.cells().saturating_sub(used))
    }

    /// The row's spans, the cells they take, and the index of the first
    /// span of the part `ends` asks for (the spans' count when none), after
    /// its lead and in its room (`place`).
    fn draw_row(
        &self,
        measure: &Measure<K>,
        place: (usize, Room),
        cells: &[(K, Cell)],
        theme: &Theme,
        ends: Ends,
    ) -> (Vec<Span<'static>>, usize, usize) {
        let (lead, room) = place;
        let (continued, from) = match ends {
            Ends::Continued => (true, None),
            Ends::AtLastCell(from) => (false, from),
        };
        let mut aligned = self.aligned(measure, lead, room);
        let lined_up = total(&self.columns, &aligned, 0);
        let lead_gap = aligned
            .iter()
            .rev()
            .find(|item| item.width.is_some())
            .map_or(0, |item| self.columns[item.index].gap);
        let flow = self.flowing(
            cells,
            (room.whole(), lead + room.whole()),
            room.after(lined_up),
            if lined_up > 0 { lead_gap } else { 0 },
        );
        if !continued && !flow.iter().any(|item| item.width.is_some()) {
            self.keep_ragged_end(&mut aligned, cells, room);
        }
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
        let mut split = None;
        for (position, &(index, width)) in drawn[..last].iter().enumerate() {
            if let Some(gap) = gap.filter(|gap| *gap > 0) {
                spans.push(Span::raw(" ".repeat(gap)));
                used += gap;
            }
            if split.is_none() && from.is_some_and(|from| index >= from) {
                split = Some(spans.len());
            }
            let column = &self.columns[index];
            let cell = lookup(cells, column.key).unwrap_or(&empty);
            let ends = !continued && position + 1 == last;
            // A capped cell is cut to its cap, and padded to its column's
            // width (wider than the cap where the column absorbs a lead).
            let cap = match column.width {
                Width::Cap(max) => max,
                _ => usize::MAX,
            };
            let (cut_to, padded) = match column.width {
                Width::Hug(_) => (width, false),
                Width::Fill => (width, !ends),
                _ if column.ragged && ends => (room.cells().saturating_sub(used).min(cap), false),
                _ => (width.min(cap), true),
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
        let split = split.unwrap_or(spans.len());
        (spans, used, split)
    }
}

/// How a drawn row ends.
#[derive(Clone, Copy, Debug)]
enum Ends {
    /// At its last cell that is not blank, with no gap after it; parted
    /// where the column at the schema index starts, when there is one.
    AtLastCell(Option<usize>),
    /// The host draws on after it: every column that shows is drawn and
    /// the gap after the last.
    Continued,
}

impl<K: Copy + Eq> ColumnSchema<K> {
    /// Brings back a ragged column that gave way for the columns after it,
    /// on a row where it is the last cell that shows: nothing after it
    /// needs it out of the way, so it runs on into the room the row has
    /// left, when that holds its floor (or its whole text, when shorter).
    fn keep_ragged_end(&self, aligned: &mut [Item], cells: &[(K, Cell)], room: Room) {
        let cell_of = |item: &Item| {
            lookup(cells, self.columns[item.index].key).filter(|cell| !cell.is_empty())
        };
        let Some(position) = aligned.iter().rposition(|item| cell_of(item).is_some()) else {
            return;
        };
        let item = aligned[position];
        let column = &self.columns[item.index];
        if item.width.is_some() || !column.ragged || column.drop_below > room.whole() {
            return;
        }
        let own = cell_of(&item).map_or(0, Cell::width);
        let before = &aligned[..position];
        let gap = before
            .iter()
            .rev()
            .find(|item| item.width.is_some())
            .map_or(0, |item| self.columns[item.index].gap);
        let left = room
            .cells()
            .saturating_sub(total(&self.columns, before, 0) + gap);
        let floor = column.shrink.map_or(1, |(_, floor)| floor).min(own).max(1);
        if left >= floor {
            aligned[position].width = Some(0);
        }
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
