//! The source scanner: which non-ASCII characters a piece of Rust source
//! draws.
//!
//! A glyph the terminal's font lacks is drawn from a fallback font and can
//! spill over its neighbor, so every character a program draws has to be
//! vetted (see [`super::VettedGlyphs`]). The scanner is the half of that
//! check that reads source: it reports each non-ASCII character in code,
//! which in Rust means string and character literals, and ignores the
//! places a character is prose, comments of every kind and `#[cfg(test)]`
//! items.
//!
//! It is a pure function over text, so a host walks its own trees (and
//! decides which files are test code) and hands each file's text here. The
//! lexing is the crate's one small lexer, which the test kit's source
//! helpers share: it understands line and nested block comments, string
//! literals with escapes, raw and byte strings, and character literals
//! versus lifetimes. It assumes `rustfmt`-formatted source, and it is a
//! tripwire for a forgotten glyph, not a parser.

use crate::lex::{Kind, Lexed, blank_range};

/// One non-ASCII character found in code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// The 1-based line it is on.
    pub line: usize,
    /// The character.
    pub glyph: char,
}

/// Every non-ASCII character `source` draws, in order, with its line.
///
/// Characters in comments (line, doc, and block) are skipped, and so is
/// every item behind a `#[cfg(test)]` attribute. Characters in string and
/// character literals count.
pub fn drawn_glyphs(source: &str) -> Vec<Drawn> {
    let lexed = Lexed::new(source);
    let mut visible = lexed.blanked(|kind| kind == Kind::Comment);
    for item in lexed.test_items() {
        blank_range(&mut visible, item);
    }
    let mut out = Vec::new();
    let mut line = 1;
    for c in visible {
        match c {
            '\n' => line += 1,
            c if !c.is_ascii() => out.push(Drawn { line, glyph: c }),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
