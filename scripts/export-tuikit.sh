#!/usr/bin/env bash
# Rehearses moving the TUI framework crate, `crates/tuikit`, into a
# repository of its own (ADR 0150, ADR 0199).
#
#   scripts/export-tuikit.sh [--self-test] [--keep] [--allow-dirty] [SCRATCH]
#
# 1. Splits the crate's history out of HEAD (`git subtree split`) into a new
#    repository at SCRATCH/tuikit. No ref is created in this repository.
# 2. Rewrites the crate's manifest so it stands alone: fields inherited from
#    `[workspace.package]`, dependencies inherited from
#    `[workspace.dependencies]`, and the `[workspace.lints.*]` tables become
#    concrete values, and an empty `[workspace]` table makes the package its
#    own workspace root. Cargo.lock is copied and pruned to the crate's
#    packages, so the export builds the versions this repository builds.
# 3. Adds the root files a repository needs (rust-toolchain.toml, LICENSE,
#    .gitignore, a CI workflow, a justfile, a dev-shell flake.nix) and
#    commits them in the export. The CI workflow and the justfile run the
#    same checks this script runs, from the one list in CHECKS below.
# 4. Checks that nothing of this repository is reachable from the export:
#    cargo takes the export's manifest as its workspace root, a path that
#    climbs out of the crate finds none of this repository's top-level
#    entries, no rustfmt, clippy, or cargo configuration sits above the
#    export, and the export resolves the same packages and features this
#    repository resolves for the crate.
# 5. Runs CHECKS in the export: formatting, clippy with warnings denied
#    (every feature, and the features off), the tests (every feature, and
#    the features off), and the docs with warnings denied. Each step is
#    timed.
#
# SCRATCH defaults to target/separation-rehearsal under the repository root.
# It must be new, empty, or left by an earlier run, and it is removed when
# the run ends (keep it with --keep). In CI give it a directory outside the
# clone, such as "$RUNNER_TEMP/separation-rehearsal". The export keeps its
# own target directory inside it: a build with every feature, clippy, and
# docs takes a few GB, so do not point SCRATCH at a small tmpfs.
#
#   --self-test    after the rehearsal passes, inject two leaks into the
#                  export, one at a time: a test that includes a file
#                  through a path climbing out of the crate, and a test
#                  that reads one at run time. Both paths resolve in this
#                  repository's layout. Each leak must fail the rehearsal at
#                  the step that should catch it; the export is restored
#                  after each.
#   --keep         keep SCRATCH (the export and the step logs) afterwards.
#   --allow-dirty  export HEAD even though the crate or a root file the
#                  export reads has uncommitted changes. They are not in
#                  the export.
#
# The real move, step by step, is crates/tuikit/docs/separation.md.

set -euo pipefail
# A `cd` that consults CDPATH prints the directory, which would end up in
# the output this script captures.
unset CDPATH

crate_prefix=crates/tuikit
crate=tuikit
# Root files of this repository the export reads, at HEAD.
root_inputs=(Cargo.toml Cargo.lock rust-toolchain.toml LICENSE)

# The checks, as `group|step name|command`. The rehearsal runs them in this
# order; the export's justfile gets one recipe per group and its CI
# workflow one step per entry. Keep the generated files free of this
# repository's vocabulary: the crate's own independence guard scans them in
# the export.
CHECKS=(
  "fmt|Check formatting|cargo fmt --all -- --check"
  "clippy|Lint every target with every feature|cargo clippy --locked --all-targets --all-features -- -D warnings"
  "clippy|Lint the library with the features off|cargo clippy --locked --no-default-features -- -D warnings"
  "clippy|Lint every target with the features off|cargo clippy --locked --no-default-features --all-targets -- -D warnings"
  "clippy|Lint the library with richtext alone|cargo clippy --locked --no-default-features --features richtext -- -D warnings"
  "test|Test with every feature|cargo test --locked --all-features"
  "test|Test with the features off|cargo test --locked --no-default-features"
  "doc|Check the docs with the features off|RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --no-default-features"
  "doc|Check the docs with every feature|RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features"
)
# The steps the self-test's leaks must fail.
LINT_ALL_STEP="Lint every target with every feature"
TEST_ALL_STEP="Test with every feature"

