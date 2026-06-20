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
use std::borrow::Cow;

use conspectus::tui::rows::RowId;
use conspectus::tui::search::{SearchItem, SubstringBackend};
use conspectus::tui::widgets::help::{HelpOverlayState, HelpOverlayWidget};
use conspectus::tui::widgets::multi_select::{MultiSelectState, MultiSelectWidget};
use conspectus::tui::widgets::search::{SearchOverlayState, SearchOverlayWidget};
use conspectus::tui::widgets::value_modal::{ValueModalState, ValueModalWidget};
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

/// Configuration for one value-modal preview variant. Holds the
/// label + value + scroll position; the widget state is built per
/// `render` so this struct stays plain data (and trivially `Send`).
struct ValueModalVariant {
    label: &'static str,
    value: String,
    scroll: u16,
    variant_name: &'static str,
    description_text: &'static str,
}

impl Ingredient for ValueModalVariant {
    fn group(&self) -> &str {
        "ValueModal"
    }

    fn name(&self) -> &str {
        self.variant_name
    }

    fn source(&self) -> &str {
        "conspectus::tui::widgets::value_modal"
    }

    fn description(&self) -> &str {
        self.description_text
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = Theme::default();
        let mut state = ValueModalState::new(self.label, &self.value);
        state.scroll = self.scroll;
        ValueModalWidget::new(&state, &theme).render(area, buf);
    }
}

/// Configuration for one help-overlay preview variant. The only
/// state is the scroll offset; the keymap is static data baked into
/// the renderer.
struct HelpVariant {
    scroll: u16,
    variant_name: &'static str,
    description_text: &'static str,
}

impl Ingredient for HelpVariant {
    fn group(&self) -> &str {
        "Help"
    }

    fn name(&self) -> &str {
        self.variant_name
    }

    fn source(&self) -> &str {
        "conspectus::tui::widgets::help"
    }

    fn description(&self) -> &str {
        self.description_text
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = Theme::default();
        let state = HelpOverlayState {
            scroll: self.scroll,
        };
        HelpOverlayWidget::new(&state, &theme).render(area, buf);
    }
}

/// One row in a search ingredient's mock-item set. Owned so the
/// ingredient struct stays plain data; `SearchItem<'a>` references
/// are built fresh inside `render`.
struct SearchMockItem {
    label: String,
    haystack: String,
    synthetic_key: &'static str,
}

/// Configuration for one search-overlay preview variant. Holds the
/// item corpus, the query to type, and an optional cursor advance
/// count for highlighting a particular match.
struct SearchVariant {
    items: Vec<SearchMockItem>,
    query: &'static str,
    cursor_advance: usize,
    variant_name: &'static str,
    description_text: &'static str,
}

impl SearchVariant {
    fn send_chars(state: &mut SearchOverlayState, text: &str) {
        for ch in text.chars() {
            let event = KeyEvent {
                code: KeyCode::Char(ch),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            };
            let _ = state.handle_key(event);
        }
    }

    fn advance_cursor(state: &mut SearchOverlayState, steps: usize) {
        for _ in 0..steps {
            let event = KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            };
            let _ = state.handle_key(event);
        }
    }
}

impl Ingredient for SearchVariant {
    fn group(&self) -> &str {
        "Search"
    }

    fn name(&self) -> &str {
        self.variant_name
    }

    fn source(&self) -> &str {
        "conspectus::tui::widgets::search"
    }

