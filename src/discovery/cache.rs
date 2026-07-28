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

/// Mutator passes whose expensive work is the process-tree `/proc`
/// walk and the pid-fed harness aux attribution (H-SERVE-PERF-001a,
/// ADR 0091). Unlike the other mutators these are **class-gated**:
/// they only re-run when the mux or harness slice re-ran this cycle,
/// and their prior contribution is preserved (not evicted) on
/// git/forge-only cycles so agent↔pane links survive without a fresh
/// `/proc` walk. `cross_link` produces the agent↔mux links;
/// `codex_log` is the pid-fed aux attribution. `hook_sidecar` and
/// `declared` stay always-rerun (cheap file reads, no `/proc`).
pub const PROCESS_TREE_MUTATORS: &[&str] = &[providers::CROSS_LINK, providers::CODEX_LOG];

/// Whether `provider` belongs to the mux or harness interval class —
/// the two classes whose re-run feeds the process-tree mutators
/// (H-SERVE-PERF-001a). Used to decide, from the freshly-run
/// providers' provenance, whether to refresh the process-tree pass.
pub fn is_mux_or_harness_provider(provider: &str) -> bool {
    matches!(
        provider_class(provider),
        Some(ProviderClass::Mux | ProviderClass::Harness)
    )
}

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
#[path = "cache_tests.rs"]
mod tests;
