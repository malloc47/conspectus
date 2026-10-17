---
id: m-22
title: "Pi Harness Support"
---

## Description

Pi (https://pi.dev, `@earendil-works/pi-coding-agent`, evaluated at 1.3.0 / upstream commit 8efd314 on 2026-10-16) is a minimal, extension-first coding-agent harness. Its state maps onto existing Conspectus seams without new graph concepts: JSONL session files under `~/.pi/agent/sessions/--<cwd>--/` whose header carries `id`, `cwd`, and `parentSession`; `/fork` and `/clone` lineage through `parentSession`; a `pi` process title; `pi --session <id>` resume; and TypeScript extensions with a `session_start` event in place of shell hooks. Gaps to design around: the lossy cwd encoding in directory names (read cwd from the header), a flat layout under `PI_CODING_AGENT_SESSION_DIR`, a short `pi` basename that rules out substring process matching, and no held-open session file for fd attribution.

Order of work: CSP-609 records the ADR and widens the v1 harness set in `docs/design.md`. CSP-610 (discovery) follows, then CSP-611 (lineage), CSP-612 (runtime attribution, launch, resume), and CSP-613 (native transcript parser) in any order. CSP-614 (hook extension and `hook init pi`) needs CSP-609 and CSP-612. CSP-615 (operator docs) closes the milestone and coordinates with CSP-546.

All tasks carry the `h-pi` label.
