# ADR 0225: Local Checks Skip The Framework Matrix When Nothing It Builds From Changed

## Status

Accepted. Changes when `just check` runs `just tuikit-features`, the
framework's feature matrix; the matrix itself, its recipe, and its CI
steps are unchanged. Not framework-owned: the checks the framework's guide
asks its contributors to run are unchanged, and the guide only says when
the host's `just check` runs them.

## Context

`just tuikit-features` builds the framework crate alone (`-p tuikit`): four
clippy runs and two `cargo test` runs, with its cargo features off and with
`richtext` alone. A workspace build turns `richtext` on through the host,
so only these builds see the feature-off crate (ADR 0152, ADR 0181).
`just check` runs it after the workspace tests.

Its six builds resolve the framework's dependencies with feature sets of
their own, its two test runs re-run the crate's 1,185 unit tests and
compile its doctests every time, and none of it can come from the
workspace build's cache. Measured on 2026-11-12 it cost 56 s per
`just check` after a host-only edit, and 116 s after a framework edit. On
2026-11-13, after a host-only edit, it took 51 to 69 s depending on load
(the clippy runs about 1 s, the two test runs the rest), and 164 s the
first time in a worktree, when its feature sets are not yet built.

A change that touches neither the crate nor what its build reads cannot
change those results. The crate imports nothing from the host
(`crates/tuikit/tests/independence.rs`), and its build reads, from outside
its directory, only the root manifest's workspace tables, the lockfile,
and the toolchain file. The framework tests that read host files, such as
`crates/tuikit/tests/doc_sync.rs` against `docs/adr/README.md`, run in the
workspace test run, which the skip does not touch.

## Decision

1. **`just check` runs the matrix only when it could fail.** Its `check`
   recipe calls `tuikit-features-if-changed`, which runs
   `scripts/tuikit-unchanged.sh` and then either skips the matrix, printing
   that it did and why, or runs `just tuikit-features`, printing why.
2. **What counts as a change.** The script compares the merge base of HEAD
   with the local `main` branch against the working tree
   (`git diff <merge base> -- <inputs>`), which covers the commits since the
   merge base and staged and unstaged edits at once, and lists untracked
   files under the inputs. The inputs are `crates/tuikit`, the root
   `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, the cargo, clippy,
   and rustfmt configuration files a build would read from the root
   (`.cargo/`, `clippy.toml`, `.clippy.toml`, `rustfmt.toml`,
   `.rustfmt.toml`, none of which exist today), and the `justfile` that
   holds the matrix. Any difference runs the matrix.
3. **When the comparison cannot be made, the matrix runs.** Outside a git
   work tree, in a shallow clone, with no local `main` branch, or with a
   HEAD that has no merge base with it (a detached or unrelated checkout),
   the script says so and the matrix runs.
4. **`just tuikit-features` always runs the matrix**, and CI keeps running
   its commands as two steps on every push and pull request. A skip moves
   when a mistake surfaces, never whether it does.
5. **The docs say when.** `AGENTS.md`, the dev shell's banner, and the
   framework's contribution guide say that `just check` runs the matrix
   only when the change touches the crate or a root file its build reads.

## Consequences

- On a branch that has not touched the framework or the root build files,
  `just check` is shorter by the matrix, 51 to 69 s. Measured on
  2026-11-13 after a one-line host edit, in a scratch clone whose branch
  touched neither, the whole check took 125 s at load 6 to 8; with the
  matrix it had taken 233 s earlier that day, at a load rising to 20.
  Once any commit on the branch touches them, every `just check` on it
  runs the matrix, until the branch is merged.
- On `main` itself the merge base is HEAD, so only uncommitted changes
  count; a commit made on `main` without a `just check` before it relies
  on CI, as any unchecked commit does.
- A stale local `main` makes the comparison cover more commits, never
  fewer, so it can only run the matrix more often.
- Changing what the matrix builds from means updating the script's list in
  the same change.

## Alternatives Considered

- **Keep running the matrix on every check.** The cost above, for results
  that cannot change.
- **Drop the matrix from `just check`.** The operator chose to keep every
  check; a branch that changes the framework would then learn of a
  feature-gating mistake only from CI.
- **Compare against the previous commit, or only the working tree.** A
  branch whose earlier commit changed the framework would skip the matrix
  once that commit was made, so a check after it would miss the change.
- **Compare against `origin/main`.** The remote-tracking branch is as
  stale as the last fetch and is absent in a clone without a remote; the
  local `main` is what this repository's branches are rebased on and
  fast-forwarded to.
- **Let cargo decide by caching the matrix's results.** Cargo caches
  builds, not test results, and the matrix's feature sets do not share the
  workspace build's artifacts; a stamp file of the last passing inputs
  would be a cache of its own to invalidate correctly.
- **Path filters in CI.** CI keeps the full matrix on every run (decision
  4); a filter there would move a mistake past `main`.

## Open Questions Answered

- *Does a framework test that reads host files get skipped?* No: it runs in
  the workspace test run, which `just check` always runs.
- *Does this change what the framework's contributors are asked to run?*
  No. Its guide still lists the matrix's commands for a change to the
  crate; it now also says when the host's `just check` runs them.
