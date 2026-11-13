//! Rich text: ANSI text, captured terminal output, Markdown with its
//! tables, and highlighted code, as styled lines.
//!
//! This is a layer 1 foundation behind the `richtext` cargo feature, which
//! brings the parsing crates (ANSI escapes, Markdown, table layout, and,
//! through the Markdown renderer, a syntax highlighter); a program that
//! shows no rich content leaves it off and builds none of them. Everything
//! here turns text into [`Line`](ratatui::text::Line)s laid out at a width
//! the caller gives, so a pager's content, a preview, or any body draws
//! the result as it draws other rows (house rule 14).
//!
//! - **ANSI text** ([`ansi_text`]): text carrying color escapes as styled
//!   text, or with its styles dropped for a host drawing without color.
//! - **Captured output** ([`layout_capture`], [`WrapMode`]): a capture of a
//!   terminal's screen fitted to a narrower area: trailing blank rows
//!   dropped, then each line wrapped (`Plain`), wrapped with its rules,
//!   borders, and padding cut or squeezed instead (`Smart`, the default),
//!   or kept at the source's own layout and clipped (`None`).
//! - **Markdown** ([`markdown`]): headings, emphasis, lists, quotes, links,
//!   and code styled, fenced code highlighted by its language, and GFM
//!   pipe tables laid out to the width, all wrapped to it.
//! - **Tables** ([`segment_body`], [`render_table`]): the pre-pass that
//!   finds pipe tables in a body of Markdown, and the column-sizing layout
//!   that draws one with receding borders.
//! - **Code** ([`code`]): source in a named language as highlighted lines.
//!
//! ```
//! use ratatui::text::Line;
//! use tuikit::richtext::{WrapMode, ansi_text, layout_capture, markdown};
//! use tuikit::theme::Theme;
//!
//! // A capture: parse its escapes, then fit it to a 30-cell area.
//! let capture = ansi_text("\x1b[32mok\x1b[0m all checks passed\n\n", true);
//! let rows = layout_capture(&capture.lines, WrapMode::Smart, None, 30);
//! assert_eq!(rows.len(), 1);
//!
//! // A body of Markdown at the same width.
//! let lines: Vec<Line> = markdown("Some *emphasis*, then a list:\n\n- one\n- two", 30, &Theme::default());
//! assert!(lines.iter().any(|line| line.to_string().ends_with("- two")));
//! ```
//!
//! # What a host keeps
//!
//! Where the text comes from (a capture it ran, a document it read), which
//! wrap mode a person picked and how the setting is spelled, the width it
//! lays out at, and how the lines are framed: a gutter beside them, a
//! window onto them, a find painted over them.

mod ansi;
mod capture;
mod markdown;
mod table;

pub use ansi::ansi_text;
pub use capture::{WrapMode, layout_capture};
pub use markdown::{code, markdown};
pub use table::{BodySegment, ParsedTable, TableAlign, render_table, segment_body};
