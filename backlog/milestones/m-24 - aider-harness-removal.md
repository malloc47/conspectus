---
id: m-24
title: "Aider Harness Removal"
---

## Description

Conspectus has shipped an aider harness adapter since Phase 3, but nobody uses it. Unlike the other harnesses, aider is per-repo: it is found by walking scan roots for `.aider*` markers, it has no session id, no single-command resume, no native transcript parser, and no hook surface. Several generic seams exist mainly to accommodate it: the external-launch escape hatch in the viewer bridge and `execute_view_session`, the `cwd` argument on `HarnessAdapter::resume_argv`, the per-repo scan path that is not gated by a harness state root, and the `harness_aider` theme key. Keeping it costs fixtures, dev scenarios, theme and badge entries, tests, and doc surface, and it makes aider the odd one out in every harness-wide change.

This workstream deprecates and deletes aider support. It changes the supported v1 harness set, so it starts with an ADR.

Order of work: the ADR and `docs/design.md` update come first. Code removal follows. Operator docs, the README, and the changelog close the milestone. CSP-211 (deferred aider transcript parser) is closed as superseded by this workstream.

All tasks carry the `h-aider-removal` label.
