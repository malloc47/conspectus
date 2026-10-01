# ADR 0015: Library API Surface

## Status

Accepted. Amended 2026-10-01 (`H-RUST-012`): the boundary is now
enforced with visibility; see the amendment at the end.

## Context

Conspectus started as a library-first CLI crate in ADR 0007. Phases 1
through 5 have now stabilized the core graph model, local discovery,
JSON and table output, forge discovery, and declared-link stores. Phase
6 needs a contract that Atelier and future consumers can depend on
without importing arbitrary implementation details from the CLI.

The existing crate already exposes broad module paths because the CLI,
integration tests, and fixtures use the same library. Tightening that
surface abruptly would break current callers and make cross-repo
delegation harder to review. The first stable contract should therefore
name the supported entry points while preserving compatibility with
existing module paths.

## Decision

Conspectus treats the following modules as the stable public library
surface:

- `model`
- `output`
- `resolve`
- `config`
- `declared`
- `discovery`
- `discovery::git`
- `discovery::tmux`
- `discovery::forge`
- `discovery::harness`
- `discovery::atelier`
- `discovery::workspace`
- `discovery::declared`
- `discovery::cross_link`

These modules are versioned under normal semver expectations once
Conspectus publishes a library release:

- patch releases may fix bugs and add compatible fields or helpers
- minor releases may add modules, types, enum variants, and optional
  fields with documented defaults
- major releases are required for removing public items, changing
  serialized graph semantics incompatibly, or changing resolver
  precedence in a way that can alter selected relationships

The stable consumer entry points are:

- `discovery::discover_local_with`
- `discovery::discover_local_at_roots`
- `discovery::LocalDiscoveryConfig`
- `discovery::DiscoveryContext`
- `discovery::DiscoveryCaches`, which callers that run discovery
  repeatedly keep and pass in through
  `LocalDiscoveryConfig::with_caches` (ADR 0099)
- `discovery::GraphFragment`
- `discovery::merge_fragments`
- injectable process seams such as `tmux::TmuxRunner` and
  `forge::GhRunner`
- provider-specific parsers and fragment builders that are pure and
  fixture-friendly, including git probes, Atelier metadata readers,
  harness adapters, tmux parsing, forge parsing, and declared-link
  application
- `resolve::resolve_snapshot`
- `resolve::resolve_links`
- `output::render_graph_json`
- `output::table::render`
- config and declared-link read/write helpers in `config` and
  `declared`
- all graph model types needed to construct, inspect, serialize, and
  resolve `GraphSnapshot` values

The crate will also expose a curated `conspectus::api` facade that
re-exports the common consumer workflow. The facade is the preferred
documentation entry point for Atelier delegation, but existing module
paths remain supported.

The CLI module and binary entry point are internal application code and
are not part of the stable library API.

## Internal Items

Items that are public only to support tests, fixtures, or current module
organization may be marked `#[doc(hidden)]` before they can be made
private. The project should prefer narrowing new internals to
`pub(crate)` when no external caller needs them.

`discovery::harness::fixtures`, fake runners such as `FakeTmux` and
`FakeGh`, and other test builders are supported for Conspectus tests but
are not part of the consumer contract. They may remain reachable while
the crate is pre-1.0, but consumers should not build production
integrations on them.

## Consequences

- Atelier can depend on a named set of library modules instead of
  treating every public item as equally stable.
- The `api` facade can become the obvious docs landing point without
  breaking existing callers that use deeper module paths.
- Public serialized graph behavior remains the durable integration
  contract; table rendering and declared-link helpers are stable because
  they are user-facing surfaces.
- Future refactors should either keep these paths compatible or schedule
  a major-version change.

## Alternatives Considered

- **Make only `conspectus::api` public.** Rejected for Phase 6 because
  existing tests and likely Atelier migration work already use deeper
  paths. A facade is useful, but a sudden visibility collapse would add
  migration risk.
- **Declare every current `pub` item stable.** Rejected because some
  items are fixtures, fake runners, or incidental implementation
  structures.
- **Extract a separate common crate immediately.** Deferred. The stable
  surface should be proven through Atelier delegation before extraction
  creates another versioning boundary.

## Open Questions Answered

- Conspectus will keep its existing public module paths working.
- A curated `conspectus::api` facade should be added for common library
  consumers.
- CLI internals are outside the library contract.
- Semver discipline applies to the named stable modules and serialized
  graph behavior.

## Amendment: Enforce The Boundary With Visibility (2026-10-01)

Implemented in `H-RUST-012`. The binary used to compile `src/cli/` as
its own crate module, so it could reach the library only through
public paths. Every module in `lib.rs` was therefore `pub`, and the
named stable surface above existed only as documentation.

The CLI now lives in the library as `conspectus::cli`, and `main.rs`
calls `cli::run`. With that in place, `lib.rs` sorts modules into
three groups:

- **Contract**, `pub` and documented: `api`, `model`, `discovery`,
  `resolve`, `output`, `config`, `declared`, `aliases`, and `rename`.
- **Crate-internal**, `pub(crate)`: `server`, `viewer`, `tui_state`,
  `pins`, `pin_bindings`, and `pin_store_registry`.
- **Reachable but not contract**, `pub` with `#[doc(hidden)]`: `cli`
  (for `main.rs`), `tui` (the widget preview example and the replay
  tests), `snapshot` (graph.bin round-trip tests), `hook` and `filter`
  (fixture and replay tests), and `dev_scenarios` (scenario snapshot
  tests).

Hiding a module from the docs is weaker than `pub(crate)`. The hidden
modules can become `pub(crate)` once the tests that reach them move
into the crate's unit tests, or once the example switches to a
dedicated preview API. Narrowing exposed six items that only tests or
nobody used. `client_ping` was deleted. `write_last_view` and the
`PinBindingsCache` builders became `#[cfg(test)]`. The viewer's
dependency allowlists moved into its tests. `PinLaunch::is_empty` was
deleted.
