# ADR 0040: Distribution Policy Amendment for SQLite

## Status

**Superseded** by [ADR 0082](0082-retire-sqlite-persistence-and-query-surface.md).

The distribution carveouts this ADR negotiated for bundled
libsqlite3 are no longer load-bearing. CSP-448.01 retired SQLite
as the persistence layer; the remaining internal usage
(`query::materialize_snapshot` for output rendering) does not
need a bundled C compile to be the user-facing default.
`rusqlite` itself stays in the dep tree for the OpenCode harness
adapter and the hook sidecar (which depend on it independently
of `src/query/`); the `bundled` feature and binary-size
carveouts this ADR set up are due to be re-evaluated as part of
CSP-448.04's final SQLite cleanup. ADR 0016's original "single
static binary, no runtime deps" property is reinforced.

Original status: Accepted.

## Context

ADR 0036 adopts SQLite as the embedded query engine and pins
`rusqlite` with its `bundled` feature mandatory. The bundled feature
compiles libsqlite3 from source and links it statically into the
release artifact, adding a ~1MB native delta to the binary.

ADR 0039 gates the engine behind a `query` Cargo feature so library
consumers opt in; the `conspectus` binary always builds with the
feature on so the shipped CLI always includes `conspectus query
<sql>`.

ADR 0016 sets the distribution policy: three channels in order
(crates.io releases, pinned git revisions, path dependencies for
local development), MSRV tied to the Nix dev shell's pinned
`rust-toolchain.toml`. ADR 0016 was written under the assumption of
a small, pure-Rust release artifact. It does not yet address native
dependencies or feature-gated build matrices.

This ADR amends ADR 0016 to capture the SQLite-specific
implications. It is a paragraph-sized amendment rather than a
rewrite — the existing distribution channels and semver policy
remain in force.

## Decision

ADR 0016's distribution policy is amended as follows.

### Build configurations and channels

- The **shipped binary** built for end users via the Nix flake or
  `cargo install conspectus` always builds with the `query` feature
  on. This is the default-on configuration for the binary target
  declared in `Cargo.toml`'s `[features]` table.
- The **library crate** ships without the `query` feature by
  default. Library consumers who want the engine opt in per
  ADR 0039.
- Pinned git revisions and path dependencies continue to work as
  ADR 0016 describes. Consumers picking those channels still
  control whether they enable the feature.

### MSRV and toolchain

- `rusqlite >= 0.39` (the version floor from ADR 0036) follows the
  "latest stable Rust at time of release" policy. The Conspectus
  `rust-toolchain.toml` already pins the stable channel; no MSRV
  change is required at v1.
- The CI matrix gains an entry that builds and tests with the
  `query` feature enabled. Both `default` and `default + query`
  configurations must pass the baseline checks
  (`just check`, `cargo doc --no-deps`) before a release.

### Native dependency disclosure

- `Cargo.toml` and the crate's README explicitly note that enabling
  the `query` feature compiles a bundled libsqlite3 ≥ 3.51.3 and
  adds approximately 1MB to the resulting binary on typical
  platforms (x86_64-linux-gnu, aarch64-apple-darwin, x86_64-pc-
  windows-msvc). The disclosure makes the cost explicit for
  consumers comparing dependency footprints.
- A CI assertion (introduced in CSP-271) fails the build if the
  bundled libsqlite3 version regresses below 3.51.3. This
  protects against silent downgrades that would expose the WAL-
  reset corruption bug class.

### Release cadence

- ADR 0016's demand-driven release cadence is unchanged. The
  query-feature commits join the normal release pipeline.
- The first release that ships `conspectus query <sql>` includes a
  release-note paragraph naming SQLite, the bundled libsqlite3
  version floor, and the size delta. Subsequent releases need
  re-disclose only if either changes.

### Nix dev shell

- The Nix dev shell continues to be the source-of-truth for
  baseline checks (ADR 0016 §"Compatibility With Nix"). The dev
  shell already has the C compiler needed to compile bundled
  libsqlite3; no nix-side changes are needed beyond verifying
  this in the CSP-271 spike.

## Consequences

- The shipped Conspectus binary grows by ~1MB on typical platforms.
  This is small in absolute terms and small relative to the
  rejected alternatives (DuckDB at ~50MB) but still real.
- The CI build matrix doubles for the touched configurations
  (`default`, `default + query`). This is offset by the engine
  feature being optional: library consumers who do not enable it
  see no change in their downstream build.
- A CI assertion on libsqlite3 ≥ 3.51.3 protects the WAL-reset
  corruption guarantee. The assertion's failure mode is a clear
  build-time error rather than a silent runtime risk.
- The release-note discipline established here applies for any
  future SQLite version-floor bumps that change user-visible
  behavior (added pragmas, changed JSON1 semantics, etc.).
- ADR 0016's distribution channels (crates.io, pinned git, path)
  remain unchanged; this ADR is additive.

## Alternatives Considered

- **No amendment; treat SQLite as a transparent dependency.**
  Rejected because the native build, the bundled version floor,
  and the CI matrix change are real operational details that
  belong in distribution policy rather than spread across release
  notes.
- **Wholesale ADR 0016 rewrite.** Rejected as disproportionate.
  The existing channels, semver policy, and Nix-as-source-of-truth
  remain correct; the SQLite addition is narrow.
- **`DUCKDB_DOWNLOAD_LIB`-style dynamic linking model.** Not
  applicable to SQLite directly, but worth noting: rusqlite does
  support a system-library mode (`rusqlite` without `bundled`).
  Rejected at v1 because the `bundled` feature dodges version-skew
  bugs (the WAL-reset corruption class being the canonical
  example) and keeps the release configuration deterministic
  across platforms.

## Open Questions Answered

- **What about minimal builds (e.g. constrained embedded targets,
  static `musl` builds)?** Library consumers who target those
  environments build without the `query` feature and pay no
  native-dependency cost. The shipped binary uses the
  default-feature-on configuration; users producing custom builds
  for constrained targets disable the feature themselves.
- **Does this change the MSRV?** No. rusqlite tracks the latest
  stable Rust, which is what Conspectus already pins. If a future
  rusqlite release bumps its MSRV ahead of Conspectus's pin, the
  conflict is resolved by updating the `rust-toolchain.toml` pin
  per the existing ADR 0016 process; no new policy is needed.
- **Does the size delta justify shipping prebuilt binaries?**
  Possibly, but that is a separate distribution-channel decision
  (`CSP-106` in `docs/backlog.md`). This ADR captures the
  amendment in scope; prebuilt-binary publication is tracked
  elsewhere.
- **Could a future `conspectus-core` crate split provide a
  zero-dependency library line?** Yes, and ADR 0039's open
  questions name this as the natural follow-up if a no-engine
  consumer emerges. The current feature-gate scheme is the
  precursor seam.
