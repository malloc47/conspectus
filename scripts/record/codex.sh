#!/usr/bin/env bash
# The Codex scenario (started by run.sh, never directly): the checks that
# tied off live state against Codex 0.167.0, in one run.
#
# Codex's terminal UI attaches to one shared app server that runs every
# session's hooks, in the environment of the FIRST terminal UI. So the first
# session here, `cx`, stays open for the whole run: the hooks of every
# session after it write to cx's trace (cx.jsonl) and carry its pane.
#
#   cx-a      a turn; an approval declined, then granted for a long tool; an
#             Esc interrupt; /clear; /compact; plan mode
#   cx-b      a question (request_user_input); a subagent, and one whose
#             command needs approval
#   cx-c/d    a terminal UI killed with kill -9 and with a closed pane (the
#             app server ends the session about a minute later)
#   cx-nd     --no-daemon: the hooks run in the UI, so its own trace, pid,
#             and pane; kill -9
#
# Pane titles are logged for each session in titles.log. About 14 short
# prompts.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

fast=${RECORD_FAST:-0}
rec_server
rec_init_codex
rec_preflight || exit 1

smp() { rec_sample "$1" "$2" codex; }

# codex_start NAME [codex options]
codex_start() {
  local n=$1; shift
  rec_start "$n" "$REC_PROJ" CODEX_HOME="$REC_CODEX_HOME" -- \
    codex --no-alt-screen -C "$REC_PROJ" -s read-only -a on-request "$@"
  rec_wait "$n" 'OpenAI Codex|Hooks need review' 40 || { rec_step "codex did not start ($n)"; rec_snap "$n" no-start; return 1; }
  rec_codex_trust "$n"
  sleep 2
  codex_default_tier "$n"
}

# codex_default_tier NAME  a new Codex session may default to the fast service
# tier, which some gateways refuse ("'priority' is not supported"). `/fast`
# toggles it, and only an explicit `default` avoids it, so toggle at least
# twice and stop on the message that says `default`. Every session needs it.
codex_default_tier() {
  local n=$1 i out
  for i in 1 2 3; do
    rec_keys "$n" C-u; rec_text "$n" "/fast"; sleep 0.4; rec_keys "$n" Enter; sleep 2
    out=$(rec_screen "$n" | grep -o 'Service tier set to [a-z]*' | tail -1)
    [ "$i" -ge 2 ] && [ "$out" = "Service tier set to default" ] && { rec_note "$n" "tier: $out"; return 0; }
  done
  # Say so and carry on: the run shows what the gateway makes of it.
  rec_step "the service tier of $n did not read default (last: ${out:-nothing}); see its tier snapshot"
  rec_snap "$n" tier
  return 0
}

# approve NAME  answer the approval dialog "yes"; decline NAME  Esc
approve() { rec_keys "$1" y; }
decline() { rec_keys "$1" Escape; }

# --- the anchor: the first terminal UI, left open ---------------------------
codex_start cx || exit 1

# --- cx-a -------------------------------------------------------------------
n=cx-a
codex_start $n || exit 1
rec_titles $n "$REC_OUT/$n/titles.log" 900
smp $n launched; rec_snap $n launched

rec_step "a plain turn"
rec_ask $n "Reply with just the word ok."; sleep 6; rec_settle $n codex 30; smp $n after-turn

rec_step "an approval, declined"
rec_ask $n "Create an empty file named deny-me.txt using the shell, then reply with just the word done."
rec_wait $n 'Would you like to run' 120 || rec_step "no approval dialog"
sleep 4; smp $n approval-open; rec_snap $n approval; decline $n; sleep 4; smp $n after-decline

rec_step "an approval, granted, for a long tool"
rec_ask $n "Run this exact shell command and nothing else: sleep 8 && touch ok.txt . Then reply with just the word done."
rec_wait $n 'Would you like to run' 120 || rec_step "no approval dialog"
sleep 2; approve $n; sleep 3; smp $n approved-3s; rec_settle $n codex 60; smp $n tool-finished

rec_step "an Esc interrupt"
rec_ask $n "Write the numbers from 1 to 600, separated by single spaces, with no other text and no tool use."
sleep 3; smp $n generating; rec_keys $n Escape; sleep 4; smp $n after-esc

rec_step "/clear, then /compact"
rec_ask $n "/clear"; sleep 5; smp $n after-clear
rec_ask $n "Reply with just the word ok."; rec_settle $n codex 30
rec_ask $n "/compact"; sleep 4; smp $n compacting; rec_settle $n codex 60; smp $n after-compact

rec_step "plan mode"
rec_ask $n "/plan"; sleep 2
rec_ask $n "Plan how to create an empty file named p.txt. Keep the plan to one line."
rec_wait $n 'Implement this plan' 120 || rec_step "no plan prompt"
sleep 2; smp $n plan-prompt; rec_snap $n plan
rec_ask $n "/quit"; sleep 3; rec_stop $n

# --- cx-b -------------------------------------------------------------------
n=cx-b
codex_start $n || exit 1
rec_titles $n "$REC_OUT/$n/titles.log" 600

rec_step "a question"
rec_ask $n "Use the request_user_input tool to ask me whether I prefer cats or dogs, then reply with just my answer."
rec_wait $n 'Question 1/1' 120 || rec_step "no question"
sleep 3; smp $n question-open; rec_snap $n question; rec_keys $n Enter; rec_settle $n codex 40; smp $n after-answer

rec_step "a subagent whose command needs approval"
rec_ask $n "Use the spawn_agent tool to start one subagent that runs the shell command touch sub-made.txt and then replies with the word hi. Wait for it to finish, then reply with just the word done."
rec_wait $n 'Would you like to run' 120 || rec_step "no approval dialog"
sleep 2; smp $n subagent-approval; rec_snap $n subagent; approve $n; rec_settle $n codex 60; smp $n subagent-done
rec_ask $n "/quit"; sleep 3; rec_stop $n

# --- terminal UIs that go away ----------------------------------------------
n=cx-c
rec_step "kill -9 of a terminal UI (the app server keeps running)"
codex_start $n || exit 1
rec_ask $n "Reply with just the word ok."; rec_settle $n codex 40; smp $n before-kill
pid=$(rec_find_pid $n 'codex' '--no-alt-screen'); rec_step "killing process ${pid:-none}"
[ -n "$pid" ] && kill -9 "$pid"
sleep 20; smp $n 20s-after-kill

n=cx-d
rec_step "a closed pane (SIGHUP)"
codex_start $n || exit 1
rec_ask $n "Reply with just the word ok."; rec_settle $n codex 40; smp $n before-close
tm kill-pane -t $n; sleep 6; smp $n after-close
if [ "$fast" != 1 ]; then sleep 75; smp $n 80s-after-close; fi

n=cx-nd
rec_step "--no-daemon: the hooks run in the terminal UI"
codex_start $n --no-daemon || exit 1
rec_ask $n "Reply with just the word ok."; sleep 8; smp $n before-kill
pid=$(rec_find_pid $n 'codex' '--no-daemon'); rec_step "killing process ${pid:-none}"
[ -n "$pid" ] && kill -9 "$pid"
sleep 20; smp $n after-kill

rec_step "done"
rec_summary cx cx-nd
