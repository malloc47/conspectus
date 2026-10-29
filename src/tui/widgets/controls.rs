//! Controls overlay (ADR 0031, ADR 0176).
//!
//! Single navigable modal that fronts view switching, per-view
//! grouping, per-view filter editing, the global sort toggle, the
//! preview pane's modes, and content search. `f` opens it on the active
//! view's row. The controls overlay is the discoverable surface;
//! single-key accelerators (`1`–`6`, `]`/`[`, `F`, `s`) reach the same
//! outcomes for muscle-memory operators.
//!
//! ## Architecture
//!
//! The panel is the framework's settings panel
//! ([`tuikit::settings_panel::SettingsPanel`]): it draws the sections,
//! moves the cursor, scrolls, pages, generates the footer, and closes.
//! This module is its adapter, and keeps what is Conspectus's:
//!
//! - [`ControlsContext`], the live settings the rows show, which the
//!   panel reads from the App on every key and every frame
//!   ([`crate::tui::App::controls_context`]), so it never holds a copy;
//! - [`sections`], the rows each view shows and the [`Msg`] each one
//!   reports, identified by [`ControlsCursor`];
//! - the editors: an editor row reports [`Msg::OpenControlsEditor`], and
//!   the reducer opens [`editor`] as a step over the panel. A checklist
//!   or a prompt, its answer steps back to the panel with the filter it
//!   sets, so the panel stays open showing it.
//!
//! Every change applies as it is made: the reducer applies the message a
//! row reports while the panel stays open, and `Esc` keeps everything.

use tuikit::checklist::{Checklist, Item};
use tuikit::glyphs::marks;
use tuikit::overlay::Outcome;
use tuikit::settings_panel::{Row, Section, SettingsPanel};
use tuikit::text_prompt::{PromptResult, TextPrompt};

use crate::filter::{
    HarnessFilter, MuxStateFilter, MuxStateKey, PrBranchFilter, PrStateFilter, PrStateKey,
    RowFilter,
};
use crate::tui::{Grouping, Modal, Msg, Sort, View};

/// Registered harness keys surfaced in the harness sub-editor.
/// Derives from the adapter registry
/// ([`crate::discovery::harness::harness_keys`]) so registering a
/// new adapter appears in the filter menu automatically.
pub fn harness_options() -> &'static [&'static str] {
    crate::discovery::harness::harness_keys()
}

/// The PR states the PRs view's state filter offers (ADR 0134).
pub const PR_STATE_OPTIONS: &[PrStateKey] = &PrStateKey::ALL;

/// Display order of the mux-state sub-editor entries.
pub const MUX_STATE_OPTIONS: &[MuxStateKey] = &[
    MuxStateKey::Attached,
    MuxStateKey::Ambiguous,
    MuxStateKey::Unmuxed,
];

/// All views displayed in the View section, in stable order. This
/// constant is the single source of truth for the controls-overlay View
/// section, the left-pane tab strip, and the `[` / `]` view-cycle
/// accelerator. It lists every [`View`].
pub const VIEW_OPTIONS: &[View] = &[
    View::Sessions,
    View::Mux,
    View::Prs,
    View::Pinned,
    View::History,
    View::Search,
];

/// Sort options surfaced in the Sort section, in stable order.
pub const SORT_OPTIONS: &[Sort] = &[Sort::Hierarchy, Sort::Recency];

/// The width of the name column of the editor and cycle rows, so their
/// values line up in one column on every view.
const NAME_WIDTH: usize = 12;

/// Read-only view of the live state the panel renders against. The
/// panel builds its rows from this on every key and every frame, so it
/// never holds a stale copy.
#[derive(Debug, Clone)]
pub struct ControlsContext<'a> {
    pub view: View,
    pub grouping: Grouping,
    pub filter: &'a RowFilter,
    pub sort: Sort,
    /// Mux-view recency basis. Read so the overlay
    /// can mark the active basis; only surfaced when the mux view is
    /// active.
    pub mux_recency: crate::tui::MuxRecency,
    /// Preview-pane wrap mode, marked active in the Preview wrap
    /// section (ADR 0106).
    pub preview_wrap: crate::tui::WrapMode,
    /// What the Preview shows for agent sessions in tmux, marked
    /// active in the Session preview section (ADR 0112).
    pub session_preview: crate::tui::SessionPreview,
    /// The active content search: its text and how far its scan has got.
    pub content: Option<(&'a str, crate::tui::ContentStatus, usize)>,
    /// Which kinds of turn a new content search reads (`[search]`).
    pub search_kinds: crate::viewer::search::ContentKinds,
    /// How content search results are listed (Search view).
    pub content_group: crate::viewer::search::Grouping,
    /// Which remembered search the Search view shows, and how many there
    /// are.
    pub search_history: Option<(usize, usize)>,
}

