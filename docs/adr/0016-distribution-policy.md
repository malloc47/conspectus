# ADR 0016: Distribution Policy

## Status

Accepted. Amended by ADR 0110: the toolchain file pins an exact release,
so the MSRV is that pinned version.

## Context

Phase 6 needs Atelier to consume Conspectus as a stable library and
CLI. The repository already builds with a Nix dev shell and a
`rust-toolchain.toml` pin to the stable channel. `Cargo.toml` currently
declares version `0.1.0`, Rust 2024, and normal crates.io-compatible
metadata.

Atelier may need to migrate before Conspectus has a published
crates.io release. Future consumers should still have a clear path to a
normal registry dependency once the API contract from ADR 0015 has been
validated.

## Decision

Conspectus supports three distribution channels, in this order:

1. **crates.io releases** are the intended default for external
   consumers once Conspectus is ready to publish.
2. **Pinned git revisions** are acceptable for Atelier and other
   first-party consumers while the crate remains pre-publish or while a
   release candidate needs cross-repo validation.
3. **Path dependencies** are only for local development, workspace
   experiments, and short-lived cross-repo review branches.

Published releases must use the semver policy from ADR 0015 for the
stable library modules and serialized graph behavior. While Conspectus
is pre-1.0, compatibility-breaking library changes should still be
called out explicitly in release notes and cross-repo coordination
notes.

The MSRV is the current stable Rust toolchain required by the checked-in
`rust-toolchain.toml` and Nix dev shell at release time. Conspectus does
not promise compatibility with older stable compilers unless a future
release adds an explicit `rust-version` field and CI coverage for it.

Release cadence is demand-driven:

- publish a patch or minor release when Atelier or another consumer
  needs a stable dependency target
- publish after the normal check suite passes in the repository dev
  shell
- prefer small releases around library contract changes instead of
  batching unrelated behavior for long periods

## Compatibility With Nix

The Nix dev shell remains the source of truth for release validation.
Before publishing or asking Atelier to pin a revision, run the baseline
checks from the dev shell:

```sh
nix develop --command just check
nix develop --command cargo doc --no-deps
```

Atelier may use a pinned git dependency that points at the same commit
validated by those checks. Local path dependencies should not be
committed to long-lived Atelier branches unless the repositories are
intentionally moved into a shared workspace later.

## Consequences

- Atelier can start delegation work against a pinned revision before a
  registry release exists.
- crates.io remains the steady-state distribution path for consumers
  that are not checked out beside Conspectus.
- The MSRV policy is simple and matches the existing stable-channel
  toolchain pin.
- Release validation stays aligned with local development and CI
  expectations.

## Alternatives Considered

- **Require crates.io before Atelier work starts.** Rejected because it
  would block useful cross-repo validation of the Phase 6 API facade.
- **Use only git dependencies permanently.** Rejected because registry
  releases provide clearer versioning and a better default for external
  consumers.
- **Commit path dependencies in Atelier.** Rejected for long-lived work
  because it couples two checkout layouts and makes reproducible builds
  harder.
- **Declare a separate fixed MSRV immediately.** Deferred until there is
  CI that tests older compilers. The current Rust 2024 codebase already
  assumes a modern stable compiler.

## Open Questions Answered

- Atelier may use a pinned git revision during migration.
- crates.io is the intended default once releases begin.
- Path dependencies are local-development only.
- The effective MSRV is the stable toolchain pinned by this repository
  and available in the Nix dev shell at release time.
