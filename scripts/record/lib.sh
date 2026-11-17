#!/usr/bin/env bash
# Shared pieces of the real-session recording driver (see README.md).
#
# Sourced by the scenario scripts, which `run.sh` starts inside a
# memory-capped systemd scope. Nothing here runs by itself.
#
# Everything a harness touches is scratch: a private tmux server (its own
# socket), a copy of the harness configuration, a scratch XDG_STATE_HOME
# (so the operator's live-state store is never written), and a throwaway
# project directory, all under $REC_ROOT, which is removed on exit.

set -u

: "${REC_ROOT:?run.sh sets REC_ROOT}"
: "${REC_OUT:?run.sh sets REC_OUT}"
: "${REC_BIN:?run.sh sets REC_BIN}"
unset CDPATH
REC_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

REC_SOCK="$REC_ROOT/tmux.sock"
REC_STATE="$REC_ROOT/state"            # XDG_STATE_HOME of every harness process
REC_PROJ="$REC_ROOT/project"           # the throwaway project directory
REC_CLAUDE_HOME="$REC_ROOT/claude-home"
REC_CODEX_HOME="$REC_ROOT/codex-home"
REC_PI_HOME="$REC_ROOT/pi-home"
REC_TRACES="$REC_STATE/conspectus/hook-trace"
REC_SECS="${RECORD_SECS:-900}"
mkdir -p "$REC_STATE" "$REC_PROJ" "$REC_OUT" "$REC_ROOT/tmux-tmp"
export TMUX_TMPDIR="$REC_ROOT/tmux-tmp"

rec_now_ms() { date +%s%3N; }

# The private tmux server. TMUX and TMUX_PANE of the caller are dropped so
# the harness never sees, or reaches, the operator's own server.
tm() { env -u TMUX -u TMUX_PANE tmux -S "$REC_SOCK" -f /dev/null "$@"; }

# rec_note NAME TEXT...  append a line to the timeline
rec_note() {
  local name=$1; shift
  printf '%s\t%s\t%s\n' "$(rec_now_ms)" "$name" "$*" >> "$REC_OUT/timeline.tsv"
}

# rec_step TEXT...  announce a scenario step on stdout and in the timeline
rec_step() { echo "[$(date +%T)] $*"; rec_note scenario "STEP: $*"; }

# The first tmux session keeps the private server up when the last harness
# session ends, so the next one does not race a server that is exiting. It
# is bounded: it ends with the run.
rec_server() { tm new-session -d -s keeper "sleep $((REC_SECS + 60))" 2>/dev/null || true; }

# rec_start NAME CWD [VAR=VALUE...] -- COMMAND [ARG...]
#   Run COMMAND in a new tmux session called NAME, in CWD, in a CLEAN
#   environment. A harness started from inside another one inherits its
#   session variables (CLAUDE_PID, CLAUDECODE, TMUX), which corrupts what
#   the hooks report, so only the variables below pass (TMUX and TMUX_PANE
#   are the private server's own, set by tmux for the pane). The hook trace
#   is named after the session. RECORD_SCRATCH marks every process of the
#   run, so cleanup finds them all, a background supervisor included.
rec_start() {
  local name=$1 cwd=$2; shift 2
  local extra=()
  while [ "$1" != "--" ]; do extra+=("$1"); shift; done
  shift
  mkdir -p "$cwd" "$REC_OUT/$name"
  rec_note "$name" "start in $cwd: $*"
  tm new-session -d -s "$name" -x 160 -y 48 -c "$cwd" \
    bash -c 'h=$1 p=$2 l=$3 u=$4 r=$5 s=$6 n=$7 root=$8; shift 8
        exec env -i HOME="$h" PATH="$p" TERM=tmux-256color LANG="$l" USER="$u" \
            XDG_RUNTIME_DIR="$r" XDG_STATE_HOME="$s" CONSPECTUS_HOOK_TRACE="$n" RECORD_SCRATCH="$root" \
            TMUX="$TMUX" TMUX_PANE="$TMUX_PANE" "$@"' _ \
    "$HOME" "$PATH" "${LANG:-C.UTF-8}" "${USER:-user}" "${XDG_RUNTIME_DIR:-}" "$REC_STATE" "$name" "$REC_ROOT" \
    "${extra[@]}" "$@"
}

