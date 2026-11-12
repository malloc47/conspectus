---
id: CSP-811
title: 'Claude Code live state: reducer, fixtures, and hook install'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 06:14'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
documentation:
  - docs/plans/live-state/research/01-claude-code.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: high
type: enhancement
ordinal: 897000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code is the harness the operator uses most and has the richest hook set, so it is the vertical slice that proves the design end to end. Its installed hook covers session start only, with a matcher that misses the newer `fork` source, so nothing reports permission waits, stops, or subagents.

Scope: Implement the Claude Code adapter's `LiveEvent` mapping and reducer from the research (`01-claude-code.md` section 6): the 14 events of ADR 0206, per-agent waits, the subagent set, compaction, `StopFailure` types, `SessionEnd`, and the Stop-time reconcile of background subagents. Extend `hook init claude-code` to install the full set idempotently (upgrading an older install), with a 5 s timeout on every entry, the typed `Notification` matcher, the complete `SessionStart` matcher, and exec-form detection; extend `hook status claude-code` to show per-event coverage; make `hook remove` remove every entry it added and nothing else. Build the fixtures from documented payload shapes: single payloads and ordered sequences for the reducer (simple turn, permission approve, deny, Esc interrupt, AskUserQuestion, plan approve, parallel and background subagents, auto-compact, `/clear`, rate limit, kill -9). Replace the existing fixtures that use the invalid `source: "user"`. Record the known gaps in the reducer's documentation.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Reducer sequence tests asserting the effective state after each event; install, upgrade, status, and remove tests in a scratch home that never touch the real `~/.claude`; the operator's own hooks and unrelated keys survive; sentinel privacy tests; fixtures are labeled synthetic until the verification story re-records them.

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The reducer maps the 14 subscribed events to the states in the research table, with a sequence test per scenario listed in the scope
- [x] #2 `hook init claude-code` installs the full set with timeouts, is idempotent, upgrades a session-start-only install, and `hook remove` removes only its entries
- [x] #3 `hook status claude-code` reports which events are installed
- [x] #4 The `fork` source is matched and the invalid-source fixtures are replaced
- [x] #5 No payload text reaches the store (sentinel test)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Vocabulary: add one neutral event, a wait confirmation (a dialog of a kind is open without saying whose: a pending wait is what it confirms, else the main agent waits), so Claude Code's delayed `Notification` types do not pin a child's permission prompt on the main agent. Reducer and privacy tests extended; existing reducer tests stay green.
2. Mapping: `src/discovery/harness/claude_code_live.rs` maps the 14 subscribed events to `LiveEvent`s through a borrowed, per-field view of the parsed payload (unknown or huge fields never read, a mistyped field reads as absent). `tool_name` classifies (question, plan, subagent tool) and is dropped; only the Agent result's status and agentId are read from `tool_response`; `background_tasks` gives running subagent ids at Stop. Main-turn events carrying `agent_id` describe a child and are dropped. Known gaps in the module docs. Per-event mapping tests.
3. Install: `hook init claude-code` installs one Conspectus-only group per subscribed event (matcher where ADR 0206 gives one, `timeout: 5`), upgrades an older install in place (matcher, timeout), never duplicates, keeps operator groups and unrelated keys, refuses to overwrite a malformed hooks table; entries recognized by command in shell or exec form. `hook remove` takes out every Conspectus handler from every event and only the arrays it emptied. `hook status` prints the overall line plus one line per event (installed / outdated / missing; overall installed / partial / not-installed). Tests in a scratch home, user and project scope.
4. Fixtures: `tests/fixtures/hooks/claude-code/` single payloads (research 6.5) and JSONL sequences with writer ms stamps and the expected effective state per step, labelled synthetic; replace the two `source: "user"` fixtures and their consumers. Sequence tests drive payload -> mapping -> LiveStore in a scratch dir -> read -> effective(). Sentinel tests at the store level and through the `hook write` binary (store, spool, stdout, stderr).
5. Docs: ADR 0204/0206 refinements, `docs/operations.md` Claude Code live-state subsection, CHANGELOG, plan README section 8. File follow-ups for out-of-scope findings. checks.sh, run-tests.sh, snapshot frames byte-identical.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning note (after CSP-809): the neutral `LiveEvent` vocabulary and one shared reducer (`src/live_store/`, `SharedReducer`, `fold`) already exist and have sequence tests drawn from the research tables. This story maps Claude Code payloads to `LiveEvent`s and extends the shared reducer only where Claude Code needs something the vocabulary lacks. Read `src/live_store/event.rs` and `reducer.rs` first.

