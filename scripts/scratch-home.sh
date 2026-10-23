#!/usr/bin/env bash
# A disposable Conspectus home for exercising the CLI and TUI by hand or
# from a script.
#
#   scripts/scratch-home.sh shell            # interactive subshell in a scratch home
#   scripts/scratch-home.sh smoke [NAME...]  # run the automated checks
#   scripts/scratch-home.sh list             # list the check names
#
# Every run gets a fresh $HOME and XDG_* tree, a small git repo with a
# remote, stub harness binaries on PATH, and a PRIVATE tmux server
# (TMUX_TMPDIR under the scratch dir, $TMUX unset). Nothing touches your
# real home, config, or tmux server; the scratch dir and its tmux server
# are removed on exit unless KEEP=1.
#
# Environment:
#   CONSPECTUS_BIN  binary to test (default: target/debug/conspectus,
#                   built with `cargo build --features snapshot` if missing)
#   KEEP=1          keep the scratch dir and print its path
#   VERBOSE=1       print the output of every command
#
# Add a check by writing `check_<name>() { ... }` using the helpers below
# and listing <name> in CHECKS. Checks share one scratch home, so each
# starts by calling `fresh` to reset state it depends on.

set -uo pipefail
unset CDPATH

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONSPECTUS_BIN="${CONSPECTUS_BIN:-$REPO_ROOT/target/debug/conspectus}"

CHECKS=(
  empty_store
  retired_verbs
  planned_lifecycle
  bad_input
  live_pin_lifecycle
  rename_lockstep
  rebuild_variants
  rebuild_plan_script
  reap_and_forget
  mux_pin_flags
  launch_profiles
  migrate_legacy
  migrate_partial_failure
  daemon_envelope
  refresh_freshness
  worktree_close
  repo_clone
)

# ---------------------------------------------------------------- setup

SCRATCH=""
PASS=0
FAIL=0
FAILED_NAMES=()

setup_scratch() {
  SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/conspectus-scratch.XXXXXX")"
  export HOME="$SCRATCH/home"
  export XDG_CONFIG_HOME="$SCRATCH/xdg/config"
  export XDG_STATE_HOME="$SCRATCH/xdg/state"
  export XDG_DATA_HOME="$SCRATCH/xdg/data"
  export XDG_CACHE_HOME="$SCRATCH/xdg/cache"
  export XDG_RUNTIME_DIR="$SCRATCH/xdg/run"
  export TMUX_TMPDIR="$SCRATCH/tmux"
  export LC_CTYPE="C.UTF-8"
  export CONSPECTUS_DISABLE_FORGE=1
  export CONSPECTUS_DISABLE_ZELLIJ=1
  export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
  unset TMUX TMUX_PANE
  mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_DATA_HOME" \
    "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR" "$TMUX_TMPDIR" "$SCRATCH/bin"
  chmod 700 "$XDG_RUNTIME_DIR"

  # Stub harnesses: they only need to stay alive in a pane.
  for h in codex claude; do
    printf '#!/bin/sh\nexec sleep 600\n' >"$SCRATCH/bin/$h"
    chmod +x "$SCRATCH/bin/$h"
  done
  export PATH="$SCRATCH/bin:$PATH"

  # A repo with a bare "remote" so clone/rebuild checks have a source.
  REMOTE="$SCRATCH/remote.git"
  REPO="$HOME/src/app"
  git init -q --bare --initial-branch main "$REMOTE"
  mkdir -p "$REPO"
  git -C "$REPO" init -q --initial-branch main
  git -C "$REPO" commit -q --allow-empty -m init
  git -C "$REPO" remote add origin "$REMOTE"
  git -C "$REPO" push -q origin main 2>/dev/null
  cd "$REPO" || exit 1
}