/// One row of the controls panel: the identity the panel's cursor keeps.
/// Section headers are not rows; the cursor skips them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsCursor {
    /// View option at index in [`VIEW_OPTIONS`].
    View(usize),
    /// Grouping option at index in
    /// `Grouping::values_for(ctx.view)`.
    Grouping(usize),
    FilterHarness,
    FilterMaxAge,
    FilterMuxState,
    /// Edits the content query that narrows the session lists.
    FilterContent,
    /// Search-view-only: list results per session or per message.
    ContentGroup,
    /// Search-view-only: pick among the remembered searches.
    SearchHistory,
    /// Open the contents search modal, for terminals that do not send
    /// Ctrl-/ and operators who have not learned `s`.
    SearchContents,
    /// Open the contents search options: what a search looks in.
    SearchOptions,
    /// Sessions-view-only checkbox: float muxed sessions to the top.
    FilterFloatMuxedSessions,
    /// Mux-view-only checkbox: float attached muxes to the top.
    FilterFloatAttachedMuxes,
    /// Mux-view-only checkbox: list every stopped mux, not just pinned ones.
    FilterShowStoppedMuxes,
    FilterPinnedOnly,
    /// History-view-only checkbox: list only records no pin points at.
    FilterHistoryUnpinnedOnly,
    /// PRs-view-only row: edits the PR state set (ADR 0134).
    FilterPrState,
    /// PRs-view-only row: cycles the local-branch filter through any,
    /// tracked, and untracked (ADR 0129).
    FilterPrBranch,
    /// PRs-view-only checkbox: float PRs with a local branch to the top.
    FilterFloatTrackedPrs,
    FilterClear,
    /// Sort option at index in [`SORT_OPTIONS`].
    Sort(usize),
    /// Mux-view-only recency basis at index in
    /// [`crate::tui::MuxRecency::ALL`].
    MuxRecency(usize),
    /// Preview wrap mode at index in [`crate::tui::WrapMode::ALL`].
    PreviewWrap(usize),
    /// Session preview at index in [`crate::tui::SessionPreview::ALL`].
    SessionPreview(usize),
}

/// The editor a Controls editor row opens as a step over the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsEditor {
    /// The harness filter's checklist.
    Harness,
    /// The max-age filter's prompt.
    MaxAge,
    /// The mux-state filter's checklist.
    MuxState,
    /// The content query's prompt.
    Content,
    /// The PR-state filter's checklist (PRs view).
    PrState,
}

/// The Controls panel as the App's modal stack holds it.
pub type ControlsPanel = SettingsPanel<ControlsCursor, Msg, crate::tui::App>;

/// The Controls panel over a context `Cx` that `context` reads the live
/// settings from: the App in the TUI, a fixture in tests and previews.
pub fn panel<Cx: ?Sized + 'static>(
    context: impl Fn(&Cx) -> ControlsContext<'_> + Send + Sync + 'static,
) -> SettingsPanel<ControlsCursor, Msg, Cx> {
    SettingsPanel::new("Controls", move |cx: &Cx| sections(&context(cx))).name_width(NAME_WIDTH)
}

/// Where the panel opens: the active view's row, since operators most
/// often open it to switch view.
pub fn opening_row(ctx: &ControlsContext<'_>) -> ControlsCursor {
    let index = VIEW_OPTIONS
        .iter()
        .position(|view| *view == ctx.view)
        .unwrap_or(0);
    ControlsCursor::View(index)
}

/// Every row in display order, for checks that ask which rows a view
/// offers.
#[cfg(test)]
pub(crate) fn flatten_rows(ctx: &ControlsContext<'_>) -> Vec<ControlsCursor> {
    sections(ctx)
        .into_iter()
        .flat_map(|section| section.rows)
        .map(|row| row.id)
        .collect()
}

