//! Undo for reversible operations.
//!
//! The executor records an [`Undo`] after an operation's write has
//! succeeded; the reducer keeps the entries on a bounded [`UndoStack`]
//! and, on the undo key, turns the newest one back into the write that
//! reverses it. Nothing here is persisted: undo reaches back through
//! this session only.
//!
//! Every store write must say whether it can be undone
//! ([`crate::tui::effect::undo_class`]), so a new reversible operation
//! has to declare its inverse here and a new irreversible one has to say
//! why it has none.

use std::collections::VecDeque;

use crate::model::{NodeId, PinSlot};
use crate::tui::effect::StoreOp;
use crate::tui::rows::RowId;

/// How many operations undo reaches back through.
pub const UNDO_DEPTH: usize = 20;

/// The inverse of one executed operation.
#[derive(Debug, Clone, PartialEq)]
pub enum Undo {
    /// A pin was created; undoing removes it.
    Unpin { id: String },
    /// A pin was removed; undoing puts it back as it was, in its old
    /// place in the pin order. `entity` is the node the pin pointed at
    /// then, so the cursor can return to it outside the Pinned view.
    RestorePin {
        slot: Box<PinSlot>,
        index: usize,
        entity: Option<NodeId>,
    },
}

impl Undo {
    /// The store write that reverses the operation.
    pub fn into_op(self) -> StoreOp {
        match self {
            Undo::Unpin { id } => StoreOp::RemovePin { id },
            Undo::RestorePin { slot, index, .. } => StoreOp::RestorePin { slot: *slot, index },
        }
    }

    /// Where to put the cursor once the reversal's refresh lands.
    pub fn focus(&self, in_pinned_view: bool) -> Option<RestoreFocus> {
        let Undo::RestorePin { slot, entity, .. } = self else {
            return None;
        };
        if in_pinned_view {
            return Some(RestoreFocus::Row(RowId::Pin {
                pin_id: slot.id.clone(),
            }));
        }
        entity
            .clone()
            .or_else(|| NodeId::parse(slot.target.as_deref()?))
            .map(RestoreFocus::Node)
    }
}

/// What to select after an undo's refresh, without leaving the view.
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreFocus {
    Row(RowId),
    Node(NodeId),
}

/// The operations that can still be undone, newest last.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UndoStack {
    entries: VecDeque<Undo>,
}

impl UndoStack {
    /// Record an operation, forgetting the oldest beyond [`UNDO_DEPTH`].
    pub fn push(&mut self, entry: Undo) {
        if self.entries.len() == UNDO_DEPTH {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Take the newest operation.
    pub fn pop(&mut self) -> Option<Undo> {
        self.entries.pop_back()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
