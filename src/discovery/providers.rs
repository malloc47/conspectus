//! Canonical per-emit provider identifiers (H-REF-009).
//!
//! Every discovery adapter stamps `node_provenance.provider`
//! and `source_metadata.adapter` with one of these strings. The
//! freshness gate (`cache::provider_class`), the eviction
//! primitive (`GraphSnapshot::evict_provider`), the always-rerun
//! mutator list (`cache::MUTATOR_PROVIDERS`), and the daemon's
//! per-provider failure isolation all key off the same values —
//! so a typo or rename anywhere would silently desynchronize
//! the warm-start path.
//!
//! Add a new provider here first, then reference the new const
//! from the adapter's stamp call and the `cache::provider_class`
//! match arm. Per-module `HARNESS_KEY` / `ADAPTER_NAME`
//! constants re-export from this module so the literal string
//! lives in exactly one place.

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
}
