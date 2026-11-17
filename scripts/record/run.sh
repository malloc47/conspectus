#!/usr/bin/env bash
# Run one recording scenario against a real harness. MANUAL: it spends the
# model quota of the account the harness is signed in to, so it is never run
# by `just check`, by CI, or by anything else. See README.md.
#
#   scripts/record/run.sh [--dry-run] [--yes] [--keep] [--out DIR] HARNESS
#
# HARNESS is claude-code, codex, or pi. Without --yes it asks you to type
# `record` at a terminal, and refuses when it has none. --dry-run prints
# what would happen and starts nothing.
#
# The environment:
#   RECORD_CONSPECTUS  the conspectus the hooks run (default: ./target/release/conspectus,
#                      else the one on PATH); it must support CONSPECTUS_HOOK_TRACE
#   RECORD_MEM         memory cap of the run's scope (default 3G, no swap)
#   RECORD_SECS        wall-clock limit (default 900)
#   RECORD_TMP         where the scratch directory goes (default $XDG_RUNTIME_DIR, else /tmp)
#   RECORD_OUT         where traces and screens go (default ./record-out/<time>-<harness>)
#   RECORD_FAST=1      skip the waits of a minute or more (idle notification and the like)
#   RECORD_KEEP=1      keep the scratch directory (same as --keep)
set -euo pipefail

unset CDPATH
DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$DIR/../.." && pwd)

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed -e '$d' -e 's/^# \{0,1\}//'; }

dry=0 yes=0 keep=${RECORD_KEEP:-0} out=${RECORD_OUT:-} harness=
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1 ;;
    --yes) yes=1 ;;
    --keep) keep=1 ;;
    --out) shift; out=${1:?--out needs a directory} ;;
    -h|--help) usage; exit 0 ;;
    -*) echo "record: unknown option $1" >&2; usage >&2; exit 2 ;;
    *) harness=$1 ;;
  esac
  shift
done
case "$harness" in
  claude-code) program=claude turns=24 ;;
  codex) program=codex turns=14 ;;
  pi) program=pi turns=8 ;;
  *) echo "record: name a harness: claude-code, codex, or pi" >&2; usage >&2; exit 2 ;;
esac

bin=${RECORD_CONSPECTUS:-}
if [ -z "$bin" ]; then
  if [ -x "$ROOT/target/release/conspectus" ]; then bin="$ROOT/target/release/conspectus"
  else bin=$(command -v conspectus || true); fi
fi
mem=${RECORD_MEM:-3G}
secs=${RECORD_SECS:-900}
tmp=${RECORD_TMP:-${XDG_RUNTIME_DIR:-/tmp}}
out=${out:-$PWD/record-out/$(date +%Y%m%d-%H%M%S)-$harness}

cat <<EOF
record: $harness scenario

  quota      about $turns short prompts (a few words each) to the model and account
             $program is signed in to, plus the waits between them; one run takes
             about 10 to 25 minutes (RECORD_FAST=1 skips the longest waits)
  reads      the $program configuration, copied (never written back), and the
             conspectus at ${bin:-<none found>}
  creates    a scratch directory $tmp/conspectus-record.XXXXXX (removed on exit,
             unless --keep), holding a private tmux server, a copy of the
             configuration, a throwaway project, and a scratch state directory
             (your live-state store is not touched)
  writes     the redacted hook traces, screen snapshots, and a timeline to
             $out
  bounded    memory $mem (no swap, the run's own systemd scope), at most ${secs}s
EOF
[ "$dry" = 1 ] && exit 0

if [ "$yes" != 1 ]; then
  if [ ! -t 0 ]; then
    echo "record: this spends model quota; pass --yes, or run it at a terminal" >&2
    exit 2
  fi
  printf 'Type "record" to start: '
  read -r answer
  [ "$answer" = record ] || { echo "record: not started" >&2; exit 2; }
fi

for tool in tmux systemd-run python3 "$program"; do
  command -v "$tool" >/dev/null || { echo "record: $tool is not installed or not on PATH" >&2; exit 1; }
done
[ -n "$bin" ] && [ -x "$bin" ] || { echo "record: no conspectus found; build one (cargo build --release) or set RECORD_CONSPECTUS" >&2; exit 1; }

mkdir -p "$out"
scratch=$(mktemp -d "$tmp/conspectus-record.XXXXXX")
chmod 700 "$scratch"
cleanup() {
  # The scenario cleans up after itself; this is the second line.
  if [ -d "$scratch" ]; then
    tmux -S "$scratch/tmux.sock" kill-server 2>/dev/null || true
    pids=$(grep -l -a "RECORD_SCRATCH=$scratch" /proc/[0-9]*/environ 2>/dev/null | sed -E 's#/proc/([0-9]+)/environ#\1#' || true)
    # shellcheck disable=SC2086
    [ -n "$pids" ] && kill -TERM $pids 2>/dev/null || true
    [ "$keep" = 1 ] || rm -rf "$scratch"
  fi
}
trap cleanup EXIT

export REC_ROOT="$scratch" REC_OUT="$out" REC_BIN="$bin" RECORD_SECS="$secs" RECORD_FAST="${RECORD_FAST:-0}" RECORD_KEEP="$keep"
# A memory cap with no swap, in its own scope, with the OOM policy set to
# `continue`: a runaway process is killed inside the scope, and systemd does
# not stop the scope of the session that started this.
systemd-run --user --scope --quiet --collect \
  -p MemoryMax="$mem" -p MemorySwapMax=0 -p OOMPolicy=continue \
  -- timeout "$secs" bash "$DIR/$harness.sh" &
child=$!
# A signal to this script goes on to the scenario (which then cleans up),
# and this waits for it to finish doing so.
trap 'kill -TERM "$child" 2>/dev/null || true' TERM INT HUP
status=0
wait "$child" || status=$?
while kill -0 "$child" 2>/dev/null; do wait "$child" || status=$?; done
exit "$status"
