---
id: CSP-531
title: Pin resume drops the pin's launch argv (ADR 0098)
status: Done
assignee: []
created_date: '2026-09-30 19:09'
labels:
  - h-pin-resume-argv
milestone: m-18
dependencies: []
ordinal: 556000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom: a pin with `launch.argv = ["atelier", "exec", "claude",
  "--dangerously-skip-permissions"]` relaunched as bare `claude
  --resume <id>` when its continuity sidecar had a session — no
  wrapper, no skip-permissions, despite the pin form showing both.
- Fix: `discovery::harness::splice_resume_argv` inserts the resume
  tokens after the harness binary inside the pin's effective argv;
  argv that never invokes the binary launches fresh with a hint and
  keeps the sidecar. Unit tests cover default, wrapper + option,
  codex subcommand, path-qualified binary, and no-binary cases.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-RESUME-ARGV-001`
