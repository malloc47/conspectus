//! Provider descriptor registry (H-EXT-001, ADR 0088).
//!
//! Every discovery adapter stamps `node_provenance.provider`
//! and `source_metadata.adapter` with one of the keys defined
//! here. The freshness gate ([`crate::discovery::cache::provider_class`]),
//! the eviction primitive ([`crate::model::GraphSnapshot::evict_provider`]),
//! the always-rerun mutator list
//! ([`crate::discovery::cache::MUTATOR_PROVIDERS`]), and the
//! daemon's per-provider failure isolation all key off the same
//! values — so a typo or rename anywhere would silently
//! desynchronize the warm-start path.
//!
//! **Registration convention (ADR 0088).** Each provider is one
//! [`ProviderDescriptor`] entry in the [`REGISTRY`] table below.
//! A heavy TTL-gated provider carries its
//! [`ProviderClass`] via [`ProviderKind::Heavy`]; a mutator pass
//! that always re-runs against the merged snapshot carries
//! [`ProviderKind::Mutator`]. Every descriptor's `key` is
//! exposed as an accompanying `pub const KEY_NAME: &str = ...`
//! constant so existing string-literal call sites keep compiling
//! and grep-friendly identifiers stay in place; the constants
//! are the same string as the descriptor's `key`, checked in
//! [`tests::descriptor_constants_agree`].
//!
//! Adding a new provider:
//!
//! 1. Add a `pub const` for its key alongside the existing ones.
//! 2. Add a descriptor entry to [`REGISTRY`] with the constant
//!    as `key` and the appropriate `kind`.
//! 3. Wire it into `discover_local_warm_with` (H-EXT-001 keeps
//!    the constructor path hand-wired; later H-EXT stories
//!    generalize that surface).
//!
//! The freshness gate reads through [`provider_class`] and the
//! mutator list reads through [`mutator_keys`], both of which
//! walk this registry — so registering the descriptor is the
//! only step required for a new provider to join the warm-start
//! and eviction paths correctly.

// ---------------------------------------------------------------
// Heavy providers (TTL-gated via [`crate::config::ServerIntervals`])
// ---------------------------------------------------------------

/// Git repo + checkout discovery rooted at scan roots.
/// Class: `git` (ADR 0079).
pub const GIT: &str = "git";

/// Observed-cwd git probe — the `observed_cwd_git_fragment`
/// post-pass that re-probes git context against cwds the heavy
/// providers surfaced. Stamped distinct from [`GIT`] so it can
/// be evicted/refreshed independently. Class: `git` (ADR 0079).
pub const GIT_CWD: &str = "git::cwd";

/// Atelier workspace discovery (per-workspace `.atelier/`
/// metadata). Class: `git`.
pub const ATELIER: &str = "atelier";

/// Generic workspace fallback discovery for trees without an
/// Atelier annotation. Class: `git`.
pub const GENERIC_WORKSPACE: &str = "generic_workspace";

/// Agent-deck multi-repo worktree discovery
/// (`$HOME/.agent-deck/multi-repo-worktrees`). Class: `git`.
pub const AGENT_DECK: &str = "agent_deck";

/// Tmux session enumeration via the configured runner.
/// Class: `mux` (ADR 0079).
pub const TMUX: &str = "tmux";

/// GitHub forge metadata via the `gh` runner. Class: `forge`.
pub const GITHUB: &str = "github";

// ---------------------------------------------------------------
// Harness providers — class: `harness` (ADR 0079)
// ---------------------------------------------------------------

/// Claude Code harness state directory scan.
pub const CLAUDE_CODE: &str = "claude-code";

/// Codex harness state directory scan.
pub const CODEX: &str = "codex";

/// openCode harness state directory scan.
pub const OPENCODE: &str = "opencode";

/// Aider harness state directory scan.
pub const AIDER: &str = "aider";

// ---------------------------------------------------------------
// Mutator passes (always-rerun, no class assigned)
// ---------------------------------------------------------------

/// Cross-link inference pass (`discovery::cross_link::infer`).
pub const CROSS_LINK: &str = "cross_link";

/// Codex transcript-log attribution pass
/// (`discovery::codex_log::apply_codex_log_attribution`).
pub const CODEX_LOG: &str = "codex_log";

/// Hook-sidecar replay pass
/// (`discovery::hook_sidecar::apply_hook_sidecars`).
pub const HOOK_SIDECAR: &str = "hook_sidecar";

/// Operator-authored declared link / alias / pin overlay
/// (`discovery::declared::apply_declared_links` + peers).
pub const DECLARED: &str = "declared";

// ---------------------------------------------------------------
// Registry
// ---------------------------------------------------------------

