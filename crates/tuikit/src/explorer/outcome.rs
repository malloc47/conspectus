//! What a key or a move of the explorer asks its host to do.

use std::fmt;

use super::ExplorerModel;

/// What a key asks the host to do. The explorer has already done its own
/// part (moved, folded, drilled, gone back); the host does the rest and
/// writes every word the reader sees.
pub enum Outcome<M: ExplorerModel> {
    /// Nothing for the host: the cursor moved, the view changed with
    /// nothing to say, or the key did nothing that needs a word (`Enter`
    /// on a property with no value).
    None,
    /// Put `text`, a property's whole value, on the clipboard; `label`
    /// names it.
    Copy {
        /// The property's label.
        label: String,
        /// The whole value.
        text: String,
    },
    /// Show `text`, a value the row draws cut, whole in a reader titled
    /// `label`.
    Inspect {
        /// The property's label.
        label: String,
        /// The whole value.
        text: String,
    },
    /// A section opened or closed.
    Toggle {
        /// Whether it is open now.
        open: bool,
    },
    /// The explorer drilled into `to`: it shows `to` now, and the thing
    /// it showed is the newest frame.
    Drill {
        /// What is shown now.
        to: M::Id,
    },
    /// A link leads to `to`, which the source could not load. Nothing
    /// changed.
    Follow {
        /// Where the link leads.
        to: M::Id,
    },
    /// The explorer went back to `to`; `saved` is what the host kept when
    /// it drilled away from it.
    Back {
        /// What is shown now.
        to: M::Id,
        /// What the host kept with the frame.
        saved: M::Saved,
    },
    /// `Back` with no frame to go back to: the next `Back` leaves.
    Armed,
    /// The explorer gives up the keys: `Back` while armed. Leaving is
    /// silent (house rule 5).
    Leave,
    /// The key did not do what it is for; the host says why (house rule
    /// 7).
    Refused(Refusal),
}

/// Why a key did nothing, for the host to word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// A fold key on a row that is no section header and sits in no
    /// section.
    NotInSection,
    /// The every-property switch changed no row: the thing has no more
    /// properties than its summary. The switch stays flipped.
    NothingMore,
    /// `Enter` on a link with no far end.
    NoFarEnd,
    /// The inspect key on a row with nothing cut.
    NothingToInspect,
    /// `Back` found no frame whose thing still loads. The frames that did
    /// not load are dropped; what is shown stays.
    Gone,
}

/// What [`Explorer::refresh`](super::Explorer::refresh) found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refreshed {
    /// The content was rebuilt; the drill, the view, and the cursor
    /// stayed.
    Kept,
    /// The thing shown is gone. The host opens the explorer again on what
    /// its own selection names.
    Gone,
}

impl<M: ExplorerModel> Clone for Outcome<M> {
    fn clone(&self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Copy { label, text } => Self::Copy {
                label: label.clone(),
                text: text.clone(),
            },
            Self::Inspect { label, text } => Self::Inspect {
                label: label.clone(),
                text: text.clone(),
            },
            Self::Toggle { open } => Self::Toggle { open: *open },
            Self::Drill { to } => Self::Drill { to: to.clone() },
            Self::Follow { to } => Self::Follow { to: to.clone() },
            Self::Back { to, saved } => Self::Back {
                to: to.clone(),
                saved: saved.clone(),
            },
            Self::Armed => Self::Armed,
            Self::Leave => Self::Leave,
            Self::Refused(why) => Self::Refused(*why),
        }
    }
}

impl<M: ExplorerModel> fmt::Debug for Outcome<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Copy { label, text } => f
                .debug_struct("Copy")
                .field("label", label)
                .field("text", text)
                .finish(),
            Self::Inspect { label, text } => f
                .debug_struct("Inspect")
                .field("label", label)
                .field("text", text)
                .finish(),
            Self::Toggle { open } => f.debug_struct("Toggle").field("open", open).finish(),
            Self::Drill { to } => f.debug_struct("Drill").field("to", to).finish(),
            Self::Follow { to } => f.debug_struct("Follow").field("to", to).finish(),
            Self::Back { to, saved } => f
                .debug_struct("Back")
                .field("to", to)
                .field("saved", saved)
                .finish(),
            Self::Armed => f.write_str("Armed"),
            Self::Leave => f.write_str("Leave"),
            Self::Refused(why) => f.debug_tuple("Refused").field(why).finish(),
        }
    }
}

impl<M: ExplorerModel> PartialEq for Outcome<M> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None) | (Self::Armed, Self::Armed) | (Self::Leave, Self::Leave) => {
                true
            }
            (
                Self::Copy { label, text },
                Self::Copy {
                    label: other_label,
                    text: other_text,
                },
            )
            | (
                Self::Inspect { label, text },
                Self::Inspect {
                    label: other_label,
                    text: other_text,
                },
            ) => label == other_label && text == other_text,
            (Self::Toggle { open }, Self::Toggle { open: other }) => open == other,
            (Self::Drill { to }, Self::Drill { to: other })
            | (Self::Follow { to }, Self::Follow { to: other }) => to == other,
            (
                Self::Back { to, saved },
                Self::Back {
                    to: other_to,
                    saved: other_saved,
                },
            ) => to == other_to && saved == other_saved,
            (Self::Refused(why), Self::Refused(other)) => why == other,
            _ => false,
        }
    }
}
