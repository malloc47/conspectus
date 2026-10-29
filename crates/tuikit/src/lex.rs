//! The small Rust lexer the source scanners share: where the comments and
//! literals of a piece of source are, and which items a `#[cfg(test)]`
//! attribute gates.
//!
//! The glyph scanner reads literals and skips comments and test items, and
//! the test kit's source helpers blank or extract the same spans, so both
//! go through this one lexer. It understands line and nested block
//! comments, string literals with escapes, raw and byte strings, and
//! character literals versus lifetimes. It assumes `rustfmt`-formatted
//! source: it is a tripwire for scans, not a parser.

use std::ops::Range;

/// The attribute that gates an item to test builds.
const TEST_GATE: &str = "#[cfg(test)]";

/// What a span of source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A line, doc, or block comment.
    Comment,
    /// A string literal (`"..."`, the quote of `b"..."` onward), quotes
    /// included.
    Text,
    /// A raw string literal (`r"..."`, `r#"..."#`, the `r` of `br"..."`
    /// onward), prefix and quotes included.
    RawText,
    /// A character literal (`'x'`, `'\n'`).
    Char,
}

/// One comment or literal: its character range and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    /// Index of its first character.
    pub start: usize,
    /// Index just past its last character.
    pub end: usize,
    /// What it is.
    pub kind: Kind,
}

/// A piece of source, lexed.
pub(crate) struct Lexed {
    /// The source's characters.
    pub chars: Vec<char>,
    /// Every comment and literal, in order.
    pub tokens: Vec<Token>,
}

impl Lexed {
    /// Lexes `source`.
    pub fn new(source: &str) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let tokens = tokens(&chars);
        Self { chars, tokens }
    }

    /// The source with the spans `blanked` chooses replaced by spaces,
    /// line breaks kept so line numbers still match.
    pub fn blanked(&self, blanked: impl Fn(Kind) -> bool) -> Vec<char> {
        let mut out = self.chars.clone();
        for token in &self.tokens {
            if blanked(token.kind) {
                blank_range(&mut out, token.start..token.end);
            }
        }
        out
    }

    /// The source with comments and literals blanked, so braces and
    /// brackets inside text cannot confuse item matching.
    pub fn skeleton(&self) -> Vec<char> {
        self.blanked(|_| true)
    }

    /// The character ranges of the items behind a `#[cfg(test)]`
    /// attribute, attribute included, through the item's closing brace or
    /// the `;` of a body-less item such as `mod tests;`.
    pub fn test_items(&self) -> Vec<Range<usize>> {
        let skeleton = self.skeleton();
        let gate: Vec<char> = TEST_GATE.chars().collect();
        let mut out = Vec::new();
        let mut index = 0;
        while index < skeleton.len() {
            if skeleton[index..].starts_with(&gate) {
                let end = item_end(&skeleton, index + gate.len());
                out.push(index..end);
                index = end;
            } else {
                index += 1;
            }
        }
        out
    }
}

/// `c` as a blank: a line break stays, anything else becomes a space.
pub(crate) fn blank(c: char) -> char {
    if c == '\n' { '\n' } else { ' ' }
}

/// Blanks `range` of `chars`.
pub(crate) fn blank_range(chars: &mut [char], range: Range<usize>) {
    for c in &mut chars[range] {
        *c = blank(*c);
    }
}

/// Whether `c` can be part of an identifier.
pub(crate) fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every comment and literal of `chars`, in order.
fn tokens(chars: &[char]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        match token_end(chars, index) {
            Some((end, kind)) => {
                out.push(Token {
                    start: index,
                    end,
                    kind,
                });
                index = end;
            }
            None => index += 1,
        }
    }
    out
}

/// The end and kind of the comment or literal starting at `index`, or
/// `None` when plain code starts there.
fn token_end(chars: &[char], index: usize) -> Option<(usize, Kind)> {
    let second = chars.get(index + 1).copied();
    match chars[index] {
        '/' if second == Some('/') => {
            let end = chars[index..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(chars.len(), |offset| index + offset);
            Some((end, Kind::Comment))
        }
        '/' if second == Some('*') => Some((block_comment_end(chars, index), Kind::Comment)),
        '"' => Some((string_end(chars, index + 1), Kind::Text)),
        'r' => raw_string_end(chars, index).map(|end| (end, Kind::RawText)),
        '\'' => char_literal_end(chars, index).map(|end| (end, Kind::Char)),
        _ => None,
    }
}

/// Index just past the string literal whose body starts at `from`.
fn string_end(chars: &[char], from: usize) -> usize {
    let mut index = from;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '"' => return index + 1,
            _ => index += 1,
        }
    }
    chars.len()
}

/// Index just past the (possibly nested) block comment starting at `start`.
fn block_comment_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0usize;
    let mut index = start;
    while index < chars.len() {
        if chars[index..].starts_with(&['/', '*']) {
            depth += 1;
            index += 2;
        } else if chars[index..].starts_with(&['*', '/']) {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    chars.len()
}

/// The number of `#`s of the raw string literal starting at the `r` at
/// `start`, or `None` when the `r` begins something else (an identifier,
/// `r#ident`).
pub(crate) fn raw_hashes(chars: &[char], start: usize) -> Option<usize> {
    let after_prefix = start == 0
        || !is_identifier_char(chars[start - 1])
        || (chars[start - 1] == 'b' && (start == 1 || !is_identifier_char(chars[start - 2])));
    if !after_prefix {
        return None;
    }
    let hashes = chars[start + 1..].iter().take_while(|c| **c == '#').count();
    (chars.get(start + 1 + hashes) == Some(&'"')).then_some(hashes)
}

/// Index just past a raw string literal starting at the `r` at `start`, or
/// `None` when the `r` begins something else.
fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    let hashes = raw_hashes(chars, start)?;
    let closing: Vec<char> = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    let end = (start + 2 + hashes..chars.len())
        .find(|&index| chars[index..].starts_with(&closing))
        .map_or(chars.len(), |index| index + closing.len());
    Some(end)
}

/// Index just past the character literal starting at `start`, or `None`
/// for a lifetime.
fn char_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start + 1) == Some(&'\\') {
        let tail = chars.get(start + 3..)?;
        let close = tail.iter().position(|c| *c == '\'')?;
        Some(start + 3 + close + 1)
    } else if chars.get(start + 2) == Some(&'\'') {
        Some(start + 3)
    } else {
        None
    }
}

/// Index just past the item that begins at `from`: through its closing
/// brace, or through the `;` of a body-less item such as `mod tests;`.
fn item_end(skeleton: &[char], from: usize) -> usize {
    let mut nesting = 0i32;
    let mut index = from;
    while index < skeleton.len() {
        match skeleton[index] {
            '(' | '[' => nesting += 1,
            ')' | ']' => nesting -= 1,
            ';' if nesting == 0 => return index + 1,
            '{' if nesting == 0 => return block_end(skeleton, index),
            _ => {}
        }
        index += 1;
    }
    skeleton.len()
}

/// Index just past the brace block that opens at `open`.
fn block_end(skeleton: &[char], open: usize) -> usize {
    let mut depth = 0i32;
    for (offset, c) in skeleton[open..].iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return open + offset + 1;
        }
    }
    skeleton.len()
}
