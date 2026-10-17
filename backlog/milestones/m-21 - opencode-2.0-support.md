---
id: m-21
title: "OpenCode 2.0 Support"
---

## Description

OpenCode 2.0 (https://anoma.ly/notes/opencode-2-0/) moves live sessions to new `session_v2` and `session_message` tables, leaving the 1.x tables stale; replaces the plugin API (1.x plugins do not load); and runs one shared background service behind every TUI, web, and desktop client. Conspectus's opencode adapter, transcript parser, hook plugin, launch and resume argv, and runtime-process signatures were all built against 1.x. After an upgrade, new sessions would be invisible, and the plugin's hook records, now written from the shared service, would attribute every session to one pane.

Order of work: CSP-602 is a sandboxed spike that captures real 2.x behavior and fixtures; the announcement and migration guide don't document storage, CLI, or the process model, and the schema notes in the tasks come from third-party reports. Then CSP-603 (discovery) and CSP-605 (stop wrong-pane attribution), which are user-visible correctness. Then CSP-604 (viewer), CSP-607 (plugin port), and CSP-608 (launch, resume, process signatures). CSP-606 picks the replacement session-to-pane evidence source and reconsiders the closed CSP-221 server-API adapter. Opencode 1.x support stays intact throughout.

All tasks carry the `opencode-v2` label.