/// The four interval classes ADR 0038 / ADR 0079 define for the
/// `[server.intervals]` table. Granular per-emit provider strings
/// (`git`, `git::cwd`, `tmux`, `github`, `claude-code`, …)
/// collapse to one of these classes via [`provider_class`].
///
/// Owned by this module (moved from `discovery::cache` in
/// H-EXT-001) because the class is a provider attribute — every
/// [`ProviderDescriptor::kind`] with [`ProviderKind::Heavy`]
/// carries a class. `discovery::cache` re-exports the type so
/// existing `cache::ProviderClass` call sites keep compiling.
///
/// The inherent methods on this type
/// ([`ProviderClass::ttl_seconds`], [`ProviderClass::ttl_duration`])
/// live in [`crate::discovery::cache`] so the
/// `crate::config::ServerIntervals` dependency stays localized to
/// the freshness-gate module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderClass {
    Git,
    Mux,
    Harness,
    Forge,
}

/// A provider's role in the warm-start pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    /// TTL-gated heavy provider whose cached slice is skipped when
    /// still fresh and re-run when stale. Carries the interval
    /// [`ProviderClass`] that names the applicable
    /// `[server.intervals]` field.
    Heavy(ProviderClass),
    /// Always-rerun mutator pass. Its output is derived from the
    /// merged snapshot rather than from disk, so caching a prior
    /// output would let stale results linger past upstream
    /// deletions.
    Mutator,
}

/// Metadata about one registered provider. H-EXT-001 keeps this
/// intentionally lean: `key` names the provider in provenance,
/// `kind` names its role in the warm-start pipeline. Later
/// H-EXT stories extend the descriptor with a constructor
/// callback (`H-EXT-004`), env-var opt-out plumbing
/// (`H-EXT-007`), and per-entity-family adapter references
/// (`H-EXT-002` for harness, `H-EXT-008` for mux, `H-EXT-012`
/// for forge, `H-EXT-014` for orchestrator). The metadata-only
/// shape below is sufficient for the freshness gate and mutator
/// list to derive from the registry rather than hard-coded
/// tables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    pub key: &'static str,
    pub kind: ProviderKind,
}

impl ProviderDescriptor {
    /// The [`ProviderClass`] this provider maps to, if any.
    /// Mutator descriptors return `None` — they have no class
    /// because they never participate in the TTL-freshness gate.
    pub const fn class(&self) -> Option<ProviderClass> {
        match self.kind {
            ProviderKind::Heavy(class) => Some(class),
            ProviderKind::Mutator => None,
        }
    }

    /// `true` for [`ProviderKind::Mutator`]. Kept as a method so
    /// the mutator-derivation call site reads as a filter over
    /// the registry rather than a match.
    pub const fn is_mutator(&self) -> bool {
        matches!(self.kind, ProviderKind::Mutator)
    }
}

/// Registered providers in stable declaration order. Every
/// production discovery adapter and mutator pass has exactly
/// one entry.
///
/// The freshness gate and the mutator list both derive from
/// this table; keep new entries in alphabetical-within-section
/// order so the diff on registration is easy to review.
pub const REGISTRY: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        key: GIT,
        kind: ProviderKind::Heavy(ProviderClass::Git),
    },
    ProviderDescriptor {
        key: GIT_CWD,
        kind: ProviderKind::Heavy(ProviderClass::Git),
    },
    ProviderDescriptor {
        key: ATELIER,
        kind: ProviderKind::Heavy(ProviderClass::Git),
    },
    ProviderDescriptor {
        key: GENERIC_WORKSPACE,
        kind: ProviderKind::Heavy(ProviderClass::Git),
    },
    ProviderDescriptor {
        key: AGENT_DECK,
        kind: ProviderKind::Heavy(ProviderClass::Git),
    },
    ProviderDescriptor {
        key: TMUX,
        kind: ProviderKind::Heavy(ProviderClass::Mux),
    },
    ProviderDescriptor {
        key: GITHUB,
        kind: ProviderKind::Heavy(ProviderClass::Forge),
    },
    ProviderDescriptor {
        key: CLAUDE_CODE,
        kind: ProviderKind::Heavy(ProviderClass::Harness),
    },
    ProviderDescriptor {
        key: CODEX,
        kind: ProviderKind::Heavy(ProviderClass::Harness),
    },
    ProviderDescriptor {
        key: OPENCODE,
        kind: ProviderKind::Heavy(ProviderClass::Harness),
    },
    ProviderDescriptor {
        key: AIDER,
        kind: ProviderKind::Heavy(ProviderClass::Harness),
    },
    ProviderDescriptor {
        key: CROSS_LINK,
        kind: ProviderKind::Mutator,
    },
    ProviderDescriptor {
        key: CODEX_LOG,
        kind: ProviderKind::Mutator,
    },
    ProviderDescriptor {
        key: HOOK_SIDECAR,
        kind: ProviderKind::Mutator,
    },
    ProviderDescriptor {
        key: DECLARED,
        kind: ProviderKind::Mutator,
    },
];

