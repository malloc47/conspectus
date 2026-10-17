---
id: CSP-610
title: Add the Pi harness discovery adapter
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 13:38'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-609
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/session-format.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/environment-variables.md
documentation:
  - docs/provider-adapter-guide.md
  - docs/adr/0023-agent-session-last-message-preview.md
  - docs/adr/0088-provider-descriptor-registry.md
priority: medium
type: feature
ordinal: 659000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi sessions on disk are invisible to Conspectus today, so Pi work never shows up in the sessions graph, repo grouping, or TUI. This story adds the read-only discovery half: a `PiAdapter` implementing `HarnessAdapter::discover` that emits `AgentSessionNode`s from Pi's JSONL session files, registered like the other harnesses (see `docs/provider-adapter-guide.md`).

Format notes (details in the Pi ADR from the dependency): the first line is a `session` header carrying `id`, `cwd`, `timestamp`, `version`, and optional `parentSession`; `session_info` entries carry the display name (latest wins); `message` entries carry user/assistant content. Version 1 and 2 files still exist on disk because Pi migrates only on load. The directory name encodes cwd lossily, so it must not be decoded back into a path.

Lineage from `parentSession` is a separate story; this one emits nodes only.

Manual checks: with Pi installed, run a short `pi` session in a repo and confirm `conspectus` lists it with the right cwd, repo grouping, title, and last-active time; confirm a run with `--no-session` leaves nothing behind.

Blockers: the Pi ADR story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Pi sessions under the default grouped layout and under a flat `PI_CODING_AGENT_SESSION_DIR` layout are discovered as `AgentSession` nodes with harness key `pi`
- [x] #2 cwd comes from the session header; a cwd containing `-` that the directory encoding makes ambiguous still resolves to the correct repo
- [x] #3 title is the latest `session_info` name when one exists; `last_active_epoch` reflects the newest entry timestamp or the file mtime
- [x] #4 `last_message_preview` follows ADR 0023 and the ADR 0086 Tier 2 cap; the header/name pass reads no payload
- [x] #5 Malformed lines, a missing or non-`session` header, v1/v2 headers, empty files, and unreadable files degrade to skipped sessions or partial nodes and never fail discovery
- [x] #6 The adapter is registered in `REGISTERED_ADAPTERS`, `HarnessDiscovery::with_default_adapters`, and the provider `REGISTRY`, and `LocalDiscoveryConfig::from_env` resolves the Pi state root as the ADR specifies
- [x] #7 Fixtures under `tests/fixtures/pi/` cover both layouts, a named session, a legacy-version header, and a malformed file; snapshot tests and `tests/adapter_conformance.rs` pass for the new adapter
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `src/discovery/harness/pi.rs`: a `PiAdapter` that scans flat `<root>/*.jsonl` and grouped `<root>/--*--/*.jsonl`. It reads the header (Tier 1), the latest `session_info` name with a substring prefilter so message bodies are never parsed, and the tail for the newest entry timestamp plus the ADR 0023 preview (Tier 2). Results are cached per file stamp.
2. Register the adapter: `providers::PI`, `REGISTRY`, `REGISTERED_ADAPTERS`, `with_default_adapters`, a `DiscoveryCaches::pi_sessions` slot, and `LocalDiscoveryConfig::from_env` via `pi::state_root_from_env` (ADR 0121 precedence).
3. Ship a minimal runtime signature (key only, no patterns) and `launch_argv = pi`, which the registry invariant requires. Attribution and resume belong to CSP-612.
4. Add fixtures under `tests/fixtures/pi/{grouped,flat}`, unit tests, corpus snapshots, and an end-to-end repo-resolution test.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the Pi discovery adapter (`src/discovery/harness/pi.rs`), registered everywhere ADR 0121 and the adapter guide require. It discovers sessions in both the grouped and flat layouts.
- cwd comes from the header, the title from the latest `session_info`, `last_active_epoch` from the newest entry timestamp (falling back to the header timestamp, then mtime), and the preview from the most recent user or assistant text (ADR 0023 cap).
- The state root follows ADR 0121: `CONSPECTUS_PI_STATE`, then `PI_CODING_AGENT_SESSION_DIR`, then an absolute global `sessionDir`, then `<agent-dir>/sessions`.
- The runtime signature carries only the key, and `launch_argv` is `pi` to satisfy the registry invariant. Attribution and resume stay in CSP-612.
- ADR 0121's preview wording now matches ADR 0023 (user or assistant text).

Verification:
- `just check`: fmt, clippy, 2341 nextest tests, rustdoc, and the diff check.
- `pi_tests.rs` (19 tests) covers both layouts, the shared-directory cwd collision, names, timestamps, previews, the filename id fallback, skipped files (non-JSON, empty, headerless, unreadable), the rescan on rewrite, and state-root precedence.
- `fixture_corpus` snapshots cover both fixture layouts.
- `harness_mux_snapshots::pi_sessions_sharing_an_encoded_cwd_directory_land_in_their_own_repos` shows `<tmp>/my-repo` and `<tmp>/my/repo` resolving to their own checkouts.
- `adapter_conformance` and the registry tests pass.
- Manual: `conspectus table sessions --harness pi` and `tui --snapshot` against the fixtures list the sessions with the right title, cwd, and preview.

Pi is not installed on this machine, so the live-session manual check (a short `pi` run and the `--no-session` check) was not done.
<!-- SECTION:FINAL_SUMMARY:END -->
