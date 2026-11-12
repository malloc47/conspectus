---
id: CSP-815
title: 'Codex live state: reducer, fixtures, and a safe hook install'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 10:24'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-810
  - CSP-811
documentation:
  - docs/plans/live-state/research/02-codex.md
  - >-
    docs/adr/0206-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
priority: medium
type: enhancement
ordinal: 901000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Codex now has a full hook system whose entries sit beside the operator's own, but the installed Conspectus entry likely does nothing on current Codex until the operator trusts it in `/hooks`, and the installer replaces an operator's `[[hooks.SessionStart]]` tables and sets no timeout. State needs the installer fixed and the reducer written.

Scope: Fix `hook init codex`: preserve the operator's array-of-tables and inline forms, set a timeout on every entry, install the Tier A set of ADR 0206 as separate matcher groups, make status and remove idempotent and exact, and print the `/hooks` review instruction. Never write a trust value. Implement the Codex `LiveEvent` mapping and reducer (`02-codex.md` section 6) including the subagent child set and `permission_mode` plan handling, fixtures from the documented shapes with sentinel text, and the `unknown event is ignored` behavior.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Install tests over: operator tables, inline hooks, a `hooks.json` beside `config.toml`, a present `hooks.state`, remove with Conspectus's group between operator groups; reducer sequence tests; privacy sentinels.

Blockers: The fast writer.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `hook init codex` preserves operator hooks in every supported form and sets timeouts
- [x] #2 The Tier A events map to the states in the research table, with a sequence test per scenario
- [x] #3 `hook init codex` tells the operator to review the hook in `/hooks`, and nothing writes a trust value
- [x] #4 `hook remove codex` removes only Conspectus's groups
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Installer fix, its own commit: a new src/cli/hook_codex.rs edits config.toml through toml_edit (already a dependency), keeping comments and layout. Groups of Conspectus's own are appended per event beside the operator's: an array of tables `[[hooks.<Event>]]` where the hooks table is a table, inline values where the hooks table or its event lists are inline. Each handler has `timeout = 5`. An older group of Conspectus's is upgraded in place. A hooks value or event list of the wrong type is refused, never replaced. Remove takes out exactly Conspectus's handlers and the groups they empty, wherever they sit, and says when that renumbers the operator's groups after them. `hooks.state` is never touched, and no trust value is written. A hooks.json beside config.toml gets a note and is never edited. Init prints the /hooks review instruction. The written text is parsed back before it replaces the file. Writes follow a symlink and keep the file mode. Status prints per-event coverage. Tests run in scratch dirs.
2. Mapping, its own commit: src/discovery/harness/codex_live.rs holds HOOK_EVENTS (the Tier A set of ADR 0206 decision 2) and live_events. is_attribution_event is true only for SessionStart, reports_live_state is true, and the shared reducer is used unless a case needs more. Plan-mode Stop maps to ready/finished, the closest existing detail. A child (agent_id) keeps its waits and tools, and its turn events are dropped. agent_type is never read. Health tests and cli_smoke move from unsupported for codex.
3. Fixtures, its own commit: tests/fixtures/hooks/codex/ gets per-event payloads with sentinels, labeled synthetic, plus seq-*.jsonl scenarios (simple turn, approval then PostToolUse, decline then interrupt, subagent waiting, subagent overlay on working and ready, compaction mid-turn and idle, plan-mode stop, crash with no SessionEnd, clear). codex-sessionstart.json moves to codex/sessionstart-startup.json and its consumer is fixed. The Claude sequence runner is shared. A `hook write codex` sentinel test covers every fixture.
4. Docs, its own commit: a Codex live-state section in operations.md, the health table, CHANGELOG, ADR 0206 "Codex, as built", and the plan README bug list.
5. Check that `tui --snapshot` frames are byte-identical, run checks.sh and run-tests.sh, finalize the task, file follow-ups.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Planning note (after CSP-809): the neutral `LiveEvent` vocabulary and the shared reducer (`src/live_store/`) already exist. This story maps Codex payloads to `LiveEvent`s and extends the shared reducer only where Codex needs something the vocabulary lacks (for example plan-mode `Stop`). Read `src/live_store/event.rs` and `reducer.rs` first.

Planning note (after CSP-810): the writer seam is on `HarnessAdapter` (`src/discovery/harness/mod.rs`): `live_events_from_payload(payload, context, observed_ms)` for event sources, `live_snapshot_from_payload` for plugin snapshots, `live_reducer` for a harness reducer, and `is_attribution_event`, which defaults to `true`. Override `is_attribution_event` so only the events that name the session (session start) take the spool and daemon path; every other event then reaches only the live-state store. `hook write` (`src/cli/hook_write.rs`) fills pid, pid start, and tmux pane/socket/session into the context; the adapter adds cwd and transcript path from its payload view. Claude Code's `LiveHook` view in `claude_code.rs` is the pattern (borrowed serde struct of whitelisted fields).

