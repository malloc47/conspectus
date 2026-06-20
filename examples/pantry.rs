//! T8-044 spike: tui-pantry preview harness for Conspectus widgets.
//!
//! Boots [`tui_pantry`] with a hand-built ingredient list so the
//! widget-iteration loop can be evaluated without going through
//! `conspectus tui --snapshot`. The smoke-test ingredient is
//! [`conspectus::tui::widgets::multi_select::MultiSelectWidget`] in
//! three prop variants: empty list, mid-selection, large list with
//! scroll.
//!
//! Run with: `cargo run --example pantry`.
//!
//! Implementation note: `Ingredient` requires `Send`, and our
//! `MultiSelectState` carries the upstream `ratatui_cheese`
//! `Option<Box<dyn Fn>>` validator which is `!Send`. The ingredients
//! therefore hold only the **configuration data** (items + initial
//! selections + cursor position) and build the state fresh inside
//! `render()`. That's cheap for a preview surface, and it
//! sidesteps the Send bound without polluting the in-tree
//! `MultiSelectState` API for a spike.
//!
//! If the spike lands "go", the ingredient list grows beyond the
//! single-file demo and migrates to the `pantry.toml` /
//! `pantry_ingredients!` proc-macro convention. If "no-go", this
//! file and the `tui-pantry` dev-dep both retire.

use conspectus::filter::{HarnessFilter, MuxStateFilter, MuxStateKey, RowFilter};
use conspectus::tui::widgets::controls::{
    ControlsContext, ControlsOverlayState, ControlsOverlayWidget,
};
use conspectus::tui::widgets::multi_select::{MultiSelectState, MultiSelectWidget};
use conspectus::tui::{Grouping, SessionsGrouping, Sort, Theme, View};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use tui_pantry::Ingredient;

/// Configuration for one multi-select preview variant. The state is
/// rebuilt per `render` so the ingredient struct stays `Send`-clean
/// regardless of the upstream `ratatui_cheese` validator's bound.
struct MultiSelectVariant {
    title: &'static str,
    items: Vec<String>,
    initial_selected: Vec<usize>,
    /// Number of `Down`-key advances applied before rendering, so the
    /// cursor lands at the desired row for the preview.
    cursor_advance: usize,
    group_label: &'static str,
    variant_name: &'static str,
    description_text: &'static str,
}

impl MultiSelectVariant {
    fn build_state(&self) -> MultiSelectState {
        let mut state = MultiSelectState::new(self.title, self.items.len(), &self.initial_selected);
        let down = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        for _ in 0..self.cursor_advance {
            state.handle_key(down);
        }
        state
    }
}

impl Ingredient for MultiSelectVariant {
    fn group(&self) -> &str {
        self.group_label
    }

    fn name(&self) -> &str {
        self.variant_name
    }

    fn source(&self) -> &str {
        "conspectus::tui::widgets::multi_select"
    }

    fn description(&self) -> &str {
        self.description_text
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let state = self.build_state();
        // `MultiSelectItem` is impl'd for `String` directly, so we
        // can hand the items slice through without converting to
        // borrowed `&str` — that would lose the static lifetime
        // the `&'static str` impl needs.
        MultiSelectWidget::new(&state, self.items.as_slice()).render(area, buf);
    }
}

/// Pre-open shape for the controls overlay sub-editor. The
/// `ControlsOverlayState` private fields force us to drive open
/// states via `handle_key`, so each variant picks the row to
/// land on, optionally navigates down before pressing Enter,
/// and ends with the sub-editor (or nothing) open.
#[derive(Clone, Copy)]
enum ControlsOpenState {
    /// Default open: cursor at View > active, no sub-editor.
    Closed,
    /// Cursor parked on Filters > Harness; press Enter to open
    /// the harness multi-select.
    HarnessOpen,
    /// Cursor at Filters > Max Age; Enter opens the max-age
    /// text input pre-seeded from `filter.max_age`.
    MaxAgeOpen,
    /// Cursor at Filters > Mux State; Enter opens the mux-state
    /// multi-select pre-seeded from `filter.mux_state`.
    MuxStateOpen,
}

/// Configuration for one controls-overlay preview variant. The
/// state is rebuilt fresh per `render` so the ingredient struct
/// stays `Send`-clean even when the sub-editor wraps `!Send`
/// MultiSelectState payloads. The `RowFilter` lives on the
/// variant; the `ControlsContext` borrows from it transiently
/// each frame.
struct ControlsVariant {
    view: View,
    grouping: Grouping,
    filter: RowFilter,
    sort: Sort,
    open_state: ControlsOpenState,
    variant_name: &'static str,
    description_text: &'static str,
}

impl ControlsVariant {
    fn handle_key(state: &mut ControlsOverlayState, ctx: &ControlsContext<'_>, code: KeyCode) {
        let event = KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        // Outcome is discarded — the preview is render-only, and
        // outcomes describe what the reducer would apply.
        let _ = state.handle_key(ctx, event);
    }
}

impl Ingredient for ControlsVariant {
    fn group(&self) -> &str {
        "Controls"
    }

