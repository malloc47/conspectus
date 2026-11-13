#!/usr/bin/env bash
# Says whether `just check` may skip the framework's feature matrix
# (`just tuikit-features`, ADR 0225).
#
#   scripts/tuikit-unchanged.sh
#
# Exits 0, printing why, when neither the working tree nor a commit since
# the merge base with `main` touches what the matrix builds from: the crate,
# the root manifest, the lockfile, the toolchain file, the cargo, clippy,
# and rustfmt configuration at the root, and the justfile that holds the
# matrix. Exits 1, printing why, when one does, and when the comparison
# cannot be made: no git work tree, a shallow clone, no `main` branch, or a
# HEAD with no merge base with it. The matrix's own results cannot change
# when nothing it reads changed: the crate imports nothing from the rest of
# the repository (crates/tuikit/tests/independence.rs).

set -uo pipefail
unset CDPATH

inputs=(
  crates/tuikit
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  .cargo
  clippy.toml
  .clippy.toml
  rustfmt.toml
  .rustfmt.toml
  justfile
)

changed() {
  printf '%s\n' "$*"
  exit 1
}

cd "$(dirname "${BASH_SOURCE[0]}")/.." || changed "cannot find the repository root"
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || changed "not a git work tree"
[[ $(git rev-parse --is-shallow-repository 2>/dev/null) == false ]] ||
  changed "the clone is shallow"
git rev-parse --verify --quiet 'main^{commit}' >/dev/null || changed "there is no main branch"
base=$(git merge-base HEAD main 2>/dev/null) || changed "HEAD has no merge base with main"
short=$(git rev-parse --short "$base")

# The merge base against the working tree: committed, staged, and unstaged
# changes to tracked files at once.
git diff --quiet "$base" -- "${inputs[@]}"
case $? in
0) ;;
1) changed "$(git diff --name-only "$base" -- "${inputs[@]}" | head -n 1) differs from the merge base with main ($short)" ;;
*) changed "git diff against the merge base with main failed" ;;
esac
untracked=$(git ls-files --others --exclude-standard -- "${inputs[@]}" | head -n 1)
[[ -z $untracked ]] || changed "$untracked is untracked"

printf 'neither the working tree nor the commits since the merge base with main (%s) touch crates/tuikit or a root file its build reads\n' "$short"