rec_keys()   { local n=$1; shift; rec_note "$n" "keys: $*"; tm send-keys -t "$n" "$@"; }
rec_text()   { local n=$1; shift; rec_note "$n" "text: $*"; tm send-keys -t "$n" -l "$*"; }   # literal, no Enter
rec_screen() { tm capture-pane -p -J -t "$1" 2>/dev/null; }
rec_title()  { tm display-message -p -t "$1" '#{pane_title}' 2>/dev/null; }
rec_pid()    { tm display-message -p -t "$1" '#{pane_pid}' 2>/dev/null; }
rec_alive()  { tm has-session -t "$1" 2>/dev/null && [ "$(tm display-message -p -t "$1" '#{pane_dead}' 2>/dev/null)" = "0" ]; }
rec_stop()   { tm kill-session -t "$1" 2>/dev/null || true; }

# rec_ask NAME TEXT  clear the input line, type TEXT, press Enter. A prompt
# typed into an open dialog is the commonest way a scenario drifts, so wait
# for the dialog you expect (rec_wait) before the next rec_ask.
rec_ask() { rec_keys "$1" C-u; rec_text "$1" "$2"; sleep 0.3; rec_keys "$1" Enter; }

# rec_wait NAME REGEX [SECS]  succeed when the screen matches REGEX within
# SECS (default 30). Match text the harness draws, never text you typed.
rec_wait() {
  local n=$1 re=$2 secs=${3:-30} i=0
  while [ "$i" -lt $((secs * 2)) ]; do
    rec_screen "$n" | grep -q -E "$re" && return 0
    sleep 0.5; i=$((i + 1))
  done
  return 1
}

# rec_snap NAME LABEL  save the screen and the pane title as
# $REC_OUT/NAME/NNN-LABEL.txt
rec_snap() {
  local n=$1 label=$2 k
  mkdir -p "$REC_OUT/$n"
  k=$(printf '%03d' $(( $(ls "$REC_OUT/$n" | wc -l) + 1 )))
  { echo "# t_ms=$(rec_now_ms) title=$(rec_title "$n")"; rec_screen "$n"; } > "$REC_OUT/$n/$k-$label.txt"
}

# rec_state [HARNESS]  the state the live-state store holds for the newest
# entry of the harness (or any harness), as `state/detail subagents`
rec_state() {
  python3 -I "$REC_DIR/summarize.py" --state "$REC_STATE/conspectus/live-state.json" "${1:-}"
}

# rec_sample NAME LABEL  record the title and stored state at this moment
rec_sample() {
  local line
  line="$2 | store: $(rec_state "${3:-}") | title: $(rec_title "$1")"
  echo "[$(date +%T)] $line"; rec_note "$1" "SAMPLE $line"
}

# rec_settle NAME HARNESS [SECS]  wait until the stored state reads ready
# with no subagents on two checks in a row (a turn can end and, a moment
# later, a subagent's report starts another).
rec_settle() {
  local harness=$2 secs=${3:-60} i=0 seen=0
  while [ "$i" -lt $((secs * 2)) ]; do
    if rec_state "$harness" | grep -q -E '^ready/.* sub=0$'; then seen=$((seen + 1)); else seen=0; fi
    [ "$seen" -ge 3 ] && return 0
    sleep 0.5; i=$((i + 1))
  done
  return 1
}

# rec_find_pid NAME NAME_REGEX [ARG_REGEX]  the first process below the
# session's pane whose executable matches NAME_REGEX (and whose arguments
# match ARG_REGEX), for the scenarios that kill a harness
rec_find_pid() { python3 -I "$REC_DIR/summarize.py" --find "$(rec_pid "$1")" "$2" "${3:-}"; }

# rec_agents  Claude Code's own list of its sessions, as the scratch
# configuration sees it, with every free-text field dropped
rec_agents() {
  env -i HOME="$HOME" PATH="$PATH" CLAUDE_CONFIG_DIR="$REC_CLAUDE_HOME" XDG_STATE_HOME="$REC_STATE" claude agents --json 2>&1 \
    | python3 -I "$REC_DIR/summarize.py" --agents
}

