//! Undo: the key, the bounded stack, the classification every store
//! write must carry, and the round trip of `p` then undo in each view.

use super::styling::all_stopped_muxes;
use super::*;
use crate::tui::Effect;
use crate::tui::effect::{StoreOp, UndoClass, lifecycle, undo_class};
use crate::tui::keybindings::{UNDO_KEY, keymap};
use crate::tui::keymap::Action;
use crate::tui::undo::{RestoreFocus, UNDO_DEPTH, Undo, UndoStack};

fn pin(id: &str) -> PinSlot {
    PinSlot {
        id: id.into(),
        kind: PinKind::Mux,
        target: Some(PinSlot::history_target("h_x")),
        pinned_at: None,
        overrides: SlotFields::new(),
        snapshot: SlotFields::new(),
        order: 0,
        legacy: false,
    }
}

fn restore(id: &str, index: usize) -> Undo {
    Undo::RestorePin {
        slot: Box::new(pin(id)),
        index,
        entity: None,
    }
}

#[test]
fn the_stack_forgets_the_oldest_beyond_its_depth() {
    let mut stack = UndoStack::default();
    for n in 0..UNDO_DEPTH + 5 {
        stack.push(restore(&format!("p{n}"), n));
    }
    assert_eq!(stack.len(), UNDO_DEPTH);
    let newest = stack.pop().unwrap();
    assert_eq!(
        newest,
        restore(&format!("p{}", UNDO_DEPTH + 4), UNDO_DEPTH + 4)
    );
    let mut oldest = None;
    while let Some(entry) = stack.pop() {
        oldest = Some(entry);
    }
    assert_eq!(
        oldest,
        Some(restore("p5", 5)),
        "the first five were dropped"
    );
}

#[test]
fn undo_steps_back_through_operations_newest_first_then_says_so() {
    let mut app = app_in(View::Pinned);
    app.update(Msg::UndoRecorded(Undo::Unpin { id: "first".into() }));
    app.update(Msg::UndoRecorded(restore("second", 1)));
    assert_eq!(app.undo_stack().len(), 2);

    let effects = app.update(Msg::Undo);
    assert_eq!(
        effects,
        vec![Effect::WriteStore(StoreOp::RestorePin {
            slot: pin("second"),
            index: 1
        })]
    );
    let effects = app.update(Msg::Undo);
    assert_eq!(
        effects,
        vec![Effect::WriteStore(StoreOp::RemovePin {
            id: "first".into()
        })]
    );
    let effects = app.update(Msg::Undo);
    assert_eq!(
        effects,
        vec![Effect::Toast(Feedback::warning("nothing to undo"))]
    );
    assert!(app.undo_stack().is_empty());
}

#[test]
fn the_undo_key_is_bound_globally_to_undo() {
    use tuikit::keys::{ActiveScopes, KeyMatcher, Scope};
    let undo = KeyMatcher::AnyModExceptCtrl(UNDO_KEY);
    let found = keymap().resolve(ActiveScopes::global(), undo.example());
    assert!(
        found.is_some_and(|binding| binding.scope() == Scope::Global
            && matches!(binding.action().action(), Action::Msg(msg) if *msg == Msg::Undo)),
        "`{UNDO_KEY}` is bound to Msg::Undo"
    );
    let others = keymap()
        .bindings()
        .iter()
        .filter(|binding| {
            binding
                .keys()
                .iter()
                .any(|key| key.overlap(&undo).is_some())
        })
        .count();
    assert_eq!(others, 1, "no other binding shares the undo key");
}

/// Every store write says whether it has an inverse. Pin and unpin
/// must; the undo writes themselves must not (undoing an undo would
/// just flip the pin back).
#[test]
fn pin_and_unpin_are_undoable_and_the_rest_say_why_not() {
    let node = crate::model::NodeId::ForgePr(crate::model::ForgePrId::new("g", "h", "o", "r", 1));
    assert!(matches!(
        undo_class(&StoreOp::PinNode { node }),
        UndoClass::Undoable(_)
    ));
    assert!(matches!(
        undo_class(&StoreOp::PinRecord {
            record_id: "h_x".into()
        }),
        UndoClass::Undoable(_)
    ));
    assert!(matches!(
        undo_class(&StoreOp::Unpin { slot: "x".into() }),
        UndoClass::Undoable(_)
    ));
    for op in [
        StoreOp::RemovePin { id: "x".into() },
        StoreOp::RestorePin {
            slot: pin("x"),
            index: 0,
        },
        StoreOp::RefreshSlot { slot: "x".into() },
        StoreOp::ForgetRecord {
            record_id: "h_x".into(),
        },
    ] {
        match undo_class(&op) {
            UndoClass::NotUndoable(reason) => assert!(!reason.is_empty(), "{op:?}"),
            UndoClass::Undoable(_) => panic!("{op:?} should not record an undo"),
        }
    }
}

