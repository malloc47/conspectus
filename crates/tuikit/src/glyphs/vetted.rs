//! The vetted-glyph table: the glyphs that have been checked to draw inside
//! one terminal cell, and how.
//!
//! A terminal falls back to a proportional font for a glyph its monospace
//! font lacks, and that glyph then overflows its cell (and a selection
//! highlight cuts it in half). So every glyph here is in DejaVu Sans Mono
//! at exactly one cell advance, in Latin-1, or in the WGL4 set that
//! Liberation Mono, Courier New, and Andale Mono also carry. Measure a new
//! glyph with `scripts/glyph-widths.py` and register it with a note saying
//! how it was checked.
//!
//! The framework's own glyphs (its marks, arrows, separators, and box
//! drawing) are in the table from the start. A host registers the glyphs
//! of its own vocabulary on top, from an adapter, so the table the source
//! scan checks against is the union.

use std::error::Error;
use std::fmt;

use super::glyph_risk;
use super::marks;
use super::scan::{Drawn, drawn_glyphs};

/// The note for a glyph in the Geometric Shapes block that the WGL4 set
/// also carries.
const WGL4_SHAPE: &str = "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous";
/// The note for the box-drawing family.
const BOX: &str = "Box Drawing; drawn by the terminal itself in most; ambiguous";
/// The note for a half block (the sides of a toast's border).
const HALF_BLOCK: &str = "WGL4 block elements; every font; drawn by the terminal itself in most";
/// The note for a quadrant block (the corners of a toast's border).
const QUADRANT: &str = "Block Elements; DejaVu Sans Mono 1.00, not in the other WGL4 fonts; \
     drawn by the terminal itself in most";
/// The note for a spinner frame.
const BRAILLE: &str =
    "Braille Patterns; kept by operator preference; some fonts draw it past one cell";

/// The framework's own entries. The marks come first, by constant, so a
/// mark's character is typed once.
const FRAMEWORK: &[(&str, &str)] = &[
    (marks::POINTER, WGL4_SHAPE),
    (marks::BACK, WGL4_SHAPE),
    (marks::EXPANDED, WGL4_SHAPE),
    (
        marks::COLLAPSED,
        "Geometric Shapes; kept by operator preference; a terminal with an emoji font may draw it wide",
    ),
    (marks::WARNING, WGL4_SHAPE),
    (
        marks::OK,
        "Mathematical Operators; DejaVu Sans Mono and Liberation Mono 1.00",
    ),
    (marks::FAIL, "Latin-1; every font"),
    (marks::BULLET, "WGL4; every font; ambiguous"),
    (marks::ELLIPSIS, "WGL4; every font; ambiguous"),
    (
        marks::NO_VALUE,
        "Latin-1 range punctuation; every font; ambiguous",
    ),
    (marks::RULE, BOX),
    (marks::DERIVED, BOX),
    (marks::SEPARATOR, "Latin-1; every font; ambiguous"),
    (marks::ARROW_LEFT, "WGL4 arrows; every font; ambiguous"),
    (marks::ARROW_UP, "WGL4 arrows; every font; ambiguous"),
    (marks::ARROW_RIGHT, "WGL4 arrows; every font; ambiguous"),
    (marks::ARROW_DOWN, "WGL4 arrows; every font; ambiguous"),
    (marks::SPINNER[0], BRAILLE),
    (marks::SPINNER[1], BRAILLE),
    (marks::SPINNER[2], BRAILLE),
    (marks::SPINNER[3], BRAILLE),
    (marks::SPINNER[4], BRAILLE),
    (marks::SPINNER[5], BRAILLE),
    (marks::SPINNER[6], BRAILLE),
    (marks::SPINNER[7], BRAILLE),
    // Punctuation the interface text uses.
    ("›", "WGL4; every font"),
    // The line drawing of frames and borders.
    ("│", BOX),
    ("┃", BOX),
    ("║", BOX),
    ("┌", BOX),
    ("┐", BOX),
    ("┘", BOX),
    ("╭", BOX),
    ("╮", BOX),
    ("╰", BOX),
    ("╯", BOX),
    ("╔", BOX),
    ("╗", BOX),
    ("╚", BOX),
    ("╝", BOX),
    // The block border of a toast (ratatui's quadrant border set).
    ("▀", HALF_BLOCK),
    ("▄", HALF_BLOCK),
    ("▌", HALF_BLOCK),
    ("▐", HALF_BLOCK),
    ("▛", QUADRANT),
    ("▜", QUADRANT),
    ("▙", QUADRANT),
    ("▟", QUADRANT),
];

