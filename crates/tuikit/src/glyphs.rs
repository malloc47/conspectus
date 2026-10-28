//! Glyphs: the one-cell characters controls draw, vetted so they fit.
//!
//! This is a layer 1 foundation. A terminal draws a glyph its monospace
//! font lacks from a fallback font, and that glyph can spill over its
//! neighbor, so a selection highlight cuts it in half. The module keeps
//! that from happening in four parts:
//!
//! - **[`marks`]**: the shared glyphs (the list cursor, the disclosure
//!   arrows, the check and failure marks, the ellipsis, the no-value dash,
//!   a rule, a spinner), each defined once. A control uses the constant,
//!   never the character.
//! - **[`VettedGlyphs`]**: the table of glyphs that have been measured and
//!   how. The framework's own glyphs are in it; a host registers the
//!   glyphs of its own vocabulary from an adapter.
//! - **[`cell_width`] and [`glyph_risk`]**: the checks a glyph passes to be
//!   registered. It must be exactly one cell, must not need a joiner or
//!   variation selector, and must not be a character terminals commonly
//!   draw as a wide emoji.
//! - **[`drawn_glyphs`]**: the source scanner. It reports the non-ASCII
//!   characters in a piece of Rust code, outside comments and test items,
//!   and [`VettedGlyphs::unvetted`] filters those to the ones nothing has
//!   vetted. The host's source test walks its trees with it and fails on a
//!   glyph nobody measured; the framework's own guard fails on a glyph
//!   typed anywhere but this module.
//!
//! Two glyphs are kept by operator preference even though they carry a
//! known risk: the collapsed-disclosure triangle ([`marks::COLLAPSED`],
//! U+25B6, which a terminal with an emoji font may draw wide) and the
//! Braille spinner frames ([`marks::SPINNER`], which some fonts draw past
//! one cell). The first is the single exemption in [`glyph_risk`]; the
//! second is not machine-checkable. Both say so in their table notes.
//!
//! ```
//! use tuikit::glyphs::{VettedGlyphs, cell_width, marks};
//!
//! // A control draws a mark by constant.
//! let row = format!("{} item", marks::POINTER);
//! assert_eq!(cell_width(marks::POINTER), 1);
//! assert!(row.starts_with(marks::POINTER));
//!
//! // A host vets the glyphs of its own vocabulary on top of the framework's.
//! let mut vetted = VettedGlyphs::default();
//! vetted
//!     .register("◆", "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous")
//!     .unwrap();
//!
//! // A source scan then reports every glyph nothing vetted.
//! let found = vetted.unvetted("const A: &str = \"◆\";\nconst B: &str = \"☃\";\n");
//! assert_eq!(found.len(), 1);
//! assert_eq!((found[0].line, found[0].glyph), (2, '☃'));
//! ```

pub mod marks;
mod scan;
mod vetted;

use unicode_width::UnicodeWidthStr;

pub use scan::{Drawn, drawn_glyphs};
pub use vetted::{RegisterError, VettedGlyphs};

/// How many terminal cells `glyph` takes.
pub fn cell_width(glyph: &str) -> usize {
    UnicodeWidthStr::width(glyph)
}

/// Why `glyph` must not be drawn in a one-cell slot, if it can't be: it
/// is wide, can take emoji presentation, or needs a joiner or variation
/// selector to draw.
///
/// [`marks::COLLAPSED`] is exempt from the emoji check by operator
/// preference.
pub fn glyph_risk(glyph: &str) -> Option<&'static str> {
    if cell_width(glyph) != 1 || glyph.chars().count() != 1 {
        return Some("not exactly one cell wide, or more than one character");
    }
    let cp = glyph.chars().next().map_or(0, |c| c as u32);
    match cp {
        0x200D | 0xFE00..=0xFE0F => Some("joiner or variation selector"),
        0x1F000..=0x1FFFF => Some("emoji block"),
        // Text-presentation characters that terminals commonly draw as
        // color emoji, wide, when an emoji font is installed. The
        // collapsed-disclosure triangle is kept by operator preference.
        _ if glyph == marks::COLLAPSED => None,
        _ if EMOJI_CAPABLE.contains(&cp) => Some("can take emoji presentation"),
        _ => None,
    }
}

/// Characters that have an emoji presentation and so may draw wide: a
/// deny-list of the ones near a terminal interface's vocabulary.
const EMOJI_CAPABLE: &[u32] = &[
    0x25B6, 0x25C0, 0x25AA, 0x25AB, 0x25FB, 0x25FC, 0x25FD, 0x25FE, 0x2699, 0x26A0, 0x2714, 0x2716,
    0x2764, 0x2611, 0x2B05, 0x2B06, 0x2B07, 0x2B1B, 0x2B1C, 0x2B50, 0x2B55, 0x231A, 0x231B, 0x23E9,
    0x23EA, 0x23F0, 0x23F3, 0x2705, 0x274C, 0x2753, 0x2757, 0x26D3, 0x2139,
];

#[cfg(test)]
#[path = "glyphs_tests.rs"]
mod tests;
