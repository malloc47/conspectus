---
id: CSP-648
title: Make the number of concurrent gh calls configurable
status: Done
assignee:
  - '@claude'
created_date: '2026-10-21 15:52'
updated_date: '2026-10-21 16:19'
labels:
  - forge
  - config
dependencies: []
priority: medium
type: enhancement
ordinal: 706000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Forge discovery runs at most four `gh` calls at once (`MAX_CONCURRENT_GH`, ADR 0115). With many remotes, a full fetch is mostly waiting on that cap. The operator wants it configurable, keeping 4 as the default.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 [forge] concurrency in the config sets how many gh calls run at once; the default stays 4
- [x] #2 Zero or a non-integer emits a config diagnostic and keeps the default
- [x] #3 The daemon, the TUI, and one-shot commands all honor it
- [x] #4 Tests cover parsing and that fetches never exceed the limit; docs/operations.md documents the key
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`[forge] concurrency` (default 4) sets how many `gh` calls run at once; zero, negative, fractional, or non-integer values are a config diagnostic and keep the default. The daemon, the TUI (refresh workers and background fetch), and one-shot commands pass it through `LocalDiscoveryConfig::with_forge_concurrency`. Documented in `docs/operations.md` (ADR 0136). Verified with config tests, an adapter test that peak in-flight calls equal the limit (1, 3, and capped by job count), and a live one-shot fetch: 6.58 s at 4 against 5.08 s at 12, 93 PRs both times.
<!-- SECTION:FINAL_SUMMARY:END -->
