---
id: CSP-543
title: Settle platform posture and CI shape
status: To Do
assignee: []
created_date: '2026-09-30 18:30'
updated_date: '2026-10-31 08:16'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 614000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: CI is ubuntu-only and runs the suite twice (`cargo test`, then
  `cargo nextest`). Process-tree attribution reads `/proc`; signal
  handling and watchers are Unix-only. Either add a macOS build/test job
  or state Linux-only in the README; drop the duplicate test run.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `REL-012`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): CI changed shape with the workspace (`.github/workflows/ci.yml`). Every cargo step takes `--workspace`; the Rust checks job also lints and tests the framework with its features off and with `richtext` alone (CSP-785) and runs `cargo doc --workspace`; and a second job, Separation rehearsal, exports `crates/tuikit` with its history and checks it with no host file (`scripts/export-tuikit.sh --self-test`, ADR 0199). The duplicate suite run this story names is still there (`cargo test --workspace`, then `cargo nextest run --workspace`). The framework reads no environment, starts no process, and writes no file (ADR 0150), so the Unix-only parts stay in the host.
<!-- SECTION:NOTES:END -->
