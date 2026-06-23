# ADR 0039: Library API Query Feature Gate

## Status

**Superseded** by [ADR 0082](0082-retire-sqlite-persistence-and-query-surface.md).

The `query` Cargo feature this ADR defined is being phased out.
P11-010 removed `conspectus query` from the binary; P11-011a
retired the SQLite persistence layer the feature gated.
`src/query/` continues to exist as an internal renderer-support
helper (P11-011b/c/d cover the final deletion), but it is no
longer the user-facing surface this ADR's feature gate was
designed to expose. Library consumers who set
`default-features = false` get nothing more than they got
before — the feature gate's audience disappears with its
content.

Original status: Accepted.

## Context

ADR 0036 adopts SQLite as the embedded query engine for the
`conspectus query <sql>` surface. The `rusqlite` crate's bundled
feature adds a ~1MB native libsqlite3 to the build artifact. While
this is small compared to alternatives the engine-selection ADR
considered (notably DuckDB's ~50MB native lib), it is still a
mandatory native dependency that every library consumer of
`conspectus::api` would inherit by default.

ADR 0015 names the stable library surface that Atelier and future
consumers depend on. The current `conspectus::api` facade exposes
the discovery → resolve → render pipeline in pure Rust types
(`GraphSnapshot`, `discover_local_with`, `resolve_snapshot`,
`render_graph_json`, `output::table::render`). None of these need
SQLite to function; they operate on in-memory model types.

The question this ADR settles: do library consumers inherit the
SQLite engine implicitly when they take a dependency on `conspectus`,
or do they opt in?

## Decision

The SQLite engine is gated behind a Cargo feature named **`query`**.

Concretely:

- The `conspectus` crate's `Cargo.toml` declares a `query` feature
  that pulls in `rusqlite` with the `bundled` sub-feature and any
  query-only modules (`src/query/` introduced in P9-002 onwards).
- Library consumers (Atelier, future crates) build by default
  without the engine. Their dependency line stays
  `conspectus = "..."`; nothing changes for them at v1.
- Consumers who want the engine opt in:
  `conspectus = { version = "...", features = ["query"] }`.
- The `conspectus` binary always builds with `--features query` so
  the user-facing `conspectus query <sql>` command is always
  available in shipped binaries. The binary's `[features]` default
  list in `Cargo.toml` includes `query`.
- Code that depends on rusqlite lives behind `#[cfg(feature = "query")]`.
  The `conspectus::api` facade re-exports the engine-related types
  (e.g. the connection builder, the loader entry point) only when
  the feature is on; without the feature, the symbol is absent
  rather than a stub.
- No engine-dependent code leaks into the always-on stable surface
  named in ADR 0015. The pure pipeline
  (`discovery → resolve → render`) keeps working unchanged.

The default-off-for-library, default-on-for-binary configuration
means the binary you install gets the full feature set, while
library consumers who do not want SQLite are not paying for it. The
release-binary size delta from the feature is captured in ADR 0040.

## Consequences

- ADR 0015 is amended: `conspectus::api` now has a *feature-gated
  appendix* (the engine surface) alongside its existing always-on
  exports. The amendment is documented in this ADR rather than by
  editing ADR 0015 in place, per the project's "supersede by
  reference" convention.
- Library consumers see no API change unless they opt in. Atelier's
  current integration continues to work without action.
- The `query` feature joins the small set of conspectus features
  that materially change the build matrix. CI must build both
  configurations (`default` and `default + query`) so neither
  regresses silently.
- The `conspectus::api` documentation gains a section explaining
  the feature gate and pointing consumers at the engine entry
  points.
- A future `conspectus-core` crate split (mentioned in ADR 0015
  alternatives as deferred) becomes easier: the feature gate is the
  natural seam.

## Alternatives Considered

- **D1: SQLite always-on (no feature gate).** Simplest from a code
  perspective; library consumers inherit ~1MB of native lib whether
  they want it or not. Rejected for principled separation of
  concerns even though the size cost is genuinely small: keeping
  the gate available preserves the option of a low-resource
  embedded build, and it enforces the discipline that the in-Rust
  pipeline (discovery → resolve → render) is the always-on ground
  truth while SQL is a consumer of it. The cost case is close but
  the principled case is clearer.
- **D2 (this ADR).** Cargo feature gate. Adopted.
- **D3: Separate `conspectus-query` crate that depends on
  `conspectus`.** Cleanest separation; the query surface lives in a
  sibling crate. Rejected at v1 because it duplicates publication
  ceremony for a feature that may not warrant its own release
  cadence. Worth revisiting if a second crate (an MCP server, a
  separate query daemon) ever wants the engine independently — the
  current feature gate is the natural seam from which D3 becomes a
  later refactor rather than an upfront cost.

## P9-001 Spike Amendment

The P9-001 implementation spike surfaced one factual correction to
this ADR's wording: `rusqlite` is **already** an unconditional
dependency of `conspectus`, used by `src/hook.rs` (the hook sidecar,
ADR 0028) and `src/discovery/harness/opencode.rs` (the OpenCode
session-store adapter, ADR 0013). The `query` Cargo feature
therefore does **not** "pull in `rusqlite`" — the dependency line
stays declared at the top level and remains always-on. The feature
gates only the new query-engine *code* under `src/query/` and the
forthcoming `conspectus query` subcommand registration.

The practical implications:

- The release-binary delta from enabling the feature is ~5.7 KB
  (measured on `x86_64-linux-gnu` in P9-001), not the
  ~1 MB the original ADR text implied. The 1 MB native library
  cost is paid regardless of the feature.
- The ADR-D recommendation (`D2` — feature gate) is still correct
  in spirit: gating the new query-engine code is the principled
  separation of concerns that this ADR captures. The cost-case
  argument simply collapses to "near-zero" rather than "small."
- The Cargo manifest declares `query` as the default feature so
  library consumers who explicitly `default-features = false`
  exclude only the new query-engine code, not the broader SQLite
  dependency. A future `conspectus-core` split (named in the
  open questions below) would be the place to also unwind the
  unconditional dependency if a truly SQLite-free build line ever
  becomes necessary.

This amendment does not change the ADR's decision; it corrects the
wording on what the feature gate technically does.

## Open Questions Answered

- **What does Atelier need to do?** Nothing. Atelier's current
  integration uses the always-on stable surface; it does not need
  the `query` feature.
- **How does CI verify both configurations?** A matrix entry that
  builds and tests with `--no-default-features` (or whatever
  combination represents "no query feature") alongside the default
  configuration. P9-001 includes this CI work as part of the spike.
- **Does the `conspectus query <sql>` CLI command vanish in builds
  without the feature?** Yes. The subcommand registration is itself
  gated, so `--features query` is required to surface it. The
  binary's default feature list includes `query`, so end users do
  not notice. Custom-built minimal binaries can omit it cleanly.
- **What about `node show`, `table`, the TUI, JSON dump?** All
  always-on. They use the in-Rust pipeline and never depend on the
  engine. This is the discipline that the feature gate enforces.
- **Does this conflict with ADR 0015's "stable library surface"
  list?** No. Every module named in ADR 0015 stays always-on. The
  feature gate adds new query-engine entry points; it does not
  remove or move existing ones.