/// Why a glyph could not be registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The glyph is already in the table.
    Duplicate {
        /// The glyph.
        glyph: &'static str,
    },
    /// The note is empty. A note records how the glyph was checked.
    NoNote {
        /// The glyph.
        glyph: &'static str,
    },
    /// The glyph cannot be drawn in one cell reliably.
    Risky {
        /// The glyph.
        glyph: &'static str,
        /// Why, as [`glyph_risk`] words it.
        why: &'static str,
    },
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { glyph } => write!(f, "`{glyph}` is registered twice"),
            Self::NoNote { glyph } => write!(
                f,
                "`{glyph}` needs a note saying how it was vetted: measure it with \
                 scripts/glyph-widths.py and record the result"
            ),
            Self::Risky { glyph, why } => write!(f, "`{glyph}` cannot be vetted: {why}"),
        }
    }
}

impl Error for RegisterError {}

/// The table of vetted glyphs.
///
/// [`Default`] gives the framework's own entries. A host registers the
/// glyphs of its own vocabulary on top and passes the table to the source
/// scan ([`VettedGlyphs::unvetted`]). A table is a plain value: nothing
/// reads it at draw time, and two tables never share state.
#[derive(Clone, Debug)]
pub struct VettedGlyphs {
    entries: Vec<(&'static str, &'static str)>,
}

impl Default for VettedGlyphs {
    fn default() -> Self {
        let mut table = Self::empty();
        for (glyph, note) in FRAMEWORK {
            table.register(glyph, note).unwrap_or_else(|error| {
                panic!("the framework's own glyph table is invalid: {error}")
            });
        }
        table
    }
}

impl VettedGlyphs {
    /// A table with no entries, for a test of the registration rules.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds `glyph`, with a note on how it was checked.
    ///
    /// # Errors
    ///
    /// Fails when the glyph is already registered, the note is empty, or
    /// [`glyph_risk`] rejects the glyph. A failed call changes nothing.
    pub fn register(
        &mut self,
        glyph: &'static str,
        note: &'static str,
    ) -> Result<(), RegisterError> {
        if note.trim().is_empty() {
            return Err(RegisterError::NoNote { glyph });
        }
        if let Some(why) = glyph_risk(glyph) {
            return Err(RegisterError::Risky { glyph, why });
        }
        if self.entries.iter().any(|(known, _)| *known == glyph) {
            return Err(RegisterError::Duplicate { glyph });
        }
        self.entries.push((glyph, note));
        Ok(())
    }

    /// Adds each `(glyph, note)` pair in order, stopping at the first
    /// error. Pairs before the failing one stay registered.
    ///
    /// # Errors
    ///
    /// The first [`RegisterError`] [`VettedGlyphs::register`] reports.
    pub fn register_all(
        &mut self,
        entries: &[(&'static str, &'static str)],
    ) -> Result<(), RegisterError> {
        for (glyph, note) in entries {
            self.register(glyph, note)?;
        }
        Ok(())
    }

    /// Whether `glyph` is vetted. Printable ASCII always is, so a host does
    /// not register `?`.
    pub fn contains(&self, glyph: &str) -> bool {
        is_printable_ascii(glyph) || self.entries.iter().any(|(known, _)| *known == glyph)
    }

    /// How `glyph` was vetted, or `None` for a glyph that is not registered.
    pub fn note(&self, glyph: &str) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|(known, _)| *known == glyph)
            .map(|(_, note)| *note)
    }

    /// The registered `(glyph, note)` pairs, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.entries.iter().copied()
    }

    /// How many glyphs are registered.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The characters `source` draws (see [`drawn_glyphs`]) that this table
    /// does not vet, in order.
    pub fn unvetted(&self, source: &str) -> Vec<Drawn> {
        let mut buffer = [0u8; 4];
        drawn_glyphs(source)
            .into_iter()
            .filter(|drawn| !self.contains(drawn.glyph.encode_utf8(&mut buffer)))
            .collect()
    }
}

/// One printable ASCII character.
fn is_printable_ascii(glyph: &str) -> bool {
    let mut chars = glyph.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if (' '..='~').contains(&c))
}

#[cfg(test)]
#[path = "vetted_tests.rs"]
mod tests;
