//! Orchestrator registration surface (H-EXT-014).
//!
//! Orchestrators are Conspectus's third registration pattern
//! (harness adapters + mux backends + forge adapters are the
//! others). An orchestrator is a piece of higher-level workflow
//! software that manages agent sessions, mux sessions, or repo
//! layouts — agent-deck today, dmux / herdr / pertmux / workmux
//! are candidates for future adapters (see the
//! `H-AGENTMUX-*` audit stream).
//!
//! Unlike harness adapters, orchestrator adapters have no
//! shared discovery trait beyond
//! [`crate::discovery::DiscoveryProvider`] — each one has its
//! own filesystem shape, its own configuration knobs, and its
//! own attribution rules. The registry here is metadata-only:
//! it names the orchestrator, records its default state-root
//! resolver, and packages a build-from-root callback so the
//! discovery driver doesn't have to name each orchestrator
//! individually.
//!
//! ADR 0060 originally punted on introducing a shared
//! orchestrator trait. H-EXT-014 preserves that stance —
//! the registry gives us the registration surface (config
//! table, env-var walk, from-env defaults) without forcing
//! a common trait that today's single adapter would
//! prematurely constrain. If a mutation-capability trait
//! is needed later (rename routing, ownership transfer),
//! it lands via H-EXT-015 and adds an orthogonal facet to
//! the descriptor.

use std::path::PathBuf;

use crate::discovery::{DiscoveryProvider, agent_deck};

/// Metadata about one registered orchestrator (H-EXT-014).
///
/// `default_root` resolves the orchestrator's on-disk root
/// from the process environment (`$CONSPECTUS_<KEY>_ROOT` first,
/// then a home-relative fallback). `build` turns a resolved
/// root into a `Box<dyn DiscoveryProvider>` the warm-start
/// pipeline can drive.
pub struct OrchestratorDescriptor {
    /// Stable key matching the `orchestrator` provider-slice
    /// tag (e.g. `agent_deck`). Also the key operators use in
    /// `[orchestrators.<key>].root = "..."` config tables
    /// (config-table parsing lands as a follow-up).
    pub key: &'static str,
    /// Environment variable that overrides the default root
    /// (e.g. `CONSPECTUS_AGENT_DECK_ROOT`). Empty string means
    /// no env override.
    pub env_root_var: &'static str,
    /// Environment variable that disables the orchestrator
    /// entirely (e.g. `CONSPECTUS_DISABLE_AGENT_DECK`). Empty
    /// string means no explicit disable var (the adapter can
    /// still be disabled via `LocalDiscoveryConfig::without_orchestrator`).
    pub env_disable_var: &'static str,
    /// Resolve a default `HOME`-relative fallback path. `None`
    /// means the orchestrator has no default root — the
    /// operator must supply one explicitly via env var or
    /// config.
    pub home_relative_default: Option<&'static str>,
    /// Turn a resolved root into a `DiscoveryProvider` the
    /// warm-start pipeline can drive. Each orchestrator ships
    /// its own constructor here; the discovery driver never
    /// names an orchestrator directly.
    pub build: fn(PathBuf) -> Box<dyn DiscoveryProvider>,
}

impl OrchestratorDescriptor {
    /// Resolve the on-disk root from process environment.
    /// Returns `None` when the orchestrator is env-disabled or
    /// has no default and no explicit env override.
    pub fn resolve_default_root(&self) -> Option<PathBuf> {
        if !self.env_disable_var.is_empty() && std::env::var_os(self.env_disable_var).is_some() {
            return None;
        }
        if !self.env_root_var.is_empty()
            && let Some(value) = std::env::var_os(self.env_root_var)
        {
            return Some(PathBuf::from(value));
        }
        let rel = self.home_relative_default?;
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rel))
    }
}

/// Registered orchestrator adapters. Adding a new
/// orchestrator (dmux / herdr / pertmux / workmux, gated on
/// their `H-AGENTMUX-*` evidence audits) is a matter of
/// pushing another entry — the discovery driver, the
/// config table parser, and env-var walk pick it up
/// automatically.
///
/// Order is stable (registration order); pin adapters slot
/// into this array in the order H-AGENTMUX-* decides.
pub const REGISTRY: &[OrchestratorDescriptor] = &[OrchestratorDescriptor {
    key: crate::discovery::providers::AGENT_DECK,
    env_root_var: "CONSPECTUS_AGENT_DECK_ROOT",
    env_disable_var: "CONSPECTUS_DISABLE_AGENT_DECK",
    home_relative_default: Some(".agent-deck/multi-repo-worktrees"),
    build: build_agent_deck,
}];

fn build_agent_deck(root: PathBuf) -> Box<dyn DiscoveryProvider> {
    Box::new(agent_deck::AgentDeckDiscovery::new(root))
}

/// Find a descriptor by orchestrator key. Returns `None` for
/// unknown keys — callers surface an error when they need
/// registered-only behavior.
pub fn descriptor_by_key(key: &str) -> Option<&'static OrchestratorDescriptor> {
    REGISTRY.iter().find(|d| d.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_deck_descriptor_matches_pre_h_ext_014_env_semantics() {
        let desc = descriptor_by_key("agent_deck").expect("registered");
        assert_eq!(desc.env_root_var, "CONSPECTUS_AGENT_DECK_ROOT");
        assert_eq!(desc.env_disable_var, "CONSPECTUS_DISABLE_AGENT_DECK");
        assert_eq!(
            desc.home_relative_default,
            Some(".agent-deck/multi-repo-worktrees")
        );
    }

    #[test]
    fn unknown_key_returns_none() {
        assert!(descriptor_by_key("nope").is_none());
    }
}
