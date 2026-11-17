#!/usr/bin/env bash
# The Claude Code scenario (started by run.sh, never directly): the checks
# that tied off live state against Claude Code 2.1.322, in one run.
#
#   main      a turn; the idle notification a minute later; a permission
#             prompt denied; a long approved tool; an Esc interrupt; a
#             question; a subagent; a background subagent that outlives the
#             turn; /compact; /clear; /exit
#   plan      a plan rejected after the dialog sat open, then approved
#   closed    a closed pane (SIGHUP)
#   killed    kill -9
#   err-*     StopFailure for a 401, a 429, and a 500, against a local
#             stand-in for the model gateway
#
# At each point it samples the stored state and Claude Code's own list of
# its sessions (`claude agents --json`), which is what the repair of live
# state reads. About 24 short prompts.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

fast=${RECORD_FAST:-0}
rec_server
rec_init_claude
rec_preflight || exit 1

# smp NAME LABEL  the stored state, the pane title, and the session list now
smp() { rec_sample "$1" "$2 | agents: $(rec_agents)" claude-code; }

# claude_start NAME [MODE]  MODE is the permission mode (default `default`):
# the operator's own settings may allow tools or skip the prompts, which would
# leave the permission steps with no dialog to record.
claude_start() {
  local n=$1 mode=${2:-default}
  rec_start "$n" "$REC_PROJ" CLAUDE_CONFIG_DIR="$REC_CLAUDE_HOME" -- claude --permission-mode "$mode" --effort low
  rec_wait "$n" 'Claude Code v' 40 || { rec_step "claude did not start ($n)"; rec_snap "$n" no-start; return 1; }
  sleep 3
}

# answer NAME  press Enter on the open permission dialog (its first choice)
answer() { rec_keys "$1" Enter; }

# --- main -------------------------------------------------------------------
n=cc-main
claude_start $n || exit 1
smp $n launched; rec_snap $n launched

rec_step "a plain turn, then the idle notification"
rec_ask $n "Reply with just the word ok."; sleep 5; rec_settle $n claude-code 30; smp $n after-turn
if [ "$fast" != 1 ]; then sleep 66; smp $n idle-66s; fi

rec_step "a permission prompt, denied"
rec_ask $n "Create an empty file named deny-me.txt using the shell, then reply with just the word done."
rec_wait $n 'Do you want to proceed' 40 || rec_step "no permission dialog"
sleep 2; smp $n permission-open; rec_snap $n permission
if rec_screen $n | grep -q -E '^ *4\. No'; then rec_keys $n 4; else rec_keys $n Escape; fi
sleep 4; smp $n after-deny-4s; sleep 6; smp $n after-deny-10s; rec_snap $n after-deny

rec_step "an approved tool that runs a while"
rec_ask $n "Run this exact shell command and nothing else: sleep 12 && touch ok.txt . Then reply with just the word done."
rec_wait $n 'Do you want to proceed' 40 || rec_step "no permission dialog"
sleep 1; answer $n; sleep 4; smp $n tool-running; rec_settle $n claude-code 60; smp $n tool-finished

rec_step "an Esc interrupt"
rec_ask $n "Write the numbers from 1 to 600, separated by single spaces, with no other text and no tool use."
sleep 3; smp $n generating; rec_keys $n Escape; sleep 4; smp $n after-esc-4s
if [ "$fast" != 1 ]; then sleep 70; smp $n after-esc-74s; fi

rec_step "a question"
rec_ask $n "Use the AskUserQuestion tool to ask me whether I prefer cats or dogs, then reply with just my answer."
rec_wait $n 'Enter to select' 60 || rec_step "no question dialog"
sleep 2; smp $n question-open; rec_snap $n question; answer $n; rec_settle $n claude-code 40; smp $n after-answer

rec_step "a subagent"
rec_ask $n "Use the Agent tool to start one subagent that runs the shell command sleep 8 and then replies with the word hi. Wait for it, then reply with just the word done."
sleep 8; smp $n subagent-running
for _ in $(seq 1 30); do
  rec_screen $n | grep -q 'Do you want to proceed' && { sleep 1; answer $n; }
  rec_state claude-code | grep -q -E '^ready/.* sub=0$' && break
  sleep 2
done
rec_settle $n claude-code 40; smp $n subagent-done