/// The panel's sections for the settings in `ctx`, each row with the
/// message acting on it reports.
pub fn sections(ctx: &ControlsContext<'_>) -> Vec<Section<ControlsCursor, Msg>> {
    let mut sections = vec![
        view_section(ctx),
        Section::new(
            format!("Grouping ({})", view_label(ctx.view)),
            Grouping::values_for(ctx.view)
                .iter()
                .enumerate()
                .map(|(index, grouping)| {
                    Row::choice(
                        ControlsCursor::Grouping(index),
                        grouping.as_str(),
                        *grouping == ctx.grouping,
                        Some(Msg::SetGrouping(*grouping)),
                    )
                })
                .collect(),
        ),
    ];
    if matches!(ctx.view, View::Search) {
        sections.push(results_section(ctx));
    }
    sections.push(Section::new(
        "Search",
        vec![
            Row::editor(
                ControlsCursor::SearchContents,
                "contents",
                "Enter searches",
                Msg::OpenContentSearchModal,
            )
            .leaves(),
            Row::editor(
                ControlsCursor::SearchOptions,
                "options",
                "Enter opens",
                Msg::OpenSearchOptions,
            )
            .leaves(),
        ],
    ));
    sections.push(filters_section(ctx));
    sections.push(Section::new(
        "Sort",
        SORT_OPTIONS
            .iter()
            .enumerate()
            .map(|(index, sort)| {
                Row::choice(
                    ControlsCursor::Sort(index),
                    sort_label(*sort),
                    *sort == ctx.sort,
                    Some(Msg::SetSort(*sort)),
                )
            })
            .collect(),
    ));
    // Mux-only recency basis: which signal the Recency sort orders the
    // mux tree by. Only meaningful, and only shown, for the mux view.
    if matches!(ctx.view, View::Mux) {
        sections.push(Section::new(
            "Recency by (Mux)",
            crate::tui::MuxRecency::ALL
                .iter()
                .enumerate()
                .map(|(index, basis)| {
                    Row::choice(
                        ControlsCursor::MuxRecency(index),
                        basis.label(),
                        *basis == ctx.mux_recency,
                        Some(Msg::SetMuxRecency(*basis)),
                    )
                })
                .collect(),
        ));
    }
    sections.push(Section::new(
        "Preview wrap",
        crate::tui::WrapMode::ALL
            .iter()
            .enumerate()
            .map(|(index, wrap)| {
                Row::choice(
                    ControlsCursor::PreviewWrap(index),
                    crate::tui::preview_wrap::label(*wrap),
                    *wrap == ctx.preview_wrap,
                    Some(Msg::SetPreviewWrap(*wrap)),
                )
            })
            .collect(),
    ));
    sections.push(Section::new(
        "Session preview",
        crate::tui::SessionPreview::ALL
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                Row::choice(
                    ControlsCursor::SessionPreview(index),
                    choice.label(),
                    *choice == ctx.session_preview,
                    Some(Msg::SetSessionPreview(*choice)),
                )
            })
            .collect(),
    ));
    sections
}

/// The views, each with the digit that switches to it outside the panel;
/// choosing the view in force changes nothing.
fn view_section(ctx: &ControlsContext<'_>) -> Section<ControlsCursor, Msg> {
    let rows = VIEW_OPTIONS
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let active = *view == ctx.view;
            Row::choice(
                ControlsCursor::View(index),
                format!("{} [{}]", view_label(*view), index + 1),
                active,
                (!active).then_some(Msg::SwitchView(*view)),
            )
        })
        .collect();
    Section::new("View", rows)
}

/// The Search view's own rows: how results are listed, and which
/// remembered search it shows.
fn results_section(ctx: &ControlsContext<'_>) -> Section<ControlsCursor, Msg> {
    use crate::viewer::search::Grouping as Content;
    let next = match ctx.content_group.granularity() {
        Content::Session => Content::Flat,
        _ => Content::Session,
    };
    let searches = match ctx.search_history {
        Some((position, total)) => format!("{position} of {total}"),
        None => "none yet".to_string(),
    };
    Section::new(
        "Results",
        vec![
            Row::cycle(
                ControlsCursor::ContentGroup,
                "listed as",
                crate::tui::app::grouping_description(ctx.content_group),
                Msg::SetContentGroup(next),
            ),
            Row::editor(
                ControlsCursor::SearchHistory,
                "searches",
                searches,
                Msg::OpenSearchHistory,
            )
            .leaves(),
        ],
    )
}

