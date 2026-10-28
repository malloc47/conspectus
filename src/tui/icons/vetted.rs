//! Conspectus's glyphs, registered with the framework's vetted table.
//!
//! `tuikit::glyphs` vets the characters every control shares. The glyphs
//! below are Conspectus's own: one per node kind, one per row state, and
//! the few others its sources draw (the transcript viewer's table borders
//! and result arrow, the graph export's star). [`register_glyphs`] adds
//! them with a note on how each was measured, so the source scan can check
//! every character either crate draws against one table.
//!
//! Measure a new glyph with the framework's `scripts/glyph-widths.py` and
//! record the result here. `ambiguous` marks glyphs whose East Asian width
//! is ambiguous: one cell in Western locales, two in some CJK ones, as
//! every Geometric Shapes glyph is. Wide and emoji-presentation glyphs are
//! not allowed at all.

use tuikit::glyphs::{RegisterError, VettedGlyphs};

use super::{NodeKind, state};

/// The shared note for the transcript viewer's table borders, which
/// `comfy-table` draws from the box-drawing block.
const TABLE: &str = "Box Drawing; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous";

/// How a node kind's default glyph was vetted. `None` for a kind that
/// draws a state glyph (a pin), which [`STATES`] registers.
///
/// The match is exhaustive, so a new kind cannot ship without a note.
fn kind_note(kind: NodeKind) -> Option<&'static str> {
    match kind {
        NodeKind::Workspace
        | NodeKind::Checkout
        | NodeKind::Repo
        | NodeKind::MuxSession
        | NodeKind::RuntimeProcess => Some("Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous"),
        NodeKind::AgentSession => {
            Some("WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous")
        }
        NodeKind::Branch => Some("Miscellaneous Technical; DejaVu Sans Mono 1.00"),
        NodeKind::Fork => Some("Mathematical Operators; DejaVu Sans Mono 1.00"),
        NodeKind::ForgePr => Some("Arrows; DejaVu Sans Mono 1.00"),
        NodeKind::Pin => None,
    }
}

/// The state glyphs and how each was vetted. `state::UNKNOWN` is ASCII,
/// which is always vetted, so it has no entry.
const STATES: &[(&str, &str)] = &[
    (
        state::LINKED,
        "Mathematical Operators; DejaVu Sans Mono 1.00",
    ),
    (
        state::UNLINKED,
        "Mathematical Operators; DejaVu Sans Mono 1.00",
    ),
    (
        state::STOPPED,
        "Mathematical Operators; DejaVu Sans Mono 1.00",
    ),
    (
        state::PR_OPEN,
        "WGL4; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        state::PR_DRAFT,
        "Geometric Shapes; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        state::PR_MERGED,
        "Mathematical Operators; DejaVu Sans Mono 1.00",
    ),
    (
        state::PR_CLOSED,
        "Mathematical Operators; DejaVu Sans Mono 1.00",
    ),
    (
        state::UNTRACKED,
        "Geometric Shapes; DejaVu Sans Mono and Liberation Mono 1.00; ambiguous",
    ),
    (
        state::PIN_RUNNING,
        "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous",
    ),
    (
        state::PIN_STOPPED,
        "Geometric Shapes; DejaVu Sans Mono 1.00; ambiguous",
    ),
];

/// Glyphs the sources draw that belong to no row vocabulary.
const OTHERS: &[(&str, &str)] = &[
    (
        "↳",
        "Arrows; DejaVu Sans Mono 1.00; leads a tool result in the transcript viewer",
    ),
    (
        "★",
        "Miscellaneous Symbols; DejaVu Sans Mono 1.00; marks the resolver's pick in the graph export",
    ),
    ("├", TABLE),
    ("┤", TABLE),
    ("┬", TABLE),
    ("┴", TABLE),
    ("┼", TABLE),
    ("═", TABLE),
    ("╞", TABLE),
    ("╡", TABLE),
    ("╪", TABLE),
    ("╤", TABLE),
    ("╧", TABLE),
    ("╠", TABLE),
    ("╣", TABLE),
    ("╬", TABLE),
    ("┆", "Box Drawing; DejaVu Sans Mono 1.00; ambiguous"),
    ("╌", "Box Drawing; DejaVu Sans Mono 1.00; ambiguous"),
];

/// Registers the node-kind glyphs, the state glyphs, and the other
/// domain glyphs the sources draw.
///
/// # Errors
///
/// A [`RegisterError`] when an entry is a duplicate of one already in
/// `table` or is not a one-cell glyph.
pub fn register_glyphs(table: &mut VettedGlyphs) -> Result<(), RegisterError> {
    for kind in NodeKind::ALL {
        if let Some(note) = kind_note(kind) {
            table.register(kind.default_glyph(), note)?;
        }
    }
    table.register_all(STATES)?;
    table.register_all(OTHERS)
}

/// The framework's vetted table with Conspectus's glyphs added: what the
/// source scan checks every drawn character against.
///
/// # Errors
///
/// The [`RegisterError`] [`register_glyphs`] reports.
pub fn vetted_glyphs() -> Result<VettedGlyphs, RegisterError> {
    let mut table = VettedGlyphs::default();
    register_glyphs(&mut table)?;
    Ok(table)
}
