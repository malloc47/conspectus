---
id: CSP-245
title: 'ADR: provider, dependency, privacy, dispatch'
status: To Do
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-ai-naming
milestone: m-11
dependencies: []
ordinal: 347000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: settle LLM provider choice (Anthropic vs pluggable), Cargo
  feature gating (e.g. `ai` feature so default builds stay HTTP-free),
  privacy / transcript-redaction posture, config schema and env-var
  convention (`ANTHROPIC_API_KEY` or equivalent), and how the call
  dispatches (synchronous blocking via existing `Cmd` runner per
  ADR 0024, or new async surface). Note whether this counts as a
  "control-plane adapter" under ADR 0028's framing.
- Tests: docs-only.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AI-NAMING-001`
