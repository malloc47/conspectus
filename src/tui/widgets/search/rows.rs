//! What a search result looks like: quick select's rows of the view and
//! entities, and content search's hits, each laid out for the width the
//! palette gives it. A hit's columns are one declaration ([`hit_columns`])
//! and its cells one builder ([`Hit::cells`]), which the Search view's hit,
//! pane, and pane-hit rows lay out too.

use std::ops::Range;

use ratatui::macros::span;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::age::Recency;
use tuikit::chip;
use tuikit::columns::{Cell, Cells, Column, ColumnSchema, Measure};
use tuikit::palette::RowContext;
use tuikit::selection::GUTTER_WIDTH;
use tuikit::text;

use super::{ContentRow, ContentTarget, OutcomeWords, SearchOutcome};
use crate::tui::Theme;
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::rows::RowId;
use crate::tui::search::SearchMatch;
use crate::tui::search_entities::{EntityMatch, SearchEntity};
use crate::tui::widgets::badge::harness_badge;

/// One result of quick select.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum QuickRow {
    /// A row of the current view that matched: its label and the text it
    /// was found in, as the view's rows say now.
    Row {
        hit: SearchMatch,
        label: String,
        haystack: String,
    },
    /// An entity found anywhere.
    Entity {
        hit: EntityMatch,
        entity: SearchEntity,
    },
}

impl QuickRow {
    /// What picking the row asks the host to do.
    pub(super) fn pick(&self) -> SearchOutcome {
        match self {
            Self::Row { hit, .. } => SearchOutcome::Confirm(Box::new(hit.id.clone())),
            Self::Entity { entity, .. } => SearchOutcome::OpenEntity(Box::new(entity.node.clone())),
        }
    }

    /// The row's cells, `row.width` wide; an entity ends with the word
    /// `outcomes` says for it.
    pub(super) fn cells(
        &self,
        row: &RowContext<'_>,
        theme: &Theme,
        outcomes: Option<OutcomeWords<'_>>,
    ) -> Vec<Span<'static>> {
        match self {
            Self::Row {
                hit,
                label,
                haystack,
            } => match_cells(label, haystack, hit, row.width, theme),
            Self::Entity { hit, entity } => {
                let word = outcomes
                    .map(|words| words(&entity.node))
                    .unwrap_or_default();
                entity_cells(entity, hit, &word, row.width + GUTTER_WIDTH, theme)
            }
        }
    }
}

/// Widths of the columns in an entity result, left to right after the
/// two-cell cursor mark and two-cell kind glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EntityColumns {
    pub label: usize,
    pub context: usize,
    pub outcome: usize,
}

/// The widest an outcome word gets (`in Sessions view`).
pub(super) const OUTCOME_WIDTH: usize = 16;
/// Cursor mark plus kind glyph.
pub(super) const ROW_LEAD: usize = 4;
pub(super) const COLUMN_GAP: usize = 2;
/// The label column stops growing here, so a wide modal gives the room to
/// the context instead of padding.
const LABEL_CAP: usize = 48;
/// A context narrower than this is not worth showing.
const MIN_CONTEXT: usize = 10;

/// Split a row `inner` cells wide into columns. The outcome word keeps its
/// width; the label takes up to 45% of the rest (at most [`LABEL_CAP`]) and
/// the context what is left, dropped when too narrow to read.
pub(crate) fn entity_columns(inner: usize) -> EntityColumns {
    let outcome = OUTCOME_WIDTH.min(inner / 3);
    let body = inner.saturating_sub(ROW_LEAD + outcome + COLUMN_GAP);
    let label = (body * 45 / 100).clamp(16, LABEL_CAP).min(body);
    let context = body.saturating_sub(label + COLUMN_GAP);
    if context < MIN_CONTEXT {
        EntityColumns {
            label: body,
            context: 0,
            outcome,
        }
    } else {
        EntityColumns {
            label,
            context,
            outcome,
        }
    }
}

