---
id: CSP-610
title: Add the Pi harness discovery adapter
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
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
- [ ] #1 Pi sessions under the default grouped layout and under a flat `PI_CODING_AGENT_SESSION_DIR` layout are discovered as `AgentSession` nodes with harness key `pi`
- [ ] #2 cwd comes from the session header; a cwd containing `-` that the directory encoding makes ambiguous still resolves to the correct repo
- [ ] #3 title is the latest `session_info` name when one exists; `last_active_epoch` reflects the newest entry timestamp or the file mtime
- [ ] #4 `last_message_preview` follows ADR 0023 and the ADR 0086 Tier 2 cap; the header/name pass reads no payload
- [ ] #5 Malformed lines, a missing or non-`session` header, v1/v2 headers, empty files, and unreadable files degrade to skipped sessions or partial nodes and never fail discovery
- [ ] #6 The adapter is registered in `REGISTERED_ADAPTERS`, `HarnessDiscovery::with_default_adapters`, and the provider `REGISTRY`, and `LocalDiscoveryConfig::from_env` resolves the Pi state root as the ADR specifies
- [ ] #7 Fixtures under `tests/fixtures/pi/` cover both layouts, a named session, a legacy-version header, and a malformed file; snapshot tests and `tests/adapter_conformance.rs` pass for the new adapter
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