Planning note (after CSP-810): the writer seam is on `HarnessAdapter` (`src/discovery/harness/mod.rs`): `live_events_from_payload(payload, context, observed_ms)` for event sources, `live_snapshot_from_payload` for plugin snapshots, `live_reducer` for a harness reducer, and `is_attribution_event`, which defaults to `true`. Override `is_attribution_event` so only the events that name the session (session start) take the spool and daemon path; every other event then reaches only the live-state store. `hook write` (`src/cli/hook_write.rs`) fills pid, pid start, and tmux pane/socket/session into the context; the adapter adds cwd and transcript path from its payload view. Claude Code's `LiveHook` view in `claude_code.rs` is the pattern (borrowed serde struct of whitelisted fields).

Slices 1-3 landed: `WaitConfirmed` in the neutral vocabulary (a dialog confirmed without naming the agent keeps a pending wait, else the main agent waits); the Claude Code mapping in `src/discovery/harness/claude_code_live.rs` (per-field view of the parsed payload, child-agent turn events dropped, child paths ignored); the install in `src/cli/hook_claude.rs` (one Conspectus group per event, upgrade in place, `--command` moves existing handlers, refuses a malformed hooks value, remove drops only what it emptied, per-event status).

Slice 4 and docs: 49 synthetic payloads and 17 sequences under `tests/fixtures/hooks/claude-code/` (README and sequence headers say synthetic); sequence runner in `src/discovery/harness/claude_code_sequence_tests.rs`; fixture-wide sentinel test through the `hook write` binary in `tests/hook_write.rs` that scans every state file after every event (checked by planting a deliberate leak, which it caught); the two `source: "user"` fixtures replaced by `sessionstart-startup.json` and `sessionstart-no-transcript.json`. ADR 0204 and 0206 refined; `docs/operations.md` Claude Code live-state section; CHANGELOG, README, plan README section 8.

Decisions to review: `stop_hook_active` changes nothing (a Stop always ends a response; the research read it as stay-working); late dialog notifications are `WaitConfirmed`; a child agent's turn events are dropped and its paths not recorded; the `TurnStarted` after an invisible interrupt keeps the old turn's `since_ms` (shared reducer rule); explicit session-start matcher rather than none; shell form only; background-task status words treated as finished are a guess; `Agent`/`Task` result `agentId` assumed equal to hook `agent_id`.

Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, diff --check) pass; run-tests.sh exit 0, 5124 passed, 0 failed, 1 ignored; `just tuikit-features` pass (2883); `tui --snapshot` frames for 9 views byte-identical before and after. Follow-ups: CSP-837 (CLAUDE_CODE_VERSION), CSP-838 (CLAUDE_CONFIG_DIR), CSP-839 (stable binary path in hook commands), CSP-840 (writer parses the whole payload into a Value); note on CSP-815 about the Codex fixture's `source: "user"`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Claude Code live state end to end. The adapter maps all 14 subscribed hook events (every `SessionStart` source including `fork`, prompts, permission requests and the two dialog tools, finished and failed tools with `is_interrupt`, typed notifications, subagents by id with the Agent tool's `async_launched`/`completed` result, `Stop` with its background subagent list, `StopFailure` by error type, compaction, `SessionEnd`) through a per-field view of the payload that never reads text fields and drops `tool_name` after classifying (`src/discovery/harness/claude_code_live.rs`, with the known gaps in its docs). The neutral vocabulary gained `WaitConfirmed` for dialogs reported late without naming the agent. `hook init claude-code` installs one Conspectus group per event with its matcher and `timeout: 5`, upgrades an older install in place without duplicates, keeps operator groups and unrelated keys, recognizes shell- and exec-form entries, and refuses a malformed hooks value; `hook remove` removes only Conspectus handlers and what that emptied; `hook status` reports per event (`src/cli/hook_claude.rs`). Synthetic fixtures (49 payloads, 17 sequences) replace the `source: "user"` ones.

Verified by: per-event mapping tests, a fixture table, 17 sequence tests through mapping, a scratch-dir store, read-back, and `effective()`; install/upgrade/status/remove unit tests and command-line tests in a scratch home for user and project scope; a sentinel test that runs every fixture payload through `hook write` and scans every state file after every event (shown to catch a planted leak). checks.sh, run-tests.sh (5124 passed, 0 failed), and `just tuikit-features` pass; `tui --snapshot` frames are byte-identical. Follow-ups: CSP-837, CSP-838, CSP-839, CSP-840.
<!-- SECTION:FINAL_SUMMARY:END -->
