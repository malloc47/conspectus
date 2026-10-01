//! Adapter conformance suite.
//!
//! Walks each entity-family registry and asserts the
//! per-family invariants every adapter must satisfy. Pins the
//! shape so a new adapter that skips one of these gets caught
//! at CI, not by a downstream mystery.
//!
//! **Families and invariants:**
//!
//! - **Harness adapters** — unique `harness_key`s across the
//!   registered set, non-empty `display_label`, uniform
//!   `runtime_signature.harness_key == harness_key()`.
//! - **Mux backends** — every `backend_key` returned by an
//!   impl must appear in `KNOWN_MUX_BACKENDS`; every
//!   `KNOWN_MUX_BACKENDS` entry maps to a registered
//!   `ProviderDescriptor` in `providers::REGISTRY` with class
//!   `Mux`.
//! - **Forge adapters** — the pre-existing `GitHubForgeProvider`
//!   and `GitLabForgeProvider` return distinct `provider()`
//!   strings; each impl's `claims_remote_url` is non-total
//!   (returns `false` for at least one URL).
//! - **Orchestrator descriptors** — `REGISTRY` keys are
//!   unique; each descriptor's `resolve_default_root` is
//!   pure over the process environment and returns a
//!   `Some` when the env has `HOME` set and no disable var.
//!
//! What this suite does **not** do:
//!
//! - Behavior tests per adapter (those live alongside each
//!   adapter's own `mod tests`).
//! - Fixture-corpus integration (H-EXT-016's Scope calls this
//!   out as a follow-up; the fixture corpus is designed to
//!   accept new adapter fixtures without editing the corpus
//!   loader).
//! - Runtime capability checks against a live server
//!   (production tests use fake runners, not the real
//!   binaries).

use std::collections::BTreeSet;

use conspectus::discovery::forge::ForgeAdapter;
use conspectus::discovery::forge::github::GitHubForgeProvider;
use conspectus::discovery::forge::gitlab::GitLabForgeProvider;
use conspectus::discovery::harness::registered_adapters;
use conspectus::discovery::orchestrator;
use conspectus::discovery::providers::{self, ProviderClass, ProviderKind};
use conspectus::discovery::tmux::{self, FakeTmux, KNOWN_MUX_BACKENDS, MuxBackend, SystemTmux};

// ---------------------------------------------------------------
// Harness family
// ---------------------------------------------------------------

#[test]
fn harness_adapter_keys_are_unique() {
    let mut seen = BTreeSet::new();
    for adapter in registered_adapters() {
        assert!(
            seen.insert(adapter.harness_key()),
            "duplicate harness_key `{}` in registered_adapters()",
            adapter.harness_key()
        );
    }
}

#[test]
fn every_harness_has_a_nonempty_display_label() {
    for adapter in registered_adapters() {
        let label = adapter.display_label();
        assert!(
            !label.is_empty(),
            "harness `{}` returned an empty display_label",
            adapter.harness_key()
        );
    }
}

#[test]
fn runtime_signature_harness_key_matches_adapter_harness_key() {
    for adapter in registered_adapters() {
        let sig = adapter.runtime_signature();
        // Some adapters share a signature struct across
        // variants; the invariant here is that the signature
        // isn't STAMPED with a mismatched key. The stub
        // signature returns "unknown" for adapters that
        // haven't opted in; skip those.
        if sig.harness_key == "unknown" {
            continue;
        }
        assert_eq!(
            sig.harness_key,
            adapter.harness_key(),
            "harness `{}` runtime_signature.harness_key mismatches",
            adapter.harness_key()
        );
    }
}

// ---------------------------------------------------------------
// Mux backend family
// ---------------------------------------------------------------

#[test]
fn every_mux_backend_key_appears_in_known_mux_backends() {
    // Instantiate every production impl and check its
    // `backend_key()` is in the compile-time registry.
    let backends: [Box<dyn MuxBackend>; 3] = [
        Box::new(SystemTmux::new()),
        Box::new(FakeTmux::with_sessions("")),
        Box::new(conspectus::discovery::zellij::SystemZellij::new()),
    ];
    for backend in &backends {
        assert!(
            KNOWN_MUX_BACKENDS.contains(&backend.backend_key()),
            "backend `{}` returned by an impl is not in KNOWN_MUX_BACKENDS",
            backend.backend_key()
        );
    }
}

