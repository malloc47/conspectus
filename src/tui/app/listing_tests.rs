use super::*;
use crate::model::GraphNode;

fn app_over(view: View, snapshot: &GraphSnapshot) -> App {
    let mut config = RunConfig::defaults();
    config.default_view = view;
    let mut app = App::new(config);
    app.update(Msg::SwitchView(view));
    let tree = crate::tui::rows::build_tree_for_view(crate::tui::rows::TreeInputs {
        snapshot,
        view,
        grouping: crate::tui::Grouping::default_for(view),
        filter: crate::filter::RowFilter::default(),
        sort: crate::tui::Sort::Hierarchy,
        mux_recency: crate::tui::MuxRecency::default(),
        cwd: None,
    });
    app.update(Msg::SetData {
        snapshot: SnapshotHandle::from_snapshot(snapshot),
        tree,
        loaded_at_epoch: 1_700_000_000,
        initial_selection_hint: None,
    });
    app
}

/// Every entity of the world, and every pin's target.
fn every_node(snapshot: &GraphSnapshot) -> Vec<NodeId> {
    let mut nodes: Vec<NodeId> = snapshot.nodes.iter().map(GraphNode::id).collect();
    for pin in crate::tui::rows::pin_nodes(snapshot) {
        nodes.extend(pin.target_node.clone());
    }
    nodes.sort();
    nodes.dedup();
    nodes
}

#[test]
fn the_view_a_reveal_would_land_in_is_where_revealing_finds_the_row() {
    let world = crate::tui::rows::pinned::all_kinds_world();
    for from in [View::Pinned, View::Sessions, View::History] {
        let app = app_over(from, &world);
        let listing = app.listing();
        for node in every_node(&world) {
            let predicted = app.reveal_view(&node, None);
            let record = app.reveal_record(&node, None);
            let mut probe = app_over(from, &world);
            let landed = probe
                .reveal_node(&node, None)
                .ok()
                .map(|()| probe.active_view());
            assert_eq!(predicted, landed, "{node} from {from:?}");
            if let Some(view) = predicted {
                assert!(listing.lists(view, &node, record.as_deref()), "{node}");
                let selected = probe.selection().expect("the reveal selects a row");
                let row = probe
                    .tree()
                    .rows
                    .iter()
                    .find(|row| &row.id == selected)
                    .expect("the selected row is in the tree");
                let by_record = matches!(&row.id, RowId::History { record_id }
                    if Some(record_id) == record.as_ref());
                assert!(
                    super::super::tree::row_matches(row, &node) || by_record,
                    "{node}: the selected row stands for it"
                );
            }
        }
    }
}

#[test]
fn no_view_lists_a_fork() {
    let world = crate::tui::rows::pinned::all_kinds_world();
    let app = app_over(View::Pinned, &world);
    let fork = NodeId::Fork(crate::model::ForkId::new("atelier/fork-1"));
    assert!(
        world.find_node(&fork).is_some(),
        "premise: the world has it"
    );
    assert_eq!(app.reveal_view(&fork, None), None);
    let listing = app.listing();
    for view in REVEAL_VIEWS {
        assert!(!listing.lists(view, &fork, None), "{view:?}");
    }
    let mut probe = app_over(View::Pinned, &world);
    assert_eq!(
        probe.reveal_node(&fork, None),
        Err("isn't listed in any view".to_string())
    );
    assert_eq!(probe.active_view(), View::Pinned, "the reveal stays put");
}

#[test]
fn a_pin_is_revealed_in_the_pinned_view() {
    // The Pinned view's rows are pin rows, which a reveal finds by the
    // pin they stand for.
    let world = crate::tui::rows::pinned::all_kinds_world();
    let pin = NodeId::Pin(crate::model::PinId::new("mux-live"));
    let mut app = app_over(View::Sessions, &world);
    assert_eq!(app.reveal_view(&pin, None), Some(View::Pinned));
    assert_eq!(app.reveal_node(&pin, None), Ok(()));
    assert_eq!(app.active_view(), View::Pinned);
    assert_eq!(
        app.selection(),
        Some(&RowId::Pin {
            pin_id: "mux-live".into()
        })
    );
}
