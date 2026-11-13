# ADR 0223: The Dev Profile Keeps Line Tables And Optimizes Dependencies

## Status

Accepted. Sets the `dev` profile, and through it the `test` profile, in
the root `Cargo.toml`; the `release` profile is unchanged. Adds no tool.
The framework's export ([ADR 0199](0199-tui-framework-separation-is-rehearsed-in-ci.md))
does not carry the profile.

## Context

A one-line edit to a function body in the `conspectus` library cost about
40 s of incremental test build and 75 s of nextest run, measured on
2026-11-12 and again on 2026-11-13 on the 8-vCPU, 16 GB dev VM. Two costs
dominated:

- **Linking debug info.** The edit relinks every test binary that depends
  on the library: 48 binaries, 8.0 GB in all, most of them about 250 MB.
  Over nine tenths of each is DWARF (`html_snapshots`: 199.5 MB of 216 MB
  in `.debug_*` sections on 2026-11-12). The linker is already LLD, Rust's
  default on `x86_64-unknown-linux-gnu`, so the time goes to moving debug
  info, not to a slow linker.
- **Unoptimized dependencies.** The randomized sweeps and the TUI
  consistency suites spend most of their CPU in `ratatui`,
  `unicode-width`, and the other third-party crates, all built at
  `opt-level = 0`. With them at `opt-level = 1`, the host's consistency
  tests dropped from 115 s to 48 s of CPU (2026-11-12).

Full debug info serves one use: a debugger that shows local variables and
types. Panic locations, backtraces, and test failure reports need only the
line tables.

## Decision

1. **Line tables only.** `[profile.dev] debug = "line-tables-only"`. Every
   crate, the workspace's and the dependencies', keeps the tables that map
   code to file and line, and nothing more. `cargo test` and nextest build
   with the `test` profile, which inherits `dev`.
2. **Dependencies at `opt-level = 1`.** `[profile.dev.package."*"]
   opt-level = 1` covers every package that is not a workspace member.
   The `"*"` override outranks `build-override`, so the dependencies' build
   scripts and procedural macros are optimized too. `conspectus` and
   `tuikit` stay at `opt-level = 0`, so an edit to them compiles as fast as
   before and a debugger steps through them line by line. Generic code from
   a dependency is compiled in the crate that uses it, at that crate's
   level.
3. **The root manifest holds it.** Cargo reads profiles only from the
   workspace root. The framework's export does not carry them: it rewrites
   only the workspace tables the crate inherits (package fields,
   dependencies, lints), a library's profiles are ignored by the programs
   that depend on it, and a separated framework chooses its own. The
   rehearsal builds the export with Cargo's default profile, as a new
   repository would.
4. **Debugging with locals is an override.** Whoever needs a debugger with
   local variables builds with
   `CARGO_PROFILE_DEV_DEBUG=true CARGO_TARGET_DIR=target/full-debug cargo ...`,
   in a target directory of its own so the shared one is not rebuilt twice.

## Consequences

Measured on 2026-11-13 on the same machine, with the same body-only edit
(a `std::hint::black_box(0_u8);` at the top of `copy_to_clipboard` in
`src/tui/clipboard.rs`), the operator's other sessions running throughout:

| | Before | After |
| --- | --- | --- |
| Incremental test build (`cargo test --no-run`) | 40.8 s (92 s user, 37 s sys) at load 0.8 | 28.2 s (55 s user, 23 s sys) at load 9 |
| Test binaries relinked | 48, 8.0 GB | 48, 2.9 GB |
| `html_snapshots` binary | 258 MB | 92 MB |
| nextest run | 74.5 s (178 s user, 106 s sys) at load 6 to 8 | 70.1 s (111 s user, 100 s sys) at load 10 to 12 |
| nextest run inside `just check` | 72.3 s at load 8 | 62.6 s at load 5 |
| Whole `just check` | 330 s (575 s user, 215 s sys) at load 8 | 306 s (419 s user, 221 s sys) at load 5 |
| Cold build of every test target | 150 to 155 s (scratch directories, full debug info) | 177 s (721 s user) |

- Linking moves a third of the bytes it did, and the tests' user CPU falls
  by about two fifths. The nextest run's wall time is now bound by
  starting 5,681 test processes (its system time is unchanged), so it
  gains least under load.
- Backtraces keep file and line for the panic and for every frame, in the
  workspace's code and in the standard library's (checked with a
  deliberately panicking test under `RUST_BACKTRACE=1`).
- A debugger shows no local variables or types; it still breaks on a
  function or a file and line, and steps.
- A cold build, paid once per worktree and again after a dependency or
  toolchain bump, takes about 25 s longer here (90 s longer on 2026-11-12).
- CI builds with the same manifest, so its tests speed up the same way
  once its dependency cache is rebuilt.
- No check is dropped.

## Alternatives Considered

Each was measured on 2026-11-13 in a fresh scratch target directory, with
the same edit, at load 6 to 11:

- **`split-debuginfo = "unpacked"` with full debug info.** Incremental
  test build 29.5 s (65 s user), 2.3 GB of binaries (`html_snapshots`
  74 MB), nextest 72.8 s, a fresh target directory of 6.9 GB. It links as
  fast as line tables and keeps local variables for a debugger, which
  reads them from the `.dwo` files beside the objects; backtraces keep file
  and line. It still generates and writes full DWARF on every compile, and
  it does nothing for the tests' CPU on its own. It is the closest
  alternative: if a debugger with locals becomes routine, switching is
  `debug = true` and `split-debuginfo = "unpacked"` in place of decision 1,
  with decision 2 kept.
- **Debug info off for dependencies only.** Incremental test build 30.7 s,
  5.9 GB of binaries (`html_snapshots` 196 MB), a fresh target directory of
  11 GB. Most of the debug info describes the workspace's own crates,
  including the generic dependency code instantiated in them, so this cut
  only a quarter of the size.
- **A faster linker.** LLD is already in use. mold is the remaining step:
  a new tool in the dev shell and CI, which would need a decision of its
  own, for a link whose cost scales with the debug info it copies, which
  decision 1 cuts by two thirds. It would not touch the tests' CPU or the
  disk.
- **Dependencies at `opt-level = 2`.** Cold build 204 s against 177 s at
  `opt-level = 1`, and the same incremental build, since dependencies are
  not rebuilt by an edit. Its nextest run (117 s, with twice the usual
  system time) was taken under contention and shows nothing either way;
  the gain over `opt-level = 1` for the dependency code these tests run is
  small, and every cold build pays for it.
- **No debug info at all.** A panic still names its location, but
  backtrace frames lose file and line, which test failures need.
- **Optimizing the workspace members too.** Slower compiles of the crate
  being edited, which is the cost this decision exists to cut.

## Open Questions Answered

- *Does the exported framework manifest carry the profile?* No (decision
  3). `just separation-rehearsal --self-test` passes with the profile in
  place.
- *Does CI change?* Only in speed: it reads the same manifest.
