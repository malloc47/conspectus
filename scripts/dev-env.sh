#!/usr/bin/env bash
# The dev shell's environment, without starting the dev shell (ADR 0226).
#
#   scripts/dev-env.sh COMMAND [ARG...]   run COMMAND in the environment
#   . scripts/dev-env.sh                  load it into the current shell
#
# `nix develop --command` evaluates the flake and runs a shell on every
# call, which takes seconds. This script asks Nix for the same environment
# once (`nix print-dev-env`), keeps it under target/dev-env/, and sources
# the kept copy afterwards, in milliseconds. The copy is keyed by flake.nix,
# flake.lock, rust-toolchain.toml, this script, and the system, so a change
# to any of them builds a new one on the next call. A profile link beside
# it keeps the environment's store paths from Nix's garbage collector.
#
# Two parts of `nix develop` are left out: the shell hook, which only
# prints the banner, and the temporary directory each call makes and points
# TMPDIR at. Commands keep the caller's TMPDIR and leave nothing in it.

# Builds the kept copy for `key` in `dir` from the flake at `root`. Runs in
# a subshell, so its lock and its shell options stay there.
__dev_env_build() ( # root dir key
  set -euo pipefail
  root=$1 dir=$2 key=$3
  command -v nix >/dev/null 2>&1 || {
    echo "dev-env: nix is not on PATH; the environment comes from the flake" >&2
    exit 1
  }
  mkdir -p "$dir"
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$dir/.lock"
    flock 9
  fi
  # Another call may have built it while this one waited for the lock.
  [[ -f $dir/$key.sh && -e $dir/$key.profile ]] && exit 0
  echo "dev-env: building the environment from the flake (once per change to flake.nix, flake.lock, or rust-toolchain.toml)" >&2
  printed=$(mktemp "$dir/.printed.XXXXXX")
  removed=$(mktemp "$dir/.removed.XXXXXX")
  kept=$(mktemp "$dir/.kept.XXXXXX")
  trap 'rm -f "$printed" "$removed" "$kept"' EXIT
  nix print-dev-env --profile "$dir/$key.profile" "$root" >"$printed"
  # The lines that make a temporary directory, point TMPDIR and its
  # siblings at it, and run the shell hook. All six must be there, so a
  # change in what Nix prints stops here instead of loading a partial
  # environment.
  # shellcheck disable=SC2016
  printf '%s\n' \
    'export NIX_BUILD_TOP="$(mktemp -d -t nix-shell.XXXXXX)"' \
    'export TMP="$NIX_BUILD_TOP"' \
    'export TMPDIR="$NIX_BUILD_TOP"' \
    'export TEMP="$NIX_BUILD_TOP"' \
    'export TEMPDIR="$NIX_BUILD_TOP"' \
    'eval "${shellHook:-}"' >"$removed"
  if [[ $(grep -c -F -x -f "$removed" "$printed") != 6 ]]; then
    echo "dev-env: nix print-dev-env printed an environment this script does not recognize; use nix develop, and fix scripts/dev-env.sh" >&2
    exit 1
  fi
  grep -v -F -x -f "$removed" "$printed" >"$kept"
  mv "$kept" "$dir/$key.sh"
  # Only the current key's copy and profile links are kept.
  find "$dir" -maxdepth 1 ! -name "$key.*" \
    \( -name '*.sh' -o -name '*.profile' -o -name '*.profile-*-link' \) -exec rm -f {} +
)

# Prints the cache key and the kept copy to source, building the copy if it
# is missing; prints nothing when this environment is already loaded.
__dev_env_prepare() {
  local root dir key
  local -a hash
  root=$(CDPATH="" cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P) || return 1
  if command -v sha256sum >/dev/null 2>&1; then
    hash=(sha256sum)
  else
    hash=(shasum -a 256)
  fi
  key=$(CDPATH="" cd -- "$root" && {
    uname -sm
    "${hash[@]}" flake.nix flake.lock rust-toolchain.toml scripts/dev-env.sh
  } | "${hash[@]}" | cut -c1-16) || return 1

  # Already loaded, by a parent command or when the session started.
  [[ ${CONSPECTUS_DEV_ENV:-} == "$key" ]] && return 0

  dir=$root/target/dev-env
  if [[ ! -f $dir/$key.sh || ! -e $dir/$key.profile ]]; then
    __dev_env_build "$root" "$dir" "$key" || return 1
  fi
  printf '%s %s\n' "$key" "$dir/$key.sh"
}

if [[ ${BASH_SOURCE[0]} == "$0" ]] && (($# == 0)); then
  sed -n '2,/^$/s/^# \{0,1\}//p' "${BASH_SOURCE[0]}" >&2
  exit 2
fi

# The copy is sourced at the top level, not in a function, so the arrays it
# declares stay global.
if __dev_env_found=$(__dev_env_prepare); then
  unset -f __dev_env_prepare __dev_env_build
  if [[ -n $__dev_env_found ]]; then
    # shellcheck disable=SC1090
    . "${__dev_env_found#* }"
    export CONSPECTUS_DEV_ENV=${__dev_env_found%% *}
  fi
  unset __dev_env_found
  if [[ ${BASH_SOURCE[0]} != "$0" ]]; then
    return 0
  fi
else
  unset -f __dev_env_prepare __dev_env_build
  unset __dev_env_found
  if [[ ${BASH_SOURCE[0]} != "$0" ]]; then
    return 1
  fi
  exit 1
fi

exec "$@"
