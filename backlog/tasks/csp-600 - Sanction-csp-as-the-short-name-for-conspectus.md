---
id: CSP-600
title: Sanction csp as the short name for conspectus
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 12:58'
updated_date: '2026-10-13 13:02'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 649000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator runs Conspectus through a private `cs` symlink. Make a short name official so every user types the same one. `cs` collides with Coursier's launcher and with csound's `/usr/bin/cs`; `csp` has no known collision and matches the CSP-### backlog prefix.

Scope (ADR 0118):
- `cargo install` installs a `csp` binary alongside `conspectus`; `default-run` keeps `cargo run` unambiguous.
- Messages that suggest a follow-up command print the name the operator invoked.
- Docs keep `conspectus` as the canonical name and introduce `csp` once, in the README, operations guide, AGENTS.md, and CHANGELOG.

Tests: `csp --help` shows `csp` usage; invoked-name unit tests.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 cargo install installs both conspectus and csp
- [x] #2 Suggested commands use the invoked name
- [x] #3 Docs name csp as the sanctioned short name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed ADR 0118 and the docs (326e640), and the csp binary with invoked-name suggestions (2e6131e).

What it does:
- `cargo install --path .` installs `csp` next to `conspectus`, and `default-run` keeps `cargo run` unambiguous.
- `csp --help` shows `Usage: csp`, while `--version` and log prefixes stay `conspectus`.
- Four suggested-command messages use `invocation::invoked_name()`.
- A pin message no longer suggests the nonexistent `session continue` command.

Verified with `just check` (2,317 tests, including a `csp --help`/`--version` smoke test and invoked-name unit tests).
<!-- SECTION:FINAL_SUMMARY:END -->
