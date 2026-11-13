//! The explorer's keys: what each does, and the one table that binds
//! them.

use ratatui::crossterm::event::KeyCode;

use crate::glyphs::marks;
use crate::help::{HelpAction, HelpEntry};
use crate::keys::{Footer, KeyAction, KeyRole, KeyRow, alias, ch, exact, key, shown};

/// What a key asks of an explorer ([`Explorer::handle`](super::Explorer::handle)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExplorerKey {
    /// The next row, stopping at the last.
    Down,
    /// The previous row, stopping at the first.
    Up,
    /// Down by the rows the zone showed in the last frame.
    PageDown,
    /// Up by the rows the zone showed in the last frame.
    PageUp,
    /// The first row.
    First,
    /// The last row.
    Last,
    /// What the row is for: copy a property, follow a link, fold a
    /// section.
    Activate,
    /// Read a cut property whole.
    Inspect,
    /// Open or close the section whose header the cursor is on.
    ToggleSection,
    /// Open the section whose header the cursor is on.
    OpenSection,
    /// Close the section the cursor heads or sits in, and put the cursor
    /// on its header.
    CloseSection,
    /// Show every property instead of the summary, or the summary again.
    ToggleFull,
    /// Go back to the thing shown before the last drill; with none, arm,
    /// and leave on the next press.
    Back,
}

/// The explorer's keys, which a host resolves before its own keymap while
/// the explorer has focus: `↑/↓ move · Enter open · e fold · o inspect ·
/// F show all · Backspace back`.
///
/// - `↑`/`k` and `↓`/`j` move, `PgUp` and `PgDn` page, `Home`/`g` and
///   `End`/`G` go to the ends.
/// - `Enter` opens the row. It binds `Enter` with no modifier
///   ([`exact`]), so `Shift-Enter` and `Alt-Enter` reach the host's own
///   keymap.
/// - `e` folds the section on its header; `l`/`→` open it and `h`/`←`
///   close it from its header or from a row inside it (the tree's fold
///   gesture).
/// - `o` reads a cut property whole, `F` shows every property.
/// - `Backspace` and `Esc` go back one level (house rule 4); from the
///   first frame they arm and then leave.
///
/// `q` and `Ctrl-C` are not the explorer's: a pane gives way to its
/// program's quit keys, so this table neither binds them nor starts with
/// `Ctrl-C` as an overlay's table does. Characters bind as typed, so
/// `Alt-j` and `Ctrl-j` are not `j`.
pub const EXPLORER_KEYS: &[KeyRow<ExplorerKey>] = &[
    shown(key(KeyCode::Up), ExplorerKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), ExplorerKey::Up),
    shown(
        key(KeyCode::Down),
        ExplorerKey::Down,
        marks::ARROW_DOWN,
        "move",
    ),
    alias(ch('j'), ExplorerKey::Down),
    alias(key(KeyCode::PageUp), ExplorerKey::PageUp),
    alias(key(KeyCode::PageDown), ExplorerKey::PageDown),
    alias(key(KeyCode::Home), ExplorerKey::First),
    alias(ch('g'), ExplorerKey::First),
    alias(key(KeyCode::End), ExplorerKey::Last),
    alias(ch('G'), ExplorerKey::Last),
    shown(
        exact(KeyCode::Enter),
        ExplorerKey::Activate,
        "Enter",
        "open",
    ),
    shown(ch('e'), ExplorerKey::ToggleSection, "e", "fold"),
    alias(ch('l'), ExplorerKey::OpenSection),
    alias(key(KeyCode::Right), ExplorerKey::OpenSection),
    alias(ch('h'), ExplorerKey::CloseSection),
    alias(key(KeyCode::Left), ExplorerKey::CloseSection),
    shown(ch('o'), ExplorerKey::Inspect, "o", "inspect"),
    shown(ch('F'), ExplorerKey::ToggleFull, "F", "show all"),
    shown(
        key(KeyCode::Backspace),
        ExplorerKey::Back,
        "Backspace",
        "back",
    ),
    alias(key(KeyCode::Esc), ExplorerKey::Back),
];

/// The keys a status line names for an explorer, from `table` (the
/// explorer's own, or a host table that includes it), with the table's
/// verbs: moving, `Enter`, folding, and going back
/// (`↑/↓ move · Enter open · e fold · Backspace back`). A hint has less
/// room than a footer, so it leaves out the keys a reader looks up
/// (`o`, `F`, the page keys). [`Explorer::footer`](super::Explorer::footer)
/// is the same with `Enter`'s verb on the cursor's row; this is what a
/// host shows with no explorer to read.
pub fn hint(table: &[KeyRow<ExplorerKey>]) -> Footer {
    Footer::only(table, |key| {
        matches!(
            key,
            ExplorerKey::Up
                | ExplorerKey::Down
                | ExplorerKey::Activate
                | ExplorerKey::ToggleSection
                | ExplorerKey::Back
        )
    })
}

impl KeyAction for ExplorerKey {
    fn role(self) -> KeyRole {
        match self {
            Self::Down | Self::Up | Self::First | Self::Last => KeyRole::Move,
            Self::PageDown | Self::PageUp => KeyRole::Page,
            Self::Activate => KeyRole::Apply,
            Self::ToggleSection | Self::OpenSection | Self::CloseSection | Self::ToggleFull => {
                KeyRole::Change
            }
            Self::Back => KeyRole::Leave,
            Self::Inspect => KeyRole::Other,
        }
    }
}

impl HelpAction for ExplorerKey {
    const LISTED: &'static [Self] = &[
        Self::Down,
        Self::Up,
        Self::PageDown,
        Self::PageUp,
        Self::First,
        Self::Last,
        Self::Activate,
        Self::Inspect,
        Self::ToggleFull,
        Self::ToggleSection,
        Self::OpenSection,
        Self::CloseSection,
        Self::Back,
    ];

    fn help(self) -> HelpEntry {
        let (section, description) = match self {
            Self::Down => ("Moving", "Move to the next row"),
            Self::Up => ("Moving", "Move to the previous row"),
            Self::PageDown => ("Moving", "Move down by the rows shown"),
            Self::PageUp => ("Moving", "Move up by the rows shown"),
            Self::First => ("Moving", "Move to the first row"),
            Self::Last => ("Moving", "Move to the last row"),
            Self::Activate => (
                "Rows",
                "Open the row: copy a property's whole value, follow a link, or fold a section",
            ),
            Self::Inspect => ("Rows", "Read a cut property whole"),
            Self::ToggleFull => ("Rows", "Show every property, or the summary again"),
            Self::ToggleSection => ("Sections", "Open or close the section on its header"),
            Self::OpenSection => ("Sections", "Open the section on its header"),
            Self::CloseSection => ("Sections", "Close the section the cursor heads or sits in"),
            Self::Back => (
                "Going back",
                "Go back to what was shown before following a link; with nothing to go back to, press twice to leave",
            ),
        };
        HelpEntry::new(section, description)
    }
}
