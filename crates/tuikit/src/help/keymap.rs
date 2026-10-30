//! A program's help page generated from its scoped keymap, with what the
//! keymap cannot say added as annotations.

use std::borrow::Cow;

use super::content::{HelpRow, HelpSection};
use crate::keys::{Binding, Keymap, key_label};

/// What a help page says that a keymap's bindings cannot, anchored to a
/// binding by its stable id or added as a section of its own.
///
/// A binding's row says what its keys do in one sentence. An annotation
/// carries the rest: more prose for a row (the keys of the overlay the
/// key opens, say), a row of its own after a binding's row (a key
/// sequence the keymap does not bind as one key), or a section of its own
/// (a legend of the marks a screen draws).
#[derive(Clone, Debug, PartialEq)]
pub enum HelpAnnotation {
    /// Prose added after the description of the row of the binding `id`,
    /// as it is written (it carries its own leading punctuation).
    More {
        /// The binding whose row it extends.
        id: &'static str,
        /// The prose.
        text: Cow<'static, str>,
    },
    /// A row of its own, after the row of the binding `id` (and after any
    /// earlier annotation's row there).
    RowAfter {
        /// The binding the row follows.
        id: &'static str,
        /// The row.
        row: HelpRow,
    },
    /// A section of its own, after every generated section.
    Section(HelpSection),
}

impl HelpAnnotation {
    /// `text` added after the description of the row of the binding `id`.
    pub fn more(id: &'static str, text: impl Into<Cow<'static, str>>) -> Self {
        Self::More {
            id,
            text: text.into(),
        }
    }

    /// `row` after the row of the binding `id`.
    pub fn row_after(id: &'static str, row: HelpRow) -> Self {
        Self::RowAfter { id, row }
    }

    /// `section` after the generated sections.
    pub fn section(section: HelpSection) -> Self {
        Self::Section(section)
    }

    /// The binding this annotation is anchored to; `None` for a section.
    pub fn anchor(&self) -> Option<&'static str> {
        match self {
            Self::More { id, .. } | Self::RowAfter { id, .. } => Some(id),
            Self::Section(_) => None,
        }
    }
}

/// The help page of `keymap`: one [`HelpRow::Keys`] row per binding, in
/// the keymap's order, under the section it names (sections in the order
/// the bindings first reach each), its keys in the order the binding
/// lists them, written by [`key_label`] and joined with ` / `, and its
/// help text as the description; then the `annotations`, each where it
/// says. Every row can act; [`keymap_sections_in`] dims what cannot now.
///
/// ```
/// use ratatui::crossterm::event::KeyCode;
/// use tuikit::help::{HelpAnnotation, HelpRow, HelpSection, keymap_sections, unanchored};
/// use tuikit::keys::{Bind, Keymap, Scope, bind, ch, key};
///
/// const KEYS: &[Bind<u8>] = &[
///     bind("list.down", Scope::Pane("list"), &[key(KeyCode::Down), ch('j')], 0)
///         .help("Moving", "Move to the next row"),
///     bind("find", Scope::Global, &[ch('/')], 1).help("Finding", "Find a row by name"),
/// ];
/// let keymap = Keymap::builder().rows(KEYS).build().unwrap();
/// let notes = [
///     HelpAnnotation::more("find", "; Enter picks it"),
///     HelpAnnotation::row_after("find", HelpRow::keys("/ then Tab", "Find anything")),
///     HelpAnnotation::section(HelpSection::new("Marks")),
/// ];
/// assert!(unanchored(&keymap, &notes).is_empty());
/// let sections = keymap_sections(&keymap, &notes);
/// assert_eq!(sections.len(), 3);
/// assert!(matches!(
///     &sections[0].rows[0],
///     HelpRow::Keys { keys, .. } if keys == "\u{2193} / j"
/// ));
/// assert!(matches!(
///     &sections[1].rows[0],
///     HelpRow::Keys { description, .. } if description == "Find a row by name; Enter picks it"
/// ));
/// assert_eq!(sections[1].rows.len(), 2);
/// ```
pub fn keymap_sections<A: Copy + PartialEq, Cx>(
    keymap: &Keymap<A, Cx>,
    annotations: &[HelpAnnotation],
) -> Vec<HelpSection> {
    generate(keymap, annotations, |_| true)
}

/// [`keymap_sections`] with each row dimmed whose binding cannot act now
/// in the host's context `cx`.
pub fn keymap_sections_in<A: Copy + PartialEq, Cx>(
    keymap: &Keymap<A, Cx>,
    annotations: &[HelpAnnotation],
    cx: &Cx,
) -> Vec<HelpSection> {
    generate(keymap, annotations, |binding| binding.can_act(cx))
}

/// The ids `annotations` are anchored to that no binding of `keymap` has:
/// an annotation that would show nowhere. Empty when every one has its
/// binding; a host asserts it, so removing or renaming a binding cannot
/// leave its prose behind unseen.
pub fn unanchored<A: Copy + PartialEq, Cx>(
    keymap: &Keymap<A, Cx>,
    annotations: &[HelpAnnotation],
) -> Vec<&'static str> {
    annotations
        .iter()
        .filter_map(HelpAnnotation::anchor)
        .filter(|id| keymap.get(id).is_none())
        .collect()
}

/// The keys of `binding` as its help row writes them.
fn row_keys<A: Copy, Cx>(binding: &Binding<A, Cx>) -> String {
    let mut labels: Vec<String> = Vec::new();
    for key in binding.keys() {
        let label = key_label(key);
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels.join(" / ")
}

fn generate<A: Copy + PartialEq, Cx>(
    keymap: &Keymap<A, Cx>,
    annotations: &[HelpAnnotation],
    can_act: impl Fn(&Binding<A, Cx>) -> bool,
) -> Vec<HelpSection> {
    let mut sections: Vec<HelpSection> = Vec::new();
    for binding in keymap.bindings() {
        let mut description = Cow::Borrowed(binding.help());
        for annotation in annotations {
            if let HelpAnnotation::More { id, text } = annotation
                && *id == binding.id()
            {
                description.to_mut().push_str(text);
            }
        }
        let row = HelpRow::Keys {
            keys: row_keys(binding),
            description,
            available: can_act(binding),
        };
        let after = annotations
            .iter()
            .filter_map(|annotation| match annotation {
                HelpAnnotation::RowAfter { id, row } if *id == binding.id() => Some(row.clone()),
                _ => None,
            });
        let index = sections
            .iter()
            .position(|section| section.title == binding.section())
            .unwrap_or_else(|| {
                sections.push(HelpSection::new(binding.section()));
                sections.len() - 1
            });
        sections[index].rows.push(row);
        sections[index].rows.extend(after);
    }
    sections.extend(
        annotations
            .iter()
            .filter_map(|annotation| match annotation {
                HelpAnnotation::Section(section) => Some(section.clone()),
                _ => None,
            }),
    );
    sections
}

#[cfg(test)]
#[path = "keymap_tests.rs"]
mod tests;
