//! What the launch form starts from besides its name: the harness, its
//! arguments, the working directory, and whether the new mux is pinned,
//! each taken from the entity the operator is looking at, so a launch
//! from a row starts as a copy of that row.
//!
//! The entity is the same one the name seed follows
//! ([`super::mux_name`]): the right pane's while it has focus, else the
//! selected row's, so one entity seeds the same form in every view. A
//! field the entity says nothing about stays `None`, and the form keeps
//! the default it has with no row selected.

use std::path::Path;

use crate::discovery::harness::adapter_in_command;
use crate::model::{
    FieldSource, FieldValue, GraphNode, GraphSnapshot, NodeId, PinKind, PinSlot, RecipeField,
    SlotFields, SlotStatus,
};
use crate::tui::actions::{Subject, action_subject, pin_named, pins_for};
use crate::tui::app::App;
use crate::tui::widgets::mux_launch::PinChoice;

/// The launch form's first values beyond its name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LaunchSeed {
    /// The entity's harness, when the form can pick it.
    pub harness: Option<String>,
    /// How the entity was launched, which carries its launch options.
    /// Only with a harness: arguments mean nothing without the program
    /// they belong to.
    pub argv: Option<Vec<String>>,
    /// The entity's directory, when it still exists.
    pub cwd: Option<String>,
    /// Whether the new mux is pinned as the entity is.
    pub pin: PinChoice,
}

/// The seed for the launch form over `app`. `known_harnesses` are the
/// harnesses the form can pick.
pub(crate) fn seed(app: &App, known_harnesses: &[String]) -> LaunchSeed {
    let Some(handle) = app.snapshot_handle() else {
        return LaunchSeed::default();
    };
    let snapshot = handle.snapshot();
    let subject = action_subject(app);
    let recipe = Recipe::of(snapshot, &subject);
    let harness = recipe
        .harness
        .filter(|harness| known_harnesses.contains(harness));
    LaunchSeed {
        argv: harness.as_ref().and(recipe.argv),
        harness,
        cwd: first_existing(
            super::derive_mux_form_cwd(app)
                .into_iter()
                .chain(recipe.cwd),
        ),
        pin: pin_choice(snapshot, &subject),
    }
}

/// The first of `paths` that is a directory now. A stopped mux, a removed
/// worktree, or a pin on a deleted checkout names one that is not.
fn first_existing(paths: impl IntoIterator<Item = String>) -> Option<String> {
    paths.into_iter().find(|path| Path::new(path).is_dir())
}

/// What an entity says about how it was launched.
#[derive(Debug, Default)]
struct Recipe {
    harness: Option<String>,
    argv: Option<Vec<String>>,
    cwd: Option<String>,
}

impl Recipe {
    fn of(snapshot: &GraphSnapshot, subject: &Subject) -> Self {
        // A mux pin is a recipe already: what its target says, under
        // what the pin overrides. A session pin has no arguments or
        // directory of its own, so the session answers for itself.
        if let Some(pin) = pins_for(snapshot, subject)
            .iter()
            .filter_map(|name| pin_named(snapshot, name))
            .find(|pin| pin.kind == PinKind::Mux)
        {
            return Self::from_fields(&pin.effective);
        }
        match &subject.node {
            Some(node @ NodeId::MuxSession(_)) => {
                Self::from_fields(&Self::mux_fields(snapshot, node, subject.record.as_deref()))
            }
            Some(node @ NodeId::AgentSession(_)) => Self::session(snapshot, node),
            _ => Self::default(),
        }
    }

    /// A mux's recipe fields as a mux pin would resolve them: the live
    /// node while it runs, else its history record.
    fn mux_fields(
        snapshot: &GraphSnapshot,
        node: &NodeId,
        record: Option<&str>,
    ) -> Vec<RecipeField> {
        let target = match record {
            Some(record) => PinSlot::history_target(record),
            None => node.to_string(),
        };
        let slot = PinSlot {
            id: String::new(),
            kind: PinKind::Mux,
            target: Some(target),
            pinned_at: None,
            overrides: SlotFields::new(),
            snapshot: SlotFields::new(),
            order: 0,
            legacy: false,
        };
        crate::resolve::slots::resolve_slot(snapshot, &slot).effective
    }

    /// A session's harness and directory, and the arguments of the mux
    /// hosting it when that mux runs the same harness.
    fn session(snapshot: &GraphSnapshot, node: &NodeId) -> Self {
        let Some(GraphNode::AgentSession(session)) = snapshot.find_node(node) else {
            return Self::default();
        };
        let argv = snapshot
            .preferred_mux_for(node)
            .map(NodeId::MuxSession)
            .filter(|mux| snapshot.is_live(mux))
            .and_then(|mux| match snapshot.find_node(&mux) {
                Some(GraphNode::MuxSession(mux)) => mux.active_pane_start_command.clone(),
                _ => None,
            })
            .map(|start| crate::argv::split_command_line(&start))
            .filter(|argv| {
                adapter_in_command(argv.iter().map(String::as_str))
                    .is_some_and(|adapter| adapter.harness_key() == session.harness_key)
            });
        Self {
            harness: Some(session.harness_key.clone()),
            argv,
            cwd: session.cwd.clone(),
        }
    }

    /// The harness, argv, and cwd among `fields`. An argv that is only
    /// the harness's own default is no statement about the entity, and
    /// the form decides its own default, which can be a launch profile's.
    fn from_fields(fields: &[RecipeField]) -> Self {
        let field = |name: &str| fields.iter().find(|field| field.name == name);
        let text = |name: &str| {
            field(name)
                .and_then(|field| field.value.as_text())
                .map(str::to_string)
        };
        Self {
            harness: text("harness"),
            argv: field("argv")
                .filter(|field| field.source != FieldSource::Default)
                .and_then(|field| match &field.value {
                    FieldValue::List(argv) if !argv.is_empty() => Some(argv.clone()),
                    _ => None,
                }),
            cwd: text("cwd"),
        }
    }
}

/// Whether the new mux is pinned as the entity is: not at all when it
/// has no pin, with its launch when its pin's target runs, without when
/// that does not. Only a mux or a session is read this way: a pinned PR
/// or checkout says nothing about whether a new mux should be.
fn pin_choice(snapshot: &GraphSnapshot, subject: &Subject) -> PinChoice {
    let Some(pin) = pins_for(snapshot, subject)
        .iter()
        .filter_map(|name| pin_named(snapshot, name))
        .find(|pin| matches!(pin.kind, PinKind::Mux | PinKind::Session))
    else {
        return PinChoice::Off;
    };
    match pin.status {
        SlotStatus::Live => PinChoice::Pin,
        SlotStatus::Gone | SlotStatus::Unknown | SlotStatus::Missing | SlotStatus::Planned => {
            PinChoice::PlanOnly
        }
    }
}
