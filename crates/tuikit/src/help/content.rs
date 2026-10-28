//! What a help page says: sections of key rows and legend rows, and the
//! generator that builds them from a key table.

use std::borrow::Cow;

use ratatui::crossterm::event::KeyModifiers;
use ratatui::text::Span;

use crate::keys::{KeyMatcher, KeyRow, key_label, rows};

/// Where an action is listed on a help page, and what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HelpEntry {
    /// The heading of the section the action is listed under. Actions that
    /// name the same heading share one section.
    pub section: &'static str,
    /// What the action does: one sentence, starting with a capital and
    /// with no full stop, as a label reads.
    pub description: &'static str,
}

impl HelpEntry {
    /// An entry under `section` that says `description`.
    pub const fn new(section: &'static str, description: &'static str) -> Self {
        Self {
            section,
            description,
        }
    }
}

/// An action enum whose key table can generate a help page.
///
/// The enum lists its actions in the order the page shows them and says,
/// for each, where it is listed and what it does. Write
/// [`help`](Self::help) as an exhaustive `match`, so a new action fails to
/// compile until it is described; [`undescribed`] catches an action the
/// list leaves out.
pub trait HelpAction: Copy + PartialEq + 'static {
    /// Every action, in the order the help page lists them. An action the
    /// table binds and this list leaves out gets no row on the page.
    const LISTED: &'static [Self];

    /// The section the action is listed under and what it does.
    fn help(self) -> HelpEntry;
}

/// One row of a help page.
#[derive(Clone, Debug, PartialEq)]
pub enum HelpRow {
    /// Keys and what they do: the keys in the page's key column, in the
    /// accent color, and the description wrapped beside them.
    Keys {
        /// The keys, as [`key_label`] writes them, several joined with
        /// ` / ` (`j / ↓`).
        keys: String,
        /// What the keys do.
        description: Cow<'static, str>,
        /// Whether the keys can act now. A row that cannot is drawn in
        /// [`unavailable_style`](crate::keys::unavailable_style).
        available: bool,
    },
    /// An entry of a legend: a mark the screen draws, its name, and what
    /// it means.
    Legend {
        /// The mark, in the style the screen draws it in.
        mark: Span<'static>,
        /// The mark's name, drawn bold in a column the section's legend
        /// rows share.
        name: Cow<'static, str>,
        /// What the mark means, wrapped beside the name.
        blurb: Cow<'static, str>,
    },
}

impl HelpRow {
    /// A row saying what `keys` do; it can act now.
    pub fn keys(keys: impl Into<String>, description: impl Into<Cow<'static, str>>) -> Self {
        Self::Keys {
            keys: keys.into(),
            description: description.into(),
            available: true,
        }
    }

    /// A legend row for `mark`, named `name`, meaning `blurb`.
    pub fn legend(
        mark: Span<'static>,
        name: impl Into<Cow<'static, str>>,
        blurb: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::Legend {
            mark,
            name: name.into(),
            blurb: blurb.into(),
        }
    }

    /// This row marked as able to act now or not. A legend row has no
    /// availability and is returned as it is.
    #[must_use]
    pub fn available(self, now: bool) -> Self {
        match self {
            Self::Keys {
                keys, description, ..
            } => Self::Keys {
                keys,
                description,
                available: now,
            },
            legend @ Self::Legend { .. } => legend,
        }
    }
}

/// A heading and the rows under it.
#[derive(Clone, Debug, PartialEq)]
pub struct HelpSection {
    /// The heading, drawn bold.
    pub title: Cow<'static, str>,
    /// The rows, top to bottom.
    pub rows: Vec<HelpRow>,
}

impl HelpSection {
    /// A section headed `title`, with no rows yet.
    pub fn new(title: impl Into<Cow<'static, str>>) -> Self {
        Self {
            title: title.into(),
            rows: Vec::new(),
        }
    }

    /// This section with `row` added at the end.
    #[must_use]
    pub fn row(mut self, row: HelpRow) -> Self {
        self.rows.push(row);
        self
    }