/// The view's filters: the editors, the view's own checkboxes and PR
/// rows, and the button that clears them all.
fn filters_section(ctx: &ControlsContext<'_>) -> Section<ControlsCursor, Msg> {
    let filter = ctx.filter;
    let toggle = |flip: fn(&mut RowFilter)| {
        let mut next = filter.clone();
        flip(&mut next);
        Msg::SetFilter(next)
    };
    let mut rows = vec![
        Row::editor(
            ControlsCursor::FilterHarness,
            "harness",
            harness_value(filter),
            Msg::OpenControlsEditor(ControlsEditor::Harness),
        ),
        Row::editor(
            ControlsCursor::FilterMaxAge,
            "max age",
            max_age_value(filter),
            Msg::OpenControlsEditor(ControlsEditor::MaxAge),
        ),
        Row::editor(
            ControlsCursor::FilterMuxState,
            "mux state",
            mux_state_value(filter),
            Msg::OpenControlsEditor(ControlsEditor::MuxState),
        ),
    ];
    if matches!(
        ctx.view,
        View::Sessions | View::Mux | View::Prs | View::Search
    ) {
        rows.push(Row::editor(
            ControlsCursor::FilterContent,
            "contains",
            content_value(ctx.content),
            Msg::OpenControlsEditor(ControlsEditor::Content),
        ));
    }
    if matches!(ctx.view, View::Sessions) {
        rows.push(Row::checkbox(
            ControlsCursor::FilterFloatMuxedSessions,
            "Float muxed sessions to top",
            filter.float_muxed_sessions_top,
            toggle(|f| f.float_muxed_sessions_top = !f.float_muxed_sessions_top),
        ));
    }
    if matches!(ctx.view, View::Mux) {
        rows.push(Row::checkbox(
            ControlsCursor::FilterFloatAttachedMuxes,
            "Float attached muxes to top",
            filter.float_attached_muxes_top,
            toggle(|f| f.float_attached_muxes_top = !f.float_attached_muxes_top),
        ));
        rows.push(Row::checkbox(
            ControlsCursor::FilterShowStoppedMuxes,
            "Show all stopped muxes",
            filter.show_stopped_muxes,
            toggle(|f| f.show_stopped_muxes = !f.show_stopped_muxes),
        ));
    }
    if matches!(ctx.view, View::Prs) {
        rows.push(Row::editor(
            ControlsCursor::FilterPrState,
            "PR state",
            pr_state_value(filter),
            Msg::OpenControlsEditor(ControlsEditor::PrState),
        ));
        rows.push(Row::cycle(
            ControlsCursor::FilterPrBranch,
            "local branch",
            pr_branch_value(filter),
            toggle(|f| f.pr_branch = PrBranchFilter::cycle(f.pr_branch)),
        ));
        rows.push(Row::checkbox(
            ControlsCursor::FilterFloatTrackedPrs,
            "Float PRs with a local branch to top",
            filter.float_tracked_prs_top,
            toggle(|f| f.float_tracked_prs_top = !f.float_tracked_prs_top),
        ));
    }
    if matches!(ctx.view, View::Sessions | View::Mux | View::Prs) {
        rows.push(Row::checkbox(
            ControlsCursor::FilterPinnedOnly,
            "Pinned only",
            filter.pinned_only,
            toggle(|f| f.pinned_only = !f.pinned_only),
        ));
    }
    if matches!(ctx.view, View::History) {
        rows.push(Row::checkbox(
            ControlsCursor::FilterHistoryUnpinnedOnly,
            "Unpinned only",
            !filter.show_pinned_history,
            toggle(|f| f.show_pinned_history = !f.show_pinned_history),
        ));
    }
    // With nothing to clear the button does nothing, silently.
    let clear = (!filter.is_empty() || ctx.content.is_some()).then_some(Msg::ClearFilters);
    rows.push(Row::button(
        ControlsCursor::FilterClear,
        "Clear all filters",
        clear,
    ));
    Section::new(format!("Filters ({})", view_label(ctx.view)), rows)
}

