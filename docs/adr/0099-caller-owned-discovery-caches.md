# ADR 0099: Caller-Owned Discovery Caches

## Status

Accepted. Implemented as `CSP-563`: the cache types are in
`src/discovery/memo.rs`. The daemon (`src/server/mod.rs`) and the TUI
(`RunConfig::discovery_caches`) each own one `DiscoveryCaches`.

## Context

ADR 0091's idle-cost work added nine caches to discovery: the git
probe cache, the tmux, zellij, and forge fragment TTL caches, the
per-file Claude Code, Codex, and OpenCode session scans, the codex-log
query cache, and the mux/harness slice fingerprint that gates the
process-tree walk. Each was a process-global `static Mutex<...>`.
That was the quickest way to keep results across daemon cycles. It
had costs:

- Tests that observed a cache had to share it with every other test
  in the process. Four serial `*_TEST_LOCK` mutexes and a set of
  `reset_*_for_tests` functions kept them apart, and two of those were
  `pub #[doc(hidden)]` so that integration tests could reach them.
  Tests that forgot the lock could read another test's cached
  fragment. The cfg(test) spawn and query counters were also global,
  so a cache-hit assertion could flake under `cargo test` when an
  unrelated test probed git at the same time.
- A library consumer that ran discovery for two unrelated workspaces
  shared one cache between them, and nothing could discard it.
- The fingerprint behind "skip the `/proc` walk when nothing changed"
  was a hidden global whose state depended on what earlier calls in
  the process had done.

The caches themselves are correct and worth keeping (ADR 0091's
retrospective measured the wins). Only where they live is the
problem.

## Decision

Move the caches into one `discovery::DiscoveryCaches` value owned by
whoever runs discovery:

- `DiscoveryCaches` is a public type with no public fields and a
  `Default` impl. Its fields are `pub(crate)` and use the small
  helpers in `discovery/memo.rs`: `TtlCache`, `StampedMap`, and
  `FileStamp`.
- `LocalDiscoveryConfig` carries an `Arc<DiscoveryCaches>`.
  `from_env()` and `empty()` start with empty caches, and
  `with_caches` passes in existing ones. `discover_local_warm_with`
  puts the caches on the `DiscoveryContext`, and adapters reach them
  through `context.caches()`.
- The daemon creates one `DiscoveryCaches` at startup and shares it
  across every class thread and `refresh` request through the context
  value that the scheduler threads and socket handlers share. The TUI
  keeps one in `RunConfig` for the whole session, and clones of the
  config share it. One-shot CLI commands start with empty caches, which is
  what a new process got before.
- `GitProbe::probe` no longer caches. `GitProbe::probe_cached` takes
  the caches and is what discovery calls.
- Tests build their own `DiscoveryCaches` and pass it in. Cache-hit
  tests count misses on that instance with cfg(test) counters (inserts
  into a `StampedMap`, SQLite queries in the codex-log cache). No test
  locks or reset hooks remain.

## Consequences

- Daemon and TUI behavior is unchanged. They keep their caches across
  cycles as before, and the daemon still starts with empty caches and
  an unset process-tree fingerprint.
- Two `DiscoveryContext` values built separately no longer share
  results. A caller that wants reuse has to pass the same `Arc`.
- `DiscoveryContext` no longer implements `PartialEq`/`Eq`, since
  comparing caches is meaningless. Nothing compared contexts.
- `apply_codex_log_attribution` and `AuxAttributionContext` take the
  caches, which changes their public signatures. This is acceptable
  before the first library release (ADR 0015) and is folded into the
  public-surface narrowing in `CSP-565`.
- Test isolation no longer depends on process isolation, so plain
  `cargo test` and nextest behave the same.

## Alternatives Considered

**Keep the globals and only deduplicate them.** This is step (a) of
`CSP-563` and landed first. It removed the repeated get, store, and
reset code but left the test locks and the hidden shared state.

**Cache inside each provider instance.** `discover_local_warm_with`
rebuilds the providers on every call, so the caches would still have
to come from the caller. The git probe cache is also shared by five
providers and the observed-cwd pass, so it can't belong to any one of
them.

**Pass caches as a separate argument to `DiscoveryProvider::discover`.**
This would change the trait and every implementation for something
the context already carries. The context is already the per-run
environment that adapters read.

## Related ADRs

- ADR 0015 (library API surface): adds `DiscoveryCaches` and
  `LocalDiscoveryConfig::with_caches` to the discovery entry points.
- ADR 0038 (serve daemon and per-class scheduler): the daemon owns
  the caches.
- ADR 0091 (serve idle cost): introduced the caches this ADR
  relocates.
