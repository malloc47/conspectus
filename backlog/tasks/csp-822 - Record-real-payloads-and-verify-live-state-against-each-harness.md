---
id: CSP-822
title: Record real payloads and verify live state against each harness
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-16 20:17'
labels:
  - live-state
  - ls-docs
  - needs-operator
milestone: m-26
dependencies:
  - CSP-811
  - CSP-815
  - CSP-816
  - CSP-817
documentation:
  - docs/plans/live-state/research/01-claude-code.md
  - docs/plans/live-state/research/02-codex.md
  - docs/plans/live-state/research/03-opencode.md
  - docs/plans/live-state/research/04-pi.md
priority: medium
type: task
ordinal: 908000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 fixtures are synthetic, built from documentation, and every research note lists claims it could not verify: whether `PermissionRequest` fires for questions, what `SessionEnd` does on SIGHUP, the order of compaction events, whether Codex hooks run silently until trusted, whether OpenCode 2.x emits `session.status`, how long Pi's `session_shutdown` takes on `kill-pane`. Until a real session confirms them, the reducers encode guesses.

Scope: This story needs a real session of each harness, so it is the operator's call when to run it (it uses their accounts). Build a redacting recorder (every non-enum string replaced by a placeholder) that runs a harness in a throwaway project and private tmux server with a settings override, so the operator's own settings are never touched. Run the manual checks listed in each note, record the payload shapes as fixtures, replace the synthetic fixtures, fix any reducer that a real sequence contradicts, and update each note's UNVERIFIED appendix.

TUI changes: none.

Tests and checks: Each note's manual-check list is answered in the final summary; the sequence tests run against recorded payloads.

Blockers: The four harness stories, and the operator running real sessions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every manual check in the four harness notes is answered or explicitly deferred
- [x] #2 Recorded, redacted payloads replace the synthetic fixtures where they differ
- [x] #3 Reducers match the recorded sequences
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Record real sessions of Claude Code 2.1.322, Codex 0.167.0, and Pi 1.5.2 in a private tmux server with scratch configuration, a scratch state directory, and the hook trace of ADR 0242 on (OpenCode skipped by the operator).
2. Read each recording against the screen, the pane title, and `claude agents --json`, and against the reducers and the notes' manual checks.
3. Fix what a recording contradicts (ADR 0243): the Codex app server as a background process, the Codex closing-event timeouts, the Pi extension's last report.
4. Keep the redacted traces as fixtures, add the real titles and background listings to their tests, and update each note's appendix, the fixture READMEs, and the operations guide.
5. Run the pre-main gate, land on main by fast-forward, and answer every manual check in the final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From CSP-815: claims the Codex slice encodes that a recording should confirm. Each is listed in `tests/fixtures/hooks/codex/README.md` and ADR 0206 decision 19.
- An approval, granted or declined, fires nothing until the next tool, `Stop`, or `Interrupt`.
- A subagent's `PostToolUse` carries its `agent_id`.
- Is `SubagentStop` per child or per child turn?
- `/clear` fires `SessionStart` (`clear`) with no `SessionEnd` for the old session.
- What does a plan-mode `Stop` mean? If a proposed plan is common, a `ready` detail may be worth an ADR 0203 amendment.
- Does `hook remove` renumbering groups really cost a `/hooks` re-review?
- Is the trust table really at `hooks.state.<key>`?
- Is the `/hooks` review wording `hook init codex` prints accurate for the recorded version?

From CSP-816 (OpenCode): the event scripts in `tests/fixtures/hooks/opencode-live/` are synthetic; their README lists the shapes to confirm. Also check in the sandbox: that 1.x `permission.asked` and `question.*` reach the plugin's `event` hook; that `dispose` runs when a 1.x TUI quits; that 2.x `ctx.event.subscribe` yields `{type, data, location: {directory}}` and whether a server plugin instance gets other locations' events (the reducer drops them); that the 2.x step and compaction events carry `data.sessionID`; the 2.x `execution.failed` error `type` words; and that a `dist/v2` entry in `opencode.jsonc` loads both entrypoints. The plugin's TypeScript was not compiled when it landed: run `npm install && npm run typecheck` in `plugins/opencode-hook` first.

