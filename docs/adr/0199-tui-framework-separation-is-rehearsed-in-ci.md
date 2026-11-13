# ADR 0199: The Framework's Separation Is Rehearsed In CI

## Status

Accepted. Carries out ADR 0150's consequence that moving the framework to
its own repository is "a directory lift plus a separation rehearsal".
Framework-owned in part: the framework restates the rehearsal and the
lock-file rule as its record 0071; the script, the CI job, and the recipe
are Conspectus's.

## Context

ADR 0150 says `tuikit` can leave this repository at any time. Its
independence guards check what a scan can see inside `crates/tuikit`: no
`conspectus` crate, no Conspectus vocabulary, no process or write. They
cannot see what only resolves because the crate sits two directories below
this workspace's root:

- fields the manifest inherits from `[workspace.package]`,
  `[workspace.dependencies]`, and `[workspace.lints]`;
- a relative path that climbs out of the crate (`include_str!`, a test that
  reads `../../src/...`), which resolves here and nowhere else;
- a test that assumes where the crate sits on disk;
- the root files a repository needs that the workspace supplies today:
  `Cargo.lock`, `rust-toolchain.toml`, `LICENSE`, CI, the `justfile`, and
  the Nix dev shell.

An unproven separation fails on exactly these. A one-off rehearsal would
prove the property once and then let it rot, so the rehearsal has to run on
every change.

The first rehearsal found two failures and a stale message, fixed in the
same change: the vocabulary guard read the export's `Cargo.lock`, where
`pin-project-lite` (pulled in through `tui-markdown`) contains a listed
word; a test of the source walk asserted that no component of an absolute
path is `target`, which fails when the export sits under `target/`; and the
guards' failure report named `crates/tuikit/docs/contributing.md`.

## Decision

1. **An export script.** `scripts/export-tuikit.sh` builds a standalone
   repository from `crates/tuikit` at HEAD, with its history:
   `git subtree split --prefix=crates/tuikit HEAD`, then a new repository
   that fetches the split commit by id, so this repository gains no branch
   or tag. It refuses a shallow clone, and a crate or root input with
   uncommitted changes unless `--allow-dirty` is given.
2. **The manifest stands alone.** `edition.workspace`-style package fields
   take the `[workspace.package]` value; a dependency with
   `workspace = true` takes its one-line `[workspace.dependencies]` entry
   plus the crate's own `optional`; `[lints] workspace = true` becomes the
   `[workspace.lints.<tool>]` tables as `[lints.<tool>]`; an empty
   `[workspace]` table makes the package its own workspace root, so cargo
   stops at the export even when it sits under this repository's `target/`.
   Any other inherited spelling, a member-level `features` list on an
   inherited dependency, or a path dependency that leaves the crate stops
   the rehearsal with a message (fail closed). Comments are left as
   written.
3. **Root files, from one list.** The script copies `LICENSE` and the
   toolchain's `[toolchain]` keys, and writes `.gitignore`, a CI workflow, a
   `justfile`, and a minimal `flake.nix` dev shell (Rust from
   `rust-toolchain.toml`, `git`, `just`, `python3` for the glyph script).
   The workflow has one step and the `justfile` one recipe line per entry
   of the script's `CHECKS` list, so all three run the same checks.
   `Cargo.lock` is this repository's, pruned by cargo to the crate's
   packages, and the script fails if it locks a version this repository
   does not. The crate's README is the export's root README. No
   `flake.lock` is written: CI has no Nix, and the real move locks the
   flake once (the script parses `flake.nix` and the `justfile` when `nix`
   and `just` are installed). The files are committed in the export.
