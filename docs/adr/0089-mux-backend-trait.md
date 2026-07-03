# ADR 0089: Mux Backend Trait

## Status

Accepted

## Context

Conspectus discovers, previews, renames, launches, and attaches
mux sessions. In v1 the only backend it knows about is tmux, and
the corresponding trait — `TmuxRunner` in
`src/discovery/tmux/mod.rs` — carries every capability the mux
surface exercises: `list_sessions`, `capture_pane`,
`rename_session`, `new_session`, `attach_session`, `send_keys`.

Pin entries already carry a `mux.backend = "tmux"` string
(ADR 0057) because the schema was designed with future backends
in mind, and `MuxSessionNode.backend` matches. But the code that
consumes those values is still tmux-only:

- Every `&dyn TmuxRunner` in the TUI runtime, CLI rename path,
  pin launch primitive, and mux-preview capture treats the
  runner as "the tmux runner." Adding a second backend would
  either fork every call site or require a runtime dispatch
  layer at each one.
- Attach-gate and pin-validation error strings hardcode `"tmux"`
  (see `src/tui/actions.rs`, `src/pins.rs`) and speak in
  binary-name terms rather than capability terms.
- The pre-H-EXT-008 `LocalDiscoveryConfig.tmux_runner` field
  is a single-slot `Option<Box<dyn TmuxRunner>>`. There's no
  place to hang a second backend's implementation.

The H-EXT-010 story (zellij backend) and H-EXT-011 story
(hook mux-context probe) are the concrete downstream consumers
that need this abstraction. H-EXT-008 is the shape story that
lands the trait rename, a backend registry, and the
`backend_key` dispatch seam. H-EXT-009 completes the surface by
migrating attach / pin validation to capability outcomes instead
of the harmless-but-lying `"tmux"` string checks.

## Decision

### `MuxBackend` trait

Rename `TmuxRunner` to `MuxBackend`. The renamed trait is
otherwise unchanged today — same six methods, same outcome
enums, same `Send + Sync` bound — but gains one required
method:

```rust
pub trait MuxBackend: Send + Sync {
    /// Backend identity. Stamped on pin entries
    /// (`mux.backend`) and `MuxSessionNode.backend`.
    fn backend_key(&self) -> &'static str;

    fn list_sessions(&self, format: &str) -> Result<TmuxOutcome>;

    fn capture_pane(...) -> Result<TmuxCaptureOutcome> {
        Ok(TmuxCaptureOutcome::Unsupported)
    }
    fn rename_session(...) -> Result<TmuxRenameOutcome> {
        Ok(TmuxRenameOutcome::Unsupported)
    }
    fn new_session(...) -> Result<TmuxNewSessionOutcome> {
        Ok(TmuxNewSessionOutcome::Unsupported)
    }
    fn attach_session(...) -> Result<TmuxAttachOutcome> {
        Ok(TmuxAttachOutcome::Unsupported)
    }
    fn send_keys(...) -> Result<TmuxSendKeysOutcome> {
        Ok(TmuxSendKeysOutcome::Unsupported)
    }
}
```

The capability methods keep their `Unsupported` defaults. A new
backend implements only the methods it actually supports; every
caller consults the returned outcome instead of assuming the
backend can perform the op (H-EXT-009 completes the
capability-gate migration by removing the pre-existing
`backend == "tmux"` string checks in `src/tui/actions.rs` and
`src/pins.rs`).

### `backend_key` semantics

`backend_key` is the source of truth for backend identity.
Every consumer keys off it:

- `MuxSessionId::backend` and `MuxSessionNode.backend` stamp
  their originating backend's key so downstream code can route
  by backend without re-detecting.
- Pin entries carry the same key in `mux.backend`.
- `LocalDiscoveryConfig::mux_backend_by_key(key)` and
  `take_mux_backend_by_key(key)` resolve a key to a registered
  backend.
- The discovery provider a backend contributes to (currently
  `TmuxDiscovery` for tmux) uses the backend's own key to stamp
  provenance.

Two backends must never share a key. The `tmux` key today aliases
`crate::discovery::providers::TMUX` so the pre-H-EXT-008 wire
strings (`"tmux"` in every pin config file, snapshot cache,
provenance stamp) stay byte-identical.

### `SystemTmux` is the first impl

`SystemTmux` implements `MuxBackend` and returns `"tmux"` from
`backend_key`. The tmux-specific outcome types (`TmuxOutcome`,
`TmuxCaptureOutcome`, `TmuxRenameOutcome`,
`TmuxNewSessionOutcome`, `TmuxAttachOutcome`,
`TmuxSendKeysOutcome`) keep their `Tmux`-prefixed names in this
Phase-C step — their variant shapes (`Sessions`, `NoTarget`,
`NameCollision`, `Unavailable`, `Failed`, `Unsupported`) are
backend-neutral, so a rename to `MuxSessionListOutcome` etc. is
a mechanical follow-up orthogonal to the trait-shape work here.

