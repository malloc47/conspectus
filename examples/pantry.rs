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

use conspectus::tui::widgets::multi_select::{MultiSelectState, MultiSelectWidget};
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
    ];
    tui_pantry::run!(ingredients)
}