4. **Nothing of this repository is reachable.** Before the checks, the
   script requires that `cargo locate-project --workspace` names the
   export's manifest; that the export's grandparent, where a path climbing
   out of the crate lands, holds none of this repository's top-level
   entries; that no `rustfmt.toml`, `clippy.toml`, or `.cargo/config.toml`
   sits above the export (Cargo's own home excepted); that `cargo tree`
   resolves the same packages and features for the crate here and there;
   and that the tree has no whitespace errors.
5. **The checks.** In the export, with its own target directory and
   `--locked`: `cargo fmt --check`; clippy with `-D warnings` over every
   target with every feature, the library and every target with the
   features off, and the library with `richtext` alone; `cargo test` with
   every feature and with the features off (doctests included); and
   `cargo doc --no-deps` with `RUSTDOCFLAGS='-D warnings'`, features off and
   every feature. Each step is timed. The doc-sync test's host checks skip,
   as designed, because no host manifest sits two directories up.
6. **A leak must fail it.** `--self-test` injects, one at a time, a test
   that `include_str!`s `../../../src/lib.rs` and a test that reads
   `../../src/lib.rs` from `CARGO_MANIFEST_DIR`. Both resolve in this
   repository's layout (the script checks that the file exists there). The
   first must fail the all-feature lint and the second the all-feature
   tests, each naming the leak; the export is restored after each.
7. **Where it runs.** The scratch directory is an argument, by default
   `target/separation-rehearsal` (git-ignored, on the build disk), and is
   removed afterwards unless `--keep` is given. The script refuses a
   directory inside the crate, one that holds this repository, a non-ignored
   one inside it, and a non-empty one that no earlier run left.
8. **CI runs it on every push and pull request.** A `separation-rehearsal`
   job in `.github/workflows/ci.yml`, beside the Rust job, clones with full
   history and runs the script with `--self-test` into `$RUNNER_TEMP`. It
   is not path-filtered: the export also depends on the root `Cargo.toml`'s
   workspace tables, `Cargo.lock`, `rust-toolchain.toml`, `LICENSE`, and the
   script, a filter would have to list them all, and a job-level path filter
   needs a third-party action.
9. **Locally, a recipe.** `just separation-rehearsal [ARGS]` runs the script
   and is named in the dev-shell banner. It is not part of `just check`,
   because it builds the crate from scratch (several minutes, a few GB).
10. **Lock files are not scanned for vocabulary.** The framework's
    vocabulary guard skips files named `Cargo.lock` and `flake.lock`: they
    list third-party package names the crate does not choose, and a
    repository of its own commits them.

## Consequences

- A change that makes the crate depend on this repository's layout fails CI
  in the rehearsal job even when every in-repository check passes.
- The CI bill grows by one job that builds the crate from scratch; it runs
  in parallel with the Rust job.
- Adding a cargo feature to the crate, or a new spelling of workspace
  inheritance to its manifest, means touching the script's `CHECKS` or its
  manifest rewrite in the same change; the rewrite fails closed.
- The real move is written down in `crates/tuikit/docs/separation.md`, and
  the export the script makes is its first step.
- Moving the framework out retires the script, its recipe, its CI job, and
  the banner line.
- The proposed Pre-Main Invariant ("the separation rehearsal passes:
  `just separation-rehearsal --self-test`, which CI runs") is left to the
  documentation story that edits `CLAUDE.md` and `AGENTS.md`.

## Alternatives Considered

- **`git filter-repo`.** Faster history rewriting, but a new tool in the dev
  shell and in CI. `git subtree` ships with Git and splits the crate's 126
  commits in well under a minute.
- **An export without history.** Simpler, but the story asks for history,
  and blame and log are part of what moves.
- **A branch for the split.** `git subtree split -b` and a clone of the
  branch is the textbook form, but it creates a ref this repository and
  every worktree share, which an interrupted run would leave behind.
  Fetching the split commit by id leaves nothing.
- **`cargo package` to make the manifest concrete.** Cargo's normalized
  manifest drops path development dependencies (the crate's dependency on
  itself for its test kit), adds generated boilerplate, and runs packaging
  rules for a crate that is not published.
- **Setting `CARGO_TARGET_DIR` or a cargo `--config` instead of a
  `[workspace]` table.** Neither stops cargo's workspace discovery from
  finding this repository's root manifest above the export.
- **A path-filtered separate workflow.** Native to GitHub, but it would
  have to list every root input and still miss a change to an input it
  forgot.
- **Copying `flake.lock`.** Pruning this repository's extra inputs needs a
  JSON tool neither CI nor the dev shell guarantees; locking is one command
  in the real move.
- **Scanning lock files with an exception.** Exceptions are per file and
  per word; a lock file's words change with every dependency update.

## Open Questions Answered

- *Where does the export go?* Under `target/` by default, not `/tmp`: the
  build takes a few GB and `/tmp` is often a small tmpfs. CI uses the
  runner's temporary directory.
- *Does the export need a README of its own?* No: the crate's README is
  already its crate-level documentation and becomes the root README; the
  script checks that it exists.
- *Is the flake checked?* It is parsed when Nix is installed. It is not
  evaluated, which would download a nixpkgs revision on every run.