/// One entity result's cells in a row `width` cells wide (the cursor
/// gutter included): kind glyph, label, context, and the outcome `word`
/// at the right edge.
fn entity_cells(
    entity: &SearchEntity,
    hit: &EntityMatch,
    word: &str,
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let columns = entity_columns(width);
    let dim = Style::default().add_modifier(theme.muted);
    let label = text::truncate_end(&entity.label, columns.label);
    let label_width = text::width(&label);
    let mut spans: Vec<Span<'static>> = vec![search_kind_glyph_span_for(
        NodeKind::from(&entity.node),
        theme,
    )];
    let label_style = if entity.gone { dim } else { Style::default() };
    let match_style = theme.match_style();
    spans.extend(text::paint(
        &[Span::styled(label.as_str(), label_style)],
        hit.label_range.clone().filter(|r| r.end <= label.len()),
        |_| match_style,
    ));
    let mut used = ROW_LEAD + label_width;
    if columns.context > 0 {
        let pad = columns.label.saturating_sub(label_width) + COLUMN_GAP;
        let context = text::truncate_end(&entity.context, columns.context);
        used += pad + text::width(&context);
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(span!(dim; "{context}"));
    }
    let word = text::truncate_end(word, columns.outcome);
    let word_width = text::width(&word);
    let gap = width.saturating_sub(used + word_width);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.push(span!(dim; "{word}"));
    spans
}

/// A column of a search hit. Every list that shows hits (the contents
/// modal, the Search view's hit, pane, and pane-hit rows) lays them out
/// from [`hit_columns`], keyed by these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HitCol {
    /// `session` or `pane`.
    Source,
    /// The harness chip, or the command a pane runs.
    Chip,
    /// The session's or the pane's name.
    Label,
    /// The hit count, or the turn or line the hit is.
    Tag,
    Age,
    /// The text around the match.
    Snippet,
}

/// Which of a hit's columns a list draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HitShape {
    /// Every column: a list of hits from sessions and panes side by side,
    /// with nothing else to tell them apart (the contents modal).
    Sourced,
    /// Every column but the source word: a tree row, whose kind glyph,
    /// chip, and tag already say what it is.
    Named,
    /// The tag, the age, and the snippet: a hit under the row that names
    /// its session or pane.
    Nested,
}

/// The source word's width: the longer of `session` and `pane`.
const SOURCE_WIDTH: usize = 7;
/// The label is a fifth of the row, kept between these.
const LABEL_MIN: usize = 12;
const LABEL_MAX: usize = 32;
/// The fewest cells the label narrows to so the snippet keeps its cells.
const LABEL_FLOOR: usize = 8;
/// A snippet narrower than this is not worth showing, so cells give way
/// to it.
pub(super) const MIN_SNIPPET: usize = 20;
/// The hit count's or the position's width, by the row's width.
const TAG_STEPS: &[(usize, usize)] = &[(100, 22), (78, 20), (0, 14)];
/// Characters a one-line snippet drops: Markdown code ticks only add
/// noise to a preview.
const MARKDOWN_TICKS: &[char] = &['`'];

