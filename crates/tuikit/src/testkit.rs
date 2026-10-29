//! The test kit: what a host's tests, and its dev tools, use to drive and
//! inspect the screens the framework draws.
//!
//! This is a layer 2 convention, behind the `testkit` cargo feature (off by
//! default, so a production build carries none of it). A host turns the
//! feature on from its dev-dependencies, and from any dev-only feature of
//! its own that draws frames outside a terminal. The crate's own tests
//! always have it.
//!
//! - **[`keys`]**: the key-script notation (`j<Tab>jjj`, `<C-r>`,
//!   `<S-Tab>`) a dev tool or a test writes a run of presses in, and
//!   constructors for single presses ([`keys::press`], [`keys::ctrl`]).
//! - **[`render`]**: the one-frame driver. [`render::FrameDriver`] draws a
//!   frame of a given size off screen through whatever draw callback the
//!   host passes, and can press a run of keys with a frame drawn before
//!   each, as a live loop does.
//! - **[`buffer`]**: reading a drawn frame: its text, the token columns of
//!   a row, the cell and style under a piece of text, a count of cells that
//!   match, the rectangle of a modal's frame, and the text inside it.
//! - **[`ansi`]**: a frame as ANSI text (colors and modifiers kept), and
//!   [`strip_ansi`] to take them out again.
//! - **[`golden`]**: a style-aware text form of a frame that keeps every
//!   color and modifier, so a change of color alone fails a comparison,
//!   and a check of a frame against a golden file.
//! - **[`conformance`]**: the conformance suite. A host lists its modals
//!   in a [`conformance::Registry`] (how to open each, its key tables, its
//!   footer) with a [`conformance::Rig`] that draws and drives its screen,
//!   and [`conformance::assert_conventions`] holds every modal to the
//!   framework's conventions: it is drawn, nothing it draws escapes its
//!   frame, moving its selection moves no text, no word is cut at its
//!   edge, its keys keep the modal key convention, its footer is the one
//!   its table generates, and it closes by the one rule.
//! - **[`source`]**: the source-scan helpers hygiene tests build on: the
//!   crate's Rust lexer (comments, literals, and test items blanked or
//!   read), word-start matching, and a read-only file walk.
//!
//! Like the rest of the crate, the kit starts no process and writes no
//! file: accepting a new golden goes through a writer the host supplies.
//!
//! ```
//! use ratatui::widgets::Paragraph;
//! use tuikit::testkit::{ansi, buffer, keys, render};
//!
//! let presses = keys::parse("j<Tab><C-c>").expect("a valid script");
//! assert_eq!(presses.len(), 3);
//!
//! let frame = render::render_widget(Paragraph::new("hello"), 8, 1);
//! assert_eq!(buffer::text(&frame), "hello   \n");
//! assert_eq!(tuikit::testkit::strip_ansi(&ansi::to_ansi(&frame, frame.area)), "hello   ");
//! ```

pub mod ansi;
pub mod buffer;
pub mod conformance;
pub mod golden;
pub mod keys;
pub mod render;
pub mod source;

pub use crate::text::strip_ansi;