/// The editor `which` names, seeded from the settings in `ctx`, as the
/// step the reducer opens over the panel. Its answer steps back to the
/// panel with the message that sets what it edited.
pub fn editor(which: ControlsEditor, ctx: &ControlsContext<'_>) -> Modal {
    let filter = ctx.filter.clone();
    match which {
        ControlsEditor::Harness => {
            let values = match &filter.harness {
                Some(HarnessFilter::Any(values)) => values.clone(),
                None => Vec::new(),
            };
            let list = checklist_of(
                "harness",
                harness_options(),
                |key| key,
                |key| values.iter().any(|value| value == key),
                move |keys| {
                    let values = keys.into_iter().map(str::to_string).collect();
                    Msg::SetFilter(with_harness(filter.clone(), values))
                },
            );
            Modal::Checklist(Box::new(list))
        }
        ControlsEditor::MuxState => {
            let values = match &filter.mux_state {
                Some(MuxStateFilter::Any(values)) => values.clone(),
                None => Vec::new(),
            };
            let list = checklist_of(
                "mux state",
                MUX_STATE_OPTIONS,
                |key| key.as_str(),
                |key| values.contains(key),
                move |values| Msg::SetFilter(with_mux_state(filter.clone(), values)),
            );
            Modal::Checklist(Box::new(list))
        }
        ControlsEditor::PrState => {
            let values = match &filter.pr_state {
                Some(PrStateFilter::Any(values)) => values.clone(),
                None => Vec::new(),
            };
            let list = checklist_of(
                "PR state",
                PR_STATE_OPTIONS,
                |key| key.as_str(),
                |key| values.contains(key),
                move |values| Msg::SetFilter(with_pr_state(filter.clone(), values)),
            );
            Modal::Checklist(Box::new(list))
        }
        ControlsEditor::MaxAge => Modal::Prompt(max_age_editor(filter)),
        ControlsEditor::Content => {
            let initial = ctx.content.map(|(text, ..)| text).unwrap_or_default();
            Modal::Prompt(content_editor(initial, ctx.search_kinds))
        }
    }
}

/// A checklist over `options`, each labeled by `label` and checked when
/// `checked` says so, whose confirm steps back to the panel with the
/// message `apply` builds from the checked options.
fn checklist_of<T: Clone + Send + Sync + 'static>(
    title: &str,
    options: &'static [T],
    label: impl Fn(&T) -> &str,
    checked: impl Fn(&T) -> bool,
    apply: impl Fn(Vec<T>) -> Msg + Send + Sync + 'static,
) -> Checklist<Msg> {
    let items = options
        .iter()
        .map(|option| Item::new(label(option)).checked(checked(option)))
        .collect();
    Checklist::new(title, items, move |indices: Vec<usize>| {
        let values = indices
            .into_iter()
            .filter_map(|index| options.get(index).cloned())
            .collect();
        Outcome::Back(apply(values))
    })
}

fn with_pr_state(mut filter: RowFilter, values: Vec<PrStateKey>) -> RowFilter {
    filter.pr_state = if values.is_empty() {
        None
    } else {
        Some(PrStateFilter::from_values(values))
    };
    filter
}

/// The max-age editor, seeded with the filter's age. Blank text clears
/// the filter; text [`parse_max_age`] refuses keeps the editor open with
/// the reason.
fn max_age_editor(filter: RowFilter) -> TextPrompt<Msg> {
    let initial = filter
        .max_age
        .map(format_duration_for_input)
        .unwrap_or_default();
    TextPrompt::new("max age", initial, move |text: &str| {
        let trimmed = text.trim();
        let max_age = if trimmed.is_empty() {
            None
        } else {
            match parse_max_age(trimmed) {
                Ok(max_age) => Some(max_age),
                Err(why) => return PromptResult::Reject(format!("max age: {why}")),
            }
        };
        PromptResult::Back(Msg::SetFilter(with_max_age(filter.clone(), max_age)))
    })
}

/// The contents editor, seeded with the current query. Blank text
/// clears the content search; other text starts one over `kinds`.
fn content_editor(initial: &str, kinds: crate::viewer::search::ContentKinds) -> TextPrompt<Msg> {
    TextPrompt::new("contains", initial, move |text: &str| {
        let text = text.trim();
        PromptResult::Back(if text.is_empty() {
            Msg::ClearContentSearch
        } else {
            Msg::StartContentSearch {
                text: text.to_string(),
                kinds,
            }
        })
    })
}

