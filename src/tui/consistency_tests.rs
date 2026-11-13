//! Invariants the whole TUI must keep, asserted across every view and
//! row kind instead of one defect at a time.
//!
//! Manual testing of the pin, history, and rebuild views found about two
//! dozen defects. They were not independent: each is an instance of one
//! of a few rules being broken in one place. These tests state the rules.
//! A test that fails today carries an `#[ignore]` naming the backlog task
//! that owns the fix; the fix removes the attribute, so each repair
//! visibly flips its tests and the next violation of the same rule is
//! caught wherever it appears.
//!
//! # The rules
//!
//! - **One gesture, one meaning.** Enter, `v`, `p`, and `.` do the same
//!   thing to the same entity in every view (`parity`).
//! - **Every change ends the same way.** A state-changing effect declares
//!   whether it attaches, freezes the acted-on row, refreshes, and
//!   follows the entity to the view where it now lives, in one place
//!   (`effect::lifecycle`), and keys are routed through an executor that
//!   can run them (`lifecycle`).
//! - **Menus depend on the entity, not the view** that opened them, and
//!   offer only what applies (`menus`).
//! - **Accelerators act on what the operator is looking at**: the right
//!   pane's entity when it has focus (`focus`).
//! - **Rows look alike wherever they appear**: columns, chips, glyphs,
//!   dimming, focus highlight, and scroll (`rendering`), and a kind of
//!   row's lined-up cells in one place at every width and in every view
//!   (`columns`).
//! - **User-visible text speaks the operator's language**: no developer
//!   artifacts, and one word for one thing (`text`).
//! - **The chrome fits and agrees with the view**: the tab strip never
//!   cuts a view's name at any width, and the header counts what the
//!   active view lists (`chrome`).
//! - **No overlay shuts the operator in**: the fixture and scenario TUI
//!   close every overlay with the keys the live TUI uses (`fixture_mode`).
//! - **Feedback says how much it matters**: the status line, toasts, and
//!   the Messages log style each report by its severity, and a refused or
//!   failed gesture never reports like a success (`feedback`).
//!
//! # Diagnosis of the reported defects
//!
//! | Defect | Root cause |
//! |---|---|
//! | `p` does not pin; `.` then Pin does | Key bindings reach the reducer through `Action::Msg`, which the runtime runs with the pure executor. That executor silently drops store writes, launches, and mux ops. The menu path uses the live executor. (`runtime.rs` `Action::Msg` arm; `execute_pure_effect`) |
//! | Pinning a PR blanks the PR list | Every store write refreshes with `refresh = true`, which bypasses the daemon snapshot and rebuilds locally with cached-only forge fetch and an empty local forge cache, so PRs vanish until a hard refresh. (`runtime.rs` `store_write_refresh_config`) |
//! | Enter on a gone row rebuilds but does not attach or refresh the view | Gone, planned, and missing rows map to `ExecSpec::Rebuild`, which runs `conspectus rebuild --no-attach` and never attaches or moves focus. (`actions.rs` `pinned_primary_effect`, `history_primary_effect`; `runtime/launch.rs` `execute_rebuild`) |
//! | Returning from attach does not freeze the row with a spinner | The hand-off freeze is keyed on `RowKind::MuxSession` and `RowKind::AgentSession`; the mixed-kind rows of the Pinned view, History view, and ghost muxes are `RowKind::Mixed` and never match. (`app.rs` `row_awaits_handoff`, `selected_row_awaits_handoff`) |
//! | `.` in Mux or Sessions offers only Unpin | The menu is built from the selected row's kind. Only a Pinned-view row gets the pin menu; native rows get `pin_entries` (Pin or Unpin) alone. (`actions.rs` `row_menu_for`, `pin_entries`, `slot_menu`) |
//! | Show rebuild plan is empty for a live mux; offered for PR and session | `slot_menu` lists every entry for every pin regardless of kind or status. |
//! | Enter on a pinned session tries to launch it as a mux | `pinned_primary_effect` maps a live session to `Rebuild`. (`actions.rs`) |
//! | `v` on a pinned session errors | `resolve_view_session` only knows `AgentSession` and `MuxSession` rows; pin, nested, and mixed rows fall to `UnsupportedRow`. (`actions.rs`) |
//! | Accelerators ignore the right pane | `pin_subject`, `row_menu_for`, and the view/attach resolvers read `App::selection()` (the left row) and never `explorer().view.focused` or the cursor's link target. |
//! | Retarget is a free-text box | `Msg::OpenSlotRetarget` opens a single-line prompt; there is no candidate picker. |
//! | Editing a pin cannot be discovered | `Msg::OpenSlotEdit` opens a form that lists every field the pin's kind can override, with the layer each value comes from. |
//! | Launch form lost up/down focus, cwd completion, harness options | The pin forms that owned those features were removed with the old pin model, and the harness launch-option toggles were deleted as unused. The new mux launch form was written without them. |
//! | Gone muxes do not align with live ones | Ghost muxes are `RowKind::Mixed` rows drawn by the mixed-kind renderer with its own column widths; live muxes use the mux row renderer. The two never share a grid. (`ui/left_panel.rs`) |
//! | Unstarted pins have no distinct marker | Planned pins show a status word only; the pin glyph exists only for pinned entities in native views. |
//! | "slot" appears where a pin is selected | Menu titles, prompts, toasts, hints, and detail text use the storage word. (`actions.rs`, `app.rs`, `runtime/executor.rs`, `widgets/help.rs`) |
//! | ADR numbers appear in the TUI | Developer references were written into user-facing strings. Nothing prevented it. |
//! | Pinned and History show the agent as plain text with a dot | The mixed-kind row's content is a pre-joined string (`harness · path`), not the badge span other views draw. (`rows/pinned.rs` `describe`, `ui/left_panel.rs`) |
//! | Sessions rows lack the agent glyph | The kind glyph is suppressed in single-kind views (by design when written); the Pinned and History views show it, so the views disagree. (`ui/left_panel.rs` session row) |
//! | The right pane keeps a highlighted row when unfocused | The explorer cursor row is always drawn `REVERSED`; the left pane switches to a weaker style when unfocused. (`ui/right_panel.rs` `render_node_field_line` and link rows) |
//! | Regrouping keeps the old scroll | `SetGrouping` rebuilds the tree in place and keeps the selection, so the left scroll follows the selection instead of resetting. (`app.rs`) |
//! | History cannot hide pinned records | There is no pinned filter on the History view's row set. |