#[test]
fn the_undo_writes_refresh_like_every_other_pin_write() {
    for op in [
        StoreOp::RemovePin { id: "x".into() },
        StoreOp::RestorePin {
            slot: pin("x"),
            index: 0,
        },
    ] {
        let lifecycle = lifecycle(&Effect::WriteStore(op.clone()));
        assert!(lifecycle.refreshes, "{op:?} must refresh the views");
    }
}

/// `p` on a pinned row, in each view that shows pins, then undo: the
/// reversal restores that same pin, and once the refresh lands the
/// cursor returns to its row.
#[test]
fn unpinning_in_each_view_and_undoing_returns_to_the_restored_row() {
    for view in [
        View::Sessions,
        View::Mux,
        View::Prs,
        View::Pinned,
        View::Search,
    ] {
        let mut app = if view == View::Search {
            searched(view, all_stopped_muxes())
        } else {
            app_over_filtered(view, &world(), all_stopped_muxes())
        };
        let ids: Vec<RowId> = app.tree().rows.iter().map(|row| row.id.clone()).collect();
        let mut tried = 0;
        for id in ids {
            app.set_selection(id.clone());
            let effects = app.update(Msg::TogglePin);
            let [Effect::WriteStore(StoreOp::Unpin { slot })] = effects.as_slice() else {
                app.close_row_menu();
                continue;
            };
            tried += 1;
            // The executor records what it removed; stand in for it.
            let removed = PinSlot {
                id: slot.clone(),
                ..pin(slot)
            };
            let entity = crate::tui::rows::row_node(&id).cloned();
            app.update(Msg::UndoRecorded(Undo::RestorePin {
                slot: Box::new(removed.clone()),
                index: 2,
                entity: entity.clone(),
            }));
            // The operator wandered off before undoing.
            app.set_selection(app.tree().rows[0].id.clone());

            let effects = app.update(Msg::Undo);
            assert_eq!(
                effects,
                vec![Effect::WriteStore(StoreOp::RestorePin {
                    slot: removed,
                    index: 2
                })],
                "{view:?}: undo restores the pin that `p` removed"
            );
            // The refresh after the restore has landed.
            app.apply_restore_focus();
            let want = if view == View::Pinned {
                Some(RowId::Pin {
                    pin_id: slot.clone(),
                })
            } else {
                Some(id.clone())
            };
            if view == View::Pinned || entity.is_some() {
                assert_eq!(
                    app.selection().cloned(),
                    want,
                    "{view:?}: the cursor returns to the restored row"
                );
            }
            app.close_row_menu();
        }
        assert!(tried > 0, "{view:?}: no pinned row was unpinned");
    }
}

#[test]
fn history_rows_undo_to_their_own_view_without_leaving_it() {
    let mut app = app_listing_all(View::History);
    let mut pinned_row = None;
    for index in 0..app.tree().rows.len() {
        let id = app.tree().rows[index].id.clone();
        app.set_selection(id.clone());
        if matches!(
            app.update(Msg::TogglePin).as_slice(),
            [Effect::WriteStore(StoreOp::Unpin { .. })]
        ) {
            pinned_row = Some(id);
            break;
        }
    }
    app.close_row_menu();
    if let Some(row) = pinned_row {
        app.update(Msg::UndoRecorded(restore("anything", 0)));
        app.set_selection(row);
        app.update(Msg::Undo);
        app.apply_restore_focus();
        assert_eq!(
            app.active_view(),
            View::History,
            "undo never switches views"
        );
    }
}

#[test]
fn restore_focus_is_the_pin_row_in_the_pinned_view_and_the_entity_elsewhere() {
    let node = crate::model::NodeId::MuxSession(MuxSessionId::new("tmux:scratch"));
    let undo = Undo::RestorePin {
        slot: Box::new(pin("scratch-pin")),
        index: 0,
        entity: Some(node.clone()),
    };
    assert_eq!(
        undo.focus(true),
        Some(RestoreFocus::Row(RowId::Pin {
            pin_id: "scratch-pin".into()
        }))
    );
    assert_eq!(undo.focus(false), Some(RestoreFocus::Node(node)));
    assert_eq!(Undo::Unpin { id: "x".into() }.focus(true), None);
}