From CSP-817 (Pi): the event scripts in `tests/fixtures/hooks/pi-live/` are synthetic; their README lists the event shapes to confirm. Also check in the sandbox: that Pi loads the extension (it was run under Node with a stand-in API, never compiled against Pi's types) and ignores its one extra export (`testOnly`); the field names of every handled event, including `after_provider_response.status` and `agent_settled.aborted`; whether a gate's dialog opens inside the tool's `tool_execution_start`/`_end` or before it; that `/reload` keeps the session id and calls the factory again; `ctx.isIdle()` at `session_start`; that a quit's and a `kill-pane`'s `session_shutdown` report lands; that the attribution record now carries Pi's own pid; and whether Pi exposes its version to an extension (CSP-851).

Added by CSP-832: the `claude agents --json` fixtures under `tests/fixtures/claude-agents/` are synthetic and unverified except the empty list `[]` (seen from a scratch run with no session). Record real output for an interactive busy, waiting (permission prompt, input needed), and idle session and a background blocked, done, failed, and stopped one, with every non-enum string redacted, and replace the fixtures. Check against ADR 0218: the field names (`pid`, `sessionId`, `status`, `state`, `waitingFor`) and values; that `pid` is the same process `CLAUDE_PID` names to the hooks (the poll matches hook entries by it); that an Esc-interrupted turn lists `idle` and an approved prompt lists `busy` while its tool runs; and whether `claude agents --json` still opens no connection with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` and writes nothing to an initialized config on the version recorded.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: @claude
created: 2026-11-13 04:56
---
When recording Codex, also record its pane titles (`tmux display-message -p '#{pane_title}'` in the private server): idle, working (spinner frames), waiting (both `[ ! ]` and `[ . ] Action Required` phases), with animations off, and with the microphone dot. Compare them with the golden table in `src/discovery/harness/codex_title_tests.rs` (ADR 0217); the strings there come from Codex's source, not a recording.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded real sessions of Claude Code 2.1.322, Codex 0.167.0, and Pi 1.5.2 on 2026-11-16, in a private tmux server with scratch configuration and state directories and the hook trace of ADR 0242 on. The operator left OpenCode out: its checks (the notes' item 5, CSP-816's list, the 1.x and 2.x event shapes) are deferred by the operator, and `tests/fixtures/hooks/opencode-live/` stays synthetic.

Changed (ADR 0243): `codex app-server` is a background process, so the hooks it runs record no pane (a second session's hook had named the first session's pane, with the app server's pid); `hook init codex` writes 3 s for `SessionEnd` and `Interrupt`, which Codex clamps to 3 s and warns about at every start (a real run against a rewritten config showed the warnings gone and no new review asked); the Pi extension's report that a session ended waits for the writer, since Pi is a Bun executable and a quit or closed pane gave the writer an empty stdin (a real Pi now reports `exited` for both). The Claude Code reducers needed no change; the synthetic sequences were the real ones. Fixtures: 21 redacted traces in `tests/fixtures/hooks/recorded/`, the real Codex titles in `codex_title_tests.rs`, three real background listings of `claude agents --json` with a parser test, and the synthetic Codex plan-mode Stop fixtures now carry `permission_mode` `default` as recorded.

Answers. Claude Code: `PermissionRequest` fires for `AskUserQuestion` and `ExitPlanMode`, each after its `PreToolUse`; a denied prompt, a rejected plan, and an Esc interrupt fire no hook, and no `idle_prompt` follows an interrupt (77 s watched); `claude agents --json` lists `idle` for all three and `busy` during an approved tool; compaction order is `PreCompact`, `SessionStart` (`compact`), `PostCompact`; a closed pane fires `SessionEnd` `other`, `kill -9` none; subagent events carry the parent's `session_id` and their own `agent_id`; `StopFailure` is `authentication_failed`, `rate_limit`, `server_error` (500 and 529), or `unknown` (`overloaded` not seen); the pane title carries no state (so CSP-869 has no vocabulary to read); `claude agents --json` opens no connection with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, starts no supervisor for interactive sessions, costs 0.2 to 0.3 s and about 125 MB, and writes only to an incomplete configuration; `pid` is the claude process. Codex: hooks are silent until trusted, with a startup review dialog and `/hooks`; trust is at `hooks.state."<config path>:<event>:<group>:<hook>"` and removing an earlier group renumbers and asks again; `SessionStart` comes with the first prompt; an accepted approval fires nothing until `PostToolUse` (the title shows the spinner meanwhile), a declined one fires `Interrupt`; `SubagentStop` is per child, and a child's `PostToolUse` and `PermissionRequest` carry its `agent_id`; `/clear` fires `SessionStart` (`clear`) and the old session's `SessionEnd` comes about 56 s later; plan mode sends `permission_mode` `default`, and its approval choice shows in no hook and no title; `request_user_input` fires no installed hook while open and the title reads `Action Required`; pane titles are idle `<project>`, working `<spinner> [<thread> | ]<project>`, waiting `[ ! ]`/`[ . ] Action Required | <thread> | <project>`, matching the golden table's parser. Pi: the extension loads and ignores `testOnly`; `ctx.isIdle()` is true at start; abort reads `ready/interrupted`; `/reload` keeps the session and `/new` ends it; the attribution record carries Pi's own pid; a failed compaction of a too-small session reads `error`.

Not exercised, so still open: Claude Code's transcript fields, `~/.claude/sessions/`, `CLAUDE_CODE_VERSION`, split-pane teammates, exec-form entries on old versions, `UserPromptSubmit` stdout, a `blocked`, `failed`, or `stopped` background listing; Codex animations off, the microphone dot, `thread_spawn_edges.status`, and `PreToolUse` coverage of `request_user_input`; Pi permission gates, the `subagent` tool, a 429, threshold and overflow compaction, and whether Pi exposes its version (CSP-851).

Verified with `just check`, `cargo test --workspace --all-targets --all-features`, the separation rehearsal, `just plugins`, nextest with no `conspectus` on PATH, and `backlog doctor` on the branch, plus the real Pi and Codex runs above.
<!-- SECTION:FINAL_SUMMARY:END -->
