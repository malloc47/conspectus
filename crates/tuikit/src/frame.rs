//! Frames: the bordered box a modal draws in and how large it is, the
//! bordered box a pane of the screen draws in with its focus cue, the
//! split that places two panes, and the rule that heads a section of a
//! pane.
//!
//! This is a layer 1 foundation. A modal (a prompt, a checklist, a
//! menu, a form, a reader, a search palette) floats over the screen in
//! a box with a border and a title. Every such box has one look and is
//! sized by one rule here, so modals line up with each other and a
//! program never computes a modal's rect twice:
//!
//! - **Sizing policies.** [`ModalSize`] says how large a modal is on a
//!   screen of any size: for each direction an [`Extent`] (a share of
//!   the screen, a cap, a minimum, and a margin kept clear on each side)
//!   and, for the height, a [`Fit`] (as tall as the body or as tall as
//!   the extent allows). Five named policies cover the usual modals:
//!   [`ModalSize::COMPACT`] for a one-line prompt or a short checklist,
//!   [`ModalSize::MENU`] for a list of choices or settings,
//!   [`ModalSize::FORM`] for labeled fields, [`ModalSize::WIDE`] for
//!   reading (help, logs, long values) and long lists, and
//!   [`ModalSize::PALETTE`] for a search palette that takes most of the
//!   screen. A policy is data, so a host with another need writes its
//!   own from the same parameters.
//! - **One frame.** [`ModalFrame`] clears its rect (painting the theme's
//!   `surface`), so nothing drawn under the modal shows through, draws
//!   the border in the theme's `accent`, and writes the title in one
//!   form: plain text in the border's color with one space on each side.
//!   A modal whose body has no row to spare for its keys (a one-line
//!   prompt) hands the frame a footer, which sits on the bottom border in
//!   the same form.
//! - **Measured once.** A [`ModalBody`] is told the width the policy
//!   gives it, reports the rows it needs, and draws in the area the
//!   frame leaves; the frame places itself from those rows, once.
//!   [`Lines`] is the body for lines laid out ahead and scrolled to keep
//!   a cursor in view.
//! - **Pane frames.** A pane of the screen (a list, a detail view) draws
//!   in a [`PaneFrame`]: plain lines on all four sides and the title on
//!   the top border, in the modal title's form after a lead that holds
//!   the focus marker. The focused pane's border and marker take the
//!   theme's `panel_focus_accent`; an unfocused pane keeps the plain
//!   border and a blank marker, so its title stays in place.
//! - **Split panes.** A [`SplitPane`] puts two panes side by side in
//!   halves while the area is at least its threshold wide, and one above
//!   the other below it. A host asks it for both areas wherever it needs
//!   them, so the screen is split by one rule.
//! - **Section rules.** Inside a pane, a [`SectionRule`] heads each
//!   section: a rule in the theme's `muted`, the section's label as a
//!   chip in `accent` under `badge`, an optional summary in `inert`
//!   before or after the chip, and a two-cell closing rule, so the labels
//!   of the sections down a pane line up near its right edge.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use tuikit::frame::{Lines, ModalFrame, ModalSize};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 80, 24);
//! let frame = ModalFrame::new(&theme, ModalSize::MENU).title("Actions");
//!
//! // Lay the body out for the width the policy gives, then draw it.
//! assert_eq!(frame.inner_width(area), 62);
//! let lines = vec![Line::from("> Open"), Line::from("  Close")];
//! let mut buf = Buffer::empty(area);
//! frame.render(area, &mut buf, Lines::new(lines).following(0..1));
//!
//! // A menu is as tall as its rows and its border, and centered.
//! assert_eq!(ModalSize::MENU.rect(area, 4), Rect::new(8, 10, 64, 4));
//! assert_eq!(buf[(8, 10)].symbol(), "┌");
//! assert_eq!(buf[(10, 10)].symbol(), "A");
//! ```
//!
//! Two panes split from one area, the first focused:
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::layout::Rect;
//! use ratatui::text::Line;
//! use tuikit::frame::{PaneFrame, SplitPane};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 40, 4);
//! let mut buf = Buffer::empty(area);
//! let [left, right] = SplitPane::new(30).areas(area);
//! PaneFrame::new(&theme).title(Line::from("list")).focused(true).render(left, &mut buf);
//! PaneFrame::new(&theme).title(Line::from("detail")).render(right, &mut buf);
//! assert_eq!(rows(&buf)[0], "┌ ► list ──────────┐┌   detail ────────┐");
//! assert_eq!(buf[(0, 0)].fg, theme.panel_focus_accent);
//! assert_ne!(buf[(20, 0)].fg, theme.panel_focus_accent);
//! ```

mod modal;
mod pane;
mod rule;
mod size;
mod split;
mod title;

pub use modal::{Lines, ModalBody, ModalFrame};
pub use pane::PaneFrame;
pub use rule::SectionRule;
pub use size::{Extent, Fit, ModalSize};
pub use split::SplitPane;

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