say() { printf 'separation: %s\n' "$*" >&2; }
die() {
  printf 'separation: error: %s\n' "$*" >&2
  exit 1
}

usage() {
  sed -n '2,/^$/s/^# \{0,1\}//p' "${BASH_SOURCE[0]}"
}

self_test=0
keep=0
allow_dirty=0
scratch=""
while (($#)); do
  case "$1" in
  --self-test) self_test=1 ;;
  --keep) keep=1 ;;
  --allow-dirty) allow_dirty=1 ;;
  -h | --help)
    usage
    exit 0
    ;;
  -*) die "unknown option $1 (see --help)" ;;
  *)
    [[ -z $scratch ]] || die "one scratch directory at most (see --help)"
    scratch=$1
    ;;
  esac
  shift
done

for tool in git cargo awk sed sort comm diff realpath; do
  command -v "$tool" >/dev/null || die "needs $tool on PATH"
done

host=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
host_git() { git -C "$host" "$@"; }
[[ $(realpath "$(host_git rev-parse --show-toplevel)") == "$host" ]] ||
  die "$host is not the top of a git work tree"
cargo_home=$(realpath -m "${CARGO_HOME:-$HOME/.cargo}")

# ---------------------------------------------------------------------------
# Timings.
# ---------------------------------------------------------------------------

timing_names=()
timing_secs=()
run_started=$SECONDS

record() { # name seconds
  timing_names+=("$1")
  timing_secs+=("$2")
}

duration() { # seconds
  if (($1 >= 60)); then
    printf '%dm %02ds' $(($1 / 60)) $(($1 % 60))
  else
    printf '%ds' "$1"
  fi
}

report_timings() {
  local i
  say "timings:"
  for i in "${!timing_names[@]}"; do
    printf '  %-66s %8s\n' "${timing_names[$i]}" "$(duration "${timing_secs[$i]}")" >&2
  done
  printf '  %-66s %8s\n' "total" "$(duration $((SECONDS - run_started)))" >&2
}

# ---------------------------------------------------------------------------
# The source: full history, a clean crate, the crate present at HEAD.
# ---------------------------------------------------------------------------

[[ $(host_git rev-parse --is-shallow-repository) == false ]] ||
  die "the clone is shallow; the export keeps the crate's history, so fetch all of it (in CI: actions/checkout with fetch-depth: 0)"
host_git cat-file -e "HEAD:$crate_prefix/Cargo.toml" 2>/dev/null ||
  die "HEAD has no $crate_prefix/Cargo.toml"
dirty=$(host_git status --porcelain -- "$crate_prefix" "${root_inputs[@]}")
if [[ -n $dirty ]]; then
  if ((allow_dirty)); then
    say "warning: uncommitted changes are not in the export, which is built from HEAD:"
    printf '%s\n' "$dirty" >&2
  else
    printf '%s\n' "$dirty" >&2
    die "the export is built from HEAD, and these changes are not committed; commit them or pass --allow-dirty"
  fi
fi

# ---------------------------------------------------------------------------
# The scratch directory.
# ---------------------------------------------------------------------------

scratch=$(realpath -m "${scratch:-$host/target/separation-rehearsal}")
marker="$scratch/.separation-rehearsal"
case "$scratch/" in
"$host/$crate_prefix/"*) die "the scratch directory cannot be inside the crate" ;;
esac
case "$host/" in
"$scratch/"*) die "the scratch directory cannot hold this repository" ;;
esac
case "$scratch/" in
"$host/"*)
  # Inside this repository only where git ignores it, so a run leaves
  # nothing untracked behind.
  if ! host_git check-ignore -q "$scratch"; then
    die "$scratch is inside the repository but not ignored; use target/... or a directory outside it"
  fi
  ;;
esac
if [[ -e $scratch ]]; then
  [[ -d $scratch ]] || die "$scratch is not a directory"
  if [[ -n $(ls -A "$scratch") && ! -e $marker ]]; then
    die "$scratch is not empty and is not an earlier run's; give a new or empty directory"
  fi
  rm -rf "$scratch"
fi
mkdir -p "$scratch/.work/logs"
touch "$marker"
work="$scratch/.work"
export_dir="$scratch/$crate"

