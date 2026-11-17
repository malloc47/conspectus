#!/usr/bin/env bash
# The Pi scenario (started by run.sh, never directly): the checks that tied
# off live state against Pi 1.5.2, in one run. The extension it tests is
# this checkout's plugins/pi-hook/conspectus-hook.ts, copied into the
# scratch agent directory.
#
#   pi-main    a turn; a tool run; an Esc abort; /compact of a session too
#              small to compact; /new; /reload; /quit
#   pi-closed  a closed pane (SIGHUP)
#   pi-killed  kill -9
#
# The pane title of a stock Pi carries no state (an extension may set one, so
# the title is sampled but not relied on). About 8 short prompts.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

rec_server
rec_init_pi
rec_preflight || exit 1

smp() { rec_sample "$1" "$2" pi; }

# pi_start NAME. CONSPECTUS_HOOK_BIN is the program the extension runs.
pi_start() {
  local n=$1
  rec_start "$n" "$REC_PROJ" PI_CODING_AGENT_DIR="$REC_PI_HOME" CONSPECTUS_HOOK_BIN="$REC_BIN" -- pi
  rec_wait "$n" 'pi [0-9]+\.[0-9]+|escape interrupt|hotkeys' 40 || { rec_step "pi did not start ($n)"; rec_snap "$n" no-start; return 1; }
  sleep 3
}

n=pi-main
pi_start $n || exit 1
smp $n launched; rec_snap $n launched

rec_step "a plain turn"
rec_ask $n "Reply with just the word ok."; sleep 4; rec_settle $n pi 30; smp $n after-turn

rec_step "a tool run"
rec_ask $n "Run the shell command sleep 6 && touch ok.txt using your bash tool, then reply with just the word done."
sleep 3; smp $n tool-3s; sleep 5; smp $n tool-8s; rec_settle $n pi 40; smp $n tool-done

rec_step "an Esc abort"
rec_ask $n "Write the numbers from 1 to 600, separated by single spaces, with no other text and no tool use."
sleep 3; smp $n generating; rec_keys $n Escape; sleep 4; smp $n after-abort; rec_snap $n abort

rec_step "/compact of a session too small to compact"
rec_ask $n "/compact"; sleep 5; smp $n after-compact; rec_snap $n compact

rec_step "/new, then /reload"
rec_ask $n "/new"; sleep 4; smp $n after-new
rec_ask $n "Reply with just the word ok."; sleep 4; rec_settle $n pi 30
rec_ask $n "/reload"; sleep 5; smp $n after-reload
rec_ask $n "/quit"; sleep 5; smp $n after-quit; rec_snap $n quit

n=pi-closed
rec_step "a closed pane"
pi_start $n || exit 1
rec_ask $n "Reply with just the word ok."; sleep 4; rec_settle $n pi 30; smp $n before-close
tm kill-pane -t $n; sleep 6; smp $n after-close

n=pi-killed
rec_step "kill -9"
pi_start $n || exit 1
rec_ask $n "Reply with just the word ok."; sleep 4; rec_settle $n pi 30; smp $n before-kill
pid=$(rec_find_pid $n 'pi|bun' ''); rec_step "killing process ${pid:-none}"
[ -n "$pid" ] && kill -9 "$pid"; sleep 6; smp $n after-kill

rec_step "done"
rec_summary pi-main pi-closed pi-killed
