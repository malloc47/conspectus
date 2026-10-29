//! Whatever an action did, the refresh after it must see the result:
//! it re-probes the muxes and harnesses instead of reusing a listing
//! cached a few seconds ago, and a running daemon is told to do the
//! same before its snapshot is read. The policy is data
//! (`Lifecycle::refresh_request`), so it can be checked for every effect.

use super::*;
use crate::tui::Effect;
use crate::tui::actions::AttachTarget;
use crate::tui::effect::{ExecSpec, RefreshRequest, lifecycle};
use crate::tui::runtime::{DaemonNudge, prepare_rescan};

fn every_sampled_effect() -> Vec<Effect> {
    let mut effects: Vec<Effect> = lifecycle::every_store_op()
        .into_iter()
        .map(Effect::WriteStore)
        .collect();
    let mux = MuxSessionId::new("tmux:scratch");
    effects.extend([
        Effect::Exec(ExecSpec::AttachMux(AttachTarget {
            mux,
            backend: "tmux".into(),
            native_id: "scratch".into(),
        })),
        Effect::Exec(ExecSpec::MuxNew {
            name: "fresh".into(),
            cwd: "/tmp".into(),
        }),
        Effect::Exec(ExecSpec::Rebuild {
            reference: "h_main".into(),
            attach: true,
        }),
        Effect::Exec(ExecSpec::Rebuild {
            reference: "h_main".into(),
            attach: false,
        }),
        Effect::Exec(ExecSpec::ShowRebuildPlan {
            reference: "h_main".into(),
        }),
        Effect::Exec(ExecSpec::ViewSession(
            crate::model::AgentSessionId::new("claude-code", "global", "s1"),
            None,
        )),
        Effect::Toast(Feedback::info("x")),
        Effect::Persist,
    ]);
    effects
}

/// Every effect that refreshes asks for a forced rescan of the mux and
/// harness classes, and one that doesn't refresh asks for nothing.
#[test]
fn every_refreshing_effect_forces_a_live_rescan() {
    let mut refreshing = 0;
    for effect in every_sampled_effect() {
        let policy = lifecycle(&effect);
        match policy.refresh_request() {
            Some(request) => {
                refreshing += 1;
                assert!(policy.refreshes, "{effect:?}");
                assert!(request.rescans_mux(), "{effect:?} must re-probe muxes");
                assert!(
                    request.classes.contains(&"harness"),
                    "{effect:?} must re-probe harnesses: agent presence depends on them"
                );
            }
            None => assert!(!policy.refreshes, "{effect:?} refreshes without a request"),
        }
    }
    assert!(refreshing >= 12, "{refreshing}");
}

/// The refresh keeps forges out unless the operator asked for them:
/// they are rate limited, and their data is never dropped by a live
/// rescan.
#[test]
fn an_action_refresh_never_asks_the_forges_again() {
    for effect in every_sampled_effect() {
        if let Some(request) = lifecycle(&effect).refresh_request() {
            assert!(!request.classes.contains(&"forge"), "{effect:?}");
        }
    }
}

/// The runtime turns each request into a daemon nudge for exactly the
/// requested classes, and every such nudge drops the cached mux listing
/// before anything is probed, daemon or not.
#[test]
fn the_runtime_carries_out_the_request_it_is_given() {
    for request in [RefreshRequest::LIVE, RefreshRequest::LIVE_AND_FORGE] {
        let nudge = DaemonNudge::for_request(&request);
        assert_eq!(nudge.classes(), request.classes);
        assert!(nudge.rescans_mux());

        let config = crate::tui::RunConfig::defaults();
        let sentinel = crate::discovery::GraphFragment {
            nodes: vec![GraphNode::MuxSession(MuxSessionNode::new(
                MuxSessionId::new("tmux:stale"),
                "tmux",
                "stale",
            ))],
            ..Default::default()
        };
        config.discovery_caches.tmux.set((), sentinel);
        assert!(config.discovery_caches.tmux.get(&()).is_some());

        prepare_rescan(&config, nudge);

        assert!(
            config.discovery_caches.tmux.get(&()).is_none(),
            "{request:?}: a listing cached before the action must not survive the refresh"
        );
    }
}

/// A timer refresh may reuse the cache; only action refreshes force.
#[test]
fn a_timer_refresh_may_reuse_the_cache() {
    assert!(!DaemonNudge::None.rescans_mux());
    assert!(DaemonNudge::None.classes().is_empty());
}
