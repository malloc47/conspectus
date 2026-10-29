//! The shared marks: the one-cell glyphs every control draws for the same
//! idea, each defined once.
//!
//! A control that needs the row cursor, a disclosure arrow, or the "no
//! value" dash uses the constant here instead of typing the character, so
//! two surfaces cannot drift to look-alike codepoints and the width and
//! font checks in [`super`] cover every use. A source guard fails on a
//! mark's character typed anywhere else in the framework.
//!
//! Every mark is exactly one terminal cell, is in the framework's vetted
//! table, and is listed in [`ALL`]. A few constants here are not marks of
//! their own but are built from them, three cells each: [`SPACED_SEPARATOR`]
//! ([`SEPARATOR`] padded with spaces) and the boxes a form draws before an
//! option ([`CHECKED`], [`UNCHECKED`], [`CHOSEN`], [`NOT_CHOSEN`]). A host
//! adds marks of its own to its copy of the table with
//! [`super::VettedGlyphs::register`].

/// The row cursor in a list and the focus marker in a pane title.
pub const POINTER: &str = "►";
/// "Go back": the mark between a title and the path that led to it.
pub const BACK: &str = "◄";
/// A row or group that is expanded.
pub const EXPANDED: &str = "▼";
/// A row or group that is collapsed.
///
/// This is U+25B6, which a terminal with an emoji font may draw two cells
/// wide. It is kept by operator preference and is the one glyph exempt
/// from the emoji deny-list in [`super::glyph_risk`]. The pointer and the
/// back mark are the look-alike U+25BA and U+25C4, which are not
/// emoji-capable; do not substitute one for the other.
pub const COLLAPSED: &str = "▶";
/// A check that passed, or a value that checked out.
pub const OK: &str = "√";
/// A check that failed, or a value that did not.
pub const FAIL: &str = "×";
/// Something that needs attention but is not an error.
pub const WARNING: &str = "▲";
/// An unchecked item in a list of choices, and, between parentheses, the
/// chosen option of a one-of-N row ([`CHOSEN`]).
pub const BULLET: &str = "•";
/// Text cut to fit: the end of a truncated label.
pub const ELLIPSIS: &str = "…";
/// A value that is not known: the placeholder in a cell or a field.
pub const NO_VALUE: &str = "—";
/// One cell of a horizontal rule.
pub const RULE: &str = "─";
/// The corner that leads in a line derived from the rows above it (the
/// command a form will run), so it reads as output, not as a field.
pub const DERIVED: &str = "└";
/// An option that is on, in a row of options any number of which can be
/// on (a checkbox): `[x]`. Three cells, all ASCII.
pub const CHECKED: &str = "[x]";
/// An option that is off, in a row of options any number of which can be
/// on: `[ ]`. Three cells, all ASCII.
pub const UNCHECKED: &str = "[ ]";
/// The chosen option of a one-of-N row (a radio button): [`BULLET`]
/// between parentheses, `(•)`. Three cells; its shape tells it from a
/// checkbox ([`CHECKED`]) without color.
pub const CHOSEN: &str = "(•)";
/// An option of a one-of-N row that is not the chosen one: `( )`. Three
/// cells, all ASCII.
pub const NOT_CHOSEN: &str = "( )";
/// The separator between the parts of one item, such as a name and its
/// state in a chip (`wrap·on`), and between short items in a line.
pub const SEPARATOR: &str = "·";
/// [`SEPARATOR`] with a space on each side: what joins the short items of
/// a line, such as the hints of a footer (`Enter pick · Esc close`). It is
/// three cells, so it is not one of the marks in [`ALL`].
pub const SPACED_SEPARATOR: &str = " · ";
/// The Up arrow key, as a key label names it.
pub const ARROW_UP: &str = "↑";
/// The Down arrow key, as a key label names it.
pub const ARROW_DOWN: &str = "↓";
/// The Left arrow key, as a key label names it.
pub const ARROW_LEFT: &str = "←";
/// The Right arrow key, as a key label names it.
pub const ARROW_RIGHT: &str = "→";
/// The frames of a busy spinner, drawn one after another.
///
/// These are Braille dot patterns, kept by operator preference even though
/// some fonts draw them wider than one cell. They are not machine-checked
/// for that risk, so the vetted table says so in their note.
pub const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

/// Every mark once, spinner frames included. The width and font checks and
/// the module's own tests run over this list.
pub const ALL: &[&str] = &[
    POINTER,
    BACK,
    EXPANDED,
    COLLAPSED,
    OK,
    FAIL,
    WARNING,
    BULLET,
    ELLIPSIS,
    NO_VALUE,
    RULE,
    DERIVED,
    SEPARATOR,
    ARROW_UP,
    ARROW_DOWN,
    ARROW_LEFT,
    ARROW_RIGHT,
    SPINNER[0],
    SPINNER[1],
    SPINNER[2],
    SPINNER[3],
    SPINNER[4],
    SPINNER[5],
    SPINNER[6],
    SPINNER[7],
];