# rec_titles NAME FILE SECS  log each change of the pane title with its
# elapsed milliseconds, in the background for SECS seconds
rec_titles() {
  local n=$1 f=$2 secs=$3
  ( local t0 last="" cur; t0=$(rec_now_ms); : > "$f"
    while [ $(( $(rec_now_ms) - t0 )) -lt $((secs * 1000)) ]; do
      cur="$(rec_title "$n")"
      if [ "$cur" != "$last" ]; then printf '%7d  %s\n' $(( $(rec_now_ms) - t0 )) "$cur" >> "$f"; last="$cur"; fi
      sleep 0.15
    done ) &
}

# rec_summary NAME...  print the compact table of each trace
rec_summary() { local n; for n in "$@"; do echo "== $n"; python3 -I "$REC_DIR/summarize.py" "$REC_TRACES/$n.jsonl"; done; }

# rec_preflight  refuse to record with a conspectus that cannot trace: the
# hooks would write nothing and the whole run would be wasted quota.
rec_preflight() {
  local probe="$REC_ROOT/probe-state"
  mkdir -p "$probe"
  printf '{}' | env -i HOME="$HOME" PATH="$PATH" XDG_STATE_HOME="$probe" CONSPECTUS_HOOK_TRACE=probe \
    "$REC_BIN" hook write claude-code >/dev/null 2>&1 || true
  if [ ! -s "$probe/conspectus/hook-trace/probe.jsonl" ]; then
    echo "record: $REC_BIN does not write a hook trace; build this checkout (cargo build --release) and pass it as RECORD_CONSPECTUS=<path>" >&2
    return 1
  fi
}

# rec_use_binary FILE...  point every hook entry of the scratch copies at
# $REC_BIN, so the run needs no change to the operator's installed link.
rec_use_binary() {
  local f
  for f in "$@"; do
    [ -f "$f" ] && sed -i -E "s#[^\"' ]*conspectus hook write#${REC_BIN} hook write#g" "$f"
  done
}

# rec_init_claude  scratch CLAUDE_CONFIG_DIR: the operator's settings.json
# (hooks, env, model) without the plugin list, and a .claude.json that
# skips onboarding and trusts only the throwaway project.
rec_init_claude() {
  local src="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
  mkdir -p "$REC_CLAUDE_HOME"
  python3 -I - "$src/settings.json" "$REC_CLAUDE_HOME/settings.json" "$REC_CLAUDE_HOME/.claude.json" "$REC_PROJ" <<'PY'
import json, sys
src, dst, cj, proj = sys.argv[1:5]
d = json.load(open(src))
for k in ("enabledPlugins", "skipDangerousModePermissionPrompt", "tui"):
    d.pop(k, None)
json.dump(d, open(dst, "w"), indent=2)
json.dump({"hasCompletedOnboarding": True, "theme": "dark",
           "projects": {proj: {"hasTrustDialogAccepted": True, "allowedTools": [], "hasCompletedProjectOnboarding": True}}},
          open(cj, "w"), indent=2)
PY
  chmod 600 "$REC_CLAUDE_HOME/settings.json" "$REC_CLAUDE_HOME/.claude.json"
  rec_use_binary "$REC_CLAUDE_HOME/settings.json"
}

# rec_init_codex  scratch CODEX_HOME: config.toml with the hook trust keys
# moved to the scratch path (Codex keys trust to the config file's path),
# and the throwaway project trusted. A changed hook command asks for a
# review at start; `rec_codex_trust` answers it.
rec_init_codex() {
  local src="${CODEX_HOME:-$HOME/.codex}"
  mkdir -p "$REC_CODEX_HOME"; chmod 700 "$REC_CODEX_HOME"
  cp -p "$src/config.toml" "$REC_CODEX_HOME/config.toml"
  [ -f "$src/installation_id" ] && cp -p "$src/installation_id" "$REC_CODEX_HOME/"
  [ -f "$src/auth.json" ] && cp -p "$src/auth.json" "$REC_CODEX_HOME/"
  sed -i "s#$src/config.toml:#$REC_CODEX_HOME/config.toml:#g" "$REC_CODEX_HOME/config.toml"
  printf '\n[projects."%s"]\ntrust_level = "trusted"\n' "$REC_PROJ" >> "$REC_CODEX_HOME/config.toml"
  rec_use_binary "$REC_CODEX_HOME/config.toml"
}