/// A hit's columns, each under `key`'s key for it, with a harness chip
/// `chip` cells wide (its footprint in the theme): the source word, the
/// chip or a pane's command, the label (a fifth of the row, 12 to 32
/// cells), the hit count or position (22, 20, or 14 cells by the row's
/// width), the age, and the snippet, which keeps [`MIN_SNIPPET`] cells.
/// The chip gives way first, then the source word, then the age, and the
/// label narrows last, to 8 cells. Shares and steps count the row's lead
/// (the modal's cursor gutter, the tree's indent), so a hit is laid out
/// alike wherever it is listed at one width.
pub(crate) fn hit_columns<K>(
    key: impl Fn(HitCol) -> K,
    chip: usize,
    shape: HitShape,
) -> Vec<Column<K>> {
    let mut columns = Vec::with_capacity(6);
    if shape == HitShape::Sourced {
        columns.push(
            Column::fixed(key(HitCol::Source), SOURCE_WIDTH)
                .gap(1)
                .drops(1),
        );
    }
    if shape != HitShape::Nested {
        columns.push(Column::fixed(key(HitCol::Chip), chip).gap(1).drops(0));
        columns.push(
            Column::share(key(HitCol::Label), 1, 5)
                .within(LABEL_MIN, LABEL_MAX)
                .shrinks(3, LABEL_FLOOR),
        );
    }
    columns.push(Column::row_steps(key(HitCol::Tag), TAG_STEPS));
    columns.push(
        Column::fixed(key(HitCol::Age), tuikit::age::CELL_WIDTH)
            .gap(1)
            .right()
            .drops(2),
    );
    columns.push(Column::fill(key(HitCol::Snippet)).at_least(MIN_SNIPPET));
    columns
}

/// What a hit's chip column shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HitChip<'a> {
    /// The harness a session runs, as its chip.
    Harness(&'a str),
    /// The command a pane runs, receding.
    Command(&'a str),
}

/// One hit, as every list that shows hits draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hit<'a> {
    /// `session` or `pane`.
    pub source: &'static str,
    /// The chip, or `None` for a hit under the row that shows it.
    pub chip: Option<HitChip<'a>>,
    /// The name, or empty for a hit under the row that shows it.
    pub label: &'a str,
    /// The hit count, or the turn or line ([`hits_tag`], [`turn_tag`],
    /// [`line_tag`]).
    pub tag: String,
    /// The age's label, or `None` when no time applies.
    pub age: Option<&'a str>,
    /// How recent the age is, which tints it; `None` leaves it receding.
    pub recency: Option<Recency>,
    pub snippet: &'a str,
    /// Byte range of the match inside `snippet`.
    pub highlight: Range<usize>,
}

impl Hit<'_> {
    /// The hit's cells, each under `key`'s key for its column: the source
    /// word and the tag receding, the chip, the label, the framework's age
    /// cell, and the snippet cell ([`snippet_cell`]).
    pub(crate) fn cells<K>(&self, key: impl Fn(HitCol) -> K, theme: &Theme) -> Cells<K> {
        let muted = Style::default().add_modifier(theme.muted);
        let chip = match self.chip {
            Some(HitChip::Harness(harness)) => Cell::chip(harness_badge(harness, theme)),
            Some(HitChip::Command(command)) => Cell::text(command, muted),
            None => Cell::empty(),
        };
        vec![
            (key(HitCol::Source), Cell::text(self.source, muted)),
            (key(HitCol::Chip), chip),
            (key(HitCol::Label), Cell::text(self.label, Style::default())),
            (key(HitCol::Tag), Cell::text(&self.tag, muted)),
            (
                key(HitCol::Age),
                Cell::age(self.age.map(str::to_string), self.recency),
            ),
            (
                key(HitCol::Snippet),
                snippet_cell(self.snippet, &self.highlight, theme),
            ),
        ]
    }
}

/// What a content search found, wherever it is shown: the text around
/// the match, cut so the match stays in view, inert in italics with the
/// match in the theme's match look, Markdown code ticks left out.
pub(crate) fn snippet_cell(snippet: &str, highlight: &Range<usize>, theme: &Theme) -> Cell {
    Cell::snippet(
        snippet,
        highlight.clone(),
        Style::default()
            .fg(theme.inert)
            .add_modifier(Modifier::ITALIC),
        theme.match_style(),
    )
    .dropping(MARKDOWN_TICKS)
}

/// A session's or pane's hit count as a tag: `1 hit`, `3 hits`.
pub(crate) fn hits_tag(count: usize) -> String {
    format!("{count} {}", if count == 1 { "hit" } else { "hits" })
}

