# ADR 0088: Provider Descriptor Registry

## Status

Accepted

## Context

Every Conspectus discovery adapter stamps its outputs with a
provider key string (`git`, `tmux`, `github`, `claude-code`,
`cross_link`, …). The key is load-bearing across three
independent parts of the warm-start pipeline:

- **Provenance** — stamped on `node_provenance.provider` and
  `source_metadata.adapter` so the freshness gate can identify
  each slice per ADR 0079.
- **Eviction** — `GraphSnapshot::evict_provider(key)` peels
  the matching contribution off the prior snapshot.
- **Classification** — the freshness gate maps each key to a
  [`ProviderClass`] (`Git` / `Mux` / `Harness` / `Forge`) so the
  TTL from `[server.intervals]` applies.
- **Mutator identification** — the always-rerun bucket
  (`cross_link`, `codex_log`, `hook_sidecar`, `declared`) is
  evicted unconditionally and re-runs against the merged
  snapshot.

Pre-H-EXT-001, these facts were spread across three tables:

- `discovery/providers.rs` held bare `pub const KEY: &str = ...`
  entries with no metadata.
- `discovery/cache.rs::provider_class` had a hand-rolled match
  from key → `ProviderClass`.
- `discovery/cache.rs::MUTATOR_PROVIDERS` had a separate
  `&[&str]` of the always-rerun keys.

Adding a new provider required touching all three, and a typo
in any one of them would silently desynchronize warm-start (the
provider's cached slice would either never be evicted or would
join the wrong TTL bucket).

The H-EXT stream calls for one entry point per new provider: a
new adapter's module and one registry entry, with everything
else — freshness gate, eviction, daemon scheduling, filter UI —
deriving from the registry. This ADR records the registration
convention that unblocks that goal.

## Decision

Introduce a single [`ProviderDescriptor`] table in
`discovery/providers.rs` as the source of truth for every
registered provider's key + role in the warm-start pipeline.
The freshness gate and the mutator list both derive from this
table.

### Descriptor shape (CSP-473)

```rust
pub struct ProviderDescriptor {
    pub key: &'static str,
    pub kind: ProviderKind,
}

pub enum ProviderKind {
    Heavy(ProviderClass),   // TTL-gated
    Mutator,                // always rerun
}
```

The descriptor is intentionally lean. `key` names the provider
in provenance; `kind` names its role. Every current provider
fits.

The registry is a static `pub const REGISTRY: &[ProviderDescriptor]`.
Ordering is stable and declaration-order — heavy providers first
(grouped by class), mutators last. New entries slot in
alphabetically within their section so the diff on registration
is easy to review.

### Registration convention

Adding a new provider requires:

1. A `pub const NAME_KEY: &str = "provider-key"` accompaniment
   alongside the existing constants. Keeps grep-friendly
   identifiers in place; the registry test
   `descriptor_constants_agree` guards against drift.
2. A [`ProviderDescriptor`] entry in [`REGISTRY`] with the
   constant as `key` and the appropriate `ProviderKind`.
3. Wiring the constructor into `discover_local_warm_with`.
   CSP-473 leaves the constructor path hand-wired; CSP-476
   / CSP-480 / CSP-484 / CSP-486 generalize the
   constructor surface per entity family (harness / mux / forge
   / orchestrator).

Everything else — freshness gate, mutator eviction, on-disk
cache invalidation, daemon per-provider scheduling — reads
through the registry and picks up the new provider without
extra edits.

### What lives outside the descriptor

- **Constructor callback.** Different providers take different
  inputs (`harness_state_roots` for harness, `TmuxRunner` for
  tmux, `GhRunner` for forge, `agent_deck_root` for
  agent-deck). A uniform constructor signature doesn't fit them
  all, and forcing one would produce a dispatch shape that
  still needs per-provider knowledge at the call site. Later
  H-EXT stories introduce per-entity-family adapter traits
  where a uniform constructor signature makes sense within a
  family; the registry descriptor stays metadata-only in the
  meantime.
- **Env-var opt-out plumbing.** The four opt-out vars
  (`CONSPECTUS_DISABLE_TMUX`, `_FORGE`, `_PROCTREE`,
  `_AGENT_DECK`, `_CODEX_LOG`) plus per-harness state-root vars
  (`CONSPECTUS_<HARNESS>_STATE`) stay on `LocalDiscoveryConfig`
  for CSP-473. `LocalDiscoveryConfig::from_env` still walks
  them explicitly. CSP-479 folds the codex-log wiring into a
  general aux-reader hook via the adapter registry; the
  per-entity opt-outs consolidate as their host adapter traits
  land.
- **Display metadata for UI surfaces.** The TUI controls
  overlay's harness filter (`HARNESS_OPTIONS` in
  `widgets/controls.rs`) and the row-label match in `rows/mod.rs`
  stay separate for CSP-473. CSP-474 folds them into
  `HarnessAdapter::display_label()` / `launch_options()`.

The metadata-only descriptor is sufficient for CSP-473's
scope: freshness gate + mutator list both derive from the
registry.

### Derivation functions

- `providers::provider_class(&str) -> Option<ProviderClass>` —
  registry lookup. `None` for mutators (no class) and for
  unknown keys (in-development providers whose cost profile is
  unknown; the gate treats them as always-evict per ADR 0079's
  conservative default).
- `providers::mutator_keys() -> Vec<&'static str>` — filter of
  `REGISTRY` where `is_mutator()`.

Both are called by the corresponding thin delegates in
`discovery/cache.rs` so the existing call paths
(`cache::provider_class(...)`, `cache::mutator_providers()`)
continue to resolve.