# Safety: every tmux call goes through here and refuses to run unless the
# private server directory is in effect.
tmx() {
  case "${TMUX_TMPDIR:-}" in
    "$SCRATCH"/*) ;;
    *) echo "refusing to run tmux outside the scratch home" >&2; return 99 ;;
  esac
  env -u TMUX tmux "$@"
}

cleanup() {
  local status=$?
  if [[ -n "$SCRATCH" && -d "$SCRATCH" ]]; then
    if [[ "${TMUX_TMPDIR:-}" == "$SCRATCH"/* ]]; then
      env -u TMUX tmux kill-server 2>/dev/null || true
    fi
    pkill -f "$SCRATCH" 2>/dev/null || true
    if [[ "${KEEP:-0}" == 1 ]]; then
      echo "kept scratch home: $SCRATCH" >&2
    else
      rm -rf "$SCRATCH"
    fi
  fi
  exit "$status"
}

ensure_binary() {
  if [[ ! -x "$CONSPECTUS_BIN" ]]; then
    echo "building $CONSPECTUS_BIN ..." >&2
    (cd "$REPO_ROOT" && cargo build --quiet --features snapshot) || exit 1
  fi
}

cx() { "$CONSPECTUS_BIN" "$@"; }

# ------------------------------------------------------------- helpers

OUT="" ERR="" RC=0

# run CMD...: capture stdout/stderr/status without failing the script.
run() {
  local o e
  o="$(mktemp)"; e="$(mktemp)"
  "$@" >"$o" 2>"$e"
  RC=$?
  OUT="$(cat "$o")"; ERR="$(cat "$e")"
  rm -f "$o" "$e"
  if [[ "${VERBOSE:-0}" == 1 ]]; then
    printf '$ %s\n%s\n%s\n(rc=%s)\n' "$*" "$OUT" "$ERR" "$RC" >&2
  fi
}

CURRENT=""
CHECK_FAILED=0

note_fail() {
  CHECK_FAILED=1
  printf '  FAIL: %s\n' "$1" >&2
  [[ -n "$OUT" ]] && printf '    stdout: %s\n' "${OUT:0:600}" >&2
  [[ -n "$ERR" ]] && printf '    stderr: %s\n' "${ERR:0:600}" >&2
}

ok()   { [[ $RC -eq 0 ]] || note_fail "${1:-command}: expected success, rc=$RC"; }
fails() { [[ $RC -ne 0 ]] || note_fail "${1:-command}: expected failure"; }
has()  { [[ "$OUT$ERR" == *"$1"* ]] || note_fail "${2:-output} should contain: $1"; }
lacks() { [[ "$OUT$ERR" != *"$1"* ]] || note_fail "${2:-output} should not contain: $1"; }
is_true() { # is_true DESCRIPTION CMD...
  local d="$1"; shift
  "$@" >/dev/null 2>&1 || note_fail "$d"
}
is_false() {
  local d="$1"; shift
  "$@" >/dev/null 2>&1 && note_fail "$d"
}

PINS() { echo "$XDG_DATA_HOME/conspectus/pins.toml"; }
HIST() { echo "$XDG_STATE_HOME/conspectus/history"; }
mode_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

# fresh: reset everything a check depends on (slots, history, config,
# tmux, caches) without recreating the repo.
fresh() {
  tmx kill-server 2>/dev/null || true
  rm -rf "$XDG_DATA_HOME/conspectus" "$XDG_STATE_HOME/conspectus" \
    "$XDG_CACHE_HOME/conspectus" "$XDG_CONFIG_HOME/conspectus" "$REPO/.conspectus.toml"
  rm -rf "$TMUX_TMPDIR"/tmux-* 2>/dev/null || true
  mkdir -p "$XDG_RUNTIME_DIR"
  cd "$REPO" || exit 1
}

start_mux() { # start_mux NAME [CWD]
  tmx new-session -d -s "$1" -c "${2:-$REPO}" sleep 600
}

mux_alive() { tmx has-session -t "=$1"; }

# --------------------------------------------------------------- checks

check_empty_store() {
  fresh
  run cx pin list; ok "pin list"; has "no pins" "pin list"
  is_false "listing must not create pins.toml" test -e "$(PINS)"
  run cx history list; ok "history list"
}

check_retired_verbs() {
  fresh
  local verb
  for verb in create launch attach bind rebind adopt; do
    run cx pin "$verb" x --harness codex
    fails "pin $verb"
    has "was retired" "pin $verb"
  done
  is_false "retired verbs must not write" test -e "$(PINS)"
}

check_planned_lifecycle() {
  fresh
  run cx pin add --planned --kind mux --name alpha --set cwd=/tmp; ok "add alpha"
  run cx pin add --planned --kind mux --name beta; ok "add beta"
  run cx pin add --planned --kind worktree --name wt; ok "add worktree-kind alias"
  run cx pin list; has alpha; has beta; has planned
  is_true "pins.toml is owner-only" test "$(mode_of "$(PINS)")" = 600
  run cx pin edit alpha --set harness=codex --unset cwd; ok "edit"
  run cat "$(PINS)"; has 'harness = "codex"'; lacks 'cwd ='
  run cx pin rename alpha gamma; ok "rename"
  run cx pin move beta --top; ok "move --top"
  run cx pin move beta --bottom; ok "move --bottom"
  run cx pin move beta --before gamma; ok "move --before"
  run cx pin move beta --after gamma; ok "move --after"
  run cx pin move beta --up; ok "move --up"
  run cx pin move beta --down; ok "move --down"
  run cx pin move beta --before nope; fails "move before unknown anchor"
  run cx pin move beta --before beta; fails "move before itself"
  run cx pin rm beta; ok "rm"
  run cx pin rm beta; fails "rm twice"; has "no pin named"
  run cx pin show gamma; ok "show"; has gamma
  run cx rebuild gamma --dry-run; fails "planned slot with no cwd"; has "no working directory"
}

check_bad_input() {
  fresh
  run cx pin add --planned --kind bogus --name x; fails; has "unknown kind"
  run cx pin add --planned --kind pr --name x --set argv=a; fails; has "no overridable field"
  run cx pin add nothing-like-this; fails; has "nothing matches"
  run cx pin edit ghost --set cwd=/x; fails; has "no pin named"
  run cx pin edit ghost; fails
  is_false "bad input must not write" test -e "$(PINS)"
}

check_live_pin_lifecycle() {
  fresh
  start_mux demo
  run cx pin add demo; ok "pin add demo"; has "pinned mux"
  is_true "pins.toml mode 600" test "$(mode_of "$(PINS)")" = 600
  is_true "history dir mode 700" test "$(mode_of "$(HIST)")" = 700
  run cx pin list; has demo; has live
  run cx pin show demo; ok; has live
  run cx history list --kind mux; ok; has demo; has "◉"
  tmx kill-server
  run cx pin list; has stopped "pin reads stopped after its server died"
  run cx history list --gone; has demo; has "◉"
  run cx rebuild demo --dry-run; ok; has demo
  # No tmux server directory at all (like a /tmp cleanup after reboot).
  rm -rf "$TMUX_TMPDIR"/tmux-*
  run cx pin list; has stopped "pin reads stopped when the socket dir vanished"
}

check_rename_lockstep() {
  fresh
  start_mux ren-a
  run cx pin add ren-a; ok
  run cx pin rename ren-a ren-b; ok "rename live slot"
  is_true "tmux session renamed" mux_alive ren-b
  is_false "old tmux name gone" mux_alive ren-a
  run cat "$(PINS)"; has 'id = "ren-b"'; has 'name = "ren-b"'
  run cx pin rename ren-b ren-c --no-mux; ok "--no-mux"
  is_true "tmux kept its name with --no-mux" mux_alive ren-b
  run cx rename mux "tmux:ren-b" ren-d; ok "rename mux follows an override"
}

check_rebuild_variants() {
  fresh
  start_mux rb-a
  run cx pin add rb-a --name rb; ok
  tmx kill-server
  run cx rebuild rb --dry-run; ok "dry-run"; has "new session"
  is_false "dry-run starts nothing" mux_alive rb-a
  run cx rebuild rb --no-attach; ok "rebuild by slot"
  is_true "mux is back" mux_alive rb-a
  run cx rebuild rb --no-attach; ok "rebuild of a live mux is an attach/no-op"
  tmx kill-server
  local hid
  hid="$(cx history list --kind mux | awk '{print $1; exit}')"
  run cx rebuild "$hid" --no-attach; ok "rebuild by history id"
  tmx kill-server
  run cx rebuild rb-a --no-attach; ok "rebuild by mux name"
  tmx kill-server
  run cx rebuild rb --session nonexistent --dry-run; has "--session" "--session on a mux with no harness is called out"
  run cx rebuild missing-thing --dry-run; fails "unknown ref"
}

check_rebuild_plan_script() {
  fresh
  start_mux plan-a
  run cx graph --format json; ok
  run cx rebuild plan-a --assume-stopped --no-attach; fails "--assume-stopped needs --dry-run"; has "--dry-run"
  run cx rebuild plan-a --dry-run --no-attach; ok; lacks "new-session" "a real plan of a running mux"
  run cx rebuild plan-a --dry-run --assume-stopped --no-attach; ok; has "hypothetical"; has "       \$ tmux new-session -d -s plan-a"
  run cx rebuild plan-a --dry-run --assume-stopped --script --no-attach; ok
  local script="$OUT"
  [[ "$script" == *"new-session -d -s plan-a"* ]] || note_fail "the script has the new-session command"

  # While it runs, the script recreates it under another name.
  local copy="${script//plan-a/plan-copy}"
  run sh -c "$copy"; ok "the pasted script runs"
  is_true "the copy started" mux_alive plan-copy

  # After it stops, the script brings the original back.
  tmx kill-server
  run sh -c "$script"; ok "the pasted script runs after a stop"
  is_true "the mux is back" mux_alive plan-a
}

check_reap_and_forget() {
  fresh
  start_mux keep-a; start_mux drop-b
  run cx graph --format json; ok
  run cx pin add keep-a; ok
  tmx kill-server
  run cx history reap --dry-run; ok; has "nothing to reap"
  run cx history reap --kind mux --older-than 0s --dry-run; ok
  has drop-b; lacks keep-a "pinned record in reap list"
  run cx history reap --kind mux --older-than 0x; fails "bad unit"
  run cx history reap --kind repo --older-than 0s --dry-run; ok
  run cx history reap --kind mux --older-than 0s; ok
  run cx history list --kind mux; has keep-a; lacks drop-b
  local hid
  hid="$(cx history list --kind mux | awk '{print $1; exit}')"
  run cx history forget "$hid"; fails "forget pinned"; has "pin(s)"
  run cx history show "$hid"; ok; has "pinned by"
  run cx history forget "$hid" --force; ok
  run cx pin list; has missing "slot outlives its record"
  run cx history show h_000000000000; fails
}

check_mux_pin_flags() {
  fresh
  run cx mux new later --pin --no-launch; ok; has "planned"
  is_false "nothing launched" mux_alive later
  run cx mux new later --pin --no-launch; fails "duplicate slot name"; has "already exists"
  run cx mux new x --no-launch; fails "--no-launch requires --pin"
  run cx mux new live1 --pin --no-attach; ok "new --pin"
  is_true "live1 running" mux_alive live1
  run cx mux launch codex --name live2 --pin --no-attach; ok "launch --pin"
  run cx pin list; has live1; has live2
  run cx mux launch codex --name nolaunch --pin --no-launch; ok
  is_false "launch --no-launch starts nothing" mux_alive nolaunch
}

check_launch_profiles() {
  fresh
  mkdir -p "$XDG_CONFIG_HOME/conspectus"
  printf '[launch.codex]\nargv = ["codex", "--from-user-profile"]\n' >"$XDG_CONFIG_HOME/conspectus/config.toml"
  run cx pin add --planned --kind mux --name prof --set harness=codex --set cwd="$REPO"; ok
  run cx rebuild prof --dry-run; ok; has "from-user-profile"; has "user launch profile"
  printf '[launch.codex]\nargv = ["codex", "--from-project-profile"]\n' >"$REPO/.conspectus.toml"
  run cx rebuild prof --dry-run; ok; has "from-project-profile"; has "project launch profile"
  run cx pin edit prof --set argv='codex --from-slot'; ok
  run cx rebuild prof --dry-run; ok; has "from-slot"
}

legacy_config() { # legacy_config FILE ID MUX CWD
  cat >>"$1" <<EOF

[pins]
schema_version = 1

[[pins.entries]]
id = "$2"
display_name = "Legacy $2"
harness = "codex"
cwd = "$4"
[pins.entries.mux]
backend = "tmux"
name = "$3"
EOF
}

check_migrate_legacy() {
  fresh
  mkdir -p "$XDG_CONFIG_HOME/conspectus"
  printf '# user comment\n[session]\npoll_ms = 250\n' >"$XDG_CONFIG_HOME/conspectus/config.toml"
  legacy_config "$XDG_CONFIG_HOME/conspectus/config.toml" user-pin user-mux "$REPO"
  legacy_config "$REPO/.conspectus.toml" proj-pin proj-mux "$REPO"
  mkdir -p "$XDG_CACHE_HOME/conspectus/pin-bindings"
  printf '{"schema_version":1,"pin_id":"proj-pin","mux_name":"proj-mux","session_id":"sess-123","harness":"codex","observed_epoch":1700000000}' \
    >"$XDG_CACHE_HOME/conspectus/pin-bindings/proj-pin.json"

  run cx pin list; ok; has user-pin; has proj-pin; has "pin migrate"
  run cx pin edit proj-pin --set cwd=/x; fails; has "pin migrate"
  run cx pin rm proj-pin; fails; has "pin migrate"
  run cx pin rename proj-pin z; fails; has "pin migrate"
  run cx rebuild proj-pin --dry-run; fails; has "pin migrate"
  run cx pin migrate --dry-run; ok; has "user-pin"; has "proj-pin"; has "sess-123"
  is_false "dry-run wrote slots" test -e "$(PINS)"
  is_true "dry-run kept legacy entry" grep -q proj-pin "$REPO/.conspectus.toml"
  run cx pin migrate; ok; has "migrated 2"
  run cat "$(PINS)"; has 'id = "user-pin"'; has 'id = "proj-pin"'
  is_true "user comments/tables survive" grep -q 'poll_ms = 250' "$XDG_CONFIG_HOME/conspectus/config.toml"
  is_false "legacy entries removed" grep -q user-pin "$XDG_CONFIG_HOME/conspectus/config.toml"
  run cat "$(HIST)/muxes.json"; has sess-123 "last session kept as hosted session"
  run cx pin list; lacks "legacy" "list after migrate"
  run cx pin migrate; ok; has "no legacy pins"
  run cx rebuild proj-pin --dry-run; ok "migrated slot rebuilds"

  # An unparsable legacy file is named, not silently skipped.
  printf '[pins]\nschema_version = 1\n[[pins.entries]]\nid = "bad"\ndisplay_name = "b"\nharness = "codex"\ncwd = "relative"\n[pins.entries.mux]\nbackend = "tmux"\nname = "bad"\n' \
    >"$REPO/.conspectus.toml"
  run cx pin migrate; fails "unparsable legacy config"; has ".conspectus.toml"
}

check_migrate_partial_failure() {
  fresh
  [[ "$(id -u)" == 0 ]] && { echo "  (skipped: root ignores chmod)"; return; }
  mkdir -p "$XDG_CONFIG_HOME/conspectus"
  legacy_config "$XDG_CONFIG_HOME/conspectus/config.toml" a a "$REPO"
  chmod 555 "$XDG_CONFIG_HOME/conspectus"
  run cx pin migrate; has "could not remove" "partial failure message"
  chmod 755 "$XDG_CONFIG_HOME/conspectus"
  run cx pin migrate; ok "re-run completes"
  is_false "entry finally removed" grep -q 'id = "a"' "$XDG_CONFIG_HOME/conspectus/config.toml"

  # A symlinked config is written through, not replaced.
  fresh
  mkdir -p "$XDG_CONFIG_HOME/conspectus" "$SCRATCH/dotfiles"
  legacy_config "$SCRATCH/dotfiles/config.toml" s s "$REPO"
  chmod 600 "$SCRATCH/dotfiles/config.toml"
  ln -s "$SCRATCH/dotfiles/config.toml" "$XDG_CONFIG_HOME/conspectus/config.toml"
  run cx pin migrate; ok
  is_true "symlink preserved" test -L "$XDG_CONFIG_HOME/conspectus/config.toml"
  is_false "target updated" grep -q 'id = "s"' "$SCRATCH/dotfiles/config.toml"
  is_true "mode preserved" test "$(mode_of "$SCRATCH/dotfiles/config.toml")" = 600
}

check_daemon_envelope() {
  fresh
  start_mux d-a
  run cx pin add d-a; ok
  local before after
  before="$(sha256sum "$(PINS)" | cut -d' ' -f1)"
  cx serve >"$SCRATCH/serve.log" 2>&1 &
  local pid=$!
  local i
  for i in $(seq 1 50); do
    [[ -S "$XDG_RUNTIME_DIR/conspectus/server.sock" ]] && break
    sleep 0.2
  done
  run cx status; ok "status via daemon"
  run cx refresh; ok "refresh via daemon"
  run cx table sessions; ok
  start_mux d-b
  # One refresh is enough: it re-probes and publishes before it returns.
  run cx refresh --class mux; ok "single mux refresh"
  run cx history list --kind mux
  has d-b "the new mux was recorded after a single refresh"
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  after="$(sha256sum "$(PINS)" | cut -d' ' -f1)"
  [[ "$before" == "$after" ]] || note_fail "the daemon rewrote pins.toml"
}

check_refresh_freshness() {
  fresh
  cx serve >"$SCRATCH/serve-fresh.log" 2>&1 &
  local pid=$!
  local i
  for i in $(seq 1 50); do
    [[ -S "$XDG_RUNTIME_DIR/conspectus/server.sock" ]] && break
    sleep 0.2
  done
  run cx refresh --class mux; ok "prime"
  start_mux fresh-a
  run cx refresh --class mux; ok "refresh after start"
  run cx table mux; has fresh-a "a started mux shows after one refresh"
  tmx kill-session -t fresh-a
  run cx refresh --class mux; ok "refresh after kill"
  run cx graph --format json
  has '"presence": "gone"' "a killed mux is gone after one refresh"
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
}

check_worktree_close() {
  fresh
  command -v wt >/dev/null || { echo "  (skipped: worktrunk 'wt' not installed)"; return; }
  local wtdir="$HOME/src/app-feat"
  git -C "$REPO" worktree add -q -b feat "$wtdir"
  start_mux pinned-wt "$wtdir"; start_mux loose-wt "$wtdir"
  run cx graph --format json; ok
  run cx pin add pinned-wt; ok
  run cx worktree close feat --discard -y; ok
  run cx history list --kind mux; has pinned-wt "pinned record kept"; lacks loose-wt "unpinned record forgotten"
  run cx pin show pinned-wt; lacks missing "slot still resolves"
}

check_repo_clone() {
  fresh
  local clone="$HOME/src/cloned"
  git clone -q "$REMOTE" "$clone"
  # Scan roots are repos or workspaces themselves, so run from inside it.
  cd "$clone" || return
  run cx pin add "repo:$clone/.git" --name cloned-repo; ok "pin the repo"
  cd "$REPO" || return
  rm -rf "$clone"
  run cx rebuild cloned-repo --dry-run; ok; has clone
  run cx rebuild cloned-repo; ok "clone"
  is_true "repo is back" test -d "$clone/.git"
  run cx rebuild cloned-repo --dry-run; has "nothing to" "live repo"
}

# ------------------------------------------------------------- drivers

cmd_smoke() {
  local selected=("$@")
  [[ ${#selected[@]} -eq 0 ]] && selected=("${CHECKS[@]}")
  local name
  for name in "${selected[@]}"; do
    if ! declare -F "check_$name" >/dev/null; then
      echo "unknown check: $name (try: $0 list)" >&2; exit 2
    fi
    CURRENT="$name"; CHECK_FAILED=0
    printf '== %s\n' "$name"
    "check_$name"
    if [[ $CHECK_FAILED -eq 0 ]]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); FAILED_NAMES+=("$name"); fi
  done
  printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
  [[ $FAIL -eq 0 ]] || { printf 'failed: %s\n' "${FAILED_NAMES[*]}"; exit 1; }
}

cmd_shell() {
  cat >&2 <<EOF
Scratch Conspectus home: $SCRATCH
  repo:      $REPO   (remote: $REMOTE)
  conspectus: $CONSPECTUS_BIN  (also on PATH as 'conspectus')
  tmux:      private server under \$TMUX_TMPDIR; \`tmx\` is not defined here, just use \`tmux\`
  stub harnesses on PATH: codex, claude (sleep in a pane)
Try: tmux new -d -s demo -c \$PWD sleep 600; conspectus pin add demo; conspectus tui
Exit the shell to clean up (KEEP=1 keeps the directory).
EOF
  mkdir -p "$SCRATCH/shellbin"
  ln -sf "$CONSPECTUS_BIN" "$SCRATCH/shellbin/conspectus"
  ln -sf "$CONSPECTUS_BIN" "$SCRATCH/shellbin/csp"
  PATH="$SCRATCH/shellbin:$PATH" PS1="[scratch] \w \$ " bash --norc -i
}

main() {
  local cmd="${1:-}"
  case "$cmd" in
    list) printf '%s\n' "${CHECKS[@]}"; return 0 ;;
    shell | smoke) ;;
    *) sed -n '2,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 2 ;;
  esac
  shift
  command -v tmux >/dev/null || { echo "tmux is required" >&2; exit 1; }
  command -v git >/dev/null || { echo "git is required" >&2; exit 1; }
  ensure_binary
  setup_scratch
  trap cleanup EXIT
  "cmd_$cmd" "$@"
}

main "$@"