#[test]
fn known_mux_backends_all_map_to_registered_providers_with_mux_class() {
    for key in KNOWN_MUX_BACKENDS {
        let desc = providers::REGISTRY
            .iter()
            .find(|d| d.key == *key)
            .unwrap_or_else(|| panic!("mux backend `{key}` missing from providers::REGISTRY"));
        match desc.kind {
            ProviderKind::Heavy(ProviderClass::Mux) => {}
            other => panic!("mux backend `{key}` has wrong provider kind: {other:?}"),
        }
    }
}

#[test]
fn known_mux_backends_keys_are_unique() {
    let mut seen = BTreeSet::new();
    for key in KNOWN_MUX_BACKENDS {
        assert!(
            seen.insert(*key),
            "duplicate key `{key}` in KNOWN_MUX_BACKENDS"
        );
    }
}

// ---------------------------------------------------------------
// Forge family
// ---------------------------------------------------------------

#[test]
fn forge_adapter_provider_strings_are_distinct() {
    let github = GitHubForgeProvider::with_runner(
        conspectus::discovery::forge::FakeGh::with_pull_requests("[]"),
    );
    let gitlab = GitLabForgeProvider::new();
    let github_provider = ForgeAdapter::provider(&github).to_string();
    let gitlab_provider = ForgeAdapter::provider(&gitlab).to_string();
    assert_ne!(github_provider, gitlab_provider);
}

#[test]
fn forge_claims_remote_url_partitions_hosts() {
    let github = GitHubForgeProvider::with_runner(
        conspectus::discovery::forge::FakeGh::with_pull_requests("[]"),
    );
    let gitlab = GitLabForgeProvider::new();

    let github_url = "git@github.com:foo/bar.git";
    let gitlab_url = "git@gitlab.com:foo/bar.git";
    assert!(github.claims_remote_url(github_url));
    assert!(!github.claims_remote_url(gitlab_url));
    assert!(gitlab.claims_remote_url(gitlab_url));
    assert!(!gitlab.claims_remote_url(github_url));

    // Non-forge URL: neither claims.
    let elsewhere = "git@example.com:foo/bar.git";
    assert!(!github.claims_remote_url(elsewhere));
    assert!(!gitlab.claims_remote_url(elsewhere));
}

// ---------------------------------------------------------------
// Orchestrator family
// ---------------------------------------------------------------

#[test]
fn orchestrator_registry_keys_are_unique() {
    let mut seen = BTreeSet::new();
    for desc in orchestrator::REGISTRY {
        assert!(
            seen.insert(desc.key),
            "duplicate orchestrator key `{}` in REGISTRY",
            desc.key
        );
    }
}

#[test]
fn every_orchestrator_can_resolve_default_root_with_home_set() {
    // Setting HOME in one thread of a `cargo test` run leaks
    // to sibling tests. Instead, snapshot HOME, ensure it's
    // set (cargo sets it), and check each descriptor either
    // resolves cleanly or explicitly opts out via its
    // disable env var.
    if std::env::var_os("HOME").is_none() {
        // Highly unusual — cargo test sets HOME. Skip
        // rather than fail.
        return;
    }
    for desc in orchestrator::REGISTRY {
        if !desc.env_disable_var.is_empty() && std::env::var_os(desc.env_disable_var).is_some() {
            // Operator disabled this orchestrator via env.
            continue;
        }
        let root = desc.resolve_default_root();
        assert!(
            root.is_some(),
            "orchestrator `{}` should resolve a default root when HOME is set and disable-var is unset",
            desc.key
        );
    }
}

// ---------------------------------------------------------------
// Cross-family: provider registry uniqueness
// ---------------------------------------------------------------

#[test]
fn provider_registry_keys_are_unique() {
    let mut seen = BTreeSet::new();
    for desc in providers::REGISTRY {
        assert!(
            seen.insert(desc.key),
            "duplicate provider key `{}` in providers::REGISTRY",
            desc.key
        );
    }
}

#[test]
fn provider_registry_mux_class_matches_known_mux_backends() {
    // Every ProviderClass::Mux entry must appear in
    // KNOWN_MUX_BACKENDS and vice versa.
    let mux_from_registry: BTreeSet<&'static str> = providers::REGISTRY
        .iter()
        .filter_map(|d| match d.kind {
            ProviderKind::Heavy(ProviderClass::Mux) => Some(d.key),
            _ => None,
        })
        .collect();
    let mux_from_known: BTreeSet<&'static str> = tmux::KNOWN_MUX_BACKENDS.iter().copied().collect();
    assert_eq!(
        mux_from_registry, mux_from_known,
        "providers::REGISTRY mux entries and KNOWN_MUX_BACKENDS diverged"
    );
}