cleanup() {
  local status=$?
  if ((keep)); then
    say "kept $scratch"
  else
    rm -rf "$scratch"
  fi
  exit "$status"
}
trap cleanup EXIT

# Build output stays in the export, whatever the caller's environment says.
export CARGO_TARGET_DIR="$export_dir/target"
unset CARGO_BUILD_TARGET_DIR

# ---------------------------------------------------------------------------
# 1. The crate's history, as a repository of its own.
# ---------------------------------------------------------------------------

say "splitting $crate_prefix out of HEAD's history"
started=$SECONDS
split=$(host_git subtree split -q --prefix="$crate_prefix" HEAD)
git init -q -b main "$export_dir"
# Fetches the split commit by id: no branch or tag is created here.
git -C "$export_dir" fetch -q --no-tags \
  --upload-pack="git -c uploadpack.allowAnySHA1InWant=true upload-pack" \
  "$host" "$split"
git -C "$export_dir" checkout -q -B main FETCH_HEAD
commits=$(git -C "$export_dir" rev-list --count HEAD)
record "split the history ($commits commits) into the export" $((SECONDS - started))
say "the export at $export_dir has $commits commits, ending in $(git -C "$export_dir" log -1 --format=%h)"

# ---------------------------------------------------------------------------
# 2. A manifest that stands alone.
# ---------------------------------------------------------------------------

