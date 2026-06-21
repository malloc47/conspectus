//! Warm-start cache gate (P7-003 phase 3).
//!
//! Given a persisted [`GraphSnapshot`], the per-class refresh
//! intervals from [`ServerIntervals`], and a wall-clock "now"
//! epoch, [`compute_freshness_gate`] decides which discovery
//! providers can be skipped (their cached slice is younger than
//! the class TTL) and which must be evicted and re-run.
//!
//! Mutator providers (`cross_link`, `codex_log`, `hook_sidecar`,
//! `declared`) are always evicted — see ADR 0079's "Always-rerun
//! providers" rule. They re-derive their outputs from whatever the
//! heavy providers produced this invocation, so caching their
//! prior output would let stale links survive upstream deletions.
//!
//! Unmapped provider keys (anything not in [`provider_class`])
//! join the always-evict set: the conservative default is correct
//! for in-development providers whose cost profile is unknown.
//!
//! The gate is pure: no I/O, no clock reads, no logging. The
//! caller passes `now` from `discovery::current_epoch()` so tests
//! can pin behavior at a fixed instant.

use std::collections::BTreeSet;

use crate::config::ServerIntervals;
use crate::discovery::providers;
use crate::model::GraphSnapshot;

/// The four interval classes ADR 0038 / ADR 0079 define for the
/// `[server.intervals]` table. Granular per-emit provider strings
/// (`git`, `git::cwd`, `tmux`, `github`, `claude-code`, …)
/// collapse to one of these classes via [`provider_class`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderClass {
    Git,
    Mux,
    Harness,
    Forge,
}

impl ProviderClass {
    /// Per-class TTL pulled from the resolved
    /// [`ServerIntervals`].
    pub fn ttl_seconds(self, intervals: &ServerIntervals) -> i64 {
        let dur = match self {
            Self::Git => intervals.git,
            Self::Mux => intervals.mux,
            Self::Harness => intervals.harness,
            Self::Forge => intervals.forge,
        };
        i64::try_from(dur.as_secs()).unwrap_or(i64::MAX)
    }
}

/// Map a granular per-emit provider string to its interval
/// class. `None` means "unmapped" — the caller treats unmapped
/// keys as always-rerun (see [`MUTATOR_PROVIDERS`]).
///
/// Keep this in sync with the table in ADR 0079; the test
/// `every_known_provider_string_maps_to_a_class` pins the
/// production set so a new provider that forgets to register
/// here breaks CI rather than silently joining the always-rerun
/// bucket.
pub fn provider_class(provider: &str) -> Option<ProviderClass> {
    match provider {
        s if s == providers::GIT
            || s == providers::GIT_CWD
            || s == providers::ATELIER
            || s == providers::GENERIC_WORKSPACE
            || s == providers::AGENT_DECK =>
        {
            Some(ProviderClass::Git)
        }
        s if s == providers::TMUX => Some(ProviderClass::Mux),
        s if s == providers::CLAUDE_CODE
            || s == providers::CODEX
            || s == providers::OPENCODE
            || s == providers::AIDER =>
        {
            Some(ProviderClass::Harness)
        }
        s if s == providers::GITHUB => Some(ProviderClass::Forge),
        _ => None,
    }
}

/// Mutator passes whose output depends on the merged snapshot.
/// Always evicted from the prior before merging; always re-run
/// after fresh discovery + warm-start merge land. See ADR 0079
/// for the rationale.
pub const MUTATOR_PROVIDERS: &[&str] = &[
    providers::CROSS_LINK,
    providers::CODEX_LOG,
    providers::HOOK_SIDECAR,
    providers::DECLARED,
];

/// Decision produced by [`compute_freshness_gate`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FreshnessGate {
    /// Heavy providers whose cached slice is still within the
    /// class TTL. The CLI skips re-running these and lets the
    /// prior snapshot's contribution flow through the
    /// backstop merge.
    pub fresh: BTreeSet<String>,
    /// Heavy providers whose cached slice has aged out. The CLI
    /// evicts these from the prior snapshot before merging and
    /// then re-runs them as part of fresh discovery.
    pub stale: BTreeSet<String>,
    /// Provider keys that must be evicted from the prior
    /// unconditionally — mutators (always-rerun) plus any
    /// unmapped keys whose freshness we cannot reason about.
    pub always_evict: BTreeSet<String>,
}

impl FreshnessGate {
    /// Convenience: union of every key the caller should evict
    /// from the prior snapshot before the warm-start merge.
    pub fn keys_to_evict(&self) -> BTreeSet<String> {
        let mut out = self.stale.clone();
        out.extend(self.always_evict.iter().cloned());
        out
    }
}

/// Categorize every provider observed in `prior` against the
/// per-class intervals.
///
/// Walk `prior.node_provenance` + `prior.candidate_links` to
/// collect each provider's max `freshness_epoch`, then compare
/// `now - max_epoch` against the class TTL.
///
/// * Provider with a class and `now - max_epoch < ttl` → fresh.
/// * Provider with a class and `now - max_epoch >= ttl` → stale.
/// * Provider in [`MUTATOR_PROVIDERS`] → always-evict.
/// * Provider with no class mapping → always-evict (conservative).
///
/// A provider mentioned in `MUTATOR_PROVIDERS` never lands in
/// the `fresh` or `stale` bucket; it is purely an eviction
/// target.
pub fn compute_freshness_gate(
    prior: &GraphSnapshot,
    intervals: &ServerIntervals,
    now: i64,
) -> FreshnessGate {
    use std::collections::BTreeMap;

    let mutators: BTreeSet<&str> = MUTATOR_PROVIDERS.iter().copied().collect();

    let mut max_epoch: BTreeMap<String, i64> = BTreeMap::new();
    let mut update = |provider: &str, epoch: Option<i64>| {
        let Some(epoch) = epoch else { return };
        let entry = max_epoch.entry(provider.to_string()).or_insert(i64::MIN);
        if epoch > *entry {
            *entry = epoch;
        }
    };

    for prov in prior.node_provenance.values() {
        update(prov.provider.as_str(), prov.freshness_epoch);
    }
    for link in &prior.candidate_links {
        update(
            link.source_metadata.adapter.as_str(),
            link.source_metadata.freshness_epoch,
        );
    }

    let mut gate = FreshnessGate::default();
    for (provider, max_epoch) in max_epoch {
        if mutators.contains(provider.as_str()) {
            gate.always_evict.insert(provider);
            continue;
        }
        let Some(class) = provider_class(&provider) else {
            // Unknown provider: conservatively re-run. Once the
            // ADR 0079 table grows to cover it, this branch goes
            // away for that key.
            gate.always_evict.insert(provider);
            continue;
        };
        let ttl = class.ttl_seconds(intervals);
        let age = now.saturating_sub(max_epoch);
        if age < ttl {
            gate.fresh.insert(provider);
        } else {
            gate.stale.insert(provider);
        }
    }

    gate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        GraphLink, GraphNode, LinkEndpoint, NodeId, NodeProvenance, Provenance, RelationKind,
        RepoId, RepoNode, SourceMetadata,
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
        // `provider_class`, or accept the always-evict fallback.
        let known = [
            "git",
            "git::cwd",
            "atelier",
            "generic_workspace",
            "agent_deck",
            "tmux",
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
    fn empty_prior_produces_empty_gate() {
        let gate = compute_freshness_gate(&GraphSnapshot::empty(), &intervals_default(), 100);
        assert!(gate.fresh.is_empty());
        assert!(gate.stale.is_empty());
        assert!(gate.always_evict.is_empty());
    }
}
