use super::*;

/// The string values are stamped into persisted snapshots, so
/// changing one invalidates existing installations' caches.
#[test]
fn canonical_strings_are_stable() {
    assert_eq!(GIT, "git");
    assert_eq!(GIT_CWD, "git::cwd");
    assert_eq!(ATELIER, "atelier");
    assert_eq!(GENERIC_WORKSPACE, "generic_workspace");
    assert_eq!(AGENT_DECK, "agent_deck");
    assert_eq!(TMUX, "tmux");
    assert_eq!(ZELLIJ, "zellij");
    assert_eq!(GITHUB, "github");
    assert_eq!(CLAUDE_CODE, "claude-code");
    assert_eq!(CODEX, "codex");
    assert_eq!(OPENCODE, "opencode");
    assert_eq!(AIDER, "aider");
    assert_eq!(PI, "pi");
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
        ZELLIJ,
        GITHUB,
        CLAUDE_CODE,
        CODEX,
        OPENCODE,
        AIDER,
        PI,
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
    // Pin the expected list so a change that flips a heavy
    // provider to a mutator (or vice versa) is caught here.
    assert_eq!(derived, vec![CROSS_LINK, CODEX_LOG, HOOK_SIDECAR, DECLARED]);
}