### Backend registry on `LocalDiscoveryConfig`

`LocalDiscoveryConfig.tmux_runner: Option<Box<dyn TmuxRunner>>`
becomes `LocalDiscoveryConfig.mux_backends: Vec<Box<dyn
MuxBackend>>`. `from_env` populates a single tmux entry
(subject to `CONSPECTUS_DISABLE_TMUX`), matching pre-H-EXT-008
behavior; a multi-backend host adds more entries via
`with_mux_backend`.

Backwards-compat builder aliases (H-EXT-008 preserves the
pre-existing names so the ~10 call sites across dev_scenarios /
integration tests keep compiling without a mass rename):

- `with_mux_backend(runner)` — new; pushes onto the registry.
- `with_tmux_runner(runner)` — deprecated alias for
  `with_mux_backend`.
- `without_tmux()` — retained; retains its historical semantic
  by clearing entries whose `backend_key() == "tmux"`.

Registry accessors:

- `mux_backend_by_key(&self, key: &str) -> Option<&dyn
  MuxBackend>` — reference-taking lookup for consumers that
  don't own the backend.
- `take_mux_backend_by_key(&mut self, key: &str) ->
  Option<Box<dyn MuxBackend>>` — consuming lookup for the
  discovery path that needs to move the backend into a
  `TmuxDiscovery` wrapper.

### Namespace concept generalizing `socket_name`

The pre-H-EXT-008 tmux methods accept `socket_name: Option<&str>`
so a callers with a non-default tmux socket can address it
directly. The H-EXT-010 zellij backend has no equivalent — it
addresses a global session set — so the parameter's tmux-specific
meaning becomes stale in a multi-backend world.

H-EXT-008 keeps the `socket_name` name unchanged. Generalizing
to a `namespace: Option<&str>` parameter across all six methods
is a documented follow-up:

- Every method's `socket_name` becomes `namespace`.
- Tmux consults the value with the same `-L <socket>` semantics.
- Zellij ignores it (or errors on non-`None` if we want strict
  behavior).
- Pin entries' `mux.socket_name` field renames to
  `mux.namespace` with a `#[serde(alias = "socket_name")]`
  compat shim.

Deferred to a follow-up because the rename touches ~15 method
signatures and every call site — landing it alongside the
zellij backend keeps the review focused.

### What stays put in this ADR

- Outcome enum names (`TmuxOutcome`, `TmuxCaptureOutcome`, …)
  keep their `Tmux`-prefixed names for now. The variants are
  already backend-neutral; the rename is orthogonal to
  trait-shape work.
- The `TmuxDiscovery` provider is still the discovery
  wrapper for the tmux backend. A future zellij backend gets
  its own `ZellijDiscovery` wrapper; both are `DiscoveryProvider`
  instances the warm-start path threads through.
- The `namespace: Option<&str>` generalization of tmux's
  `socket_name` (see above).

## Consequences

**For consumer code today.** Every `&dyn TmuxRunner` becomes
`&dyn MuxBackend`. Every `Box<dyn TmuxRunner>` becomes
`Box<dyn MuxBackend>`. Impls add a `backend_key()` method. No
behavior changes; snapshot cache, pin config wire format,
provenance stamps are byte-identical.

**For H-EXT-009.** Attach and pin validation branch on the
outcome returned by `attach_session` / `new_session` / etc.
instead of on `backend == "tmux"`. The trait's `Unsupported`
default is the mechanism.

**For H-EXT-010.** Adding a zellij backend is:
1. Author `discovery/zellij/mod.rs` with `struct SystemZellij;
   impl MuxBackend for SystemZellij { fn backend_key() { "zellij" }
   … }`. Implement `list_sessions` + `attach_session`; leave
   rename / new-session / capture-pane / send-keys as their
   `Unsupported` defaults.
2. Add a `ZellijDiscovery` provider that wraps a zellij backend
   and stamps `MuxSessionNode.backend = "zellij"`.
3. Push the backend onto `mux_backends` in `from_env`.

Zero edits to `TmuxDiscovery`, zero edits to the CLI rename
path, zero edits to pin-launch dispatch beyond what H-EXT-009
already delivers.

**For H-EXT-011.** The hook mux-context probe (which today
carries hardcoded tmux-shaped fields — `session_name`,
`native_id`, `pane_id`, `socket_path` — into
`HookTmuxRecord`) generalizes to
`MuxBackend::current_session_context() ->
MuxSessionContext { session, pane?, namespace? }`. Sidecar
records grow a `backend` field so the reader knows which
backend produced them; ADR 0028's schema version bumps.

**For operator wire configs.** `.conspectus.toml` pin entries
already carry `mux.backend = "tmux"`. Adding
`mux.backend = "zellij"` requires the zellij backend to be
registered but changes no schema.