    fn description(&self) -> &str {
        self.description_text
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let theme = Theme::default();
        let items: Vec<SearchItem<'_>> = self
            .items
            .iter()
            .map(|m| SearchItem {
                id: RowId::Synthetic(m.synthetic_key),
                label: Cow::Borrowed(m.label.as_str()),
                haystack: Cow::Borrowed(m.haystack.as_str()),
            })
            .collect();
        let mut state = SearchOverlayState::new();
        Self::send_chars(&mut state, self.query);
        let backend = SubstringBackend;
        state.refresh_matches(&backend, &items);
        Self::advance_cursor(&mut state, self.cursor_advance);
        SearchOverlayWidget::new(&state, &items, &theme).render(area, buf);
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
        Box::new(ValueModalVariant {
            label: "cwd",
            value: "/home/malloc47/src/conspectus".to_string(),
            scroll: 0,
            variant_name: "Single-line value",
            description_text: "Short scalar that fits on one row — the common `cwd` / id case.",
        }),
        Box::new(ValueModalVariant {
            label: "command",
            value: [
                "claude --dangerously-skip-permissions --resume 7b4a01c9-3d2e-4e5d-89ce-2839ff721cad",
                "  --max-tokens 200000",
                "  --model claude-sonnet-4-6-20251001",
                "  --append-system-prompt 'reviewing the spike outcome and recording findings'",
                "  --setting-source local",
                "  --ide vscode-attached",
                "  --working-directory /home/malloc47/src/conspectus",
                "  --hooks /home/malloc47/.claude/hooks.json",
            ]
            .join("\n"),
            scroll: 0,
            variant_name: "Multi-line wrapped",
            description_text: "Eight-line command argument list — exercises the wrap renderer at the modal width.",
        }),
        Box::new(ValueModalVariant {
            label: "last_message_preview",
            value: (0..40)
                .map(|i| format!("line-{i:02}  …content that would scroll if the modal can't fit it all on one frame"))
                .collect::<Vec<_>>()
                .join("\n"),
            scroll: 15,
            variant_name: "Scrolled (line 15)",
            description_text: "40-line synthetic preview with the viewport scrolled 15 rows down — exercises the mid-scroll render path.",
        }),
        Box::new(ValueModalVariant {
            label: "last_message_preview",
            value: (0..40)
                .map(|i| format!("line-{i:02}  …content that would scroll if the modal can't fit it all on one frame"))
                .collect::<Vec<_>>()
                .join("\n"),
            scroll: u16::MAX,
            variant_name: "Scrolled (bottom)",
            description_text: "Same content as the mid-scroll variant but scrolled to the bottom (u16::MAX clamps to the last line).",
        }),
        Box::new(HelpVariant {
            scroll: 0,
            variant_name: "Top of keymap",
            description_text:
                "Help modal at scroll=0 — the operator's first glance when they press `?`.",
        }),
        Box::new(HelpVariant {
            scroll: 20,
            variant_name: "Scrolled mid (line 20)",
            description_text:
                "Mid-scroll view — around the Pins (ADR 0057) section in the default keymap.",
        }),
        Box::new(HelpVariant {
            scroll: 50,
            variant_name: "Icon legend (scroll 50)",
            description_text:
                "Scrolled so the Node kind icons (ADR 0073) section is in view — exercises the per-glyph rendering through `push_icon_legend`.",
        }),
        Box::new(SearchVariant {
            items: search_mock_items(),
            query: "",
            cursor_advance: 0,
            variant_name: "Empty query",
            description_text:
                "Modal just opened — the `(start typing)` placeholder under the leading `/`.",
        }),
        Box::new(SearchVariant {
            items: search_mock_items(),
            query: "zzz_no_match_anywhere",
            cursor_advance: 0,
            variant_name: "No matches",
            description_text:
                "Query that doesn't match any item — the `(no matches)` placeholder under the typed query.",
        }),
        Box::new(SearchVariant {
            items: search_mock_items(),
            query: "claude",
            cursor_advance: 0,
            variant_name: "Several matches (cursor on first)",
            description_text:
                "Substring backend ranks every item containing `claude`; cursor parks on the top match.",
        }),
        Box::new(SearchVariant {
            items: search_mock_items(),
            query: "code",
            cursor_advance: 2,
            variant_name: "Cursor on a non-first match",
            description_text:
                "Same backend, different query — cursor advanced two rows to exercise the non-cursor + cursor row styling side by side.",
        }),
    ];
    tui_pantry::run!(ingredients)
}

/// Synthetic item corpus shared across every search variant. Each
/// row uses a `RowId::Synthetic("…")` key so no graph database is
/// required. Labels mirror the kind of strings the live row tree
/// surfaces (harness/session/repo/forge); haystacks add the
/// extra fields the search backend ranks against.
fn search_mock_items() -> Vec<SearchMockItem> {
    [
        (
            "ses_166f opencode · Add minibuffer workflow",
            "ses_166f opencode add minibuffer workflow",
            "ses_166f",
        ),
        (
            "2739a53f claude · Visual verification spike",
            "2739a53f claude visual verification spike",
            "ses_2739",
        ),
        (
            "019eddb5 codex · Stylize the conspectus tui",
            "019eddb5 codex stylize conspectus tui",
            "ses_019e",
        ),
        (
            "conspectus / atelier (group)",
            "conspectus atelier agent-deck group",
            "grp_conspectus",
        ),
        (
            "mux:agentdeck_local-command",
            "agentdeck local-command tmux mux",
            "mux_local",
        ),
        (
            "PR #87 (open) Stylesheet adjustments",
            "pr 87 open stylesheet adjustments code",
            "pr_87",
        ),
        (
            "PR #92 (merged) Pantry harness for widgets",
            "pr 92 merged pantry harness widgets code",
            "pr_92",
        ),
        (
            "Pin: ingest-refactor (unbound)",
            "pin ingest refactor unbound codex",
            "pin_ingest",
        ),
    ]
    .into_iter()
    .map(|(label, haystack, key)| SearchMockItem {
        label: label.to_string(),
        haystack: haystack.to_string(),
        synthetic_key: key,
    })
    .collect()
}
