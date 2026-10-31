# Leaving the host repository

The crate is built to leave the repository that hosts it by a directory
lift: its history, sources, tests, and pages move as they are, and the few
root files its host's workspace supplies today are written for it. The host
rehearses that move on every change, so the claim stays true. This page
says what stands alone already, what the rehearsal checks, how to read a
failure, and how to do the real move, step by step. The decision is record
[0071](decisions/0071-the-separation-is-rehearsed.md).

Contents:

- [What stands alone already](#what-stands-alone-already)
- [The rehearsal](#the-rehearsal)
- [What the export changes](#what-the-export-changes)
- [When the rehearsal fails](#when-the-rehearsal-fails)
- [The real move](#the-real-move)

## What stands alone already

- Sources, tests, goldens, pages, scripts, and the decision log live in the
  crate's directory and refer to one another by paths inside it.
- The README is the crate-level documentation and becomes the new
  repository's README.
- The independence guards keep the host's crates, words, processes, and
  writes out ([contributing.md](contributing.md#independence-guards)).
- The doc-sync test holds the decision log to the host's records only while
  the host's workspace manifest sits two directories up and names the crate
  as a member. Alone, those checks print a note and pass
  ([contributing.md](contributing.md#the-pages-and-the-code-stay-in-step)).

What the host supplies sits outside the directory: the manifest fields the
crate inherits from the workspace (edition, license, lint levels, dependency
versions), the lockfile, the toolchain file, the license, CI, the task
runner, and the dev shell.

## The rehearsal

The host repository keeps an export script that makes the move into a
scratch directory and checks the result there, and its CI runs it, with the
self-test below, on every change. In the host this crate grew in, the
script is `scripts/export-tuikit.sh` and its task-runner recipe is
`just separation-rehearsal`. The script:

1. Splits the crate's history out of the host's (`git subtree split` with
   the crate's directory as the prefix) into a new repository, which
   fetches the split commit by its id, so the host gains no branch.
2. Rewrites the manifest so it stands alone
   ([What the export changes](#what-the-export-changes)), and copies the
   host's lockfile, pruned by cargo to the crate's packages, so the export
   builds the versions the host builds.
3. Writes the root files and commits them in the export: the toolchain
   file, the license, `.gitignore`, a CI workflow, a `justfile`, and a
   minimal `flake.nix` dev shell. The workflow and the `justfile` run the
   checks of step 5, from the one list the script keeps.
4. Checks that nothing of the host is reachable from the export: cargo
   takes the export's own manifest as its workspace root; the export's
   grandparent, where a relative path that climbs out of the crate lands,
   holds none of the host's top-level entries; no `rustfmt.toml`,
   `clippy.toml`, or `.cargo/config.toml` sits in a directory above it
   (Cargo's own home excepted); the export resolves the same packages with
   the same features the host resolves for the crate; and the tree has no
   whitespace errors.
5. Runs the checks in the export, each one timed: formatting; clippy with
   warnings denied over every target with every feature, the library and
   every target with the features off, and the library with `richtext`
   alone; the tests, doctests included, with every feature and with the
   features off; and the docs with warnings denied, with the features off
   and with every feature.

With `--self-test` it then injects two leaks into the export, one at a
time, and requires each to fail where it should: a test that includes
`../../../src/lib.rs` fails the first lint, and a test that reads
`../../src/lib.rs` from the crate's directory fails the tests with every
feature. Both paths resolve in the host's layout, so every check inside
the host passes with them. The export is restored after each.

The scratch directory is the script's argument and defaults to
`target/separation-rehearsal` in the host, which git ignores. It is removed
after the run unless `--keep` is given. The export builds into its own
`target/` there, a few GB with every feature, clippy, and docs, so the
directory belongs on a disk with room rather than a small in-memory file
system. CI passes a directory in the runner's temporary space, and its clone
fetches the whole history (a fetch depth of 0), because the export keeps
it.

## What the export changes

The manifest is the one file of the crate the export edits:

- A package field inherited from the workspace (`edition.workspace = true`)
  takes the workspace's value.
- A dependency with `workspace = true` takes the workspace's one-line entry,
  with the crate's own `optional` added.
- `[lints]` with `workspace = true` becomes the workspace's lint tables, one
  `[lints.<tool>]` table per tool.
- An empty `[workspace]` table makes the package its own workspace root, so
  cargo does not look above the repository for one, even when the
  repository sits inside another project's directory.
- The crate's dependency on itself (`path = "."`, for its test kit) stays.
- Comments stay as written, so the ones that mention the workspace's tables
  read stale in the export; the real move rewords them.

Anything else stops the rehearsal with a message instead of exporting a
manifest that only resolves inside the workspace: an inherited dependency
whose workspace entry is a table of its own, an inherited dependency that
adds a `features` list, any other inherited spelling, and a path
dependency that leaves the crate.

No `flake.lock` is written: the rehearsal's CI has no Nix, and the real move
locks the flake once. The script parses `flake.nix` and the `justfile` when
`nix` and `just` are installed.

## When the rehearsal fails

| The rehearsal reports | What it means | The fix |
| --- | --- | --- |
| an inherited spelling that is not rewritten | the manifest inherits a field in a form the export cannot make concrete | spell it as the other entries are, or teach the export script the form |
| a step fails on a missing file | code, a test, or a page includes or reads a file outside the crate | move the file into the crate, or have the caller pass the content or the path in |
| a test about paths fails | a test assumes where the crate sits on disk | compare paths below the crate's root, never whole paths |
| the vocabulary guard fails on a root file | a file the export writes uses a word of the guard's list | reword the export script's template |
| a configuration file would configure the export | a `rustfmt.toml`, `clippy.toml`, or cargo configuration above the scratch directory would apply to the export | choose a scratch directory outside that tree |
| a top-level entry of the host is reachable | the scratch directory sits where a path climbing out of the crate finds the host's files | choose a scratch directory deeper or elsewhere |
| other packages or features resolve | the manifest rewrite changed what a dependency brings in | read the difference the script prints, and fix the rewrite |
| a check fails that passes in the host | the crate depends on something only the workspace supplies | fix the crate, as for any failing check |

## The real move

1. **Start green.** Land the open changes to the crate, then run the
   rehearsal with its self-test on the commit that will move:
   `just separation-rehearsal --self-test`.
2. **Export for real.** Run the export script with `--keep` and a directory
   outside the host repository. The export it leaves at `<directory>/tuikit`
   is the new repository: the crate's history, a commit that adds the root
   files, and the pruned lockfile. By hand, the history step is
   `git subtree split --prefix=<the crate's directory> HEAD`, which prints a
   commit; then `git init`, `git fetch <host repository> <commit>`, and
   `git switch -c main FETCH_HEAD` in the new directory, and the manifest
   edits above.
3. **Finish the root files.** Reword the manifest comments that describe
   the host's workspace. Run `nix flake lock` and commit `flake.lock`
   (the guards skip lock files). Change the README's dependency lines from
   a `path` in the host to the form hosts will use, a `git` dependency on
   the new repository. If the crate is renamed now, the name is the
   package name, the README's title, and the pages' `tuikit::` paths.
4. **Publish it.** Create the repository, push `main`, and confirm its CI,
   the workflow the export wrote, passes.
5. **Move the host onto it.** In the host: replace the path dependencies
   (the normal one and the development one, each with its features) with
   `git` dependencies on a fixed commit or tag; delete the crate's
   directory and its workspace member; keep the host's `ratatui` at the
   version the framework declares (see the README); drop the crate's trees
   from the host's source-scanning tests and move their minimums
   ([contributing.md](contributing.md#repository-wide-hygiene-scans)); and
   delete the export script, its recipe, and its CI job. Run the host's
   checks.
6. **Hand over the decision log.** From the new repository's first commit
   its log is canonical (record [0070](decisions/0070-the-decision-log.md)):
   the host's records that the log restates keep a pointer to the
   framework's record, and the host numbers in the records' Origin sections
   become history. The doc-sync test's host checks already skip; delete
   them, and the index's table of host records not included, when they
   stop being useful.
7. **Keep the guards.** The independence guards stay: the word list still
   keeps the former host's vocabulary out of a crate that other hosts will
   use, and the dependency allowlist still needs a record for each new
   crate.
