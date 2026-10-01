use super::*;
use crate::model::{
    GraphLink, GraphNode, LinkEndpoint, NodeId, NodeProvenance, Provenance, RelationKind, RepoId,
    RepoNode, SourceMetadata,
};

fn intervals_default() -> ServerIntervals {
    ServerIntervals::default()
}

fn prov_node(snap: &mut GraphSnapshot, provider: &str, epoch: i64, id_suffix: &str) {
    let repo = RepoNode::new(RepoId::new(format!("/{provider}/{id_suffix}/.git")));
    let node_id = NodeId::Repo(repo.id.clone());
    snap.nodes.push(GraphNode::Repo(repo));
    snap.node_provenance.insert(
        node_id,
        NodeProvenance {
            provider: provider.to_string(),
            freshness_epoch: Some(epoch),
        },
    );
}

fn prov_link(snap: &mut GraphSnapshot, provider: &str, epoch: i64, id: &str) {
    let source = NodeId::Repo(RepoId::new(format!("/{id}-src/.git")));
    let target = NodeId::Repo(RepoId::new(format!("/{id}-tgt/.git")));
    let mut link = GraphLink::new(
        id,
        source,
        LinkEndpoint::Node { id: target },
        RelationKind::BelongsToRepo,
        Provenance::StrongDiscovered,
    );
    link.source_metadata = SourceMetadata {
        adapter: provider.to_string(),
        freshness_epoch: Some(epoch),
        ..Default::default()
    };
    snap.candidate_links.push(link);
}

#[test]
fn every_known_provider_string_maps_to_a_class() {
    // Production set per the commit that wired adapter
    // instrumentation across the codebase. Adding a new heavy
    // provider should either update this list AND
    // `provider_class` (via the registry descriptor per
    // ADR 0088), or accept the always-evict fallback.
    let known = [
        "git",
        "git::cwd",
        "atelier",
        "generic_workspace",
        "agent_deck",
        "tmux",
        "zellij",
        "claude-code",
        "codex",
        "opencode",
        "aider",
        "github",
    ];
    for key in known {
        assert!(
            provider_class(key).is_some(),
            "ADR 0079: provider key `{key}` must map to a class"
        );
    }
}

/// ADR 0088: the `MUTATOR_PROVIDERS` constant and the
/// registry-derived
/// [`super::mutator_providers`] must agree. Guards against a
/// mutator entry that lands in one place without the other.
#[test]
fn mutator_const_matches_registry() {
    let derived = super::mutator_providers();
    let expected: Vec<&str> = MUTATOR_PROVIDERS.to_vec();
    assert_eq!(derived, expected);
}

#[test]
fn mutator_provider_strings_have_no_class() {
    for key in MUTATOR_PROVIDERS {
        assert!(
            provider_class(key).is_none(),
            "mutator `{key}` must not map to a class"
        );
    }
}

#[test]
fn fresh_when_age_is_under_ttl_stale_when_over() {
    // `git` class TTL defaults to 30s. Stamp two providers in
    // the git class with epochs straddling the boundary at
    // now=100.
    let mut snap = GraphSnapshot::empty();
    prov_node(&mut snap, "git", 80, "fresh"); // age 20s, ttl 30s → fresh
    prov_node(&mut snap, "atelier", 60, "stale"); // age 40s, ttl 30s → stale

    let gate = compute_freshness_gate(&snap, &intervals_default(), 100);

    assert!(gate.fresh.contains("git"));
    assert!(gate.stale.contains("atelier"));
    assert!(gate.always_evict.is_empty());
}

#[test]
fn link_max_epoch_wins_over_older_node_epoch() {
    // Same provider, two stamping sites with different
    // epochs. The freshness gate takes the *most recent*
    // observation as the slice age — partial re-emits should
    // not stale-flag a provider that already wrote half its
    // slice on this clock tick.
    let mut snap = GraphSnapshot::empty();
    prov_node(&mut snap, "github", 40, "old"); // would be stale alone
    prov_link(&mut snap, "github", 95, "recent-link"); // pulls max forward

    // forge ttl = 300s, now = 100 → age = 5s → fresh
    let gate = compute_freshness_gate(&snap, &intervals_default(), 100);
    assert!(gate.fresh.contains("github"));
    assert!(!gate.stale.contains("github"));
}

