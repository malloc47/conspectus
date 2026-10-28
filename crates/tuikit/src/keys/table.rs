//! Key rows, resolving a key against a table, and the checks a table is
//! held to.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::matcher::{KeyMatcher, key_label};

/// How a bound key appears in a footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    /// Shown as `label` under `verb`. Rows that share a verb share one
    /// footer segment, their labels joined with `/` (`↑/↓ move`). The
    /// label is the key as [`key_label`] writes it; [`mislabeled`] finds
    /// a row whose label names another key.
    Shown {
        /// The key, as [`key_label`] writes it.
        label: &'static str,
        /// What the key does: a short imperative verb, with no "to".
        verb: &'static str,
    },
    /// Bound but left out of the footer on purpose: an alternate key for
    /// an action the footer already shows (`j` beside `↓`, `Ctrl-N`), and
    /// the universal `Ctrl-C`.
    Alias,
}

/// One row of a key table.
///
/// A table is a `const` slice of rows, written with the builders
/// ([`shown`], [`alias`], [`include()`]). Order matters twice: the first
/// row that matches a key wins ([`resolve`]), and footer segments appear
/// in the order their first row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRow<A: 'static> {
    /// A key bound to an action.
    Bind {
        /// The key events the row fires on.
        key: KeyMatcher,
        /// What the row means; the control matches on it.
        action: A,
        /// How the row appears in a footer.
        hint: Hint,
        /// Whether the action can run now. A footer and a help page draw
        /// an unavailable action dimmed ([`with_availability`] sets it).
        /// The key still resolves, so the control can say why nothing
        /// happened instead of letting the key fall through.
        available: bool,
    },
    /// Footer text with no key of its own, such as "type to filter".
    Note(&'static str),
    /// Every row of another table of the same actions, in place: how a
    /// table builds on a standard one without copying its rows.
    Include(&'static [KeyRow<A>]),
}

/// A row bound to `action` and shown in the footer as `label verb`.
pub const fn shown<A>(
    key: KeyMatcher,
    action: A,
    label: &'static str,
    verb: &'static str,
) -> KeyRow<A> {
    KeyRow::Bind {
        key,
        action,
        hint: Hint::Shown { label, verb },
        available: true,
    }
}

/// A row bound to `action` and left out of the footer.
pub const fn alias<A>(key: KeyMatcher, action: A) -> KeyRow<A> {
    KeyRow::Bind {
        key,
        action,
        hint: Hint::Alias,
        available: true,
    }
}

/// Every row of `rows`, in place.
pub const fn include<A>(rows: &'static [KeyRow<A>]) -> KeyRow<A> {
    KeyRow::Include(rows)
}

/// The rows of `table` with every [`KeyRow::Include`] replaced by the
/// rows it names, in order: the bound keys and notes a footer, a help
/// page, or a check reads.
pub fn rows<A>(table: &[KeyRow<A>]) -> Rows<'_, A> {
    Rows {
        stack: vec![table.iter()],
    }
}

/// The iterator [`rows`] returns.
#[derive(Debug)]
pub struct Rows<'a, A: 'static> {
    stack: Vec<std::slice::Iter<'a, KeyRow<A>>>,
}

impl<'a, A> Iterator for Rows<'a, A> {
    type Item = &'a KeyRow<A>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(row) = self.stack.last_mut()?.next() else {
                self.stack.pop();
                continue;
            };
            match row {
                KeyRow::Include(inner) => self.stack.push(inner.iter()),
                other => return Some(other),
            }
        }
    }
}

/// The action of the first row in `table` that `event` fires, if any.
///
/// A key no row binds is the control's to use as it likes: a text field
/// types it, anything else ignores it. Availability does not change the
/// answer.
pub fn resolve<A: Copy>(table: &[KeyRow<A>], event: KeyEvent) -> Option<A> {
    for row in table {
        match row {
            KeyRow::Bind { key, action, .. } if key.matches_event(event) => return Some(*action),
            KeyRow::Include(inner) => {
                if let Some(action) = resolve(inner, event) {
                    return Some(action);
                }
            }
            _ => {}
        }
    }
    None
}

/// A copy of `table`'s rows (included tables spliced in) with each bound
/// row marked available when it was and `available` says its action can
/// run now. Draw the copy's footer or help page to dim what cannot.
pub fn with_availability<A: Copy>(
    table: &[KeyRow<A>],
    available: impl Fn(A) -> bool,
) -> Vec<KeyRow<A>> {
    rows(table)
        .map(|row| match *row {
            KeyRow::Bind {
                key,
                action,
                hint,
                available: was,
            } => KeyRow::Bind {
                key,
                action,
                hint,
                available: was && available(action),
            },
            other => other,
        })
        .collect()
}

/// Rows whose footer label is not the key they bind as [`key_label`]
/// writes it, as `(label, bound key)` pairs. Empty when the table is
/// consistent.
pub fn mislabeled<A>(table: &[KeyRow<A>]) -> Vec<(&'static str, String)> {
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind {
                key,
                hint: Hint::Shown { label, .. },
                ..
            } if *label != key_label(key) => Some((*label, key_label(key))),
            _ => None,
        })
        .collect()
}

/// Shown rows whose key does not resolve to their own action in
/// `table`, because an earlier row takes it: the keys a footer would
/// advertise for something they do not do, as `(label, bound key)`
/// pairs. Empty when every key a footer names does what it says.
pub fn unresolved<A: Copy + PartialEq>(table: &[KeyRow<A>]) -> Vec<(&'static str, String)> {
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind {
                key,
                action,
                hint: Hint::Shown { label, .. },
                ..
            } if resolve(table, key.example()) != Some(*action) => Some((*label, key_label(key))),
            _ => None,
        })
        .collect()
}

/// Rows that bind a character but ignore its Control or Alt chords,
/// against the overlay rule for characters (see [`KeyMatcher`]), by
/// bound key. Empty when the table follows the rule.
pub fn loose_characters<A>(table: &[KeyRow<A>]) -> Vec<String> {
    rows(table)
        .filter_map(|row| match row {
            KeyRow::Bind { key, .. } if key.ignores_chords_of_a_character() => Some(key_label(key)),
            _ => None,
        })
        .collect()
}

/// The character a key types into a text field, if it types one: a
/// plain or shifted character, with no Control or Alt.
pub fn typed_char(event: KeyEvent) -> Option<char> {
    match event.code {
        KeyCode::Char(c)
            if !event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Some(c)
        }
        _ => None,
    }
}
