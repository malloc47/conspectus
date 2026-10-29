//! What a search result looks like: quick select's rows of the view and
//! entities, and content search's hits, each laid out for the width the
//! palette gives it. A content hit's cells come from one column plan
//! ([`hit_plan`]), which a list outside the modal can lay out too.

use ratatui::macros::span;
use ratatui::style::Style;
use ratatui::text::Span;
use tuikit::chip;
use tuikit::palette::{Column, ColumnLayout, ColumnPlan, RowContext};
use tuikit::selection::GUTTER_WIDTH;
use tuikit::text;

use super::{ContentRow, ContentTarget, OutcomeWords, SearchOutcome};
use crate::tui::Theme;
use crate::tui::icons::{NodeKind, node_kind_style};
use crate::tui::rows::RowId;
use crate::tui::search::SearchMatch;
use crate::tui::search_entities::{EntityMatch, SearchEntity};

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

/// The widths of a content result's cells, as the hit plan lays them
/// out.
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
    layout: ColumnLayout,
}

/// The source word's width: the longer of `session` and `pane`.
const SOURCE_WIDTH: usize = 7;
/// The age's width: `999d` at most.
const AGE_WIDTH: usize = 4;
/// A snippet narrower than this is not worth showing, so cells give way
/// to it.
pub(super) const MIN_SNIPPET: usize = 20;
/// The hit count's or the position's width, by the row's width.
const TAG_STEPS: &[(usize, usize)] = &[(100, 22), (78, 20), (0, 14)];
/// Characters a one-line snippet drops: Markdown code ticks only add
/// noise to a preview.
pub(crate) const MARKDOWN_TICKS: &[char] = &['`'];

/// Where each cell of a content result sits in [`hit_plan`].
const SOURCE: usize = 0;
const CHIP: usize = 1;
const LABEL: usize = 2;
const TAG: usize = 3;
const AGE: usize = 4;

impl ContentColumns {
    /// Cells the snippet gets: what is left after the other cells, their
    /// two-cell cursor mark, and the gaps between them.
    pub(super) fn snippet(&self) -> usize {
        self.layout.fill()
    }
}

/// The column plan of a content result, shared out over the row with its
/// cursor gutter: where it was found, the harness chip or pane command
/// (`chip` cells, a harness chip's footprint from the theme), the label
/// (a fifth of the row, 12 to 32 cells), the hit count or position (by
/// the row's width), the age, and the snippet, which keeps
/// [`MIN_SNIPPET`] cells: the chip gives way first, then the source word,
/// then the age, and the label narrows last, to 8 cells.
pub(crate) fn hit_plan(chip: usize) -> ColumnPlan {
    ColumnPlan::new([
        Column::fixed(SOURCE_WIDTH).gap(1).drops(1),
        Column::fixed(chip).gap(1).drops(0),
        Column::share(1, 5).within(12, 32).gap(2).shrinks(3, 8),
        Column::steps(TAG_STEPS).gap(2),
        Column::fixed(AGE_WIDTH).gap(1).drops(2).end(),
        Column::fill(MIN_SNIPPET),
    ])
    .lead(GUTTER_WIDTH)
}

/// The content result's cells in a row `inner` cells wide, the cursor
/// gutter included, with a chip `chip` cells wide: [`hit_plan`] laid out.
pub(crate) fn content_columns(inner: usize, chip: usize) -> ContentColumns {
    let layout = hit_plan(chip).layout(inner);
    ContentColumns {
        inner,
        source: layout.width(SOURCE),
        chip: layout.width(CHIP),
        label: layout.width(LABEL),
        tag: layout.width(TAG),
        age: layout.width(AGE),
        layout,
    }
}

/// One contents result's cells, `width` wide after the cursor gutter:
/// where it was found, harness chip or pane command, label, hit count or
/// position, age, and the snippet with the match marked.
pub(super) fn content_cells(row: &ContentRow, width: usize, theme: &Theme) -> Vec<Span<'static>> {
    use crate::tui::widgets::badge::harness_badge;

    let columns = content_columns(width + GUTTER_WIDTH, chip::width(theme));
    let layout = &columns.layout;
    let dim = Style::default().add_modifier(theme.muted);
    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some(cell) = layout.fit(SOURCE, row.target.source_word()) {
        spans.push(Span::styled(cell, dim));
    }
    if layout.is_shown(CHIP) {
        match row.target {
            ContentTarget::Session { .. } => {
                spans.push(harness_badge(&row.chip, theme));
                spans.push(Span::raw(" ".repeat(layout.gap(CHIP))));
            }
            ContentTarget::Pane { .. } => {
                if let Some(cell) = layout.fit(CHIP, &row.chip) {
                    spans.push(Span::styled(cell, dim));
                }
            }
        }
    }
    spans.push(Span::raw(layout.fit(LABEL, &row.label).unwrap_or_default()));
    let tag = match &row.turn {
        Some(turn) => turn.clone(),
        None => format!(
            "{} {}",
            row.hits,
            if row.hits == 1 { "hit" } else { "hits" }
        ),
    };
    spans.push(Span::styled(layout.fit(TAG, &tag).unwrap_or_default(), dim));
    if let Some(cell) = layout.fit(AGE, &row.age) {
        spans.push(Span::styled(cell, dim));
    }
    let budget = columns.snippet().max(8);
    let snippet = text::fit_snippet(
        &row.snippet,
        row.highlight.clone().unwrap_or(0..0),
        budget,
        MARKDOWN_TICKS,
    );
    let match_style = theme.match_style();
    spans.extend(text::paint(
        &[Span::styled(snippet.text, dim)],
        snippet.highlight,
        |_| match_style,
    ));
    spans
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
