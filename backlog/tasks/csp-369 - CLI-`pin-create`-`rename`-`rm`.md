---
id: CSP-369
title: CLI `pin create` / `rename` / `rm`
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-366
  - CSP-367
ordinal: 302000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: write commands that persist user intent via the CSP-366
  helpers. `create` uses nearest-store selection by default;
  `--store` overrides. `rename` changes `id` and/or `display_name`;
  `--display` change applies the ADR 0029 lockstep mux rename when
  the pin is currently bound (delegate to the CSP-239 lockstep
  helper). `rm` removes from the first matching store. All commands
  refuse to mutate a malformed config file and surface a clear
  diagnostic instead.
- Tests: CLI integration tests for create-into-project-config,
  create-into-user-config, explicit `--store`, rename
  (with-display-change-and-lockstep, with-id-change-only),
  rm-from-project, rm-not-found, idempotent re-create, and a
  snapshot of the generated TOML.
- Blockers: `CSP-366`, `CSP-367`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
CLI create/rename/rm persist pins through the shared
TOML helpers, preflight malformed/duplicate inputs, and preserve
unrelated config sections.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-009`