**For ADR 0057.** ADR 0057 §Launch Semantics speaks in
tmux-specific terms
(`tmux new-session`, `tmux send-keys`, `tmux attach-session`).
The prose is superseded by "the registered backend's
`new_session` / `send_keys` / `attach_session` capability
method" wording; the guarantees (idempotent create, argv
scoped to Conspectus-constructed, no operator-typed input)
carry through unchanged. A follow-up commit annotates ADR 0057
with the substitution.

**For test call sites.** Existing tests using `FakeTmux` +
`with_tmux_runner` continue to compile because
`FakeTmux::backend_key()` returns `"tmux"` and
`with_tmux_runner` is a deprecated alias for
`with_mux_backend`. Tests that need to inject a non-tmux
backend construct their own `struct FakeBackend; impl MuxBackend
for FakeBackend { fn backend_key() { "test-*" } … }`.

## Alternatives Considered

**Keep `TmuxRunner` as the trait name; add `backend_key` as a
method.** Rejected: the rename is the point. `TmuxRunner`
suggests "the runner that runs tmux commands"; every future
backend would have to lie about its identity to satisfy the
type. The trait rename to `MuxBackend` matches how the
codebase already talks about backends (pin `mux.backend`,
`MuxSessionNode.backend`).

**Split into per-capability traits (`MuxLister`, `MuxRenamer`,
`MuxLauncher`, …) with a struct that combines them.**
Rejected. Consumers today call multiple capabilities from the
same code path (attach then optionally send-keys, list then
capture). Splitting adds ceremony without buying anything —
the `Unsupported` outcome already models "this backend can't do
this."

**Give each backend its own trait (`TmuxBackend`, `ZellijBackend`,
…) with a shared parent.** Rejected. Downstream consumers
want to route by key at runtime; different traits per backend
force a match at every call site, which is exactly what
`backend_key` + `MuxBackend` avoids.

**Fold the outcome types into one giant `MuxOutcome` enum with
tagged variants per operation.** Rejected. Method-specific
outcome types keep return sites narrow (a caller who invoked
`rename_session` should only need to match on rename
variants). A one-big-enum shape would push every call site to
match on operations it didn't perform.

**Move `discover()` onto `MuxBackend` and drop the
`TmuxDiscovery` wrapper.** Rejected for now. The wrapper
carries the provenance stamping + row-parsing logic that's
tmux-specific; folding it into the backend trait would require
every backend to reimplement provenance stamping. A future
refactor can extract the shared parts into a helper the
backend calls; today the wrapper is a clean seam.

## Open Questions Answered

- **Is `TmuxRunner` gone?** Yes. `MuxBackend` is the new name.
- **Do the outcome types rename too?** Not in this ADR. They
  stay `Tmux`-prefixed until the mechanical rename lands (a
  follow-up commit; see "What stays put" above).
- **Is the socket_name → namespace rename here?** No; deferred
  to land alongside zellij.
- **Can multiple backends share a key?** No. `backend_key`
  values must be unique across the registry, matching the
  `providers::REGISTRY` uniqueness convention (ADR 0088).

## Open Questions Deferred

- **Whether `TmuxOutcome` etc. eventually become
  `MuxSessionListOutcome` etc.** The variants are already
  backend-neutral; the rename is cosmetic. Land alongside
  H-EXT-010 or as its own mechanical commit.
- **Whether the discovery `TmuxDiscovery` wrapper collapses
  into `MuxBackend::discover(&self, ctx) -> Result<GraphFragment>`.**
  Requires deciding how to share provenance stamping across
  backends. Deferred.
- **`namespace` semantics for backends that don't have a
  socket concept.** Two options: (a) ignore the value, (b)
  return `Unsupported` when a non-`None` namespace is passed.
  Decide when the second backend lands.

## Related ADRs

- ADR 0028 (hook sidecar records) — the hook-mux-context probe
  target of H-EXT-011; ADR 0028's schema will grow a `backend`
  field once multiple backends can produce sidecars.
- ADR 0029 (lockstep session aliases) — the rename surface
  that becomes capability-gated in H-EXT-009.
- ADR 0057 (session pins) — carries the `mux.backend` field
  this ADR keys on; supersession of the tmux-specific launch
  prose is a follow-up prose commit.
- ADR 0087 (mutation envelope) — pin-launch send-keys and
  new-session stay within envelope category 3 (mux lifecycle,
  operator-initiated). No new mutations licensed by this ADR.
- ADR 0088 (provider descriptor registry) — the same
  registration convention (unique key, static registry) applies
  here. A future consolidation could unify both under one
  registry type; today they stay separate because
  provider-registration and mux-backend-registration have
  different lifetime patterns (providers are always static;
  mux backends may be config-driven).
