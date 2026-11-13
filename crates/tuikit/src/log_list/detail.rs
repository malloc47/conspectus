//! The selected entry's detail: text the embedded pager reads, wrapped at
//! the width it is drawn at.

use std::ops::Range;

use ratatui::text::Line;

use crate::keys::KeyRow;
use crate::pager::{NoKeys, PagerContent, PagerEffect};
use crate::scroll::WrappedRows;
use crate::theme::Theme;

/// The text of one entry's detail, laid out a line at a time and wrapped
/// by the framework's rules. The layout is kept while the width and the
/// text are the same, so a draw that only scrolls wraps nothing again.
#[derive(Clone, Debug, Default)]
pub(super) struct Detail {
    text: String,
    rows: WrappedRows,
    laid_out: Option<u16>,
}

impl Detail {
    /// The text shown.
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// Shows `text` instead; the next layout wraps it.
    pub(super) fn set(&mut self, text: String) {
        self.text = text;
        self.laid_out = None;
    }
}

impl PagerContent for Detail {
    type Message = ();
    type Action = NoKeys;
    const KEYS: &'static [KeyRow<NoKeys>] = &[];

    fn layout(&mut self, width: u16, _theme: &Theme) -> usize {
        if self.laid_out != Some(width) {
            let lines: Vec<Line<'_>> = self.text.lines().map(Line::raw).collect();
            self.rows = WrappedRows::new(&lines, usize::from(width));
            self.laid_out = Some(width);
        }
        self.rows.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.rows.rows()[range].to_vec()
    }

    /// An embedded pager draws no header.
    fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
        Line::default()
    }

    fn apply(&mut self, action: NoKeys) -> PagerEffect<()> {
        match action {}
    }
}