# rec_codex_trust NAME  answer Codex's "Hooks need review" start-up dialog
# with "Trust all and continue", and leave the session at its prompt.
#
# The dialog can come several seconds AFTER the first screen, and keys sent
# before it are read by it: one run's `/fast` opened the hooks list, and its
# Enter switched the PermissionRequest hook off for the rest of the run. So
# wait for the dialog, then for the prompt, and close anything else.
rec_codex_trust() {
  local n=$1 i
  for i in $(seq 1 24); do
    if rec_screen "$n" | grep -q 'Hooks need review'; then
      rec_keys "$n" 2; sleep 0.5; rec_keys "$n" Enter; sleep 3
      break
    fi
    sleep 0.5
  done
  for i in 1 2 3; do
    rec_screen "$n" | grep -q -E 'Turn hooks on or off|Lifecycle hooks|Hooks need review' || break
    rec_keys "$n" Escape; sleep 1
  done
  rec_wait "$n" 'Ask Codex to do anything' 20
}

# rec_init_pi  scratch PI_CODING_AGENT_DIR: settings and model files copied,
# every extension of the operator's linked (they may provide the model) but
# the Conspectus one, which is this checkout's own, so the run tests the
# extension in the repository.
rec_init_pi() {
  local src="${PI_CODING_AGENT_DIR:-$HOME/.pi/agent}" f
  mkdir -p "$REC_PI_HOME/extensions"; chmod 700 "$REC_PI_HOME"
  for f in auth.json settings.json models.json models-store.json keybindings.json; do
    [ -f "$src/$f" ] && cp -p "$src/$f" "$REC_PI_HOME/$f"
  done
  [ -d "$src/themes" ] && cp -rp "$src/themes" "$REC_PI_HOME/themes"
  for f in "$src"/extensions/*; do
    [ -e "$f" ] || continue
    case "$(basename "$f")" in conspectus-hook.ts) continue ;; esac
    case " ${RECORD_PI_SKIP:-} " in *" $(basename "$f") "*) continue ;; esac
    ln -s "$(readlink -f "$f")" "$REC_PI_HOME/extensions/$(basename "$f")"
  done
  cp "$REC_DIR/../../plugins/pi-hook/conspectus-hook.ts" "$REC_PI_HOME/extensions/conspectus-hook.ts"
}

# rec_finish  copy every trace and a summary into $REC_OUT. The traces are
# redacted by the writer (ADR 0242); nothing else is copied out of scratch.
rec_finish() {
  mkdir -p "$REC_OUT/traces"
  cp "$REC_TRACES"/*.jsonl "$REC_OUT/traces/" 2>/dev/null || true
  local f
  for f in "$REC_OUT"/traces/*.jsonl; do
    [ -e "$f" ] || continue
    { echo "== $(basename "$f" .jsonl)"; python3 -I "$REC_DIR/summarize.py" "$f"; } >> "$REC_OUT/summary.txt"
  done
  echo "traces and screens are in $REC_OUT"
}

# rec_reap  end every process this run started, found by the RECORD_SCRATCH
# marker in its environment, then the private tmux server.
rec_reap() {
  local pass pid
  tm kill-server 2>/dev/null || true
  for pass in TERM KILL; do
    for pid in $(grep -l -a "RECORD_SCRATCH=$REC_ROOT" /proc/[0-9]*/environ 2>/dev/null | sed -E 's#/proc/([0-9]+)/environ#\1#'); do
      [ "$pid" = "$$" ] && continue
      kill -"$pass" "$pid" 2>/dev/null || true
    done
    sleep 1
  done
}

# On the way out, however the scenario ends (a `timeout` sends TERM): keep
# the traces, then end every process of the run. run.sh removes the scratch
# directory.
rec_cleanup() { trap - EXIT; rec_finish || true; rec_reap || true; }
trap rec_cleanup EXIT
trap 'exit 143' TERM INT HUP