### Type location

[`ProviderClass`] moves from `discovery/cache.rs` to
`discovery/providers.rs`. The class is a provider attribute —
every `Heavy` descriptor carries one — so it belongs where the
descriptor lives. The `cache` module re-exports the type
(`pub use crate::discovery::providers::ProviderClass;`) so
existing `cache::ProviderClass` call sites continue to compile.
The inherent methods that consume `ServerIntervals` stay on the
`cache` side to keep the `crate::config` dependency out of
`providers`.

## Consequences

**For the freshness gate.** `cache::provider_class` becomes a
one-line delegate. The pre-H-EXT-001 match arms disappear —
adding a new heavy provider is a two-line diff (the const and
the descriptor) instead of a four-place synchronize.

**For the mutator list.** `cache::mutator_providers()` (new,
derived) accompanies `cache::MUTATOR_PROVIDERS` (kept as a
`&'static [&'static str]` alias for callers that expect an
indexable slice). The test `mutator_const_matches_registry`
guards against the const drifting from the registry.

**For the H-EXT sequence.** CSP-474 through CSP-486 extend
the descriptor incrementally — adding a
`construct: fn(&LocalDiscoveryConfig) -> Option<Box<dyn Provider>>`
field, or a `display_label: &'static str`, or a
`env_disable_var: Option<&'static str>` — without disrupting
the registry's shape.

**For contributor onboarding.** The CSP-489 contributor guide
will cite this ADR when explaining "how do I register a new
provider" for phase-A callers. The answer today: const + entry
in `REGISTRY` + hand-wire into `discover_local_warm_with`.

**For snapshot compatibility.** The provider key strings are
unchanged. Existing on-disk `graph.bin` snapshots read back
byte-identically; the CSP-091-pinned
`canonical_strings_are_stable` test enforces this.

## Alternatives Considered

**Keep the three tables and add a linting test.** Rejected. A
lint that "every const has a class arm and a mutator arm" is
expensive to author, hard to keep matching the code, and gives
no benefit over deriving from one table.

**Encode `ProviderKind` on `ProviderClass` (e.g. add
`Mutator` as a fifth `ProviderClass` variant).** Rejected. The
class is a TTL bucket; treating mutators as a bucket with
"TTL = 0" pollutes every class-consuming call site with a
special case (`match class { Mutator => ..., Git => ..., }`).
Separating the axes — `is this heavy vs a mutator` from `what
TTL class` — keeps consumers linear.

**Split into per-family registries
(`HARNESS_REGISTRY`, `MUX_REGISTRY`, ...).** Rejected for
CSP-473. The freshness gate and mutator eviction need a
single unified table anyway (they don't care whether a heavy
provider is a harness or a mux). Per-family lists arrive
implicitly in later H-EXT stories once the descriptor grows a
family discriminator via adapter references.

**Move the descriptor to a builder-based DSL (e.g. `provider!(git,
class = Git)`).** Rejected as premature. The declaration is
already dense; a macro would obscure the `#[cfg]`-friendly
struct literal (which supports conditional inclusion in future
stories) and impose a compile-time debug burden for zero
maintenance saving.

**Runtime-mutable registry.** Rejected. Every provider that
Conspectus ships with is compile-time known; a dynamic registry
would introduce ordering questions (does the mutator schedule
change if an adapter registers late?) with no consumer that
needs it. The static `&[...]` model matches how the code
actually consumes the table.

## Open Questions Answered

- **Do the pre-existing string constants stay?** Yes.
  `pub const GIT: &str = "git"` and friends stay in
  `providers.rs` and continue to be the grep-friendly names
  for the string keys. The descriptors reference them via
  `key: GIT`.
- **Where does `ProviderClass` live?** In `providers.rs` with
  its enum definition; the inherent methods live in `cache.rs`
  to keep the `ServerIntervals` dependency localized.
- **Does `MUTATOR_PROVIDERS` (const) go away?** No, it stays
  as an alias for slice-indexing callers, and a test pins it
  to the registry-derived list.

## Open Questions Deferred

- **Constructor uniform signature.** Deferred to per-entity
  H-EXT stories (CSP-476 harness, CSP-480 mux, CSP-484
  forge, CSP-486 orchestrator). Each family may end up with a
  different constructor shape; the registry descriptor grows
  a family-specific reference (e.g. `harness_adapter: &'static
  dyn HarnessAdapter`) once the trait shape is decided.
- **Env-var opt-out consolidation.** Deferred to CSP-479 and
  the per-family H-EXT stories. The current pattern
  (`CONSPECTUS_DISABLE_<X>`) is uniform enough that a
  descriptor-level `env_disable_var: Option<&'static str>`
  works; landing that alongside the constructor work keeps the
  change reviewable.
- **Registry ordering as public API.** Not yet. Callers that
  need a specific ordering (the daemon's per-provider failure
  isolation, the freshness gate's iteration) walk the registry
  in declaration order today; whether that ordering ever
  becomes a documented invariant depends on a consumer that
  needs it.

## Related ADRs

- ADR 0038 (warm-start cache) — established the TTL-per-class
  model this descriptor encodes.
- ADR 0079 (warm-start freshness gate) — defined the freshness
  gate's semantics; the registry now provides its lookup table.
- ADR 0087 (mutation envelope) — the descriptor entry for
  each provider stays within envelope category 2 (rebuildable
  observations) or category 1 (declared) as before; the
  registry doesn't license any new writes.
- CSP-473 backlog entry — the story this ADR closes out.