fn with_harness(mut filter: RowFilter, values: Vec<String>) -> RowFilter {
    filter.harness = if values.is_empty() {
        None
    } else {
        Some(HarnessFilter::from_values(values))
    };
    filter
}

fn with_mux_state(mut filter: RowFilter, values: Vec<MuxStateKey>) -> RowFilter {
    filter.mux_state = if values.is_empty() {
        None
    } else {
        Some(MuxStateFilter::from_values(values))
    };
    filter
}

fn with_max_age(mut filter: RowFilter, max_age: Option<std::time::Duration>) -> RowFilter {
    filter.max_age = max_age;
    filter
}

/// Render a duration as the most compact `<n><unit>` shape that
/// round-trips through [`parse_max_age`]. Used to seed the
/// text-input editor with a value the operator can edit in place.
pub fn format_duration_for_input(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs == 0 {
        return "0s".to_string();
    }
    if secs.is_multiple_of(86_400) {
        format!("{}d", secs / 86_400)
    } else if secs.is_multiple_of(3_600) {
        format!("{}h", secs / 3_600)
    } else if secs.is_multiple_of(60) {
        format!("{}m", secs / 60)
    } else {
        format!("{secs}s")
    }
}

/// Parse a duration token used in the max-age editor. Mirrors the
/// CLI `parse_filter_duration` shape so a value typed in the
/// overlay matches what the operator would put on the command
/// line. Returns the duration as a [`std::time::Duration`]; failure
/// is propagated as a short error string the host can show inline.
pub fn parse_max_age(raw: &str) -> Result<std::time::Duration, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty duration".into());
    }
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(|| "missing unit (expected ms/s/m/h/d)".to_string())?;
    let (digits, suffix) = trimmed.split_at(split);
    let value: u64 = digits
        .parse()
        .map_err(|_| format!("not a non-negative integer: `{digits}`"))?;
    let secs = match suffix {
        "ms" => return Ok(std::time::Duration::from_millis(value)),
        "s" => value,
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(3_600),
        "d" => value.saturating_mul(86_400),
        other => return Err(format!("unknown unit `{other}` (expected ms/s/m/h/d)")),
    };
    Ok(std::time::Duration::from_secs(secs))
}

fn pr_state_value(filter: &RowFilter) -> String {
    match &filter.pr_state {
        Some(PrStateFilter::Any(values)) if !values.is_empty() => values
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

fn pr_branch_value(filter: &RowFilter) -> String {
    filter
        .pr_branch
        .map_or_else(|| "any".to_string(), |value| value.as_str().to_string())
}

fn harness_value(filter: &RowFilter) -> String {
    match &filter.harness {
        Some(HarnessFilter::Any(values)) if !values.is_empty() => values.join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

/// How the Content row reads: the query and where its scan is.
fn content_value(content: Option<(&str, crate::tui::ContentStatus, usize)>) -> String {
    match content {
        None => marks::NO_VALUE.to_string(),
        Some((text, crate::tui::ContentStatus::Running(progress), _)) => {
            format!(
                "\"{text}\" searching {}/{}",
                progress.scanned, progress.total
            )
        }
        Some((text, crate::tui::ContentStatus::Done(_), found)) => {
            format!(
                "\"{text}\" {found} {}",
                if found == 1 { "session" } else { "sessions" }
            )
        }
    }
}

fn max_age_value(filter: &RowFilter) -> String {
    filter
        .max_age
        .map_or_else(|| marks::NO_VALUE.to_string(), format_duration_for_input)
}

fn mux_state_value(filter: &RowFilter) -> String {
    match &filter.mux_state {
        Some(MuxStateFilter::Any(values)) if !values.is_empty() => values
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => marks::NO_VALUE.to_string(),
    }
}

pub(crate) fn view_label(view: View) -> &'static str {
    match view {
        View::Sessions => "Sessions",
        View::Mux => "Mux",
        View::Prs => "PRs",
        View::Pinned => "Pinned",
        View::History => "History",
        View::Search => "Search",
    }
}

fn sort_label(sort: Sort) -> &'static str {
    match sort {
        Sort::Hierarchy => "Hierarchy",
        Sort::Recency => "Recency",
    }
}

#[cfg(test)]
#[path = "controls_tests.rs"]
mod tests;