/// Look up a provider's [`ProviderClass`] via the registry.
/// Returns `None` for mutator descriptors and for unknown keys
/// (in-development providers whose cost profile is unknown).
///
/// This is the derivation the freshness gate consults; the
/// wrapper in `discovery::cache::provider_class` delegates here.
pub fn provider_class(provider: &str) -> Option<ProviderClass> {
    REGISTRY
        .iter()
        .find(|d| d.key == provider)
        .and_then(|d| d.class())
}

/// Mutator provider keys derived from the registry. Kept as a
/// `Vec` because the caller (the always-evict bucket +
/// per-mutator scheduling) usually iterates it multiple times
/// per warm-start invocation and cheap `contains` checks want an
/// owned collection; the small allocation is fine at
/// warm-start cadence.
pub fn mutator_keys() -> Vec<&'static str> {
    REGISTRY
        .iter()
        .filter(|d| d.is_mutator())
        .map(|d| d.key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity: the string values must match what was stamped
    /// pre-H-REF-009 so the cache can be read back from existing
    /// installations without invalidating the on-disk snapshot.
    #[test]
    fn canonical_strings_are_stable() {
        assert_eq!(GIT, "git");
        assert_eq!(GIT_CWD, "git::cwd");
        assert_eq!(ATELIER, "atelier");
        assert_eq!(GENERIC_WORKSPACE, "generic_workspace");
        assert_eq!(AGENT_DECK, "agent_deck");
        assert_eq!(TMUX, "tmux");
        assert_eq!(GITHUB, "github");
        assert_eq!(CLAUDE_CODE, "claude-code");
        assert_eq!(CODEX, "codex");
        assert_eq!(OPENCODE, "opencode");
        assert_eq!(AIDER, "aider");
        assert_eq!(CROSS_LINK, "cross_link");
        assert_eq!(CODEX_LOG, "codex_log");
        assert_eq!(HOOK_SIDECAR, "hook_sidecar");
        assert_eq!(DECLARED, "declared");
    }

    /// Every descriptor key round-trips through the module's
    /// `pub const` accompaniment. Catches a rename that updates
    /// the descriptor entry without touching the constant (or
    /// vice versa).
    #[test]
    fn descriptor_constants_agree() {
        let expected = [
            GIT,
            GIT_CWD,
            ATELIER,
            GENERIC_WORKSPACE,
            AGENT_DECK,
            TMUX,
            GITHUB,
            CLAUDE_CODE,
            CODEX,
            OPENCODE,
            AIDER,
            CROSS_LINK,
            CODEX_LOG,
            HOOK_SIDECAR,
            DECLARED,
        ];
        assert_eq!(REGISTRY.len(), expected.len());
        for (desc, key) in REGISTRY.iter().zip(expected.iter()) {
            assert_eq!(desc.key, *key);
        }
    }

    /// Every heavy descriptor round-trips through
    /// [`provider_class`]. Guards the freshness gate against a
    /// new heavy provider that forgets to declare its class.
    #[test]
    fn heavy_descriptors_round_trip_provider_class() {
        for desc in REGISTRY {
            match desc.kind {
                ProviderKind::Heavy(class) => {
                    assert_eq!(
                        provider_class(desc.key),
                        Some(class),
                        "heavy provider `{}` did not round-trip",
                        desc.key
                    );
                }
                ProviderKind::Mutator => {
                    assert_eq!(
                        provider_class(desc.key),
                        None,
                        "mutator `{}` unexpectedly reports a class",
                        desc.key
                    );
                }
            }
        }
    }

    /// Unknown keys stay unmapped so the freshness gate treats
    /// them as always-evict per ADR 0079's conservative default.
    #[test]
    fn unknown_keys_are_unmapped() {
        assert_eq!(provider_class("no_such_provider"), None);
    }

    /// The derived mutator list matches every descriptor with
    /// `ProviderKind::Mutator`, in registry order.
    #[test]
    fn mutator_keys_derive_from_registry() {
        let derived = mutator_keys();
        let expected: Vec<&str> = REGISTRY
            .iter()
            .filter(|d| d.is_mutator())
            .map(|d| d.key)
            .collect();
        assert_eq!(derived, expected);
        // Sanity anchor against the pre-H-EXT-001 table so a
        // rename that flips a heavy to a mutator (or vice
        // versa) is caught here.
        assert_eq!(derived, vec![CROSS_LINK, CODEX_LOG, HOOK_SIDECAR, DECLARED]);
    }
}
