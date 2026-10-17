---
id: CSP-614
title: Ship the Pi hook extension and hook init/remove support
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-609
  - CSP-612
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/environment-variables.md
documentation:
  - docs/adr/0028-hook-sidecar-mux-attribution.md
  - docs/adr/0049-opencode-hook-plugin-distribution.md
  - docs/adr/0087-mutation-envelope.md
  - plugins/opencode-hook/README.md
priority: medium
type: feature
ordinal: 663000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Process-level attribution goes stale when an operator switches sessions inside a running Pi (`/resume`, `/new`, `/fork`, `/clone`), and Pi does not keep its session file open, so fd scans can't correct it. Claude-code and opencode solve this with hook sidecar records (ADR 0028) written by `conspectus hook write <harness>`. Pi has no shell-hook config; its equivalent is a TypeScript extension that Pi loads through jiti with no build step from `<agent-dir>/extensions/` or `.pi/extensions/`.

The extension subscribes to `session_start` (reasons startup, reload, new, resume, fork) and forwards `{session_id, cwd, transcript_path, hook_event_name}` to `conspectus hook write pi`, following ADR 0049's contract: the `CONSPECTUS_HOOK_BIN` override, a silent no-op when the binary is missing, and no destabilizing Pi. `--no-session` runs have no session file, so the payload omits `transcript_path` or the extension skips the write, whichever the Pi ADR chose. `hook init` / `status` / `remove` gain a `pi` target that installs or removes exactly the Conspectus-owned extension file, inside the ADR 0087 hook-installation category.

Manual checks: `conspectus hook init pi`, start `pi` in tmux, `/new` and `/resume` to another session, and confirm the sidecar record and the TUI follow the switch; `conspectus hook remove pi` leaves other extensions untouched.

Blockers: the Pi ADR story (distribution and envelope decision) and the runtime attribution story (`harness_binaries` reads the Pi runtime signature for pid resolution).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `conspectus hook write pi` accepts the extension payload and writes an ADR 0028 record with harness key `pi`, pid/ppid resolved through the Pi runtime signature, and tmux context
- [ ] #2 The in-repo extension emits one record per `session_start`, and a `/resume` switch inside one Pi process updates the pane attribution to the new session
- [ ] #3 `conspectus hook init pi`, `hook status pi`, and `hook remove pi` install, report, and remove only the Conspectus-owned extension file at user and project scope; any other files in the extensions directory are left untouched
- [ ] #4 Any new write path or program is listed in `docs/capabilities.toml` citing ADR 0087, and the capability tests pass
- [ ] #5 Tests cover payload parsing (missing or empty `session_id`, absent `transcript_path`), install/remove idempotence, and the stale-attribution demotion under harness key `pi`
- [ ] #6 The extension README documents installation, the `CONSPECTUS_HOOK_BIN` override, and how to opt out
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