# Reads this repository's root manifest (first file) for `[workspace.package]`,
# `[workspace.dependencies]`, and `[workspace.lints.<tool>]`, and writes the
# crate's manifest (second file) with every inherited value made concrete.
# Fails on any spelling it does not rewrite, so the rehearsal stops instead
# of exporting a manifest that only resolves inside the workspace.
# shellcheck disable=SC2016
MANIFEST_AWK='
function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
function fail(message) {
  printf "separation: error: %s/Cargo.toml: %s\n", prefix, message > "/dev/stderr"
  failed = 1
  exit 1
}
function header(line,   h) {
  h = line
  sub(/^[ \t]*\[+[ \t]*/, "", h)
  sub(/[ \t]*\]+[ \t]*(#.*)?$/, "", h)
  return h
}
function is_dependencies(s) {
  return s ~ /^(dev[-_]|build[-_])?dependencies$/ || s ~ /^target\..+\.(dev[-_]|build[-_])?dependencies$/
}
function inherits(value) { return value ~ /[{ ,]workspace[ \t]*=[ \t]*true[ \t]*[,}]/ }
# The keys an inherited dependency adds to the workspace entry. Cargo allows
# `optional` and `features`; merging feature lists is not done here.
function additions(key, value,   inner, n, parts, i, part, out) {
  inner = value
  sub(/^\{[ \t]*/, "", inner)
  sub(/[ \t]*\}$/, "", inner)
  if (inner ~ /\[/) fail("`" key "` adds a list to an inherited dependency; merge it into the entry by hand or teach the rehearsal")
  n = split(inner, parts, ",")
  out = ""
  for (i = 1; i <= n; i++) {
    part = trim(parts[i])
    if (part ~ /^workspace[ \t]*=[ \t]*true$/) continue
    if (part !~ /^optional[ \t]*=[ \t]*(true|false)$/) fail("`" key "` sets `" part "` on an inherited dependency")
    out = (out == "") ? part : (out ", " part)
  }
  return out
}
function inherit(name, added,   spec, inner) {
  if (name in table_dependencies) fail("`" name "` inherits a [workspace.dependencies." name "] table; only one-line entries are rewritten")
  if (!(name in dependencies)) fail("`" name "` inherits from [workspace.dependencies], which does not list it")
  spec = dependencies[name]
  if (spec ~ /^"[^"]*"$/) {
    inner = "version = " spec
  } else if (spec ~ /^\{.*\}$/) {
    inner = spec
    sub(/^\{[ \t]*/, "", inner)
    sub(/[ \t]*\}$/, "", inner)
  } else {
    fail("[workspace.dependencies] spells `" name "` as `" spec "`, which is not rewritten")
  }
  if (inner ~ /(^|[ ,])path[ \t]*=/) fail("`" name "` is a path dependency of the workspace and cannot leave with the crate")
  if (added != "") inner = inner ", " added
  return "{ " inner " }"
}
function check_path(key, value,   path) {
  if (match(value, /(^|[{ ,])path[ \t]*=[ \t]*"[^"]*"/)) {
    path = substr(value, RSTART, RLENGTH)
    sub(/^[{ ,]?path[ \t]*=[ \t]*"/, "", path)
    sub(/"$/, "", path)
    if (path ~ /^\.\.(\/|$)/ || path ~ /^\//) fail("`" key "` is a path dependency outside the crate (`" path "`)")
  }
}
function emit_lints(   i) {
  if (tools == 0) fail("[lints] inherits from the workspace, which has no [workspace.lints.<tool>] table")
  print "# The lint levels of the workspace the crate was built in."
  for (i = 1; i <= tools; i++) {
    if (i > 1) print ""
    print "[lints." tool_names[i] "]"
    printf "%s", lints[tool_names[i]]
  }
}

# The workspace root manifest.
FNR == NR {
  if ($0 ~ /^[ \t]*\[/) {
    section = header($0)
    if (section ~ /^workspace\.lints\.[A-Za-z0-9_-]+$/) {
      tool = section
      sub(/^workspace\.lints\./, "", tool)
      tool_names[++tools] = tool
      lints[tool] = ""
    }
    if (section ~ /^workspace\.dependencies\./) {
      name = section
      sub(/^workspace\.dependencies\./, "", name)
      table_dependencies[name] = 1
    }
    next
  }
  if ($0 ~ /^[ \t]*(#.*)?$/ || index($0, "=") == 0) next
  eq = index($0, "=")
  key = trim(substr($0, 1, eq - 1))
  value = trim(substr($0, eq + 1))
  if (section == "workspace.package") package[key] = value
  else if (section == "workspace.dependencies") dependencies[key] = value
  else if (section ~ /^workspace\.lints\./) lints[tool] = lints[tool] key " = " value "\n"
  next
}

# The crate manifest.
{
  line = $0
  if (line ~ /^[ \t]*\[/) {
    section = header(line)
    if (section == "workspace" || section ~ /^workspace\./) fail("the crate already has a [" section "] table")
    if (section == "lints") next
    print line
    next
  }
  if (line ~ /^[ \t]*(#.*)?$/) {
    print line
    next
  }
  eq = index(line, "=")
  key = trim(substr(line, 1, eq - 1))
  value = trim(substr(line, eq + 1))
  if (section == "lints") {
    if (key == "workspace" && value == "true") {
      emit_lints()
      next
    }
    fail("[lints] sets `" key "`; only `workspace = true` is rewritten")
  }
  if (section == "package" && key ~ /\.workspace$/ && value == "true") {
    name = key
    sub(/\.workspace$/, "", name)
    if (!(name in package)) fail("package." name " inherits from [workspace.package], which does not set it")
    print name " = " package[name]
    next
  }
  if (is_dependencies(section)) {
    if (key ~ /\.workspace$/ && value == "true") {
      name = key
      sub(/\.workspace$/, "", name)
      print name " = " inherit(name, "")
      next
    }
    if (value ~ /^\{.*\}$/ && inherits(value)) {
      print key " = " inherit(key, additions(key, value))
      next
    }
    check_path(key, value)
  }
  if (line ~ /workspace/) fail("`" trim(line) "` inherits from the workspace in a form that is not rewritten")
  print line
}

END {
  if (failed) exit 1
  print ""
  print "# The package is its own workspace root, so cargo looks no further up for"
  print "# one, even when the repository sits inside the tree of another project."
  print "[workspace]"
}
'

say "writing a manifest that stands alone"
host_git show HEAD:Cargo.toml >"$work/host-Cargo.toml"
awk -v prefix="$crate" "$MANIFEST_AWK" "$work/host-Cargo.toml" "$export_dir/Cargo.toml" >"$work/Cargo.toml"
mv "$work/Cargo.toml" "$export_dir/Cargo.toml"

# ---------------------------------------------------------------------------
# 3. The root files.
# ---------------------------------------------------------------------------

say "adding the root files"

# The toolchain: this repository's release and components, with a comment of
# the export's own.
{
  printf '%s\n' \
    "# The exact Rust release the dev shell, CI, and rustup all use. To move to" \
    "# another: change \`channel\`, then fix whatever the new clippy reports in" \
    "# the same change."
  host_git show HEAD:rust-toolchain.toml | grep -v '^[[:space:]]*#'
} >"$export_dir/rust-toolchain.toml"

host_git show HEAD:LICENSE >"$export_dir/LICENSE"

printf '/target/\n' >"$export_dir/.gitignore"

write_justfile() {
  local entry group name cmd last="" groups=()
  {
    printf '%s\n' \
      "# The checks CI runs (.github/workflows/ci.yml), in the same order." \
      "# \`just\` runs them all." \
      "" \
      "default: check"
    for entry in "${CHECKS[@]}"; do
      IFS='|' read -r group name cmd <<<"$entry"
      if [[ $group != "$last" ]]; then
        printf '\n%s:\n' "$group"
        groups+=("$group")
        last=$group
      fi
      printf '    %s\n' "$cmd"
    done
    printf '\n'
    printf '%s\n' \
      "# Whitespace errors in changes not yet committed." \
      "diff-check:" \
      "    git diff --check"
    printf '\ncheck: %s diff-check\n' "${groups[*]}"
  } >"$export_dir/justfile"
}
write_justfile

write_workflow() {
  local entry group name cmd
  mkdir -p "$export_dir/.github/workflows"
  {
    # shellcheck disable=SC2016
    printf '%s\n' \
      'name: CI' \
      '' \
      'on:' \
      '  pull_request:' \
      '  push:' \
      '    branches:' \
      '      - main' \
      '' \
      'env:' \
      '  CARGO_TERM_COLOR: always' \
      '' \
      'jobs:' \
      '  rust:' \
      '    name: Rust checks' \
      '    runs-on: ubuntu-latest' \
      '' \
      '    steps:' \
      '      - uses: actions/checkout@v4' \
      '' \
      '      # rust-toolchain.toml names the exact release; install exactly it.' \
      '      - name: Read the Rust release' \
      '        id: toolchain' \
      '        run: echo "channel=$(sed -n '"'"'s/^channel *= *"\(.*\)"/\1/p'"'"' rust-toolchain.toml)" >> "$GITHUB_OUTPUT"' \
      '' \
      '      - name: Install Rust' \
      '        uses: dtolnay/rust-toolchain@master' \
      '        with:' \
      '          toolchain: ${{ steps.toolchain.outputs.channel }}' \
      '          components: clippy,rustfmt' \
      '' \
      '      - name: Cache the Cargo registry and build output' \
      '        uses: Swatinem/rust-cache@v2'
    for entry in "${CHECKS[@]}"; do
      IFS='|' read -r group name cmd <<<"$entry"
      printf '\n      - name: %s\n        run: %s\n' "$name" "$cmd"
    done
  } >"$export_dir/.github/workflows/ci.yml"
}
write_workflow

cat >"$export_dir/flake.nix" <<'EOF'
{
  description = "tuikit, a terminal UI framework on ratatui (dev shell)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # Supplies the Rust release rust-toolchain.toml names.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        # rust-toolchain.toml is the one place the Rust release is named;
        # editors also get the standard library source and rust-analyzer.
        rustToolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.git
            pkgs.just
            # Runs scripts/glyph-widths.py.
            pkgs.python3
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];

          shellHook = ''
            echo "tuikit dev shell (system: ${system}): \`just\` runs the checks CI runs"
          '';
        };
      });
}
EOF

[[ -f $export_dir/README.md ]] ||
  die "the crate has no README.md; the export's root README is the crate's"

# The generated files parse, where the tool that reads them is installed.
if command -v just >/dev/null; then
  just --justfile "$export_dir/justfile" --summary >/dev/null ||
    die "the export's justfile does not parse"
else
  say "just is not installed; the justfile is not parsed"
fi
if command -v nix-instantiate >/dev/null; then
  nix-instantiate --parse "$export_dir/flake.nix" >/dev/null ||
    die "the export's flake.nix does not parse"
else
  say "nix is not installed; flake.nix is not parsed"
fi

# ---------------------------------------------------------------------------
# 4. The lockfile: this repository's, pruned to the crate's packages.
# ---------------------------------------------------------------------------

say "pruning Cargo.lock to the crate's packages"
started=$SECONDS
host_git show HEAD:Cargo.lock >"$export_dir/Cargo.lock"
(cd "$export_dir" && cargo metadata --format-version 1 >/dev/null 2>"$work/metadata.err") || {
  cat "$work/metadata.err" >&2
  die "cargo cannot read the export's manifest"
}
if grep -q '^warning' "$work/metadata.err"; then
  cat "$work/metadata.err" >&2
  die "cargo warns about the export's manifest"
fi
(cd "$export_dir" && cargo metadata --locked --format-version 1 >/dev/null) ||
  die "the pruned Cargo.lock does not satisfy the export's manifest"
host_git show HEAD:Cargo.lock >"$work/host-Cargo.lock"
lock_packages() { awk '/^name = /{ name = $3 } /^version = /{ print name, $3 }' "$1" | sort -u; }
unlocked=$(comm -13 <(lock_packages "$work/host-Cargo.lock") <(lock_packages "$export_dir/Cargo.lock"))
[[ -z $unlocked ]] || die "the export locks package versions this repository does not: $unlocked"
record "prune Cargo.lock" $((SECONDS - started))
say "Cargo.lock keeps $(grep -c '^\[\[package\]\]' "$export_dir/Cargo.lock") of $(grep -c '^\[\[package\]\]' "$work/host-Cargo.lock") packages"

git -C "$export_dir" add -A
git -C "$export_dir" -c user.name="Separation rehearsal" -c user.email="rehearsal@localhost" \
  -c commit.gpgsign=false commit -q \
  -m "chore: add the files a standalone repository needs" \
  -m "The manifest stands alone (inherited fields made concrete, its own workspace root), and the toolchain file, license, lockfile, CI workflow, justfile, and dev shell are the repository's own."

# ---------------------------------------------------------------------------
# 5. Nothing of this repository is reachable from the export.
# ---------------------------------------------------------------------------

say "checking that nothing of this repository is reachable from the export"
started=$SECONDS

root=$(cd "$export_dir" && cargo locate-project --workspace --message-format plain)
[[ $(realpath "$root") == "$(realpath "$export_dir/Cargo.toml")" ]] ||
  die "cargo takes $root as the export's workspace root, not the export's own manifest"

# The crate sits two levels below this repository's root, so a relative path
# that climbs out of the crate here lands at the export's grandparent there.
above=$(realpath "$export_dir/../..")
while IFS= read -r entry; do
  if [[ -e $above/$entry || -L $above/$entry ]]; then
    die "$above/$entry exists, so a path climbing out of the crate would still find a file of this repository's layout; choose another scratch directory"
  fi
done < <(host_git ls-tree --name-only HEAD)

# rustfmt, clippy, and cargo read configuration from every directory above
# the one they run in; Cargo's own home is the caller's and stays.
dir=$(dirname "$export_dir")
while :; do
  for name in rustfmt.toml .rustfmt.toml clippy.toml .clippy.toml .cargo/config.toml .cargo/config; do
    [[ -e $dir/$name ]] || continue
    if [[ $name == .cargo/* && $(realpath "$dir/.cargo") == "$cargo_home" ]]; then
      continue
    fi
    die "$dir/$name would configure the export's build; choose a scratch directory outside $dir"
  done
  [[ $dir != / ]] || break
  dir=$(dirname "$dir")
done

# The export resolves the packages and features this repository resolves for
# the crate: the manifest rewrite dropped or added nothing.
dependency_graph() { # directory
  (cd "$1" && cargo tree --locked -p "$crate" --all-features -e normal,build,dev \
    --prefix none --no-dedupe --format '{p} [{f}]') |
    sed -E 's# \([^)]*/[^)]*\)##; s# \(\*\)$##' | sort -u
}
dependency_graph "$host" >"$work/graph-host.txt"
dependency_graph "$export_dir" >"$work/graph-export.txt"
if ! diff -u "$work/graph-host.txt" "$work/graph-export.txt" >&2; then
  die "the export resolves other packages or features than this repository does for the crate"
fi

# Whitespace errors across the whole export, which has no .gitattributes.
empty_tree=$(git -C "$export_dir" hash-object -t tree /dev/null)
git -C "$export_dir" diff --check "$empty_tree" HEAD ||
  die "the export has whitespace errors"

record "check that nothing of this repository is reachable" $((SECONDS - started))
say "isolated: the workspace root is the export's manifest, and $(wc -l <"$work/graph-export.txt") packages resolve as they do here"

# ---------------------------------------------------------------------------
# 6. The checks, in the export.
# ---------------------------------------------------------------------------

failed_step=""
failed_log=""
run=0

# Runs CHECKS in the export, stopping at the first failure; sets
# failed_step and failed_log. `label` prefixes the timings.
run_checks() { # label
  local label=$1 entry group name cmd started status log
  failed_step=""
  failed_log=""
  run=$((run + 1))
  for entry in "${CHECKS[@]}"; do
    IFS='|' read -r group name cmd <<<"$entry"
    log="$work/logs/$run-${name// /-}.log"
    say "$label: $name"
    say "  \$ $cmd"
    started=$SECONDS
    status=0
    (cd "$export_dir" && bash -c "$cmd") >"$log" 2>&1 || status=$?
    record "$label: $name" $((SECONDS - started))
    if ((status != 0)); then
      failed_step=$name
      failed_log=$log
      say "$label: '$name' failed (exit $status); the end of its output:"
      tail -n 60 "$log" >&2
      return 1
    fi
  done
  return 0
}

if ! run_checks "rehearsal"; then
  report_timings
  if ((keep)); then
    die "the export fails '$failed_step'; its output is $failed_log"
  fi
  die "the export fails '$failed_step'; rerun with --keep to keep the export and the logs"
fi
say "the export passes every check alone"

# ---------------------------------------------------------------------------
# 7. The self-test: a leak must fail the rehearsal.
# ---------------------------------------------------------------------------

restore_export() {
  git -C "$export_dir" clean -fdq
  git -C "$export_dir" checkout -q -- .
  [[ -z $(git -C "$export_dir" status --porcelain) ]] || die "self-test: the export did not restore"
}

# Injects `source` as `file` in the export, runs the checks, and requires
# them to fail at `step` with `needle` in that step's output.
expect_caught() { # label file step needle source
  local label=$1 file=$2 step=$3 needle=$4 source=$5
  say "self-test: injecting $label as $file"
  printf '%s\n' "$source" >"$export_dir/$file"
  (cd "$export_dir" && cargo fmt --all)
  if run_checks "self-test, $label"; then
    die "self-test: the rehearsal passed with $label in the export"
  fi
  [[ $failed_step == "$step" ]] ||
    die "self-test: $label failed '$failed_step', not '$step'"
  grep -q -- "$needle" "$failed_log" ||
    die "self-test: the failure of '$step' does not mention $needle"
  restore_export
  say "self-test: $label failed '$step' as it should, and the export is restored"
}

if ((self_test)); then
  # Both leaks name a file that exists two levels above the crate here and
  # nowhere above the export, so each passes in this repository's layout
  # and fails in the export's.
  [[ -f $host/$crate_prefix/../../src/lib.rs ]] ||
    die "self-test: no src/lib.rs two levels above the crate; point the leaks at another file"
  [[ ! -e $export_dir/../../src/lib.rs ]] ||
    die "self-test: src/lib.rs exists two levels above the export"

  expect_caught "a compile-time include" tests/separation_leak.rs \
    "$LINT_ALL_STEP" "separation_leak.rs" "$(
      cat <<'EOF'
//! A deliberate leak: includes a file through a path that climbs out of
//! the crate, which resolves only in the layout the crate came from.

const ABOVE: &str = include_str!("../../../src/lib.rs");

#[test]
fn includes_a_file_from_above_the_crate() {
    assert!(!ABOVE.is_empty());
}
EOF
    )"

  expect_caught "a run-time read" tests/separation_leak.rs \
    "$TEST_ALL_STEP" "reads_a_file_from_above_the_crate" "$(
      cat <<'EOF'
//! A deliberate leak: reads a file through a path that climbs out of the
//! crate, which resolves only in the layout the crate came from.

use std::path::Path;

#[test]
fn reads_a_file_from_above_the_crate() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib.rs");
    assert!(Path::new(path).is_file(), "{path} is not a file");
}
EOF
    )"
  say "self-test: both leaks fail the rehearsal"
fi

say "the export's build output took $(du -sh "$CARGO_TARGET_DIR" | cut -f1)"
report_timings
