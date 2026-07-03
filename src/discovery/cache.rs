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

/// Re-export so existing `cache::ProviderClass` call sites keep
/// compiling. The type moved to
/// [`crate::discovery::providers`] in H-EXT-001 because the class
/// is a provider attribute — every
/// [`crate::discovery::providers::ProviderDescriptor`] with
/// [`crate::discovery::providers::ProviderKind::Heavy`] carries
/// one. The inherent methods below live in this module (not on
/// the type's home) so the `ServerIntervals` dependency stays
/// localized to the freshness-gate module.
pub use crate::discovery::providers::ProviderClass;

impl ProviderClass {
    /// Per-class TTL pulled from the resolved
    /// [`ServerIntervals`].
    pub fn ttl_seconds(self, intervals: &ServerIntervals) -> i64 {
        let dur = self.ttl_duration(intervals);
        i64::try_from(dur.as_secs()).unwrap_or(i64::MAX)
    }

    /// Same as [`Self::ttl_seconds`] but returns the underlying
    /// [`std::time::Duration`] so `std::thread::sleep` / channel
    /// timeouts can consume it directly without round-tripping
    /// through seconds. The daemon's per-class scheduler is the
    /// primary caller (P7-006 layer B).
    pub fn ttl_duration(self, intervals: &ServerIntervals) -> std::time::Duration {
        match self {
            Self::Git => intervals.git,
            Self::Mux => intervals.mux,
            Self::Harness => intervals.harness,
            Self::Forge => intervals.forge,
        }
    }

    /// Every granular provider key this class owns. Inverse of
    /// [`provider_class`]; the daemon's per-class scheduler uses
    /// this to evict a class's slice from the prior cache before
    /// re-running just that class's providers.
    pub fn providers(self) -> &'static [&'static str] {
        match self {
            Self::Git => &[
                providers::GIT,
                providers::GIT_CWD,
                providers::ATELIER,
                providers::GENERIC_WORKSPACE,
                providers::AGENT_DECK,
            ],
            Self::Mux => &[providers::TMUX],
            Self::Harness => &[
                providers::CLAUDE_CODE,
                providers::CODEX,
                providers::OPENCODE,
                providers::AIDER,
            ],
            Self::Forge => &[providers::GITHUB],
        }
    }

    /// Stable name used in log lines / future status output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Mux => "mux",
            Self::Harness => "harness",
            Self::Forge => "forge",
        }
    }

    /// The full set of classes the daemon schedules. Returned in
    /// a stable order so logs and tests are deterministic.
    pub fn all() -> &'static [ProviderClass] {
        &[Self::Git, Self::Mux, Self::Harness, Self::Forge]
    }

    /// Parse a class identifier produced by [`Self::name`] back
    /// into a `ProviderClass`. The daemon's `refresh --class`
    /// command uses this to map operator input ("forge") to the
    /// internal enum without coupling the CLI to the enum
    /// variant directly.
    pub fn parse(name: &str) -> Option<ProviderClass> {
        match name {
            "git" => Some(Self::Git),
            "mux" => Some(Self::Mux),
            "harness" => Some(Self::Harness),
            "forge" => Some(Self::Forge),
            _ => None,
        }
    }
}

/// Map a granular per-emit provider string to its interval
/// class. `None` means "unmapped" — the caller treats unmapped
/// keys as always-rerun (see [`mutator_providers`]).
///
/// H-EXT-001 (ADR 0088) folded the classification table into the
/// [`crate::discovery::providers::REGISTRY`]. This function is a
/// thin delegate that re-exports the registry lookup at its
/// long-standing call path.
pub fn provider_class(provider: &str) -> Option<ProviderClass> {
    providers::provider_class(provider)
}

/// Mutator passes whose output depends on the merged snapshot.
/// Always evicted from the prior before merging; always re-run
/// after fresh discovery + warm-start merge land. See ADR 0079
/// for the rationale.
///
/// H-EXT-001 (ADR 0088) derives the list from the registry so a
/// new mutator provider added to
/// [`crate::discovery::providers::REGISTRY`] joins the eviction
/// bucket automatically.
pub fn mutator_providers() -> Vec<&'static str> {
    providers::mutator_keys()
}

/// Backwards-compatible const alias for the pre-H-EXT-001
/// callers that read the mutator list as a `&'static [&'static str]`.
/// Callers that only need iteration should prefer
/// [`mutator_providers`]; this constant stays for callers that
/// index into a slice (`for key in MUTATOR_PROVIDERS`).
///
/// The runtime value must stay in sync with
/// [`crate::discovery::providers::REGISTRY`]; a change here
/// without a matching descriptor registration is caught by
/// [`tests::mutator_const_matches_registry`].
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
        // `provider_class` (via the registry descriptor per
        // ADR 0088), or accept the always-evict fallback.
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

    /// H-EXT-001 / ADR 0088: the pre-registry `MUTATOR_PROVIDERS`
    /// constant and the registry-derived
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
}