    fn name(&self) -> &str {
        self.variant_name
    }

    fn source(&self) -> &str {
        "conspectus::tui::widgets::controls"
    }

    fn description(&self) -> &str {
        self.description_text
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = Theme::default();
        let ctx = ControlsContext {
            view: self.view,
            grouping: self.grouping,
            filter: &self.filter,
            sort: self.sort,
        };
        let mut state = match self.open_state {
            ControlsOpenState::Closed => ControlsOverlayState::new(&ctx),
            ControlsOpenState::HarnessOpen
            | ControlsOpenState::MaxAgeOpen
            | ControlsOpenState::MuxStateOpen => {
                // `new_at_filters` lands the cursor on Filters >
                // Harness; we drive Down to reach Max Age / Mux
                // State before pressing Enter to open.
                ControlsOverlayState::new_at_filters(&ctx)
            }
        };

        match self.open_state {
            ControlsOpenState::Closed => {}
            ControlsOpenState::HarnessOpen => {
                Self::handle_key(&mut state, &ctx, KeyCode::Enter);
            }
            ControlsOpenState::MaxAgeOpen => {
                Self::handle_key(&mut state, &ctx, KeyCode::Down);
                Self::handle_key(&mut state, &ctx, KeyCode::Enter);
            }
            ControlsOpenState::MuxStateOpen => {
                Self::handle_key(&mut state, &ctx, KeyCode::Down);
                Self::handle_key(&mut state, &ctx, KeyCode::Down);
                Self::handle_key(&mut state, &ctx, KeyCode::Enter);
            }
        }

        ControlsOverlayWidget::new(&state, ctx, &theme).render(area, buf);
    }
}

fn main() -> std::io::Result<()> {
    let ingredients: Vec<Box<dyn Ingredient>> = vec![
        Box::new(MultiSelectVariant {
            title: " harness (empty) ",
            items: Vec::new(),
            initial_selected: Vec::new(),
            cursor_advance: 0,
            group_label: "MultiSelect",
            variant_name: "Empty list",
            description_text: "Sub-editor with zero items — exercises the empty-state render path.",
        }),
        Box::new(MultiSelectVariant {
            title: " harness ",
            items: ["claude-code", "codex", "opencode", "aider"]
                .into_iter()
                .map(String::from)
                .collect(),
            initial_selected: vec![0, 2],
            cursor_advance: 2,
            group_label: "MultiSelect",
            variant_name: "Mid-selection",
            description_text: "Typical sub-editor state: two items checked, cursor mid-list.",
        }),
        Box::new(MultiSelectVariant {
            title: " 40 options ",
            items: (0..40)
                .map(|i| format!("option-{i:02} — long enough label to test wrapping"))
                .collect(),
            initial_selected: vec![3, 7, 11, 17, 23, 29, 35, 39],
            cursor_advance: 0,
            group_label: "MultiSelect",
            variant_name: "Large list (scroll)",
            description_text: "40-option set with 8 checked entries — exercises the upstream scroll path.",
        }),
        Box::new(ControlsVariant {
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter::default(),
            sort: Sort::Hierarchy,
            open_state: ControlsOpenState::Closed,
            variant_name: "Default (sessions view, no filters)",
            description_text: "Overlay with cursor on the active View row; every filter cleared. The baseline shape an operator sees on `f`.",
        }),
        Box::new(ControlsVariant {
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter {
                harness: Some(HarnessFilter::Any(vec![
                    "claude-code".to_string(),
                    "opencode".to_string(),
                ])),
                ..RowFilter::default()
            },
            sort: Sort::Hierarchy,
            open_state: ControlsOpenState::HarnessOpen,
            variant_name: "Harness sub-editor (two selected)",
            description_text: "Harness multi-select open with claude-code + opencode pre-checked; previews the modal-on-modal stacking.",
        }),
        Box::new(ControlsVariant {
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter {
                max_age: Some(std::time::Duration::from_secs(60 * 60 * 24 * 7)),
                ..RowFilter::default()
            },
            sort: Sort::Hierarchy,
            open_state: ControlsOpenState::MaxAgeOpen,
            variant_name: "Max-age sub-editor (7d pre-filled)",
            description_text: "Max-age text input open with a 7-day window seeded from the filter; previews the text-input modal frame.",
        }),
        Box::new(ControlsVariant {
            view: View::Sessions,
            grouping: Grouping::Sessions(SessionsGrouping::Graph),
            filter: RowFilter {
                mux_state: Some(MuxStateFilter::Any(vec![
                    MuxStateKey::Attached,
                    MuxStateKey::Ambiguous,
                ])),
                ..RowFilter::default()
            },
            sort: Sort::Hierarchy,
            open_state: ControlsOpenState::MuxStateOpen,
            variant_name: "Mux-state sub-editor (attached + ambiguous)",
            description_text: "Mux-state multi-select open with attached + ambiguous pre-checked; the third filter sub-editor.",
        }),
    ];
    tui_pantry::run!(ingredients)
}