    /// This section with a row saying what `keys` do ([`HelpRow::keys`]).
    #[must_use]
    pub fn keys(self, keys: impl Into<String>, description: impl Into<Cow<'static, str>>) -> Self {
        self.row(HelpRow::keys(keys, description))
    }

    /// This section with a legend row ([`HelpRow::legend`]).
    #[must_use]
    pub fn legend(
        self,
        mark: Span<'static>,
        name: impl Into<Cow<'static, str>>,
        blurb: impl Into<Cow<'static, str>>,
    ) -> Self {
        self.row(HelpRow::legend(mark, name, blurb))
    }
}

/// The help page of `table`: a section per heading the actions name, in
/// the order [`HelpAction::LISTED`] first reaches each, and in it one
/// [`HelpRow::Keys`] row per listed action that `table` binds.
///
/// A row joins every key the table binds to its action (included tables
/// spliced in), written by [`key_label`], plain keys before Control and
/// Alt chords and otherwise in table order, with ` / ` (`q / Ctrl-C`).
/// Aliases are listed like shown keys: a help page lists every key, where
/// a footer shows one of each. An action no key binds is left out, as
/// there is nothing to press. A row is available when any of its keys
/// is, so a table copied through
/// [`with_availability`](crate::keys::with_availability) dims what
/// cannot act now.
pub fn sections_for<A: HelpAction>(table: &[KeyRow<A>]) -> Vec<HelpSection> {
    let mut sections: Vec<HelpSection> = Vec::new();
    let mut seen: Vec<A> = Vec::new();
    for &action in A::LISTED {
        if seen.contains(&action) {
            continue;
        }
        seen.push(action);
        let Some((keys, available)) = keys_of(table, action) else {
            continue;
        };
        let entry = action.help();
        let row = HelpRow::Keys {
            keys,
            description: Cow::Borrowed(entry.description),
            available,
        };
        match sections
            .iter_mut()
            .find(|section| section.title == entry.section)
        {
            Some(section) => section.rows.push(row),
            None => sections.push(HelpSection {
                title: Cow::Borrowed(entry.section),
                rows: vec![row],
            }),
        }
    }
    sections
}

/// The keys `table` binds to `action`, joined for a row, and whether any
/// of them is available; `None` when it binds none.
fn keys_of<A: HelpAction>(table: &[KeyRow<A>], action: A) -> Option<(String, bool)> {
    let mut plain: Vec<String> = Vec::new();
    let mut chords: Vec<String> = Vec::new();
    let mut available = false;
    for row in rows(table) {
        let KeyRow::Bind {
            key,
            action: bound,
            available: now,
            ..
        } = row
        else {
            continue;
        };
        if *bound != action {
            continue;
        }
        available |= *now;
        let label = key_label(key);
        if plain.contains(&label) || chords.contains(&label) {
            continue;
        }
        if is_chord(key) {
            chords.push(label);
        } else {
            plain.push(label);
        }
    }
    if plain.is_empty() && chords.is_empty() {
        return None;
    }
    plain.extend(chords);
    Some((plain.join(" / "), available))
}

/// Whether `key` is a Control or Alt chord, which a row lists after the
/// plain keys.
fn is_chord(key: &KeyMatcher) -> bool {
    matches!(
        key,
        KeyMatcher::Exact { modifiers, .. }
            if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    )
}

/// The keys `table` binds that its help page ([`sections_for`]) would not
/// describe, by bound key: keys of an action [`HelpAction::LISTED`] leaves
/// out, and keys of an action whose entry has an empty section or
/// description. Empty when every key the table binds has a row that says
/// what it does; a host runs it over every table it generates a page for.
pub fn undescribed<A: HelpAction>(table: &[KeyRow<A>]) -> Vec<String> {
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind { key, action, .. } if !described(*action) => Some(key_label(key)),
            _ => None,
        })
        .collect()
}

/// Whether `action` is listed and its entry says something.
fn described<A: HelpAction>(action: A) -> bool {
    let entry = action.help();
    A::LISTED.contains(&action)
        && !entry.section.trim().is_empty()
        && !entry.description.trim().is_empty()
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