Note from CSP-811: `tests/fixtures/hooks/codex-sessionstart.json` also carries `"source": "user"` and a Claude model name; re-check both against the Codex hook docs when building the Codex fixtures. The Claude Code slice keeps its install logic in `src/cli/hook_claude.rs` and its subscription list (`HOOK_SUBSCRIPTIONS`) beside the mapping in `src/discovery/harness/claude_code_live.rs`, and adds the neutral `WaitConfirmed` event; the same shape may suit Codex.

From CSP-813: `hook status` reports this harness's live-state health as `unsupported` until its adapter's `HarnessAdapter::reports_live_state` returns true; flip it when the hooks report live state, and update the `unsupported` row of the health table in `docs/operations.md`.

Built in four commits: the installer fix, the mapping, the fixtures and sequences, and the docs.

Decisions for review:
- Every new Codex handler is synchronous, with no matcher and `timeout = 5`. A new event list takes the form of the table it goes into: `[[hooks.<Event>]]` in a plain table, inline lists in an inline or dotted table or in one that already holds inline lists.
- A matcher counts as covering only when it is absent or empty. Codex reads matchers as regular expressions.
- With a `hooks.json` beside `config.toml`, the install still goes into `config.toml` and prints a note on stderr. `hooks.json` is never edited.
- The `/hooks` review line prints on every `hook init codex`, dry runs included. Trust is never read or written.
- `hook remove codex` warns on stderr when removing an entry moves one of the operator's up a place, since Codex keys trust by position (the research marks this UNVERIFIED).
- A plan-mode Stop maps to ready/finished. No detail was added to the closed set.
- Events carrying `agent_id` are the child's. A child's session start, prompt, stop, interrupt, compaction, and session end are dropped.
- The shared reducer needed no change.
- Settings writes, Claude Code's included, now go through the shared atomic writer `declared::write_atomic`. It follows symlinks, keeps the mode, and refuses read-only files. The capability manifest names its `.<name>.tmp-*` temp files.
- The Claude Code sequence runner moved to the shared `harness/live_sequences.rs`.
- cli_smoke's `isolated_cmd` now pins `CODEX_HOME` to the scratch home.

Cosmetic: adding keys into an inline `hooks = { ... }` table leaves `] , Key` spacing after the operator's last value. It is valid TOML and is kept so that remove restores the original bytes.

Validation: checks.sh passes (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check). run-tests.sh on the final tree: exit 0, 5272 passed, 0 failed, 1 ignored. just tuikit-features passes. tui --snapshot frames for 9 views are byte-identical to the pre-story build. nextest was not run separately; it runs the same tests.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Codex now reports live state, and its installer no longer replaces an operator's hooks.

- Installer fix, its own commit. `src/cli/hook_codex.rs` edits `config.toml` in place through toml_edit, which was already a dependency. It keeps comments, layout, and the operator's entries in every form: `[[hooks.<Event>]]` tables, inline lists under `[hooks]` or in dotted keys, and an inline `hooks = { ... }` table.
  - Each event gets its own Conspectus group after the operator's, with no matcher and a synchronous handler with `timeout = 5`. An older group is upgraded in place, and `--command` moves every handler.
  - A wrong-typed hooks value is refused. `hooks.state` and every non-list key are left alone, and no trust value is written.
  - A `hooks.json` gets a note and is never edited.
  - The edited text is parsed back before the write. Writes follow symlinks, keep the file mode, and refuse read-only files, through the shared `declared::write_atomic`; the capability manifest names its temp files.
  - `hook init codex` prints the `/hooks` review step. `hook remove codex` removes exactly Conspectus's handlers and warns when that moves an operator entry. `hook status codex` reports per event.
- Mapping. `codex_live.rs` maps the Tier A set of ADR 0206 decision 2 onto the neutral events, with the shared reducer unchanged. `is_attribution_event` is SessionStart only, and `reports_live_state` is true, so the health line is now receiving, silent, no-events, or off.
- Fixtures. `tests/fixtures/hooks/codex/` holds 27 synthetic payloads with SENTINEL-codex text in every payload field, 11 `seq-*.jsonl` scenarios, and the TOML install cases under `install/`. `codex-sessionstart.json` (with `source: "user"`) is replaced, and its consumer in `tests/fixture_corpus.rs` is fixed. The Claude Code sequence runner is now shared (`harness/live_sequences.rs`).
- Docs: a Codex live-state section in `docs/operations.md`, ADR 0206 "Codex, as built" (decisions 17 to 19), the CHANGELOG, and the plan's bug list.

Verified:
- Unit tests: `cli::hook_codex` (21), `codex::live` (18 mapping and 15 sequence).
- CLI tests in scratch homes with CODEX_HOME pinned: byte-for-byte install and remove per case, hooks.json, hooks.state, between-groups remove, symlink and mode, project scope, and the health line.
- `tests/hook_write.rs` runs every Codex payload, as written and with sentinels planted, through `hook write codex`: silent, exit 0, nothing leaked.
- checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, diff check) passes. run-tests.sh passes on the final tree. `just tuikit-features` passes.
- `tui --snapshot` frames (9 views) are byte-identical before and after.

Follow-ups: CSP-844 (CODEX_HOME for discovery) and CSP-845 (ask tmux about the hook's own pane). CSP-822 notes list the Codex claims to record.
<!-- SECTION:FINAL_SUMMARY:END -->
