# ADR 0110: Pinned Rust Toolchain

## Status

Accepted. Amends ADR 0007 (the toolchain pin) and ADR 0016 (the MSRV
definition).

## Context

ADR 0007 asked for a `rust-toolchain.toml` that pins the stable
toolchain, and ADR 0016 defines the MSRV as the toolchain that file and
the Nix dev shell provide at release time. In practice the two sources
disagreed:

- `rust-toolchain.toml` said `channel = "stable"`, so rustup users and CI
  (`dtolnay/rust-toolchain@stable`) ran whatever stable Rust was newest.
- The Nix dev shell took `rustc`, `cargo`, and `clippy` from nixpkgs, so
  it ran whatever version the locked nixpkgs revision carried: 1.94 in
  October 2026.

When Rust 1.99 shipped, its clippy flagged 15 sites that the dev shell's
1.94 accepted. `just check` stayed green locally while CI went red, and
the failure masked several unrelated CI-only test problems behind it.
Every Rust release can repeat this, because new clippy lints land with
the compiler.

## Decision

`rust-toolchain.toml` pins an exact Rust release and is the single source
of truth for the toolchain everywhere:

1. **The file pins a version.** `channel` names a release (`"1.99.0"`),
   not `stable`, and lists the `clippy` and `rustfmt` components. Rustup
   users get exactly that toolchain from the file.
2. **The dev shell reads the file.** The flake takes the toolchain from
   `rust-overlay`'s `fromRustupToolchainFile`, adding `rust-src` and
   `rust-analyzer` for editors. The binaries are the official rustup
   builds, fetched through Nix.
3. **CI reads the file.** The workflow parses `channel` from the file and
   installs that version with `dtolnay/rust-toolchain`.
4. **Bumps are deliberate.** To move to a new release, change `channel`,
   update the `rust-overlay` input if it predates the release, fix
   whatever the new clippy reports, and land it as one change.

## Consequences

- The dev shell, CI, and rustup users run the same compiler and clippy, so
  `just check` predicts CI again.
- New lints arrive only when someone bumps the pin, not on Rust's release
  schedule.
- Per ADR 0016 the effective MSRV is the pinned version.
- The flake gains a `rust-overlay` input (its `nixpkgs` follows ours). The
  first `nix develop` after a bump downloads the new toolchain instead of
  using nixpkgs' cache.
- `cargo-nextest` still comes from nixpkgs in the dev shell and from the
  latest release in CI. It runs tests rather than compiling or linting
  them, so version skew there does not change results the way clippy
  drift did.

## Alternatives Considered

- **Keep floating on stable.** Rejected: CI breaks on Rust's release
  schedule, independent of any change to the code, and the dev shell
  cannot float the same way.
- **Pin CI to the nixpkgs compiler version.** No new input, but the
  version is tied to whatever nixpkgs carries, and a bump means editing
  both `flake.lock` and the workflow or toolchain file in step.
- **Run CI inside the Nix dev shell.** Exact parity for every tool, but CI
  would install Nix and build the shell on each run, including Backlog.md
  from source without a binary cache.
- **fenix `fromToolchainFile`.** Equivalent result, but it needs a
  toolchain hash recorded and updated on every bump; `rust-overlay` does
  not.