#[test]
fn mutator_providers_always_evict_regardless_of_age() {
    let mut snap = GraphSnapshot::empty();
    // `cross_link` slice that is well within any TTL — still
    // gets always-evicted because mutators always re-run.
    prov_link(&mut snap, "cross_link", 99, "xl");

    let gate = compute_freshness_gate(&snap, &intervals_default(), 100);
    assert!(gate.always_evict.contains("cross_link"));
    assert!(gate.fresh.is_empty());
    assert!(gate.stale.is_empty());
}

#[test]
fn unmapped_provider_lands_in_always_evict() {
    // An in-development provider that has not yet registered
    // its class falls back to always-evict instead of
    // silently being treated as fresh. The conservative
    // default means a new heavy provider doesn't get a
    // too-generous TTL just because someone forgot to update
    // `provider_class`.
    let mut snap = GraphSnapshot::empty();
    prov_node(&mut snap, "experimental_thing", 99, "x");

    let gate = compute_freshness_gate(&snap, &intervals_default(), 100);
    assert!(gate.always_evict.contains("experimental_thing"));
}

#[test]
fn keys_to_evict_unions_stale_and_always_evict() {
    let mut gate = FreshnessGate::default();
    gate.stale.insert("git".to_string());
    gate.always_evict.insert("cross_link".to_string());
    let keys = gate.keys_to_evict();
    assert!(keys.contains("git"));
    assert!(keys.contains("cross_link"));
    assert_eq!(keys.len(), 2);
}

#[test]
fn class_providers_is_the_inverse_of_provider_class() {
    // Every key in any class's providers() list must map back
    // to that class via provider_class. A divergence between
    // the two means the daemon's per-class scheduler would
    // evict-and-re-run a slice the freshness gate doesn't
    // consider part of the class — silently leaking work.
    for class in ProviderClass::all() {
        for provider in class.providers() {
            let recovered = provider_class(provider);
            assert_eq!(
                recovered,
                Some(*class),
                "ProviderClass::{class:?}.providers() lists `{provider}` \
                     but provider_class maps it to {recovered:?}"
            );
        }
    }
}

#[test]
fn empty_prior_produces_empty_gate() {
    let gate = compute_freshness_gate(&GraphSnapshot::empty(), &intervals_default(), 100);
    assert!(gate.fresh.is_empty());
    assert!(gate.stale.is_empty());
    assert!(gate.always_evict.is_empty());
}

#[test]
fn is_mux_or_harness_provider_covers_mux_and_harness_keys_only() {
    // The process-tree refresh decision keys off
    // whether a mux/harness provider ran. Mux + harness keys are in;
    // git/forge and the mutator keys themselves are out.
    for key in [
        providers::TMUX,
        providers::ZELLIJ,
        providers::CLAUDE_CODE,
        providers::CODEX,
        providers::OPENCODE,
        providers::AIDER,
    ] {
        assert!(
            is_mux_or_harness_provider(key),
            "{key} should be mux/harness"
        );
    }
    for key in [
        providers::GIT,
        providers::GITHUB,
        providers::ATELIER,
        providers::CROSS_LINK,
        providers::CODEX_LOG,
    ] {
        assert!(
            !is_mux_or_harness_provider(key),
            "{key} should not be mux/harness"
        );
    }
}

#[test]
fn process_tree_mutators_are_a_subset_of_all_mutators() {
    // The class-gated slice must be a strict subset of the always-run
    // mutator set (hook_sidecar / declared stay always-run).
    let all: std::collections::BTreeSet<&str> = MUTATOR_PROVIDERS.iter().copied().collect();
    for key in PROCESS_TREE_MUTATORS {
        assert!(all.contains(key), "{key} must be a known mutator");
    }
    assert!(
        !PROCESS_TREE_MUTATORS.contains(&providers::HOOK_SIDECAR),
        "hook_sidecar reads files, not /proc — stays always-run",
    );
    assert!(
        !PROCESS_TREE_MUTATORS.contains(&providers::DECLARED),
        "declared reads config, not /proc — stays always-run",
    );
}