rec_step "a background subagent that outlives the turn"
rec_ask $n "Use the Agent tool with run_in_background set to true to start a subagent that runs the shell command sleep 20 and then replies with the word fin. Do not wait for it; reply with just the word launched."
sleep 10; smp $n background-launched
for _ in $(seq 1 20); do rec_screen $n | grep -q 'Do you want to proceed' && { sleep 1; answer $n; }; sleep 2; done
rec_settle $n claude-code 60; smp $n background-done

rec_step "compaction, then /clear"
rec_ask $n "/compact"; sleep 2; smp $n compacting; rec_settle $n claude-code 60; smp $n after-compact
rec_ask $n "/clear"; sleep 5; smp $n after-clear; rec_snap $n after-clear
rec_ask $n "/exit"; sleep 5; rec_snap $n after-exit

# --- plan mode --------------------------------------------------------------
n=cc-plan
rec_step "a plan rejected after it sat open, then approved"
claude_start $n plan || exit 1
rec_ask $n "Plan how to create an empty file named plan1.txt. Keep the plan to one line, then present it with ExitPlanMode."
rec_wait $n 'Would you like to proceed' 60 || rec_step "no plan dialog"
sleep 8; smp $n plan-open-8s; rec_snap $n plan-dialog; rec_keys $n Escape; sleep 5; smp $n after-plan-reject
rec_ask $n "Plan how to create an empty file named plan2.txt. Keep the plan to one line, then present it with ExitPlanMode."
rec_wait $n 'Would you like to proceed' 60 || rec_step "no plan dialog"
sleep 2; answer $n; sleep 6
for _ in $(seq 1 15); do rec_screen $n | grep -q 'Do you want to proceed' && { sleep 1; answer $n; }; sleep 2; done
rec_settle $n claude-code 60; smp $n after-plan-approve
rec_ask $n "/exit"; sleep 5

# --- closed pane and kill -9 ------------------------------------------------
n=cc-closed
rec_step "a closed pane"
claude_start $n || exit 1
rec_ask $n "Reply with just the word ok."; rec_settle $n claude-code 40; smp $n before-close
tm kill-pane -t $n; sleep 6; smp $n after-close

n=cc-killed
rec_step "kill -9"
claude_start $n || exit 1
rec_ask $n "Reply with just the word ok."; rec_settle $n claude-code 40; smp $n before-kill
pid=$(rec_find_pid $n 'claude' ''); rec_step "killing process ${pid:-none}"
[ -n "$pid" ] && kill -9 "$pid"; sleep 6; smp $n after-kill; rec_stop $n

# --- StopFailure against a stand-in gateway ---------------------------------
rec_step "StopFailure, against a local stand-in for the gateway"
port=$((20000 + RANDOM % 20000)); mode="$REC_ROOT/gateway-mode"; echo 401 > "$mode"
RECORD_SCRATCH="$REC_ROOT" python3 -I "$REC_DIR/stub-gateway.py" "$port" 400 "$mode" &
sleep 1
real_home=$REC_CLAUDE_HOME; REC_CLAUDE_HOME="$REC_ROOT/claude-home-err"
cp -r "$real_home" "$REC_CLAUDE_HOME"
python3 -I - "$REC_CLAUDE_HOME/settings.json" "$port" <<'PY'
import json, sys
path, port = sys.argv[1:3]
settings = json.load(open(path))
settings.setdefault("env", {}).update({"ANTHROPIC_BASE_URL": f"http://127.0.0.1:{port}",
                                       "CLAUDE_CODE_MAX_RETRIES": "0", "API_TIMEOUT_MS": "8000"})
json.dump(settings, open(path, "w"), indent=2)
PY
for code in 401 429 500; do
  echo $code > "$mode"; n=cc-err-$code
  claude_start $n || continue
  rec_ask $n "Reply with just the word ok."
  for _ in $(seq 1 60); do grep -q StopFailure "$REC_TRACES/$n.jsonl" 2>/dev/null && break; sleep 1; done
  sleep 3; rec_snap $n after-error; rec_sample $n "after $code" claude-code
  rec_keys $n C-c; sleep 1; rec_keys $n C-c; sleep 2; rec_stop $n
done
REC_CLAUDE_HOME=$real_home

rec_step "done"
rec_summary cc-main cc-plan cc-closed cc-killed cc-err-401 cc-err-429 cc-err-500