/// A matching turn as a tag, counting from 1: `turn 4 · message`.
pub(crate) fn turn_tag(turn_index: usize, kind: &str) -> String {
    format!("turn {} · {kind}", turn_index + 1)
}

/// A matching scrollback line as a tag: `line 17`.
pub(crate) fn line_tag(line: usize) -> String {
    format!("line {line}")
}

impl ContentRow {
    /// The result as a hit.
    fn hit(&self) -> Hit<'_> {
        Hit {
            source: self.target.source_word(),
            chip: Some(match self.target {
                ContentTarget::Session { .. } => HitChip::Harness(&self.chip),
                ContentTarget::Pane { .. } => HitChip::Command(&self.chip),
            }),
            label: &self.label,
            tag: self.turn.clone().unwrap_or_else(|| hits_tag(self.hits)),
            age: Some(&self.age),
            recency: self.recency,
            snippet: &self.snippet,
            highlight: self.highlight.clone().unwrap_or(0..0),
        }
    }
}

/// The contents modal's results laid out from the hit's columns, measured
/// once a frame over every result.
pub(super) struct ContentGrid {
    schema: ColumnSchema<HitCol>,
    measure: Measure<HitCol>,
}

impl ContentGrid {
    /// The grid of `rows` in `theme`, whose chip width it takes.
    pub(super) fn new(rows: &[ContentRow], theme: &Theme) -> Self {
        let schema = content_schema(chip::width(theme));
        let mut measure = Measure::new();
        for row in rows {
            measure.add(&schema, GUTTER_WIDTH, &row.hit().cells(same, theme));
        }
        Self { schema, measure }
    }

    /// One result's cells, `width` wide after the cursor gutter.
    pub(super) fn cells(
        &self,
        row: &ContentRow,
        width: usize,
        theme: &Theme,
    ) -> Vec<Span<'static>> {
        let cells = row.hit().cells(same, theme);
        self.schema
            .draw(&self.measure, GUTTER_WIDTH, width, &cells, theme)
    }
}

/// The modal's hit schema: every column, with a chip `chip` cells wide.
fn content_schema(chip: usize) -> ColumnSchema<HitCol> {
    ColumnSchema::new(hit_columns(same, chip, HitShape::Sourced))
}

/// The modal keys its cells by the hit's own columns.
const fn same(col: HitCol) -> HitCol {
    col
}

/// The widths of a content result's cells, as the hit's columns lay them
/// out in a row `inner` cells wide, the cursor gutter included.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContentColumns {
    /// The row's width, the cursor gutter included.
    pub inner: usize,
    /// `session` or `pane`; 0 when dropped.
    pub source: usize,
    /// The harness chip or pane command; 0 when dropped.
    pub chip: usize,
    pub label: usize,
    /// The hit count or turn; at least a few cells.
    pub tag: usize,
    /// The age; 0 when dropped.
    pub age: usize,
    snippet: usize,
}

#[cfg(test)]
impl ContentColumns {
    /// Cells the snippet gets: what is left after the other cells, their
    /// two-cell cursor mark, and the gaps between them.
    pub(super) fn snippet(&self) -> usize {
        self.snippet
    }
}

/// A content result's columns in a row `inner` cells wide, the cursor
/// gutter included, with a chip `chip` cells wide.
#[cfg(test)]
pub(crate) fn content_columns(inner: usize, chip: usize) -> ContentColumns {
    let schema = content_schema(chip);
    let mut measure = Measure::new();
    measure.add(
        &schema,
        GUTTER_WIDTH,
        &[(HitCol::Snippet, Cell::text("snippet", Style::default()))],
    );
    let layout = schema.layout(&measure, GUTTER_WIDTH, inner.saturating_sub(GUTTER_WIDTH));
    let width = |col| layout.width(col).unwrap_or(0);
    ContentColumns {
        inner,
        source: width(HitCol::Source),
        chip: width(HitCol::Chip),
        label: width(HitCol::Label),
        tag: width(HitCol::Tag),
        age: width(HitCol::Age),
        snippet: width(HitCol::Snippet),
    }
}

