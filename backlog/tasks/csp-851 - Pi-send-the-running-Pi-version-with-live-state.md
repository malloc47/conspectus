---
id: CSP-851
title: 'Pi: send the running Pi version with live state'
status: To Do
assignee: []
created_date: '2026-11-12 12:06'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies:
  - CSP-822
priority: low
type: enhancement
ordinal: 936000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pi's live state degrades on older versions: before 0.84.4 no dialog event fires, so a session never reads as waiting, and before 1.1.0 an interrupted run is told only by its stop reason. `hook status pi` can only state that floor, because the extension does not know which Pi it runs in: research note 04 (section 5.5) could not confirm that Pi exposes a version to extensions, and Conspectus does not probe processes for one. If a real Pi shows a version an extension can read (a package export, the extension API, or the context), the extension can send it beside its snapshot and `hook status pi` can name a running Pi below the floor. Found while building the Pi slice (CSP-817); it needs the live check of CSP-822.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Whether Pi exposes its version to an extension is confirmed against a real Pi
- [ ] #2 If it does, the extension sends it, the writer keeps it only when it looks like a version, and hook status pi names a running Pi older than 0.84.4
- [ ] #3 If it does not, research note 04 and ADR 0206 decision 28 say so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
