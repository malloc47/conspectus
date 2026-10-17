---
id: CSP-629
title: Make Pi a consistent peer of the other harnesses
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 18:19'
updated_date: '2026-10-17 18:42'
labels:
  - h-pi
milestone: m-22
dependencies: []
priority: medium
type: task
ordinal: 687000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A consistency review of the Pi harness work (CSP-609 to CSP-615) found that Pi doesn't yet read as a peer of claude-code, codex, opencode, and aider. The operator's bar: nothing in the code, tests, or docs should make it obvious that Pi was added later. The review found:
- Pi is the only harness tested from checked-in directory trees rather than the `HarnessFixture` builders.
- Pi tests sit in Pi-only blocks at the ends of shared test files.
- Several per-harness tables, lists, and tests skip Pi.
- `hook_sidecar` special-cases codex and Pi by harness key.
- Some logic is duplicated.

It also found five bugs:
- The viewer never tags Pi aborts.
- Table rows for Pi are uncolored.
- The viewer's file lookup can open the wrong session when ids contain `_`.
- Discovery rescans whole Pi files on every append.
- ADR 0121 claims a live-Pi verification that never happened.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The viewer tags aborted Pi exchanges (assistant stopReason aborted and the preceding user prompt) like the other parsers
- [x] #2 Table output colors Pi rows; the viewer finds a session file by exact id even when ids contain underscores; discovery does not rescan whole Pi files on every append
- [x] #3 ADR 0121 states accurately what was and was not verified, and a backlog task tracks the live-Pi checks
- [x] #4 Every per-harness production list, table, glossary, help text, comment, and doc that enumerates harnesses includes Pi or stops enumerating
- [x] #5 Hook sidecar state-scope inference goes through the adapter registry with no harness-key branches; shared Pi helpers are not duplicated within discovery
- [x] #6 Pi tests use builders in the shared fixture module, sit beside the equivalent tests for other harnesses with mechanism-based names, and per-harness tests include Pi
- [x] #7 The showcase scenario and its rendered fixtures include a Pi session
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Fix the five reviewed bugs (with tests), route state-scope inference through the adapter registry, dedupe Pi helpers, add a PiSessionRecord builder and move Pi tests onto it beside their peers, add Pi to every per-harness list, test, comment, and doc, and seed Pi in the showcase.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Acted on the consistency review of the Pi work so Pi reads as a peer of the other harnesses.

Bugs fixed:
- The viewer tags `stopReason: "aborted"` exchanges (the assistant message and the prompt it answered) like the opencode parser.
- Table output colors Pi rows (bright cyan).
- The viewer finds a session file by exact id, so `b` no longer matches `<ts>_a_b.jsonl`.
- Discovery continues the `session_info` scan from the last offset instead of re-reading whole files on every append. A shrunk file gets a full rescan, and a partial last line waits for the next scan.
- ADR 0121 now says the `/proc` comm assumption is unverified, and CSP-630 tracks the live-Pi checks.

Production consistency:
- `HarnessAdapter::state_scope_for_transcript`, with Codex and Pi overrides, replaces the harness-key branches in `hook_sidecar`.
- One `pi::agent_dir_from_env` is shared by discovery and `hook init`.
- `build_lineage_link` uses `json!`. The Pi-only `lineage_fidelity` is dropped and ADR 0121 is amended to match.
- `ScannedSession` is renamed.
- The `hook.rs` Pi methods follow codex, and the module doc is harness-neutral.
- Pi was added to the table palette, the glossary, and the `pin create --harness` help.
- Comments that enumerated harnesses now say "every registered harness".
- `library-api.md` and `atelier-migration.md` gained `CONSPECTUS_PI_STATE`.
- `design.md` lists five harnesses plainly. `operations.md` folds Pi into the state-root section and gives one hook-install block for all three harnesses. The README no longer links Pi alone.

Tests:
- A `PiSessionRecord` builder (`HarnessFixture::write_pi_session`) now backs the Pi adapter tests and the integration test.
- The viewer parser tests use an inline `fixture()` helper, like codex and claude.
- The corpus is now four flat files under `tests/fixtures/pi/`, loaded by `fixture_corpus.rs` like the claude corpus. The Pi insta snapshots and the Pi-named widget snapshots are gone.
- In `cross_link_tests` and `cli_smoke`, the Pi tests moved next to their peers with names based on the mechanism under test. In `hook_sidecar_tests`, the Pi block became one `pi_hook_record_synthesizes_session_with_pi_state_scope`, and the `pane_running_harness` test moved next to its peer.
- Removed duplicates: the config theme test and the controls filter test.
- Pi was added to the per-harness tests: resume argv, launch options, `harness_binaries`, cache provider keys, badge color, the `viewer_bridge` locator, hook payload unit tests, and the traced capabilities state root.

Showcase: it seeds a named Pi session with an exchange, `tests/fixtures/showcase.json` was regenerated, the coverage test expects `pi`, and the README screenshots were regenerated (the header now counts 11 sessions).

Verification:
- `just check` passes: 2389 tests, fmt, clippy, rustdoc, diff check.
- New tests cover the abort tagging, exact-id lookup, incremental name scan (including a partial line and a shrunk file), Pi state-scope synthesis for both layouts, and the builder.
- `just regen-showcase-fixture` and `just screenshots` were run.
<!-- SECTION:FINAL_SUMMARY:END -->