/// A match row's cells, `width` wide after the cursor gutter: the kind
/// glyph, the label, and a snippet of the haystack around the matched
/// range. The snippet's context recedes so the matched portion pops; the
/// matched bytes are drawn in the match style.
pub(super) fn match_cells(
    label: &str,
    haystack: &str,
    m: &SearchMatch,
    width: usize,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let dim = Style::default().add_modifier(theme.muted);
    let mut spans: Vec<Span<'static>> = Vec::new();
    // Prepend a kind glyph so operators scan
    // results by symbol (`● session`, `▣ mux`, `⇄ pr`, …) instead
    // of relying on the textual `kind:` prefix some labels carry.
    // RowIds without a NodeKind mapping (Pin, Synthetic) get two
    // spaces so the label column stays aligned across the result
    // list — operators don't see the label jiggle row by row.
    spans.push(search_kind_glyph_span(&m.id, theme));
    spans.push(Span::raw(label.to_string()));

    // If we can show a snippet (haystack present and either
    // distinct from the label or carrying a match range), append
    // `· …<context>…` after the label so the operator sees *why*
    // the row matched.
    let matched_range = m.matched_range.clone().unwrap_or(0..0);
    if !haystack.is_empty() && haystack != label {
        spans.push(span!(dim; "  · "));
        // The snippet takes the rest of the row, so its cut ends in `…`
        // before the frame; a long label leaves it a few cells.
        let budget = width.saturating_sub(text::spans_width(&spans)).max(8);
        let snippet = text::fit_snippet(haystack, matched_range, budget, &[]);
        let match_style = theme.match_style();
        spans.extend(text::paint(
            &[Span::styled(snippet.text, dim)],
            snippet.highlight,
            |_| match_style,
        ));
    }
    spans
}

/// Resolve the NodeKind a search result row represents, when there
/// is one. `RowId::Pin` and `RowId::Synthetic` are not graph nodes
/// and return `None`; callers render two spaces in the glyph slot
/// to keep label-column alignment across the result list.
fn search_row_node_kind(id: &RowId) -> Option<NodeKind> {
    match id {
        RowId::Group(node_id) | RowId::AgentSession(node_id) | RowId::MuxSession(node_id) => {
            Some(NodeKind::from(node_id))
        }
        RowId::Hit { session, .. } => Some(NodeKind::from(session)),
        RowId::Pane { mux, .. } | RowId::PaneHit { mux, .. } => Some(NodeKind::from(mux)),
        RowId::Pr(_) => Some(NodeKind::ForgePr),
        RowId::Pin { .. } => Some(NodeKind::Pin),
        RowId::Nested { node, .. } => Some(NodeKind::from(node)),
        RowId::History { .. } | RowId::Remote(_) | RowId::Bucket(_) | RowId::Synthetic(_) => None,
    }
}

/// Build the leading glyph span for a search result row. Two cells
/// wide: `<glyph> ` for graph-backed rows; `  ` for Pin / Synthetic
/// rows that don't carry a NodeKind. The glyph keeps its kind color on
/// the cursor row too, where the row's highlight covers it, so the
/// symbol's identity stays legible.
pub(super) fn search_kind_glyph_span(id: &RowId, theme: &Theme) -> Span<'static> {
    let Some(kind) = search_row_node_kind(id) else {
        return Span::raw("  ");
    };
    search_kind_glyph_span_for(kind, theme)
}

/// The glyph span for a known kind.
fn search_kind_glyph_span_for(kind: NodeKind, theme: &Theme) -> Span<'static> {
    // A PR's glyph takes the `pr_open` hue: search results carry no
    // PR state.
    let style = node_kind_style(kind, theme);
    span!(Style::default().fg(style.color); "{} ", style.glyph)
}