use std::path::{Path, PathBuf};

use ratatui::layout::Rect;
use tuikit::feedback::Feedback;

use super::*;
use crate::filter::RowFilter;
use crate::model::{
    AgentSessionId, AgentSessionNode, FieldValue, ForgePrId, ForgePrNode, GraphNode, GraphSnapshot,
    MuxSessionId, MuxSessionNode, NodeId, PinKind, PinSlot, SlotFields,
};
use crate::resolve::resolve_snapshot;
use crate::tui::app::{App, Msg, SnapshotHandle};
use crate::tui::rows::RowId;
use crate::tui::ui::{buffer_to_string, render_to_buffer};

/// The plumbing the source-scanning tests share, included from the
/// integration tests' support directory so one list of scanned trees, and
/// one set of minimum file counts, serves them all.
#[path = "../../tests/support/source_tree.rs"]
mod source_tree;

mod actions;
mod chrome;
mod columns;
mod content_filter;
mod content_interactions;
mod conventions;
mod docs;
mod feedback;
mod fixture_mode;
mod fixtures;
mod focus;
mod forms;
mod freshness;
mod glyphs;
mod help;
mod keys;
mod layout;
mod lifecycle;
mod marks;
mod menus;
mod parity;
mod registry;
mod rendering;
mod scrolling;
mod search_flat;
mod search_quick;
mod search_view;
mod selection;
mod state;
mod styling;
mod text;
mod text_entry;
mod undo;

use fixtures::*;

/// Source files of this suite, for checks that read their own text.
fn suite_sources() -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tui");
    let mut files = vec![root.join("consistency_tests.rs")];
    if let Ok(entries) = std::fs::read_dir(root.join("consistency_tests")) {
        files.extend(entries.flatten().map(|entry| entry.path()));
    }
    files
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            Some((path, text))
        })
        .collect()
}

/// Every ignored test names the task that owns its fix, so none is
/// parked without an owner.
#[test]
fn all_ignored_tests_are_owned() {
    let mut owned = 0;
    for (path, text) in suite_sources() {
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if !line.starts_with("#[ignore") {
                continue;
            }
            owned += 1;
            let ok = ["2", "3", "4", "5"].iter().any(|n| {
                line.contains(&format!("backlog CSP-692.0{n}:"))
                    && line.split("CSP-692.0").nth(1).is_some_and(|rest| {
                        rest.chars().nth(1) == Some(':') && rest[2..].trim_start().len() > 3
                    })
            });
            assert!(
                ok,
                "{}:{}: an ignored test must read `#[ignore = \"backlog CSP-692.0N: reason\"]`",
                path.display(),
                index + 1
            );
        }
    }
    // Once every defect is fixed no test is parked, which is the goal.
    let _ = owned;
}
