---
id: CSP-797
title: Decide whether the main screen's letters follow the one character rule
status: To Do
assignee: []
created_date: '2026-10-30 17:09'
updated_date: '2026-10-30 17:10'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-760
priority: low
ordinal: 883000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Overlays bind a character as typed (plain or with Shift, never a Control or Alt chord of it; ADR 0160 decision 3). The main screen's keymap, now one scoped `tuikit::keys::Keymap` (ADR 0192), kept its own matchers when it was extracted: most letters fire with any modifier but Control (`Alt-a` attaches, `Alt-p` pins), and the tree's `j`, `k`, and `G` fire with any modifier at all (`Ctrl-j` moves the tree). The extraction left this unchanged on purpose because changing it stops every `Alt` chord of a main-screen letter. Decide whether the program's keys adopt the overlay rule, and if so, which chords stop working and whether any should be bound on purpose.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The keymap's matchers for letters are one documented rule, the overlay rule or a stated exception
- [ ] #2 Every key that stops or starts working is listed in the change's summary and in the CHANGELOG
- [ ] #3 `keys::loose_characters`, or an equivalent keymap check, runs over the keymap's bindings in a test
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
